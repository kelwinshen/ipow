// A tunnel: a conversion between two programmable networks, as two swaps of
// Conversion linked by one Bitcoin payment (docs/drafts/ipow-conversion-tunnel.md).
// An operator's node serves the tunnel API: what it trades and at what
// price, a quote, and the registration of the buy the user opened. This is
// the client of that API, the message a registration is signed with, the
// arithmetic of an estimate at the operator's prices, and the search that
// links a funded buy to the sale paying its address on the source network.

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

export type TunnelQuote = { sats: bigint; amountOut: bigint };

export type TunnelRegistration = {
  /** The destination network (the node's name) and the buy the user opened there. */
  to: string;
  swapId: bigint | number;
  /** The source network and what is sold there, in smallest units. */
  from: string;
  fromToken: string;
  amountIn: bigint;
};

/** What the buy's owner signs to register a tunnel: the same words in the
 *  node (`tunnel_message`), so nobody else can set the tunnel's terms. */
export function tunnelMessage(r: TunnelRegistration): string {
  return `iPoW tunnel: buy ${r.swapId} on ${r.to}; sell ${r.amountIn} of ${r.fromToken} on ${r.from}`;
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
    const r = await this.call<{ sats: number; amountOut: string }>(`/quote?${q}`);
    return { sats: BigInt(r.sats), amountOut: BigInt(r.amountOut) };
  }

  /** Registers the buy the user opened, with the owner's `signature` over
   *  `tunnelMessage(r)`: from then on the operator takes its sale at the
   *  terms it quoted. */
  async register(r: TunnelRegistration, signature: string): Promise<TunnelQuote> {
    const out = await this.call<{ sats: number; amountOut: string }>("/register", {
      method: "POST",
      headers: { "content-type": "application/json" },
      body: JSON.stringify({ to: r.to, swapId: Number(r.swapId), from: r.from, fromToken: r.fromToken, amountIn: r.amountIn.toString(), signature }),
    });
    return { sats: BigInt(out.sats), amountOut: BigInt(out.amountOut) };
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
