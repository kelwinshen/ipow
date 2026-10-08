// Tempo's part of the vaults' genesis (docs/drafts/ipow-vault-genesis.md),
// which packages/sdk/scripts/genesis.ts cannot send: Tempo's transactions
// are its own type, sent here with viem's Tempo support as the vault's
// deploy does. For each Tempo test token asked for, Tempo → Solana:
// registered with the pair's home vault if it is not, its receipt made on
// Solana (G1), an amount minted to the deployer and locked to `--recipient`
// on Solana, and that lock issued there (G2). Written to the same ledger
// (ethereum/deployments/genesis-testnet.json), so genesis-check.ts reads it.
// Run from programmable-network/tempo:
//
//   node scripts/genesis-tempo.ts [--symbols EURC,USDY,syrupUSDC] [--amount 10000] [--recipient <solana address>] [--dry]
//
// The recipient is the operator's Solana address by default (the Solana
// CLI key); naming your own wallet puts the receipts there at once. Keys:
// Tempo's .env (the deployer, genesis key of its vault), Solana's
// ~/.config/solana/id.json (the genesis key of Solana's pairs). Nothing
// secret is printed. SOLANA_RPC_URL picks Solana's endpoint.

import fs from "node:fs";
import { createRequire } from "node:module";
import { homedir } from "node:os";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { createClient, http, Account } from "viem/tempo";
import { encodeFunctionData } from "viem";
import { Contract, FetchRequest, JsonRpcProvider } from "ethers";

import { NETWORKS } from "../../ethereum/deploy/networks.ts";
import { SolanaVault } from "../../../packages/sdk/src/index.ts";

const here = dirname(fileURLToPath(import.meta.url));
// Solana's library as the SDK resolves it, so its classes are the SDK's own.
const web3 = createRequire(join(here, "..", "..", "..", "packages", "sdk", "package.json"))("@solana/web3.js");
const argv = process.argv.slice(2);
const flag = (name: string) => (argv.includes(name) ? argv[argv.indexOf(name) + 1] : undefined);
const DRY = argv.includes("--dry");
const SYMBOLS = (flag("--symbols") ?? "EURC,USDY,syrupUSDC").split(",").map((x) => x.trim());
const WHOLE = flag("--amount") ?? "10000";
const solanaKey = web3.Keypair.fromSecretKey(Uint8Array.from(JSON.parse(fs.readFileSync(join(homedir(), ".config/solana/id.json"), "utf8"))));
const RECIPIENT = new web3.PublicKey(flag("--recipient") ?? solanaKey.publicKey.toBase58());
const TEMPO = NETWORKS.tempo.number;

const env = Object.fromEntries(
  fs.readFileSync(join(here, "..", ".env"), "utf8").split("\n").filter((l) => l.includes("=") && !l.startsWith("#")).map((l) => {
    const i = l.indexOf("=");
    return [l.slice(0, i).trim(), l.slice(i + 1).trim().replace(/^"|"$/g, "")];
  })
);
const s = NETWORKS.tempo;
const account = Account.fromSecp256k1(env[s.keyEnv.testnet]);
const client = createClient({ account, testnet: true, transport: http(env[s.rpcEnv.testnet]) });
const PATHUSD = s.coin.kind === "token" ? s.coin.token.testnet! : "";
const request = new FetchRequest(env[s.rpcEnv.testnet]);
request.setHeader("user-agent", "ipow-genesis");
const reader = new JsonRpcProvider(request, undefined, { staticNetwork: true });

const record = JSON.parse(fs.readFileSync(join(here, "..", "..", "ethereum", "deployments", "tempo-testnet.json"), "utf8"));
const vault = record.vaults.find((v: { peer: number; genesisEnd?: number }) => v.peer === 2);
if (!vault?.genesisEnd) throw new Error("Tempo's vault with Solana is not a genesis vault");
const tokens = JSON.parse(fs.readFileSync(join(here, "..", "..", "ethereum", "deployments", "tempo-testnet-rwa.json"), "utf8")).tokens;

const HOME = ["function registerAsset(address) returns (uint32)", "function assetOfToken(address) view returns (uint32)", "function getAsset(uint32) view returns (tuple(address token, uint8 decimals, uint8 recordDecimals, uint256 unit))", "function lockCount() view returns (uint64)", "function lock(uint32,bytes32,uint64,uint64,uint64) payable returns (uint64)"];
const TOKEN = ["function mint(address,uint256)", "function balanceOf(address) view returns (uint256)", "function approve(address,uint256) returns (bool)"];
const home = new Contract(vault.home, HOME, reader);

/** A call sent as Tempo's own transaction, fees in PathUSD, in the ordinary nonce lane. */
async function call(to: string, abi: string[], fn: string, args: unknown[]) {
  const data = encodeFunctionData({ abi: new Contract(to, abi).interface.fragments.map((f) => JSON.parse(f.format("json"))) as any, functionName: fn, args });
  const nonce = await client.getTransactionCount({ address: account.address });
  const hash = await client.sendTransaction({ to: to as `0x${string}`, data, nonce, nonceKey: 0n, feeToken: PATHUSD } as any);
  const receipt = await client.waitForTransactionReceipt({ hash });
  if (receipt.status !== "success") throw new Error(`${fn}: reverted, ${hash}`);
  return hash;
}
const until = async <T,>(read: () => Promise<T>, ok: (v: T) => boolean): Promise<T> => {
  let v = await read();
  for (let i = 0; !ok(v) && i < 12; i++) {
    await new Promise((r) => setTimeout(r, 5_000));
    v = await read();
  }
  return v;
};

const connection = new web3.Connection(process.env.SOLANA_RPC_URL ?? "https://api.devnet.solana.com", "confirmed");
const wallet = {
  publicKey: solanaKey.publicKey,
  signTransaction: async (t: any) => (t.partialSign(solanaKey), t),
  signAllTransactions: async (ts: any[]) => ts.map((t) => (t.partialSign(solanaKey), t)),
};
const solana = new SolanaVault(connection, TEMPO, wallet as any);

const LEDGER = join(here, "..", "..", "ethereum", "deployments", "genesis-testnet.json");
const ledger = fs.existsSync(LEDGER) ? JSON.parse(fs.readFileSync(LEDGER, "utf8")) : { started: new Date().toISOString(), entries: [] };
const save = () => !DRY && fs.writeFileSync(LEDGER, JSON.stringify(ledger, null, 2) + "\n");

console.log(`tempo→solana genesis: ${SYMBOLS.join(", ")}, ${WHOLE} each, to ${RECIPIENT.toBase58()}${DRY ? " (dry run)" : ""}`);
for (const symbol of SYMBOLS) {
  const t = tokens[symbol];
  if (!t) {
    console.log(`  ${symbol}: not a Tempo test token`);
    continue;
  }
  try {
    let plusOne = Number(await home.assetOfToken(t.address));
    if (!plusOne) {
      console.log(`  ${symbol}: register`);
      if (DRY) continue;
      await call(vault.home, HOME, "registerAsset", [t.address]);
      plusOne = Number(await until(() => home.assetOfToken(t.address), (v) => Number(v) > 0));
    }
    const n = plusOne - 1;
    const a = await home.getAsset(n);
    const unit = BigInt(a.unit);
    const recordDecimals = Number(a.recordDecimals);
    let e = ledger.entries.find((x: any) => x.home === "tempo" && x.receiptSide === "solana" && x.asset === n);
    if (!e) ledger.entries.push((e = { home: "tempo", receiptSide: "solana", asset: n, symbol, token: t.address, recordDecimals, recipient: RECIPIENT.toBase58() }));
    if (e.issueTx) {
      console.log(`  ${symbol}: issued already`);
      continue;
    }
    if (!(await connection.getAccountInfo(solana.receiptMint(n)))) {
      console.log(`  ${symbol}: G1, asset ${n}, ${recordDecimals} decimals`);
      if (DRY) continue;
      e.receiptTx = await solana.genesisMakeReceipt(n, t.address, recordDecimals);
      save();
    }
    if (!e.lockId) {
      const amount = BigInt(WHOLE) * 10n ** BigInt(t.decimals);
      const records = amount / unit;
      console.log(`  ${symbol}: lock ${WHOLE} (${records} record units) to ${RECIPIENT.toBase58()}`);
      if (DRY) continue;
      const token = new Contract(t.address, TOKEN, reader);
      if (BigInt(await token.balanceOf(account.address)) < amount) await call(t.address, TOKEN, "mint", [account.address, amount]);
      await call(t.address, TOKEN, "approve", [vault.home, records * unit]);
      const before = BigInt(await home.lockCount());
      const hash = await call(vault.home, HOME, "lock", [n, "0x" + RECIPIENT.toBuffer().toString("hex"), records, 0n, 0n]);
      const lockId = BigInt(await until(() => home.lockCount(), (v) => BigInt(v) > before));
      Object.assign(e, { lockId: String(lockId), value: String(records), lockTx: hash });
      save();
    }
    if ((await solana.evmLockMark(BigInt(e.lockId)))?.issued) {
      e.issueTx = e.issueTx ?? "issued already (read from the chain)";
    } else {
      console.log(`  ${symbol}: G2, lock ${e.lockId}`);
      if (DRY) continue;
      e.issueTx = await solana.genesisIssue(BigInt(e.lockId), n, BigInt(e.value), RECIPIENT.toBase58());
    }
    save();
  } catch (err) {
    console.log(`  ${symbol}: stopped: ${(err as Error).message}`);
  }
}
save();
console.log(`done; ledger ${LEDGER}`);
