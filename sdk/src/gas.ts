// A gas limit for a call that opens a job. A gas estimate runs at a price
// of zero on many nodes, where the fee paths cost less, so it is about
// 20,000 gas too low (spec V14): the limit here is the estimate with a
// quarter more and 50,000 on top.

import type { Contract } from "ethers";

export async function jobGas(contract: Contract, method: string, args: unknown[], value: bigint): Promise<bigint> {
  const estimate: bigint = await contract.getFunction(method).estimateGas(...args, { value });
  return (estimate * 5n) / 4n + 50_000n;
}
