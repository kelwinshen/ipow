// Checks the vaults' genesis from the chain (docs/drafts/ipow-vault-genesis.md,
// D145). Read only:
//
//   node scripts/genesis-check.ts
//
// What it reads is the receipt sides' own record of genesis, not the run's
// ledger: every GenesisReceipt and GenesisIssued event of every vault pair,
// on each EVM network from its receipts part's logs and on Solana from the
// Anchor events in each pair's transactions. Then:
// - each GenesisIssued names a lock that exists on its home, of the same
//   asset, whose amount and fast fee are the value issued, for the same
//   recipient; and no lock is issued twice by genesis;
// - each GenesisReceipt states the home asset's token and record decimals;
// - each receipt made at genesis has a supply equal to what genesis issued
//   of it (run at the end of genesis, before claims add to supplies);
// - the run's ledger (ethereum/deployments/genesis-testnet.json), if any,
//   matches the events both ways.
// It prints each finding and exits 1 on any.

import { BorshCoder, EventParser } from "@coral-xyz/anchor";
import { Connection, PublicKey, type ConfirmedSignatureInfo } from "@solana/web3.js";
import { Contract, FetchRequest, Interface, JsonRpcProvider, ZeroAddress, getAddress, zeroPadValue, type Log } from "ethers";
import { existsSync, readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

import { NETWORKS } from "../../../programmable-network/ethereum/deploy/networks.ts";
import { SolanaVault, getLock, homeAssets, network, type Deployment } from "../src/index.ts";

const here = dirname(fileURLToPath(import.meta.url));
const pn = join(here, "..", "..", "..", "programmable-network");
const SOLANA = 2;
const SDK_NAME: Record<string, string> = { ethereum: "ethereum-testnet", base: "base-testnet", robinhood: "robinhood-testnet", hyperliquid: "hyperliquid-testnet", arbitrum: "arbitrum-testnet", hedera: "hedera-testnet", polkadot: "polkadot-testnet", tempo: "tempo-testnet" };
const connection = new Connection(process.env.SOLANA_RPC_URL ?? "https://api.devnet.solana.com", "confirmed");
const vaultIdl = JSON.parse(readFileSync(join(pn, "solana", "programs", "ipow-vault", "idls", "ipow_vault.json"), "utf8"));
const programId = new PublicKey(vaultIdl.address);
const parser = new EventParser(programId, new BorshCoder(vaultIdl));
const GENESIS = new Interface([
  "event GenesisReceipt(uint32 indexed asset, bytes32 token, uint8 decimals)",
  "event GenesisIssued(uint64 indexed lockId, uint32 indexed asset, uint64 value, address to)",
]);

function readEnv(file: string): Record<string, string> {
  const out: Record<string, string> = {};
  for (const line of readFileSync(file, "utf8").split("\n")) {
    const m = line.match(/^\s*([A-Z0-9_]+)\s*=\s*"?([^"\n]*)"?/);
    if (m) out[m[1]] = m[2].trim();
  }
  return out;
}
const evms = new Map<string, { d: Deployment; provider: JsonRpcProvider }>();
function evm(name: string) {
  if (!evms.has(name)) {
    const request = new FetchRequest(readEnv(join(pn, name, ".env"))[NETWORKS[name].rpcEnv.testnet!]);
    request.setHeader("user-agent", "ipow-genesis-check");
    evms.set(name, { d: network(SDK_NAME[name] as any), provider: new JsonRpcProvider(request, undefined, { staticNetwork: true }) });
  }
  return evms.get(name)!;
}
const nameOfNumber = (n: number) => (n === SOLANA ? "solana" : Object.keys(SDK_NAME).find((k) => evm(k).d.number === n)!);
const pairOn = (name: string, peer: number) => evm(name).d.vaults.findIndex((v) => v.peer === peer);
const b32 = (key: PublicKey) => "0x" + key.toBuffer().toString("hex");

/** One genesis event, wherever it was read. */
type Issued = { side: string; home: string; lockId: bigint; asset: number; value: bigint; to: string };
type Made = { side: string; home: string; asset: number; token: string; decimals: number };
const issued: Issued[] = [];
const made: Made[] = [];
const findings: string[] = [];

/** The first block at or after `time` (seconds), by bisection. */
async function blockAt(provider: JsonRpcProvider, time: number): Promise<number> {
  let lo = 0;
  let hi = await provider.getBlockNumber();
  while (lo < hi) {
    const mid = Math.floor((lo + hi) / 2);
    const b = await provider.getBlock(mid);
    if (b && b.timestamp < time) lo = mid + 1;
    else hi = mid;
  }
  return lo;
}

/** Logs of `address` from `from` to `to`, the range halved where a provider refuses it. */
async function logs(provider: JsonRpcProvider, address: string, from: number, to: number): Promise<Log[]> {
  try {
    return await provider.getLogs({ address, fromBlock: from, toBlock: to, topics: [[GENESIS.getEvent("GenesisReceipt")!.topicHash, GENESIS.getEvent("GenesisIssued")!.topicHash]] });
  } catch (e) {
    if (to - from < 1) throw e;
    const mid = Math.floor((from + to) / 2);
    return [...(await logs(provider, address, from, mid)), ...(await logs(provider, address, mid + 1, to))];
  }
}

// EVM receipt sides: every vault made in genesis.
for (const name of Object.keys(SDK_NAME)) {
  const record = JSON.parse(readFileSync(join(pn, "ethereum", "deployments", `${name}-testnet.json`), "utf8"));
  for (const v of record.vaults as { peer: number; receipts: string; genesisEnd?: number; at?: string }[]) {
    if (!v.genesisEnd) continue;
    const { provider } = evm(name);
    const from = v.at ? await blockAt(provider, Math.floor(Date.parse(v.at) / 1000) - 3_600) : 0;
    const home = nameOfNumber(v.peer);
    for (const log of await logs(provider, v.receipts, from, await provider.getBlockNumber())) {
      const p = GENESIS.parseLog(log)!;
      if (p.name === "GenesisReceipt") made.push({ side: name, home, asset: Number(p.args.asset), token: String(p.args.token).toLowerCase(), decimals: Number(p.args.decimals) });
      else issued.push({ side: name, home, lockId: BigInt(p.args.lockId), asset: Number(p.args.asset), value: BigInt(p.args.value), to: getAddress(p.args.to) });
    }
  }
}

// Solana's receipt side: every pair's transactions, their Anchor events.
for (const name of Object.keys(SDK_NAME)) {
  const peer = evm(name).d.number;
  const config = PublicKey.findProgramAddressSync([Buffer.from("config"), Buffer.from([peer])], programId)[0];
  if (!(await connection.getAccountInfo(config))) continue;
  let before: string | undefined;
  for (;;) {
    const page: ConfirmedSignatureInfo[] = await connection.getSignaturesForAddress(config, { before, limit: 1000 });
    if (!page.length) break;
    for (const sig of page) {
      if (sig.err) continue;
      const tx = await connection.getTransaction(sig.signature, { maxSupportedTransactionVersion: 0, commitment: "confirmed" });
      for (const ev of parser.parseLogs(tx?.meta?.logMessages ?? [])) {
        const d: any = ev.data;
        if (ev.name === "GenesisReceipt") made.push({ side: "solana", home: name, asset: Number(d.asset), token: getAddress("0x" + Buffer.from(d.token).toString("hex")).toLowerCase(), decimals: Number(d.decimals) });
        if (ev.name === "GenesisIssued") issued.push({ side: "solana", home: name, lockId: BigInt(d.lockId.toString()), asset: Number(d.asset), value: BigInt(d.value.toString()), to: new PublicKey(d.recipient).toBase58() });
      }
    }
    before = page[page.length - 1].signature;
  }
}

// Each receipt made states its home asset.
for (const m of made) {
  const tag = `${m.home}→${m.side} receipt of asset ${m.asset}`;
  if (m.home === "solana") {
    const a = (await new SolanaVault(connection, evm(m.side).d.number, undefined, { vault: programId.toBase58() }).assets()).find((x) => x.number === m.asset);
    const token = a ? (a.mint ? b32(new PublicKey(a.mint)) : "0x" + "00".repeat(32)) : null;
    if (!a) findings.push(`${tag}: no such asset on Solana`);
    else if (token !== m.token || a.recordDecimals !== m.decimals) findings.push(`${tag}: states ${m.token}, ${m.decimals} decimals; the asset is ${token}, ${a.recordDecimals}`);
  } else {
    const h = evm(m.home);
    const a = (await homeAssets(h.provider, h.d, pairOn(m.home, m.side === "solana" ? SOLANA : evm(m.side).d.number))).find((x) => x.number === m.asset);
    const token = a ? (m.side === "solana" ? getAddress(a.token).toLowerCase() : zeroPadValue(a.token, 32).toLowerCase()) : null;
    if (!a) findings.push(`${tag}: no such asset on ${m.home}`);
    else if (token !== m.token || a.recordDecimals !== m.decimals) findings.push(`${tag}: states ${m.token}, ${m.decimals} decimals; the asset is ${token}, ${a.recordDecimals}`);
  }
}

// Each lock issued exists on its home as issued, once.
const seen = new Set<string>();
for (const g of issued) {
  const tag = `${g.home}→${g.side} lock ${g.lockId} (asset ${g.asset})`;
  const key = `${g.home}|${g.side}|${g.lockId}`;
  if (seen.has(key)) findings.push(`${tag}: issued twice by genesis`);
  seen.add(key);
  try {
    if (g.home === "solana") {
      const l = await new SolanaVault(connection, evm(g.side).d.number, undefined, { vault: programId.toBase58() }).getLock(g.lockId);
      if (l.asset !== g.asset || l.amount + l.fastFee !== g.value || getAddress(l.recipient) !== g.to) findings.push(`${tag}: the lock is of asset ${l.asset}, value ${l.amount + l.fastFee}, for ${l.recipient}; genesis issued ${g.value} to ${g.to}`);
    } else {
      const h = evm(g.home);
      const l = await getLock(h.provider, h.d, g.lockId, pairOn(g.home, g.side === "solana" ? SOLANA : evm(g.side).d.number));
      const want = g.side === "solana" ? b32(new PublicKey(g.to)) : zeroPadValue(g.to, 32);
      const value = BigInt(l.amount) + BigInt(l.fastFee);
      if (l.asset !== g.asset || value !== g.value || String(l.recipient).toLowerCase() !== want.toLowerCase()) findings.push(`${tag}: the lock is of asset ${l.asset}, value ${value}, for ${l.recipient}; genesis issued ${g.value} to ${g.to}`);
    }
  } catch (e) {
    findings.push(`${tag}: no such lock on ${g.home} (${(e as Error).message})`);
  }
}

// Each receipt made at genesis holds what genesis issued of it.
for (const m of made) {
  const total = issued.filter((g) => g.side === m.side && g.home === m.home && g.asset === m.asset).reduce((s, g) => s + g.value, 0n);
  let supply: bigint;
  if (m.side === "solana") {
    supply = BigInt((await connection.getTokenSupply(new SolanaVault(connection, evm(m.home).d.number, undefined, { vault: programId.toBase58() }).receiptMint(m.asset))).value.amount);
  } else {
    const r = evm(m.side);
    const receipts = new Contract(r.d.vaults[pairOn(m.side, m.home === "solana" ? SOLANA : evm(m.home).d.number)].receipts, ["function receiptOf(uint32) view returns (address)"], r.provider);
    const token = await receipts.receiptOf(m.asset);
    supply = token === ZeroAddress ? 0n : BigInt(await new Contract(token, ["function totalSupply() view returns (uint256)"], r.provider).totalSupply());
  }
  if (supply !== total) findings.push(`${m.home}→${m.side} receipt of asset ${m.asset}: supply ${supply}, genesis issued ${total}`);
}

// The run's ledger against the chain, both ways.
const ledgerFile = join(pn, "ethereum", "deployments", "genesis-testnet.json");
if (existsSync(ledgerFile)) {
  const entries: { home: string; receiptSide: string; asset: number; lockId?: string; value?: string; issueTx?: string }[] = JSON.parse(readFileSync(ledgerFile, "utf8")).entries;
  for (const e of entries) {
    if (!e.issueTx) findings.push(`ledger ${e.home}→${e.receiptSide} asset ${e.asset}: not issued yet`);
    else if (!issued.some((g) => g.home === e.home && g.side === e.receiptSide && String(g.lockId) === e.lockId && String(g.value) === e.value)) findings.push(`ledger ${e.home}→${e.receiptSide} lock ${e.lockId}: no GenesisIssued on chain matches it`);
  }
  for (const g of issued) if (!entries.some((e) => e.home === g.home && e.receiptSide === g.side && e.lockId === String(g.lockId))) findings.push(`${g.home}→${g.side} lock ${g.lockId}: issued by genesis, not in the ledger`);
}

console.log(`read ${made.length} receipts made and ${issued.length} locks issued at genesis`);
for (const f of findings) console.log(`FINDING ${f}`);
console.log(findings.length ? `${findings.length} findings` : "every genesis receipt states its home asset, every genesis issue is backed by its lock, and nothing else was issued");
process.exit(findings.length ? 1 : 0);
