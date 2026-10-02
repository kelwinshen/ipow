// What a job costs to open, priced at the network's price now. The price
// can move before the job's transaction is mined, so a quote carries a
// margin on top of the commitment fee; what is paid above the fees goes to
// the operator (D79), so the margin is spent, not refunded. In the first
// live run the escrow rose 4.6% in the minute between quote and send.

import { Contract, type Provider } from "ethers";

import { ABIS } from "./generated/abis.ts";
import { rpcScale, type Deployment } from "./networks.ts";

export type Quote = {
  network: string;
  confirmations: number;
  /** The price of work the contract will read, in its units. */
  price: bigint;
  commitmentFee: bigint;
  escrow: bigint;
  escrowFee: bigint;
  /** Paid on top, against a rise in price; goes to the operator. */
  margin: bigint;
  /** All the job takes, in the coin's units as a contract counts them. */
  pay: bigint;
  /** What the wallet sends with the call: `pay` in its RPC's units for a
   *  native coin, 0 for a token coin (taken with an approval). */
  value: bigint;
};

export type QuoteOptions = {
  /** Six by default: a window of 30 blocks (D15). */
  confirmations?: number;
  /** The margin, in basis points of the commitment fee: 100% by default. */
  marginBps?: bigint;
};

/** The price of work the protocol reads now: the base fee, or on Hedera
 *  the network's gas price, in a contract's units (D139). */
export async function priceNow(provider: Provider, d: Deployment): Promise<bigint> {
  if (d.coin.kind === "native" && d.coin.price === "gasPrice") {
    const gasPrice = (await provider.getFeeData()).gasPrice;
    if (gasPrice === null) throw new Error(`${d.name}: no gas price`);
    return gasPrice / rpcScale(d);
  }
  const block = await provider.getBlock("latest");
  if (!block?.baseFeePerGas) throw new Error(`${d.name}: no base fee`);
  return block.baseFeePerGas;
}

/** A vault checkpoint job (D116): its escrow is the vault's least
 *  certifying escrow (D118), or 5 times the fee if that is more (D56). */
export async function quoteCheckpoint(provider: Provider, d: Deployment, options: QuoteOptions = {}): Promise<Quote> {
  const confirmations = options.confirmations ?? 6;
  const protocol = new Contract(d.protocol, ABIS.protocol, provider);
  const vault = new Contract(d.vaults[0].vault, ABIS.vault, provider);
  const price = await priceNow(provider, d);
  const [commitmentFee, multiple, bps, least] = await Promise.all([
    protocol.commitmentFeeAt(confirmations, price) as Promise<bigint>,
    protocol.MIN_ESCROW_MULTIPLE() as Promise<bigint>,
    protocol.DEFAULT_ESCROW_FEE_BPS() as Promise<bigint>,
    vault.minCertifyingEscrow() as Promise<bigint>,
  ]);
  const escrow = multiple * commitmentFee > least ? multiple * commitmentFee : least;
  const escrowFee = (escrow * BigInt(bps)) / 10_000n;
  const margin = (commitmentFee * (options.marginBps ?? 10_000n)) / 10_000n;
  const pay = commitmentFee + margin + escrowFee;
  return {
    network: d.name,
    confirmations,
    price,
    commitmentFee,
    escrow,
    escrowFee,
    margin,
    pay,
    value: d.coin.kind === "native" ? pay * rpcScale(d) : 0n,
  };
}
