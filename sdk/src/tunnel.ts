// A tunnel: a conversion between two programmable networks, as two swaps of
// Conversion linked by one Bitcoin payment (docs/specs/ipow-conversion-tunnel.md).
// An operator's node serves the tunnel API: what it trades and at what
// price, and a quote. The user's buy carries the tunnel's terms in its memo
// (`tunnelMemo`), which the operator reads from the chain. This is the
// client of that API, the memo, the arithmetic of an estimate at the
// operator's prices, and the search that links a funded buy to the sale
// paying its address on the source network.

import type { Provider } from "ethers";

import { swapsOf, type SwapView } from "./conversion.ts";
import type { Deployment } from "./networks.ts";

/** The network's own coin, as the API names it. */
export const NATIVE = "native";

/** A coin or token an operator trades on one network, and its prices: what
 *  it pays for one whole unit sold, and asks for one bought. */
export type TunnelAsset = {
  /** The node's name for the network (its settings'). */
  network: string;
  /** `native`, or the token's address. */
  token: string;
  symbol: string;
  decimals: number;
  paySats: bigint;
  askSats: bigint;
};

export type TunnelAssets = {
  /** By the node's network name. */
  networks: Record<string, TunnelAsset[]>;
  /** The most sats one tunnel moves. */
  maxSats: bigint;
};

export type TunnelQuote = {
  sats: bigint;
  amountOut: bigint;
  /** Whether the operator could take the tunnel now (the BTC for its sale,
   *  the coin to lock); absent from an older node. */
  canTake?: boolean;
};

/** A tunnel's terms as the buy's memo, read by operators from the chain:
 *  the sale that will pay the buy, on the source network (the node's name
 *  for it), of `amountIn` (smallest units) of `fromToken`. The buy's own
 *  owner wrote it with the buy, so nobody else sets the terms. Single
 *  spaces and decimal digits: a node refuses any other spelling. */
export function tunnelMemo(from: string, fromToken: string, amountIn: bigint): string {
  return `ipow-tunnel/1 ${from} ${fromToken} ${amountIn}`;
}

/** The terms a tunnel memo names, or null for a memo that is not one. */
export function parseTunnelMemo(memo: string): { from: string; fromToken: string; amountIn: bigint } | null {
  const m = /^ipow-tunnel\/1 (\S+) (\S+) (\d+)$/.exec(memo.trim());
  return m ? { from: m[1], fromToken: m[2], amountIn: BigInt(m[3]) } : null;
}

/** The operator node's tunnel API, at `base`: the node itself, or a server
 *  of the app's that adds the node's key. */
export class TunnelApi {
  readonly base: string;
  private readonly options: { headers?: Record<string, string>; timeoutMs?: number };

  constructor(base: string, options: { headers?: Record<string, string>; timeoutMs?: number } = {}) {
    this.base = base;
    this.options = options;
  }

  private async call<T>(path: string, init: RequestInit = {}): Promise<T> {
    let r: Response;
    try {
      r = await fetch(`${this.base.replace(/\/$/, "")}${path}`, {
        ...init,
        headers: { ...(init.headers ?? {}), ...(this.options.headers ?? {}) },
        signal: AbortSignal.timeout(this.options.timeoutMs ?? 60_000),
      });
    } catch {
      throw new Error("The operator did not answer: try again in a moment");
    }
    const body = await r.json().catch(() => ({}));
    if (!r.ok) throw new Error(body?.error ?? `The operator refused (${r.status})`);
    return body as T;
  }

  /** What the operator trades, with its prices. */
  async assets(): Promise<TunnelAssets> {
    const r = await this.call<{ networks: { name: string; coins: { token: string; symbol: string; decimals: number; paySats: number; askSats: number }[] }[]; maxSats: number }>("/assets");
    const networks: Record<string, TunnelAsset[]> = {};
    for (const n of r.networks) {
      networks[n.name] = n.coins.map((c) => ({ network: n.name, token: c.token, symbol: c.symbol, decimals: c.decimals, paySats: BigInt(c.paySats), askSats: BigInt(c.askSats) }));
    }
    return { networks, maxSats: BigInt(r.maxSats) };
  }

  /** What the operator gives for `amountIn` (smallest units) of `fromToken`
   *  on `from`: the sats between the networks, and `token` on `to`. */
  async quote(from: string, fromToken: string, amountIn: bigint, to: string, token = NATIVE): Promise<TunnelQuote> {
    const q = new URLSearchParams({ from, fromToken, amount: amountIn.toString(), to, token });
    const r = await this.call<{ sats: number; amountOut: string; canTake?: boolean }>(`/quote?${q}`);
    return { sats: BigInt(r.sats), amountOut: BigInt(r.amountOut), canTake: r.canTake };
  }
}

/** A decimal amount in a coin's smallest units, without floating point. */
export function toUnits(amount: string, decimals: number): bigint {
  const [whole, frac = ""] = amount.trim().split(".");
  if (!/^\d*$/.test(whole) || !/^\d*$/.test(frac)) throw new Error("Not an amount");
  return BigInt((whole || "0") + frac.slice(0, decimals).padEnd(decimals, "0"));
}

/** Smallest units as a decimal number, for display. */
export function fromUnits(units: bigint, decimals: number): number {
  return Number(units) / 10 ** decimals;
}

/** Kept off each estimate, so a swap made from it is within the operator's
 *  price after rounding. */
const MARGIN = 99n;

/** Sats the operator pays for `amount` (a decimal string) of the asset. */
export function satsFor(asset: TunnelAsset, amount: string): bigint {
  return (toUnits(amount, asset.decimals) * asset.paySats * MARGIN) / 100n / 10n ** BigInt(asset.decimals);
}

/** The asset the operator gives for `sats`, as a decimal number. */
export function coinFor(asset: TunnelAsset, sats: number): number | null {
  return asset.askSats === 0n ? null : (sats / Number(asset.askSats)) * 0.99;
}

/** The sale of `user`'s on an EVM network that pays `address` (a funded
 *  buy's, on the other network) and is still live: the tunnel's other leg.
 *  Null when there is none. */
export async function sellPaying(provider: Provider, d: Deployment, user: string, address: string): Promise<SwapView | null> {
  const mine = await swapsOf(provider, d, user);
  return mine.find((s) => s.side === "Sell" && s.address === address && s.state !== "Refunded" && s.state !== "Cancelled") ?? null;
}
