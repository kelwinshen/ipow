// The protocol's vault on an EVM network (spec section 11): lock an asset
// for its receipt on the pair's other network, and burn a receipt for its
// asset back. Amounts in a vault are in record units, the receipt's
// smallest unit (gwei for ETH); the SDK converts from the coin's own units
// and refuses an amount that does not divide. What happens on the other
// network (the receipt issued, a burn paid) is read there.

import { Contract, ZeroAddress, getAddress, type Provider, type Signer } from "ethers";

import { ABIS } from "./generated/abis.ts";
import { rpcScale, type Deployment } from "./networks.ts";
import type { LockMark } from "./solana.ts";

/** Solana's network number (D133). */
const SOLANA = 2;

export type HomeAsset = {
  number: number;
  /** Zero for the network's native coin. */
  token: string;
  decimals: number;
  /** The receipt's decimals: records count in its unit. */
  recordDecimals: number;
  /** The asset's own units per record unit. */
  unit: bigint;
};

export type LockQuote = {
  network: string;
  pair: number;
  asset: HomeAsset;
  /** In record units. */
  amount: bigint;
  fee: bigint;
  fastFee: bigint;
  /** All the lock takes, in the asset's own units. */
  total: bigint;
  /** What the wallet sends with the call: `total` in its RPC's units for the
   *  native coin, 0 for a token (taken with an approval). */
  value: bigint;
};

export type LockState = {
  lockId: bigint;
  /** `Locked` until an operator's message carrying it is judged true here,
   *  then `Carried`; `Returned` if it was given back. */
  stage: "Locked" | "Carried" | "Returned";
  owner: string;
  asset: number;
  amount: bigint;
  fee: bigint;
  fastFee: bigint;
  lockedAt: number;
  recipient: string;
};

export type BurnState = {
  requestId: bigint;
  /** `Burned` until an operator's message carrying it is judged true here,
   *  then `Carried`; the asset is paid on the other network. */
  stage: "Burned" | "Carried";
  owner: string;
  asset: number;
  amount: bigint;
  fee: bigint;
  fastFee: bigint;
  at: number;
  to: string;
};

function pairOf(d: Deployment, pair: number) {
  const p = d.vaults[pair];
  if (!p) throw new Error(`${d.name}: no vault pair ${pair}`);
  return p;
}

// Genesis (docs/specs/ipow-vault-genesis.md): a pair's receipts part may
// start in genesis, when its genesis key makes receipts and issues named
// locks of the peer until it finalizes or its end.

/** Pair `pair`'s genesis on this network: its key (null with none), its
 *  end (seconds), and whether it is open now. */
export async function genesisOf(provider: Provider, d: Deployment, pair = 0): Promise<{ key: string | null; end: number; finalized: boolean; open: boolean }> {
  const r = new Contract(pairOf(d, pair).receipts, ABIS.vaultReceipts, provider);
  const [key, end, finalized, open] = await Promise.all([r.genesisKey(), r.genesisEnd(), r.genesisFinalized(), r.genesisOpen()]);
  return { key: key === ZeroAddress ? null : key, end: Number(end), finalized, open };
}

/** G1: makes the receipt here of the peer's asset `asset`: its token there
 *  (32 bytes; zero for the coin) and its record decimals there. */
export async function genesisMakeReceipt(signer: Signer, d: Deployment, pair: number, asset: number, token: string, decimals: number, overrides: Record<string, unknown> = {}): Promise<string> {
  const r = new Contract(pairOf(d, pair).receipts, ABIS.vaultReceipts, signer);
  const tx = await r.genesisMakeReceipt(asset, token, decimals, overrides);
  await tx.wait();
  return tx.hash;
}

/** G2: issues the peer's lock `lockId` of `asset`, its value (amount and
 *  fast fee, record units) to `to` here. */
export async function genesisIssue(signer: Signer, d: Deployment, pair: number, lockId: bigint | number, asset: number, value: bigint, to: string, overrides: Record<string, unknown> = {}): Promise<string> {
  const r = new Contract(pairOf(d, pair).receipts, ABIS.vaultReceipts, signer);
  const tx = await r.genesisIssue(lockId, asset, value, to, overrides);
  await tx.wait();
  return tx.hash;
}

/** Ends pair `pair`'s genesis on this network for good. */
export async function finalizeGenesis(signer: Signer, d: Deployment, pair = 0, overrides: Record<string, unknown> = {}): Promise<string> {
  const r = new Contract(pairOf(d, pair).receipts, ABIS.vaultReceipts, signer);
  const tx = await r.finalizeGenesis(overrides);
  await tx.wait();
  return tx.hash;
}

/**
 * A recipient on the pair's other network in the 32 bytes a vault takes:
 * a Solana account from base58, or an EVM address padded on the left.
 */
export function encodeRecipient(peer: number, address: string): string {
  if (peer !== SOLANA) return "0x" + getAddress(address).slice(2).toLowerCase().padStart(64, "0");
  const A = "123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz";
  let n = 0n;
  for (const c of address) {
    const d = A.indexOf(c);
    if (d < 0) throw new Error(`not a Solana address: ${address}`);
    n = n * 58n + BigInt(d);
  }
  const zeros = address.length - address.replace(/^1+/, "").length;
  const hex = n === 0n ? "" : n.toString(16).padStart(Math.ceil(n.toString(16).length / 2) * 2, "0");
  if (zeros + hex.length / 2 !== 32) throw new Error(`not a 32-byte Solana address: ${address}`);
  return "0x" + hex.padStart(64, "0");
}

/** The assets this network's vault holds for its pair, asset 0 its coin. */
export async function homeAssets(provider: Provider, d: Deployment, pair = 0): Promise<HomeAsset[]> {
  const home = new Contract(pairOf(d, pair).home, ABIS.vaultHome, provider);
  const count = Number(await home.assetCount());
  // Read together: one round trip's wait, not one per asset.
  const all = await Promise.all(Array.from({ length: count }, (_, i) => home.getAsset(i)));
  return all.map((a, i) => ({ number: i, token: a.token, decimals: Number(a.decimals), recordDecimals: Number(a.recordDecimals), unit: a.unit }));
}

/** The receipts made on this network of the other network's assets, by
 *  that asset's number there. */
export async function receiptTokens(provider: Provider, d: Deployment, pair = 0, upTo = 32): Promise<Map<number, string>> {
  const receipts = new Contract(pairOf(d, pair).receipts, ABIS.vaultReceipts, provider);
  const found = new Map<number, string>();
  const all = await Promise.all(Array.from({ length: upTo }, (_, i) => receipts.receiptOf(i) as Promise<string>));
  all.forEach((r, i) => r !== ZeroAddress && found.set(i, r));
  return found;
}

/**
 * Prices a lock of `amount` of asset `asset`, in the asset's own units, with
 * a fee for the operator who carries it (D113) and a fast fee for an
 * attester who issues the receipt at once (D122), also in its own units.
 */
export async function quoteLock(
  provider: Provider,
  d: Deployment,
  options: { asset?: number; amount: bigint; fee?: bigint; fastFee?: bigint; pair?: number }
): Promise<LockQuote> {
  const pair = options.pair ?? 0;
  const asset = (await homeAssets(provider, d, pair))[options.asset ?? 0];
  if (!asset) throw new Error(`${d.name}: no asset ${options.asset}`);
  const record = (x: bigint, what: string) => {
    if (x % asset.unit !== 0n) throw new Error(`${what} must be a whole number of record units (${asset.unit} each)`);
    return x / asset.unit;
  };
  const amount = record(options.amount, "the amount");
  if (amount === 0n) throw new Error("an amount above zero");
  const fee = record(options.fee ?? 0n, "the fee");
  const fastFee = record(options.fastFee ?? 0n, "the fast fee");
  const total = (amount + fee + fastFee) * asset.unit;
  const native = asset.token === ZeroAddress;
  return { network: d.name, pair, asset, amount, fee, fastFee, total, value: native ? total * rpcScale(d) : 0n };
}

function event(contract: Contract, receipt: any, name: string) {
  for (const log of receipt.logs) {
    if (log.address.toLowerCase() !== String(contract.target).toLowerCase()) continue;
    const parsed = contract.interface.parseLog(log);
    if (parsed?.name === name) return parsed.args;
  }
  throw new Error(`no ${name} in ${receipt.hash}`);
}

/** Locks for the receipt on the other network, to `recipient` there. A
 *  token is approved first, for exactly what the lock takes. */
/** `overrides` are set on each transaction it sends (e.g. a legacy gas
 *  price where a network's relay misprices typed ones, as Hedera's). */
export async function lock(signer: Signer, d: Deployment, quote: LockQuote, recipient: string, overrides: Record<string, unknown> = {}): Promise<{ lockId: bigint; txHash: string }> {
  if (quote.network !== d.name) throw new Error(`a quote for ${quote.network}, not ${d.name}`);
  const p = pairOf(d, quote.pair);
  const home = new Contract(p.home, ABIS.vaultHome, signer);
  if (quote.asset.token !== ZeroAddress) {
    const token = new Contract(quote.asset.token, ["function approve(address,uint256) returns (bool)"], signer);
    await (await token.approve(p.home, quote.total, overrides)).wait();
  }
  const tx = await home.lock(quote.asset.number, encodeRecipient(p.peer, recipient), quote.amount, quote.fee, quote.fastFee, { ...overrides, value: quote.value });
  const receipt = await tx.wait();
  return { lockId: event(home, receipt, "Locked").lockId as bigint, txHash: tx.hash };
}

export async function getLock(provider: Provider, d: Deployment, lockId: bigint | number, pair = 0): Promise<LockState> {
  const home = new Contract(pairOf(d, pair).home, ABIS.vaultHome, provider);
  const l = await home.getLock(lockId);
  if (l.owner === ZeroAddress) throw new Error(`${d.name}: no lock ${lockId}`);
  return {
    lockId: BigInt(lockId),
    stage: l.returned ? "Returned" : l.feePaid ? "Carried" : "Locked",
    owner: l.owner,
    asset: Number(l.asset),
    amount: l.amount,
    fee: l.fee,
    fastFee: l.fastFee,
    lockedAt: Number(l.lockedAt),
    recipient: l.recipient,
  };
}

/**
 * Burns receipts of the other network's asset `asset` (its number there)
 * for the asset, paid to `to` there. In record units: the receipt's own.
 * The receipts are taken by the vault, which mints them; no approval.
 */
export async function burn(
  signer: Signer,
  d: Deployment,
  options: { asset: number; to: string; amount: bigint; fee?: bigint; fastFee?: bigint; pair?: number }
): Promise<{ requestId: bigint; txHash: string }> {
  const p = pairOf(d, options.pair ?? 0);
  const receipts = new Contract(p.receipts, ABIS.vaultReceipts, signer);
  const tx = await receipts.burn(options.asset, encodeRecipient(p.peer, options.to), options.amount, options.fee ?? 0n, options.fastFee ?? 0n);
  const receipt = await tx.wait();
  return { requestId: event(receipts, receipt, "Burned").requestId as bigint, txHash: tx.hash };
}

export async function getBurn(provider: Provider, d: Deployment, requestId: bigint | number, pair = 0): Promise<BurnState> {
  const receipts = new Contract(pairOf(d, pair).receipts, ABIS.vaultReceipts, provider);
  const b = await receipts.getBurn(requestId);
  if (b.owner === ZeroAddress) throw new Error(`${d.name}: no burn ${requestId}`);
  return {
    requestId: BigInt(requestId),
    stage: b.feePaid ? "Carried" : "Burned",
    owner: b.owner,
    asset: Number(b.asset),
    amount: b.amount,
    fee: b.fee,
    fastFee: b.fastFee,
    at: Number(b.at),
    to: b.to,
  };
}

/** Reads ids from `count` down, 20 at a time, keeping those `mine` accepts,
 *  at most `limit`: a test network's few locks or burns. A busy network
 *  wants its events, by owner, instead. */
async function newestOf<T>(count: number, limit: number, read: (id: number) => Promise<T>, mine: (x: T) => boolean): Promise<T[]> {
  const out: T[] = [];
  for (let top = count; top >= 1 && out.length < limit; top -= 20) {
    const ids = Array.from({ length: Math.min(20, top) }, (_, i) => top - i);
    for (const x of await Promise.all(ids.map(read))) if (mine(x) && out.length < limit) out.push(x);
  }
  return out;
}

/** The locks `owner` made on this network, newest first, at most `limit`. */
export async function locksOf(provider: Provider, d: Deployment, owner: string, limit = 50, pair = 0): Promise<LockState[]> {
  const count = Number(await new Contract(pairOf(d, pair).home, ABIS.vaultHome, provider).lockCount());
  const who = owner.toLowerCase();
  return newestOf(count, limit, (id) => getLock(provider, d, id, pair), (l) => l.owner.toLowerCase() === who);
}

/** The burns `owner` made on this network, newest first, at most `limit`. */
export async function burnsOf(provider: Provider, d: Deployment, owner: string, limit = 50, pair = 0): Promise<BurnState[]> {
  const count = Number(await new Contract(pairOf(d, pair).receipts, ABIS.vaultReceipts, provider).burnCount());
  const who = owner.toLowerCase();
  return newestOf(count, limit, (id) => getBurn(provider, d, id, pair), (b) => b.owner.toLowerCase() === who);
}

/** Whether lock `lockId` made on the pair's other network had its receipt
 *  issued here; null while no claim or attest has touched it. */
export async function receiptMark(
  provider: Provider,
  d: Deployment,
  lockId: bigint | number,
  pair = 0
): Promise<LockMark | null> {
  const m = await new Contract(pairOf(d, pair).receipts, ABIS.vaultReceipts, provider).getMark(lockId);
  if (!m.issued && !m.givenUp && Number(m.attests) === 0) return null;
  return { issued: m.issued, givenUp: m.givenUp, attests: Number(m.attests), settled: Number(m.settled) };
}

/** Whether burn `requestId` made on the pair's other network was paid here
 *  by its claim (an attester may have paid it at once before). */
export async function requestPaid(provider: Provider, d: Deployment, requestId: bigint | number, pair = 0): Promise<boolean> {
  return new Contract(pairOf(d, pair).home, ABIS.vaultHome, provider).requestPaid(requestId);
}
