// Conversion on an EVM network (contracts/applications/conversion/Conversion.sol, design
// docs/specs/ipow-conversion-app.md): sell the network's coin or a token
// for BTC, or buy it with BTC, an operator taking the other side. Apps give
// and show Bitcoin addresses; the contract holds output scripts.

import { Contract, ZeroAddress, hexlify, toUtf8Bytes, toUtf8String, type Provider, type Signer } from "ethers";

import { Bitcoin } from "./bitcoin.ts";
import { addressToScript, scriptToAddress } from "./btcAddress.ts";
import { jobGas } from "./gas.ts";
import { ABIS } from "./generated/abis.ts";
import { getJob, type JobView } from "./jobs.ts";
import { rpcScale, type Deployment } from "./networks.ts";
import { priceNow, type QuoteOptions } from "./quote.ts";

export const SIDES = ["None", "Sell", "Buy"] as const;
export const SWAP_STATES = ["None", "Open", "Funded", "Done", "Refunded", "Cancelled", "Reclaimed"] as const;

export type SwapQuote = {
  network: string;
  confirmations: number;
  commitmentFee: bigint;
  escrowFee: bigint;
  /** Paid on top against a rise in price; goes to the operator (D79). */
  margin: bigint;
  /** The job's fees: what `fees` is in a sell or a buy. */
  fees: bigint;
};

export type SwapView = {
  id: bigint;
  side: (typeof SIDES)[number];
  state: (typeof SWAP_STATES)[number];
  user: string;
  token: string;
  amount: bigint;
  /** Sell: the least the user accepts. Buy: what the user pays. */
  sats: bigint;
  /** Sell: the user's address, to be paid. Buy: the operator's, once named. */
  address: string | null;
  job: JobView;
  /** Buy, once funded: the Bitcoin blocks the user's payment must be in. */
  payBlocks: { first: number; last: number } | null;
  /** Sell in a tunnel: the Bitcoin blocks its payment must be mined in; null
   *  for an ordinary sell. */
  payWindow: { first: number; last: number } | null;
  /** Buy: the user's own Bitcoin address, as named when the buy was opened;
   *  null for none. Payments are accepted from anywhere. */
  userAddress: string | null;
  /** Buy: the note for operators, as text; empty for none. */
  memo: string;
  /** What the swap waits for now, in words an app can show. */
  waitingFor: string | null;
};

const conversion = (d: Deployment, runner: Provider | Signer) => new Contract(d.conversion, ABIS.conversion, runner);

/** The fees of a swap's job now, with a margin (100% of the commitment fee
 *  by default): the escrow is the protocol's lowest, 5 times the fee (D56). */
export async function quoteSwap(provider: Provider, d: Deployment, options: QuoteOptions = {}): Promise<SwapQuote> {
  const confirmations = options.confirmations ?? 6;
  const protocol = new Contract(d.protocol, ABIS.protocol, provider);
  const c = conversion(d, provider);
  const price = await priceNow(provider, d);
  const [commitmentFee, multiple, bps, BPS] = await Promise.all([
    protocol.commitmentFeeAt(confirmations, price) as Promise<bigint>,
    protocol.MIN_ESCROW_MULTIPLE() as Promise<bigint>,
    c.ESCROW_FEE_BPS() as Promise<bigint>,
    protocol.BPS() as Promise<bigint>,
  ]);
  const escrow = multiple * commitmentFee || 1n;
  const escrowFee = (escrow * BigInt(bps)) / BigInt(BPS);
  const margin = (commitmentFee * (options.marginBps ?? 10_000n)) / 10_000n;
  return { network: d.name, confirmations, commitmentFee, escrowFee, margin, fees: commitmentFee + margin + escrowFee };
}

function opened(c: Contract, receipt: any, name: "Sold" | "Bought") {
  for (const log of receipt.logs) {
    if (log.address.toLowerCase() !== String(c.target).toLowerCase()) continue;
    const parsed = c.interface.parseLog(log);
    if (parsed?.name === name) return { swapId: parsed.args.swapId as bigint, jobId: parsed.args.jobId as bigint };
  }
  throw new Error(`no ${name} in ${receipt.hash}`);
}

async function approve(signer: Signer, token: string, spender: string, amount: bigint) {
  const t = new Contract(token, ["function approve(address,uint256) returns (bool)"], signer);
  await (await t.approve(spender, amount)).wait();
}

/** Sells `amount` of `token` (zero for the network's coin) for at least
 *  `sats`, paid to the Bitcoin address `to`. */
export async function sell(
  signer: Signer,
  d: Deployment,
  quote: SwapQuote,
  options: { token?: string; amount: bigint; sats: bigint; to: string }
): Promise<{ swapId: bigint; jobId: bigint; txHash: string }> {
  if (d.coin.kind !== "native") throw new Error(`${d.name}: a token coin's swap needs its approvals, not built yet`);
  const token = options.token ?? ZeroAddress;
  const c = conversion(d, signer);
  if (token !== ZeroAddress) await approve(signer, token, d.conversion, options.amount);
  const value = ((token === ZeroAddress ? options.amount : 0n) + quote.fees) * rpcScale(d);
  const args = [token, options.amount, options.sats, addressToScript(options.to), quote.confirmations, quote.fees];
  const tx = await c.sell(...args, { value, gasLimit: await jobGas(c, "sell", args, value) });
  return { ...opened(c, await tx.wait(), "Sold"), txHash: tx.hash };
}

/**
 * A sell as one leg of a tunnel (T1, docs/specs/ipow-conversion-tunnel.md):
 * as `sell`, paid to `to` (the address the funded buy on the other network
 * named), its payment counting only when mined in that buy's payment blocks,
 * `window`. A payment mined outside them refunds the user.
 */
export async function sellInWindow(
  signer: Signer,
  d: Deployment,
  quote: SwapQuote,
  options: { token?: string; amount: bigint; sats: bigint; to: string; window: { first: number; last: number } }
): Promise<{ swapId: bigint; jobId: bigint; txHash: string }> {
  if (d.coin.kind !== "native") throw new Error(`${d.name}: a token coin's swap needs its approvals, not built yet`);
  const token = options.token ?? ZeroAddress;
  const c = conversion(d, signer);
  if (token !== ZeroAddress) await approve(signer, token, d.conversion, options.amount);
  const value = ((token === ZeroAddress ? options.amount : 0n) + quote.fees) * rpcScale(d);
  const args = [token, options.amount, options.sats, addressToScript(options.to), options.window.first, options.window.last, quote.confirmations, quote.fees];
  const tx = await c.sellInWindow(...args, { value, gasLimit: await jobGas(c, "sellInWindow", args, value) });
  return { ...opened(c, await tx.wait(), "Sold"), txHash: tx.hash };
}

/** A buy whose coin goes to `recipient`, the signer paying the fees (T2):
 *  how an operator opens a tunnel's buy for the user. */
export async function buyFor(
  signer: Signer,
  d: Deployment,
  quote: SwapQuote,
  options: { recipient: string; token?: string; amount: bigint; sats: bigint }
): Promise<{ swapId: bigint; jobId: bigint; txHash: string }> {
  if (d.coin.kind !== "native") throw new Error(`${d.name}: a token coin's swap needs its approvals, not built yet`);
  const c = conversion(d, signer);
  const args = [options.recipient, options.token ?? ZeroAddress, options.amount, options.sats, quote.confirmations, quote.fees];
  const value = quote.fees * rpcScale(d);
  const tx = await c.buyFor(...args, { value, gasLimit: await jobGas(c, "buyFor", args, value) });
  return { ...opened(c, await tx.wait(), "Bought"), txHash: tx.hash };
}

/** Buys `amount` of `token` (zero for the network's coin) for `sats`. Once
 *  an operator locks it, the swap names where and when to pay. `from` is
 *  the user's own Bitcoin address, kept on the swap for apps to show (any
 *  wallet may pay); `memo` a note for operators (a tunnel's terms,
 *  `tunnelMemo`), as text. */
export async function buy(
  signer: Signer,
  d: Deployment,
  quote: SwapQuote,
  options: { token?: string; amount: bigint; sats: bigint; from?: string; memo?: string }
): Promise<{ swapId: bigint; jobId: bigint; txHash: string }> {
  if (d.coin.kind !== "native") throw new Error(`${d.name}: a token coin's swap needs its approvals, not built yet`);
  const c = conversion(d, signer);
  const memo = options.memo ? hexlify(toUtf8Bytes(options.memo)) : "0x";
  const args = [options.token ?? ZeroAddress, options.amount, options.sats, options.from ? addressToScript(options.from) : "0x", memo, quote.confirmations, quote.fees];
  const value = quote.fees * rpcScale(d);
  const tx = await c.buy(...args, { value, gasLimit: await jobGas(c, "buy", args, value) });
  return { ...opened(c, await tx.wait(), "Bought"), txHash: tx.hash };
}

/** A memo's bytes as the text they were written as; hex when not text. */
function memoText(hex: string | undefined): string {
  if (!hex || hex === "0x") return "";
  try {
    return toUtf8String(hex);
  } catch {
    return hex;
  }
}

export async function getSwap(provider: Provider, d: Deployment, swapId: bigint | number): Promise<SwapView> {
  const c = conversion(d, provider);
  const s = await c.getSwap(swapId);
  const side = SIDES[Number(s.side)];
  if (side === "None") throw new Error(`${d.name}: no swap ${swapId}`);
  const state = SWAP_STATES[Number(s.state)];
  const job = await getJob(provider, d, s.jobId);
  const address = s.script === "0x" ? null : scriptToAddress(s.script);
  let payBlocks: SwapView["payBlocks"] = null;
  if (side === "Buy" && state === "Funded") {
    const anchor = Number((await new Contract(d.protocol, ABIS.protocol, provider).getDuty(s.jobId)).anchor.height);
    payBlocks = { first: anchor + 1, last: anchor + Number(await c.PAY_BLOCKS()) };
  }
  const sats = BigInt(s.sats);
  const waiting: Record<string, string | null> = {
    "Sell/Open":
      job.stage === "Auction" || job.stage === "Assigned"
        ? `an operator to pay at least ${sats} sats to ${address}; then the job's proof and its lock`
        : job.stage === "Proven"
          ? `the proof's lock to end; then the operator collects the coin`
          : `a refund: the job ${job.stage === "Expired" ? "found no operator" : "failed"}`,
    "Buy/Open": `an operator to lock the coin, within 30 minutes of the auction's end`,
    "Buy/Funded": payBlocks ? `your payment of ${sats} sats to ${address}, in Bitcoin blocks ${payBlocks.first} to ${payBlocks.last}` : null,
  };
  return {
    id: BigInt(swapId),
    side,
    state,
    user: s.user,
    token: s.token,
    amount: s.amount,
    sats,
    address,
    job,
    payBlocks,
    payWindow: Number(s.payFrom ?? 0) === 0 ? null : { first: Number(s.payFrom), last: Number(s.payTo) },
    userAddress: s.userScript && s.userScript !== "0x" ? scriptToAddress(s.userScript) : null,
    memo: memoText(s.memo),
    waitingFor: waiting[`${side}/${state}`] ?? null,
  };
}

/** A tunnel's sell whose proven payment was mined outside its payment window
 *  (T1): it does not complete the sell, and its user takes the coin back
 *  (`refundSell`, no transaction needed). */
export function paidOutsideWindow(s: SwapView): boolean {
  const h = s.job.provenHeight;
  if (s.side !== "Sell" || !s.payWindow || h === null) return false;
  if (s.job.stage !== "Proven" && s.job.stage !== "Settled") return false;
  return h < s.payWindow.first || h > s.payWindow.last;
}

/** Gives a sell's coin back once its job found no operator, missed its
 *  deadline, or was slashed (with the escrow's share then). Anyone may call.
 *  The job's fees are not in it: when no operator took the job, they return
 *  through `expireJob` and then `withdrawCredit`.
 *  When the job is proven but its transaction pays the user too little, the
 *  refund takes that transaction: `rawTx` (see `provenPayment`). */
export async function refundSell(signer: Signer, d: Deployment, swapId: bigint | number, rawTx = "0x"): Promise<{ txHash: string }> {
  const tx = await conversion(d, signer).refundSell(swapId, rawTx);
  await tx.wait();
  return { txHash: tx.hash };
}

/**
 * What a swap's proven Bitcoin transaction pays its Bitcoin address: for a
 * sell, the user's; null before the proof. A sell paid too little is
 * refunded with `raw` (the transaction without witness data).
 */
export async function provenPayment(s: SwapView, bitcoin = new Bitcoin()): Promise<{ txid: string; paid: bigint; enough: boolean; raw: () => Promise<string> } | null> {
  const txid = s.job.provenTxid;
  if (!txid || !s.address) return null;
  const script = addressToScript(s.address).slice(2).toLowerCase();
  const outputs = await bitcoin.outputs(txid);
  // An explorer that does not know the transaction yet: unknown, not unpaid.
  if (outputs.length === 0) return null;
  const paid = outputs.filter((o) => o.script.toLowerCase() === script).reduce((a, o) => (o.sats > a ? o.sats : a), 0n);
  return { txid, paid, enough: paid >= s.sats, raw: () => bitcoin.rawTx(txid) };
}

/** Pays a sell's operator once its proven transaction pays the user enough
 *  and the proof's lock has ended. Anyone may call; the operator's node
 *  does. */
export async function completeSell(signer: Signer, d: Deployment, swapId: bigint | number, bitcoin = new Bitcoin()): Promise<{ txHash: string }> {
  const s = await getSwap(signer.provider!, d, swapId);
  if (!s.job.provenTxid) throw new Error(`swap ${swapId}: its job is not proven`);
  const tx = await conversion(d, signer).completeSell(swapId, await bitcoin.rawTx(s.job.provenTxid));
  await tx.wait();
  return { txHash: tx.hash };
}

/**
 * Gives a buy's coin to its user once the operator's proven transaction (its
 * receipt) spends the user's payment. Anyone may call, the user's wallet
 * too (the design's step 5): the payment is found among what the receipt
 * spends, the output paying the operator's address at least the sats.
 */
export async function completeBuy(signer: Signer, d: Deployment, swapId: bigint | number, bitcoin = new Bitcoin()): Promise<{ txHash: string }> {
  const s = await getSwap(signer.provider!, d, swapId);
  if (s.side !== "Buy" || s.state !== "Funded") throw new Error(`swap ${swapId} is not a funded buy`);
  const found = await provenReceipt(s, bitcoin);
  if (!found) throw new Error(`swap ${swapId}: its proven transaction spends no payment of ${s.sats} sats to ${s.address}`);
  const { receipt, payment } = found;
  const [receiptRaw, paymentRaw] = await Promise.all([bitcoin.rawTx(receipt), bitcoin.rawTx(payment.txid)]);
  const tx = await conversion(d, signer).completeBuy(swapId, receiptRaw, paymentRaw, payment.vout);
  await tx.wait();
  return { txHash: tx.hash };
}

/**
 * A funded buy's proven transaction, when it is a receipt: it spends the
 * user's payment, an output paying the operator's address at least the
 * sats. Null otherwise; a proven close (no payment came) is not one.
 */
export async function provenReceipt(
  s: SwapView,
  bitcoin = new Bitcoin()
): Promise<{ receipt: string; payment: { txid: string; vout: number } } | null> {
  const receipt = s.job.provenTxid;
  if (s.side !== "Buy" || !receipt || !s.address) return null;
  const script = addressToScript(s.address).slice(2).toLowerCase();
  const payment = (await bitcoin.inputs(receipt)).find((i) => i.script.toLowerCase() === script && i.sats >= s.sats);
  return payment ? { receipt, payment: { txid: payment.txid, vout: payment.vout } } : null;
}

/** Whether a slashed swap's user was given the escrow's share. */
export async function compensated(provider: Provider, d: Deployment, swapId: bigint | number): Promise<boolean> {
  return conversion(d, provider).compensated(swapId);
}

/** Gives a slashed swap's user the escrow's share. Anyone may call, once. */
export async function compensate(signer: Signer, d: Deployment, swapId: bigint | number): Promise<{ txHash: string }> {
  const tx = await conversion(d, signer).compensate(swapId);
  await tx.wait();
  return { txHash: tx.hash };
}

/** Ends a buy that no operator took or locked in time; the user paid
 *  nothing on Bitcoin. Anyone may call. */
export async function cancelBuy(signer: Signer, d: Deployment, swapId: bigint | number): Promise<{ txHash: string }> {
  const tx = await conversion(d, signer).cancel(swapId);
  await tx.wait();
  return { txHash: tx.hash };
}

/**
 * The swaps of `user`, newest first, at most `limit`. Read swap by swap from
 * the newest, which suits a test network's few swaps; a busy network wants
 * its `Sold` and `Bought` events, by user, instead.
 */
export async function swapsOf(provider: Provider, d: Deployment, user: string, limit = 50): Promise<SwapView[]> {
  const c = conversion(d, provider);
  const count = Number(await c.swapCount());
  const mine: number[] = [];
  const who = user.toLowerCase();
  for (let top = count; top >= 1 && mine.length < limit; top -= 20) {
    const ids = Array.from({ length: Math.min(20, top) }, (_, i) => top - i);
    const swaps = await Promise.all(ids.map((id) => c.getSwap(id)));
    swaps.forEach((s: any, i: number) => {
      if (String(s.user).toLowerCase() === who && mine.length < limit) mine.push(ids[i]);
    });
  }
  return Promise.all(mine.map((id) => getSwap(provider, d, id)));
}
