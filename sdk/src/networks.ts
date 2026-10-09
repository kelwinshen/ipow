// The deployed networks, as data generated from this repository
// (scripts/sync.ts). An app picks one by name and never hard-codes an
// address.

import { DEPLOYMENTS } from "./generated/deployments.ts";

export type NetworkName = keyof typeof DEPLOYMENTS;
export type Deployment = (typeof DEPLOYMENTS)[NetworkName];

export const NETWORK_NAMES = Object.keys(DEPLOYMENTS) as NetworkName[];

export function network(name: NetworkName): Deployment {
  const d = DEPLOYMENTS[name];
  if (!d) throw new Error(`unknown network ${name}; known: ${NETWORK_NAMES.join(", ")}`);
  return d;
}

/**
 * Units a wallet sends per unit a contract counts, for the native coin.
 * 1 everywhere but Hedera, whose RPC counts HBAR in 18 decimals while a
 * contract counts tinybars, 8 (D139): 10^10.
 */
export function rpcScale(d: Deployment): bigint {
  return d.coin.kind === "native" ? 10n ** BigInt(18 - d.coin.decimals) : 1n;
}
