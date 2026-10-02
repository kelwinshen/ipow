// A lock followed across its pair: made on an EVM network, its receipt
// issued on Solana. One journey an app shows as one progress bar.

import type { Provider } from "ethers";

import type { Deployment } from "./networks.ts";
import type { SolanaVault } from "./solana.ts";
import { getLock, type LockState } from "./vault.ts";

export type LockJourney = {
  /** `Locked`: waiting for an operator to carry it. `Carried`: a message
   *  carrying it was judged true on the lock's network. `Issued`: its
   *  receipt was issued on Solana (after a claim, or at once by an attester).
   *  `GivenUp` or `Returned`: it ends without a receipt. */
  stage: "Locked" | "Carried" | "Issued" | "GivenUp" | "Returned";
  lock: LockState;
  attests: number;
};

export async function followLock(provider: Provider, d: Deployment, lockId: bigint | number, solana: SolanaVault): Promise<LockJourney> {
  if (solana.peer !== d.number) throw new Error(`the Solana side of ${d.name} is the pair with ${d.number}, not ${solana.peer}`);
  const [lock, mark] = await Promise.all([getLock(provider, d, lockId), solana.evmLockMark(lockId)]);
  const stage = lock.stage === "Returned" ? "Returned" : mark?.givenUp ? "GivenUp" : mark?.issued ? "Issued" : lock.stage;
  return { stage, lock, attests: mark?.attests ?? 0 };
}
