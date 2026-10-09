// BETA on an EVM network (contracts/applications/beta/BetaBaskets.sol): a basket of up
// to its parts, each a token or the network's coin, with what one whole
// BETA holds of each; minted by depositing the parts, burned for them back.
// The parts are usually receipts made by the vault, such as vSOL, next to
// the network's own coin. One whole BETA is 10^9 of its smallest units.

import { Contract, ZeroAddress, type Provider, type Signer } from "ethers";

import { ABIS } from "./generated/abis.ts";
import { rpcScale, type Deployment } from "./networks.ts";

export type BasketPart = {
  /** Zero for the network's coin, and for a receipt the vault has not made yet (E5). */
  token: string;
  /** For a part that is another network's asset: the vault's receipts
   *  contract and that asset's number there (E5); zero otherwise. */
  receipts: string;
  asset: number;
  /** False while a receipt part's receipt is not made: nothing can be minted
   *  until the asset is bridged here. */
  made: boolean;
  /** What one whole BETA holds of it when the basket is new. */
  amount: bigint;
  /** What the basket holds of it for its holders now. */
  held: bigint;
  /** Set aside: owed to burners who deferred it, and fees not collected. */
  owed: bigint;
};

export type Basket = {
  key: string;
  creator: string;
  id: bigint;
  feeTo: string;
  mintFeeBps: number;
  burnFeeBps: number;
  /** The basket's BETA token. */
  beta: string;
  /** The token's name and symbol, the creator's (E4). */
  name: string;
  symbol: string;
  /** The token's metadata URI (E4): JSON with a description and an image;
   *  empty when the creator gave none. */
  uri: string;
  supply: bigint;
  parts: BasketPart[];
};

/** The limits of a basket's naming (BetaBaskets.MAX_NAME, MAX_SYMBOL, MAX_URI), in bytes. */
export const BASKET_NAMING = { name: 64, symbol: 16, uri: 2048 } as const;

export type MintQuote = {
  key: string;
  amount: bigint;
  /** By part: what minting takes, and the creator's fee on top. */
  need: bigint[];
  fee: bigint[];
  parts: string[];
  /** What the wallet sends with the call, for a part that is the coin. */
  value: bigint;
};

const baskets = (d: Deployment, runner: Provider | Signer) => new Contract(d.betaBaskets, ABIS.betaBaskets, runner);

export async function basketKey(provider: Provider, d: Deployment, creator: string, id: bigint | number): Promise<string> {
  return baskets(d, provider).basketKey(creator, id);
}

/** A part as the creator names it: a token here (`token`, zero for the
 *  coin), or another network's asset as the vault's receipt of it, made or
 *  not yet (`receipts` and `asset`, E5). */
export type BasketPartSpec = { token?: string; receipts?: string; asset?: number; amount: bigint };

/** Creates basket `id` of the signer: its token's name and symbol, a
 *  metadata URI (JSON with a description and an image; may be empty), its
 *  parts and fees (at most 1% each, fixed for good). */
export async function createBasket(
  signer: Signer,
  d: Deployment,
  options: { id: bigint | number; name: string; symbol: string; uri?: string; parts: BasketPartSpec[]; mintFeeBps?: number; burnFeeBps?: number }
): Promise<{ key: string; beta: string; txHash: string }> {
  const c = baskets(d, signer);
  const tx = await c.createBasket(
    options.id,
    options.name,
    options.symbol,
    options.uri ?? "",
    options.parts.map((p) => ({ token: p.token ?? ZeroAddress, receipts: p.receipts ?? ZeroAddress, asset: p.asset ?? 0, amount: p.amount })),
    options.mintFeeBps ?? 0,
    options.burnFeeBps ?? 0
  );
  const receipt = await tx.wait();
  for (const log of receipt.logs) {
    if (log.address.toLowerCase() !== d.betaBaskets.toLowerCase()) continue;
    const parsed = c.interface.parseLog(log);
    if (parsed?.name === "BasketCreated") return { key: parsed.args.key, beta: parsed.args.beta, txHash: tx.hash };
  }
  throw new Error(`no basket created in ${tx.hash}`);
}

export async function getBasket(provider: Provider, d: Deployment, key: string): Promise<Basket> {
  const c = baskets(d, provider);
  const b = await c.getBasket(key);
  if (b.creator === ZeroAddress) throw new Error(`${d.name}: no basket ${key}`);
  // A receipt part not made yet has no holding to read (PartNotMadeYet).
  const tokens: string[] = await Promise.all(b.parts.map((_: unknown, i: number) => c.partToken(key, i)));
  const made = b.parts.map((p: any, i: number) => p.receipts === ZeroAddress || tokens[i] !== ZeroAddress);
  const held: bigint[] = await Promise.all(b.parts.map((_: unknown, i: number) => (made[i] ? c.held(key, i) : Promise.resolve(0n))));
  const token = new Contract(b.beta, ["function totalSupply() view returns (uint256)", "function name() view returns (string)", "function symbol() view returns (string)"], provider);
  const [supply, name, symbol]: [bigint, string, string] = await Promise.all([token.totalSupply(), token.name(), token.symbol()]);
  return {
    key,
    creator: b.creator,
    id: b.id,
    feeTo: b.feeTo,
    mintFeeBps: Number(b.mintFeeBps),
    burnFeeBps: Number(b.burnFeeBps),
    beta: b.beta,
    name,
    symbol,
    uri: b.uri ?? "",
    supply,
    parts: b.parts.map((p: any, i: number) => ({ token: tokens[i], receipts: p.receipts, asset: Number(p.asset), made: made[i], amount: p.amount, held: held[i], owed: p.owed })),
  };
}

/** What minting `amount` BETA (smallest units) takes now, by part. */
export async function quoteMint(provider: Provider, d: Deployment, key: string, amount: bigint): Promise<MintQuote> {
  const c = baskets(d, provider);
  const [[need, fee], b] = await Promise.all([c.mintCost(key, amount), c.getBasket(key)]);
  const parts: string[] = b.parts.map((p: any) => p.token);
  let value = 0n;
  parts.forEach((t, i) => {
    if (t === ZeroAddress) value += (need[i] + fee[i]) * rpcScale(d);
  });
  return { key, amount, need: [...need], fee: [...fee], parts, value };
}

/** Mints with a quote: each token part approved for exactly what it takes,
 *  the coin sent with the call (any more than needed is sent back). */
export async function mint(signer: Signer, d: Deployment, quote: MintQuote): Promise<{ txHash: string }> {
  for (const [i, token] of quote.parts.entries()) {
    if (token === ZeroAddress) continue;
    const t = new Contract(token, ["function approve(address,uint256) returns (bool)"], signer);
    await (await t.approve(d.betaBaskets, quote.need[i] + quote.fee[i])).wait();
  }
  const tx = await baskets(d, signer).mint(quote.key, quote.amount, { value: quote.value });
  await tx.wait();
  return { txHash: tx.hash };
}

/** Burns `amount` BETA for each part's share, paid to `to` (the signer by
 *  default). A part whose index is in `defer` is owed instead, and
 *  collected later with `collectOwed`: for a part that cannot move now. */
export async function burn(signer: Signer, d: Deployment, key: string, amount: bigint, options: { defer?: number[]; to?: string } = {}): Promise<{ txHash: string }> {
  const defer = (options.defer ?? []).reduce((bits, i) => bits | (1 << i), 0);
  const tx = await baskets(d, signer).burn(key, amount, defer, options.to ?? (await signer.getAddress()));
  await tx.wait();
  return { txHash: tx.hash };
}

export async function collectOwed(signer: Signer, d: Deployment, key: string, index: number, to?: string): Promise<{ txHash: string }> {
  const tx = await baskets(d, signer).collectOwed(key, index, to ?? (await signer.getAddress()));
  await tx.wait();
  return { txHash: tx.hash };
}

/** The basket's fees of part `index`, for whoever receives them. */
export async function collectFees(signer: Signer, d: Deployment, key: string, index: number, to?: string): Promise<{ txHash: string }> {
  const tx = await baskets(d, signer).collectFees(key, index, to ?? (await signer.getAddress()));
  await tx.wait();
  return { txHash: tx.hash };
}
