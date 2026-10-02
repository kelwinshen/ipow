// Conversion on an EVM network (contracts/apps/Conversion.sol, design
// docs/drafts/ipow-conversion-app.md): sell the network's coin or a token
// for BTC, or buy it with BTC, an operator taking the other side. Apps give
// and show Bitcoin addresses; the contract holds output scripts.

import { Contract, ZeroAddress, type Provider, type Signer } from "ethers";

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

/** Buys `amount` of `token` (zero for the network's coin) for `sats`. Once
 *  an operator locks it, the swap names where and when to pay. */
export async function buy(
  signer: Signer,
  d: Deployment,
  quote: SwapQuote,
  options: { token?: string; amount: bigint; sats: bigint }
): Promise<{ swapId: bigint; jobId: bigint; txHash: string }> {
  if (d.coin.kind !== "native") throw new Error(`${d.name}: a token coin's swap needs its approvals, not built yet`);
  const c = conversion(d, signer);
  const args = [options.token ?? ZeroAddress, options.amount, options.sats, quote.confirmations, quote.fees];
  const value = quote.fees * rpcScale(d);
  const tx = await c.buy(...args, { value, gasLimit: await jobGas(c, "buy", args, value) });
  return { ...opened(c, await tx.wait(), "Bought"), txHash: tx.hash };
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
    waitingFor: waiting[`${side}/${state}`] ?? null,
  };
}

/** Gives a sell's coin back once its job found no operator, missed its
 *  deadline, or was slashed (with the escrow's share then). Anyone may call.
 *  The job's fees are not in it: when no operator took the job, they return
 *  through `expireJob` and then `withdrawCredit`.
 *  The case of a proven payment too small needs its transaction, not built
 *  here. */
export async function refundSell(signer: Signer, d: Deployment, swapId: bigint | number): Promise<{ txHash: string }> {
  const tx = await conversion(d, signer).refundSell(swapId, "0x");
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
