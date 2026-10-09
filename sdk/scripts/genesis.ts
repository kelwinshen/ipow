// Runs the vaults' genesis on the test networks (docs/specs/ipow-vault-genesis.md):
// for every vault pair and both directions, each asset's receipt made on
// the receipt side (G1), an amount locked on its home to the operator, and
// that lock issued on the receipt side (G2). Run once the vaults are
// redeployed in genesis and `pnpm sync` has written their addresses:
//
//   node scripts/genesis.ts --operator-evm <0x…> --operator-solana <base58> [--coin <network>=<amount> …] [--rwa <whole tokens>] [--symbols A,B,…] [--coins-only] [--round N] [--only <network>] [--dry]
//
// - Tokens: every test RWA token of the home network (ethereum/deployments/
//   <network>-testnet-rwa.json, solana/testnet-rwa.json), registered with
//   the pair's home vault first if it is not (anyone may), and minted to the
//   deployer for the lock (the deployer is their minter; Solana's mint
//   authority is solana/.keys/mock-rwa-mint-authority.json). --rwa sets how
//   many whole tokens each lock is: 10,000 by default.
// - Coins: locked only where --coin names an amount (in whole coins, e.g.
//   --coin ethereum=0.2 --coin solana=20): testnet coins come from faucets.
//   --coins-only leaves the RWA tokens out of the run.
// - Rounds: one lock per asset and pair per round. --round 2 (and on) locks
//   again what round 1 locked, as a top-up while genesis is open; its
//   entries are kept apart in the ledger by their round.
// - Keys: each EVM network's deployer, the genesis key of its vaults, from
//   its .env (evm/.env for Ethereum, networks/<network>/.env for the others);
//   Solana's from ~/.config/solana/id.json.
//   Nothing secret is printed.
// - Tempo sends its own transaction type, which ethers cannot: its pair is
//   skipped here and reported.
//
// Every action is written to ethereum/deployments/genesis-testnet.json as it
// happens; a rerun skips what is done, and scripts/genesis-check.ts reads it.

import { Connection, Keypair, PublicKey, SystemProgram, Transaction, TransactionInstruction } from "@solana/web3.js";
import { Contract, FetchRequest, JsonRpcProvider, Wallet, ZeroAddress, getAddress, parseUnits, zeroPadValue, type Signer } from "ethers";
import { existsSync, readFileSync, writeFileSync } from "node:fs";
import { homedir } from "node:os";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

import { NETWORKS, envFile } from "../../evm/deploy/networks.ts";
import { SolanaVault, genesisIssue, genesisMakeReceipt, homeAssets, lock, locksOf, network, quoteLock, receiptMark, receiptTokens, type Deployment } from "../src/index.ts";

const here = dirname(fileURLToPath(import.meta.url));
// A dropped connection inside a library's own background request (a read
// timing out on a busy RPC) must not end a long run: it is logged, and the
// step it belonged to fails and is retried on the next run.
process.on("unhandledRejection", (e) => console.log(`  network error, carrying on: ${(e as Error)?.message ?? e}`));
const root = join(here, "..", "..");
const argv = process.argv.slice(2);
const flag = (name: string) => (argv.includes(name) ? argv[argv.indexOf(name) + 1] : undefined);
const flags = (name: string) => argv.flatMap((a, i) => (a === name ? [argv[i + 1]] : []));
const DRY = argv.includes("--dry");
const ONLY = flag("--only");
/** Pairs with any of these networks are left for another run (--skip hyperliquid,tempo). */
const SKIP = (flag("--skip") ?? "").split(",").filter(Boolean);
const RWA_WHOLE = flag("--rwa") ?? "10000";
const OPERATOR_EVM = getAddress(flag("--operator-evm") ?? "");
const OPERATOR_SOLANA = new PublicKey(flag("--operator-solana") ?? "").toBase58();
/** Only these RWA symbols, when given (--symbols AAPL,TSLA,…); every one otherwise. */
const SYMBOLS = flag("--symbols")?.split(",").map((x) => x.trim().toUpperCase());
const COINS_ONLY = argv.includes("--coins-only");
const wantSymbol = (symbol: string) => !COINS_ONLY && (!SYMBOLS || SYMBOLS.includes(symbol.toUpperCase()));
const COIN: Record<string, string> = Object.fromEntries(flags("--coin").map((c) => c.split("=") as [string, string]));
/** The round of this run: round 1's entries carry no round, as the ledger was first written. */
const ROUND = Number(flag("--round") ?? "1");
if (!(Number.isInteger(ROUND) && ROUND >= 1)) throw new Error("--round: a whole number from 1");
const SOLANA_RPC = process.env.SOLANA_RPC_URL ?? "https://api.devnet.solana.com";

const TOKEN_PROGRAM = new PublicKey("TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA");
const ATA_PROGRAM = new PublicKey("ATokenGPvbdGVxr1b2hvZbsiqW5xWH25efTNsLJA8knL");
const SOLANA = 2;

/** The deployments names the SDK uses, by folder name. */
const SDK_NAME: Record<string, string> = { ethereum: "ethereum-testnet", base: "base-testnet", robinhood: "robinhood-testnet", hyperliquid: "hyperliquid-testnet", arbitrum: "arbitrum-testnet", hedera: "hedera-testnet", polkadot: "polkadot-testnet", tempo: "tempo-testnet" };

// ---------------------------------------------------------------------------
// The ledger
// ---------------------------------------------------------------------------

type Entry = {
  home: string;
  receiptSide: string;
  asset: number;
  symbol: string;
  token: string;
  recordDecimals: number;
  receiptTx?: string;
  lockId?: string;
  /** The value issued, record units: the lock's amount and fast fee. */
  value?: string;
  recipient: string;
  /** The round, when not the first (--round). */
  round?: number;
  lockTx?: string;
  issueTx?: string;
};
const LEDGER = join(root, "evm", "deployments", "genesis-testnet.json");
const ledger: { started: string; entries: Entry[] } = existsSync(LEDGER) ? JSON.parse(readFileSync(LEDGER, "utf8")) : { started: new Date().toISOString(), entries: [] };
const save = () => !DRY && writeFileSync(LEDGER, JSON.stringify(ledger, null, 2) + "\n");
/** Whether another ledger entry of this direction already holds the lock (in any round). */
const held = (e: Entry, lockId: bigint) => ledger.entries.some((x) => x !== e && x.home === e.home && x.receiptSide === e.receiptSide && x.lockId === String(lockId));
function entry(home: string, receiptSide: string, asset: number, init: Omit<Entry, "home" | "receiptSide" | "asset">): Entry {
  let e = ledger.entries.find((x) => x.home === home && x.receiptSide === receiptSide && x.asset === asset && (x.round ?? 1) === ROUND);
  if (!e) ledger.entries.push((e = { home, receiptSide, asset, ...init, ...(ROUND > 1 ? { round: ROUND } : {}) }));
  return e;
}

// ---------------------------------------------------------------------------
// Keys and connections
// ---------------------------------------------------------------------------

function readEnv(file: string): Record<string, string> {
  const out: Record<string, string> = {};
  for (const line of readFileSync(file, "utf8").split("\n")) {
    const m = line.match(/^\s*([A-Z0-9_]+)\s*=\s*"?([^"\n]*)"?/);
    if (m) out[m[1]] = m[2].trim();
  }
  return out;
}

type Evm = { name: string; d: Deployment; provider: JsonRpcProvider; signer: Signer; nativeDecimals: number; overrides: Record<string, unknown> };
const evms = new Map<string, Evm>();
async function evm(name: string): Promise<Evm> {
  if (evms.has(name)) return evms.get(name)!;
  const s = NETWORKS[name];
  const vars = readEnv(envFile(name));
  const request = new FetchRequest(vars[s.rpcEnv.testnet!]);
  request.setHeader("user-agent", "ipow-genesis");
  const provider = new JsonRpcProvider(request, undefined, { staticNetwork: true });
  const key = vars[s.keyEnv.testnet!];
  const signer = new Wallet(key.startsWith("0x") ? key : "0x" + key, provider);
  // Hedera's relay estimates fees wrongly for typed transactions.
  const overrides = name === "hedera" ? { type: 0, gasPrice: (await provider.getFeeData()).gasPrice } : {};
  const e = { name, d: network(SDK_NAME[name] as any), provider, signer, nativeDecimals: s.coin.kind === "native" ? s.coin.decimals : 18, overrides };
  evms.set(name, e);
  return e;
}

const solanaKey = Keypair.fromSecretKey(Uint8Array.from(JSON.parse(readFileSync(join(homedir(), ".config/solana/id.json"), "utf8"))));
const mintAuthority = Keypair.fromSecretKey(Uint8Array.from(JSON.parse(readFileSync(join(root, "solana", ".keys", "mock-rwa-mint-authority.json"), "utf8"))));
const connection = new Connection(SOLANA_RPC, "confirmed");
const wallet = {
  publicKey: solanaKey.publicKey,
  signTransaction: async <T extends Transaction>(t: T) => (t.partialSign(solanaKey), t),
  signAllTransactions: async <T extends Transaction>(ts: T[]) => ts.map((t) => (t.partialSign(solanaKey), t)),
};
const solanaVault = (peer: number) => new SolanaVault(connection, peer, wallet as any);

const evmRwa = (name: string): { symbol: string; address: string; decimals: number }[] => {
  const f = join(root, "evm", "deployments", `${name}-testnet-rwa.json`);
  if (!existsSync(f)) return [];
  return Object.entries<any>(JSON.parse(readFileSync(f, "utf8")).tokens).map(([symbol, t]) => ({ symbol, address: getAddress(t.address), decimals: t.decimals }));
};
const solanaRwa: { symbol: string; mint: string; decimals: number }[] = Object.entries<any>(JSON.parse(readFileSync(join(root, "solana", "testnet-rwa.json"), "utf8")).tokens).map(([symbol, t]) => ({ symbol, mint: t.mint, decimals: t.decimals }));

const ata = (owner: PublicKey, mint: PublicKey) => PublicKey.findProgramAddressSync([owner.toBuffer(), TOKEN_PROGRAM.toBuffer(), mint.toBuffer()], ATA_PROGRAM)[0];
const b32 = (key: string) => "0x" + new PublicKey(key).toBuffer().toString("hex");

// ---------------------------------------------------------------------------
// The receipt side: G1 and G2
// ---------------------------------------------------------------------------

/** Whether the receipt of `home`'s asset `n` exists on `side`. */
async function receiptMade(home: string, side: string, n: number): Promise<boolean> {
  if (side === "solana") {
    const d = (await evm(home)).d;
    return (await connection.getAccountInfo(solanaVault(d.number).receiptMint(n))) !== null;
  }
  const r = await evm(side);
  const peer = home === "solana" ? SOLANA : (await evm(home)).d.number;
  const pair = r.d.vaults.findIndex((v) => v.peer === peer);
  return (await receiptTokens(r.provider, r.d, pair, n + 1)).has(n);
}

async function g1(home: string, side: string, n: number, token: string, decimals: number): Promise<string> {
  if (side === "solana") return solanaVault((await evm(home)).d.number).genesisMakeReceipt(n, token, decimals);
  const r = await evm(side);
  const peer = home === "solana" ? SOLANA : (await evm(home)).d.number;
  const pair = r.d.vaults.findIndex((v) => v.peer === peer);
  const bytes = home === "solana" ? (token === ZeroAddress ? "0x" + "00".repeat(32) : b32(token)) : zeroPadValue(token, 32);
  return genesisMakeReceipt(r.signer, r.d, pair, n, bytes, decimals, r.overrides);
}

async function g2(home: string, side: string, lockId: bigint, n: number, value: bigint, recipient: string): Promise<string> {
  if (side === "solana") return solanaVault((await evm(home)).d.number).genesisIssue(lockId, n, value, recipient);
  const r = await evm(side);
  const peer = home === "solana" ? SOLANA : (await evm(home)).d.number;
  const pair = r.d.vaults.findIndex((v) => v.peer === peer);
  return genesisIssue(r.signer, r.d, pair, lockId, n, value, recipient, r.overrides);
}

/** Whether `home`'s lock `lockId` is marked issued on `side` already (a
 *  G2 whose ledger write was lost, or a claim). */
async function issuedOn(home: string, side: string, lockId: bigint): Promise<boolean> {
  if (side === "solana") return (await solanaVault((await evm(home)).d.number).evmLockMark(lockId))?.issued ?? false;
  const r = await evm(side);
  const peer = home === "solana" ? SOLANA : (await evm(home)).d.number;
  return (await receiptMark(r.provider, r.d, lockId, r.d.vaults.findIndex((v) => v.peer === peer)))?.issued ?? false;
}

/** G2 for a ledger entry, unless the chain shows it done; nothing in a dry run. */
async function issue(e: Entry, home: string, side: string) {
  const lockId = BigInt(e.lockId!);
  if (await issuedOn(home, side, lockId)) {
    e.issueTx = e.issueTx ?? "issued already (read from the chain)";
    save();
    return;
  }
  console.log(`  ${home}→${side} ${e.symbol}: G2, lock ${e.lockId}`);
  if (DRY) return;
  e.issueTx = await g2(home, side, lockId, e.asset, BigInt(e.value!), e.recipient);
  save();
}

// ---------------------------------------------------------------------------
// The home side: register, mint, lock
// ---------------------------------------------------------------------------

const HOME_ABI = ["function registerAsset(address) returns (uint32)", "function assetOfToken(address) view returns (uint32)"];
const MOCK_ABI = ["function mint(address,uint256)", "function balanceOf(address) view returns (uint256)", "function approve(address,uint256) returns (bool)"];

/** Runs one asset's steps; a failure is reported and the other assets go on. */
const failed: string[] = [];
async function each(tag: string, run: () => Promise<void>) {
  try {
    await run();
  } catch (e) {
    failed.push(`${tag}: ${(e as Error).message}`);
    console.log(`  ${tag}: stopped: ${(e as Error).message}`);
  }
}

/** Locks on EVM network `home` toward `side`; each asset through G1, the lock, G2. */
async function fromEvm(home: string, side: string) {
  const h = await evm(home);
  const peer = side === "solana" ? SOLANA : (await evm(side)).d.number;
  const pair = h.d.vaults.findIndex((v) => v.peer === peer);
  if (pair < 0) throw new Error(`${home}: no vault paired with ${side}`);
  const recipient = side === "solana" ? OPERATOR_SOLANA : OPERATOR_EVM;
  const vaultHome = new Contract(h.d.vaults[pair].home, HOME_ABI, h.signer);
  const wanted: { symbol: string; token: string; whole: string; decimals: number }[] = [];
  if (COIN[home]) wanted.push({ symbol: "coin", token: ZeroAddress, whole: COIN[home], decimals: h.nativeDecimals });
  for (const t of evmRwa(home).filter((t) => wantSymbol(t.symbol))) wanted.push({ symbol: t.symbol, token: t.address, whole: RWA_WHOLE, decimals: t.decimals });
  for (const w of wanted) await each(`${home}→${side} ${w.symbol}`, async () => {
    // Registered with this pair's home vault (asset 0 is the coin).
    let n = w.token === ZeroAddress ? 0 : Number(await vaultHome.assetOfToken(w.token)) - 1;
    if (n < 0) {
      if (DRY) {
        console.log(`  ${home}→${side} ${w.symbol}: would register`);
        return;
      }
      await (await vaultHome.registerAsset(w.token, h.overrides)).wait();
      // An RPC a block behind may not show the registration yet: read again.
      n = Number(await vaultHome.assetOfToken(w.token)) - 1;
      for (let i = 0; n < 0 && i < 12; i++) {
        await new Promise((r) => setTimeout(r, 5_000));
        n = Number(await vaultHome.assetOfToken(w.token)) - 1;
      }
    }
    // A registration just made may not be readable yet (an RPC behind by a
    // block): read again for up to a minute.
    let a = (await homeAssets(h.provider, h.d, pair)).find((x) => x.number === n);
    for (let i = 0; !a && i < 12; i++) {
      await new Promise((r) => setTimeout(r, 5_000));
      a = (await homeAssets(h.provider, h.d, pair)).find((x) => x.number === n);
    }
    if (!a) throw new Error(`asset ${n} is registered but not readable yet; run again`);
    const e = entry(home, side, n, { symbol: w.symbol, token: w.token, recordDecimals: a.recordDecimals, recipient });
    if (e.issueTx) return;
    if (!(await receiptMade(home, side, n))) {
      console.log(`  ${home}→${side} ${w.symbol}: G1, asset ${n}, ${a.recordDecimals} decimals`);
      if (DRY) return;
      e.receiptTx = await g1(home, side, n, w.token, a.recordDecimals);
      save();
    }
    if (!e.lockId) {
      const amount = parseUnits(w.whole, w.decimals);
      if (w.token !== ZeroAddress) {
        const t = new Contract(w.token, MOCK_ABI, h.signer);
        const have: bigint = await t.balanceOf(await h.signer.getAddress());
        if (have < amount && !DRY) await (await t.mint(await h.signer.getAddress(), amount - have, h.overrides)).wait();
      }
      const q = await quoteLock(h.provider, h.d, { asset: n, amount, pair });
      // A lock of this asset to the operator not yet issued, whose ledger
      // write was lost: use it rather than lock again.
      const want = side === "solana" ? "0x" + new PublicKey(recipient).toBuffer().toString("hex") : zeroPadValue(recipient, 32);
      const mine = await locksOf(h.provider, h.d, await h.signer.getAddress(), 50, pair);
      const earlier = [];
      for (const l of mine) if (l.asset === n && l.amount === q.amount && String(l.recipient).toLowerCase() === want.toLowerCase() && !held(e, l.lockId) && !(await issuedOn(home, side, l.lockId))) earlier.push(l);
      if (earlier.length) {
        Object.assign(e, { lockId: String(earlier[0].lockId), value: String(earlier[0].amount + earlier[0].fastFee), lockTx: "found on the chain" });
        save();
      } else {
        console.log(`  ${home}→${side} ${w.symbol}: lock ${w.whole} (${q.amount} record units) to ${recipient}`);
        if (DRY) return;
        // An RPC a block behind may not see the approval (or the mint) yet:
        // the lock's estimate then fails, and is tried again shortly.
        let l: Awaited<ReturnType<typeof lock>> | undefined;
        for (let attempt = 0; !l; attempt++) {
          try {
            l = await lock(h.signer, h.d, q, recipient, h.overrides);
          } catch (err) {
            const behind = /InsufficientAllowance|InsufficientBalance|0xfb8f41b2|0xe450d38c|transfer amount exceeds/i.test(String((err as any)?.data ?? "") + String((err as Error).message));
            if (!behind || attempt >= 5) throw err;
            await new Promise((r) => setTimeout(r, 6_000));
          }
        }
        Object.assign(e, { lockId: String(l.lockId), value: String(q.amount + q.fastFee), lockTx: l.txHash });
        save();
      }
    }
    await issue(e, home, side);
  });
}

/** Locks on Solana toward EVM network `side`. */
async function fromSolana(side: string) {
  const r = await evm(side);
  const v = solanaVault(r.d.number);
  const wanted: { symbol: string; mint: string | null; whole: string; decimals: number }[] = [];
  if (COIN.solana) wanted.push({ symbol: "SOL", mint: null, whole: COIN.solana, decimals: 9 });
  for (const t of solanaRwa.filter((t) => wantSymbol(t.symbol))) wanted.push({ symbol: t.symbol, mint: t.mint, whole: RWA_WHOLE, decimals: t.decimals });
  for (const w of wanted) await each(`solana→${side} ${w.symbol}`, async () => {
    let a = w.mint === null ? (await v.assets())[0] : await v.assetOfMint(w.mint);
    if (!a) {
      if (DRY) {
        console.log(`  solana→${side} ${w.symbol}: would register`);
        return;
      }
      await v.registerToken(w.mint!);
      a = (await v.assetOfMint(w.mint!))!;
    }
    const n = a.number;
    const token = w.mint ?? ZeroAddress;
    const e = entry("solana", side, n, { symbol: w.symbol, token, recordDecimals: a.recordDecimals, recipient: OPERATOR_EVM });
    if (e.issueTx) return;
    if (!(await receiptMade("solana", side, n))) {
      console.log(`  solana→${side} ${w.symbol}: G1, asset ${n}, ${a.recordDecimals} decimals`);
      if (DRY) return;
      e.receiptTx = await g1("solana", side, n, token, a.recordDecimals);
      save();
    }
    if (!e.lockId) {
      const amount = parseUnits(w.whole, w.decimals);
      if (w.mint && !DRY) {
        const mint = new PublicKey(w.mint);
        const to = ata(solanaKey.publicKey, mint);
        const have = (await connection.getAccountInfo(to)) ? BigInt((await connection.getTokenAccountBalance(to)).value.amount) : 0n;
        if (have < amount) {
          const data = Buffer.alloc(9);
          data[0] = 7; // MintTo
          data.writeBigUInt64LE(amount - have, 1);
          const tx = new Transaction().add(
            new TransactionInstruction({ programId: ATA_PROGRAM, data: Buffer.from([1]), keys: [
              { pubkey: solanaKey.publicKey, isSigner: true, isWritable: true }, { pubkey: to, isSigner: false, isWritable: true },
              { pubkey: solanaKey.publicKey, isSigner: false, isWritable: false }, { pubkey: mint, isSigner: false, isWritable: false },
              { pubkey: SystemProgram.programId, isSigner: false, isWritable: false }, { pubkey: TOKEN_PROGRAM, isSigner: false, isWritable: false },
            ] }),
            new TransactionInstruction({ programId: TOKEN_PROGRAM, data, keys: [
              { pubkey: mint, isSigner: false, isWritable: true }, { pubkey: to, isSigner: false, isWritable: true }, { pubkey: mintAuthority.publicKey, isSigner: true, isWritable: false },
            ] })
          );
          await connection.sendTransaction(tx, [solanaKey, mintAuthority]).then((sig) => connection.confirmTransaction(sig, "confirmed"));
        }
      }
      const value = amount / a.unit;
      // A lock of this asset to the operator not yet issued, whose ledger
      // write was lost: use it rather than lock again.
      const mine = await v.locksOf(solanaKey.publicKey.toBase58(), 50);
      const earlier = [];
      for (const l of mine) if (l.asset === n && l.amount === value && getAddress(l.recipient) === OPERATOR_EVM && !held(e, l.lockId) && !(await issuedOn("solana", side, l.lockId))) earlier.push(l);
      if (earlier.length) {
        Object.assign(e, { lockId: String(earlier[0].lockId), value: String(earlier[0].amount + earlier[0].fastFee), lockTx: "found on the chain" });
        save();
      } else {
        console.log(`  solana→${side} ${w.symbol}: lock ${w.whole} (${value} record units) to ${OPERATOR_EVM}`);
        if (DRY) return;
        const l = await v.lock({ asset: n, recipient: OPERATOR_EVM, amount: value * a.unit });
        Object.assign(e, { lockId: String(l.lockId), value: String(value), lockTx: l.signature });
        save();
      }
    }
    await issue(e, "solana", side);
  });
}

// ---------------------------------------------------------------------------
// Every pair, both directions
// ---------------------------------------------------------------------------

const pairs: [string, string][] = [];
for (const name of Object.keys(SDK_NAME)) {
  const d = network(SDK_NAME[name] as any);
  for (const v of d.vaults as readonly { peer: number; genesisEnd?: number }[]) {
    // Only vaults made in genesis (a network left out, as Hedera, keeps its old ones).
    if (!v.genesisEnd) continue;
    if (v.peer === SOLANA) pairs.push([name, "solana"]);
    else {
      const other = Object.keys(SDK_NAME).find((k) => network(SDK_NAME[k] as any).number === v.peer);
      if (other && d.number < v.peer) pairs.push([name, other]);
    }
  }
}
console.log(`genesis: ${pairs.length} pairs; operator ${OPERATOR_EVM} / ${OPERATOR_SOLANA}; ${RWA_WHOLE} of each RWA token; coins ${JSON.stringify(COIN)}${COINS_ONLY ? ", coins only" : ""}${ROUND > 1 ? `, round ${ROUND}` : ""}${DRY ? " (dry run)" : ""}`);
const skipped: string[] = [];
for (const [a, b] of pairs) {
  if (ONLY && a !== ONLY && b !== ONLY) continue;
  if (SKIP.includes(a) || SKIP.includes(b)) continue;
  if (a === "tempo" || b === "tempo") {
    skipped.push(`${a}–${b}: Tempo's own transactions need its sender`);
    continue;
  }
  console.log(`${a}–${b}`);
  try {
    if (b === "solana") {
      await fromEvm(a, "solana");
      await fromSolana(a);
    } else {
      await fromEvm(a, b);
      await fromEvm(b, a);
    }
  } catch (e) {
    skipped.push(`${a}–${b}: ${(e as Error).message}`);
    console.log(`  stopped: ${(e as Error).message}`);
  }
}
save();
const done = ledger.entries.filter((e) => e.issueTx).length;
console.log(`done: ${done} of ${ledger.entries.length} locks issued; ledger ${LEDGER}`);
for (const s of [...skipped, ...failed]) console.log(`not done: ${s}`);
