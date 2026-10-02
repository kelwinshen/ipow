// Opening a vault checkpoint job (D116), the simplest job: the operator
// writes one tagged Bitcoin transaction and proves it. Paid with a quote
// from quote.ts; what is paid above the fees goes to the operator (D79).

import { Contract, type Signer } from "ethers";

import { ABIS } from "./generated/abis.ts";
import { jobGas } from "./gas.ts";
import type { Deployment } from "./networks.ts";
import type { Quote } from "./quote.ts";

export async function openCheckpoint(signer: Signer, d: Deployment, quote: Quote): Promise<{ jobId: bigint; txHash: string }> {
  if (d.coin.kind !== "native") throw new Error(`${d.name}: a token coin's checkpoint needs its approval first, not built yet`);
  if (quote.network !== d.name) throw new Error(`a quote for ${quote.network}, not ${d.name}`);
  const vault = new Contract(d.vaults[0].vault, ABIS.vault, signer);
  const args = [quote.confirmations, quote.pay];
  const tx = await vault.openCheckpoint(...args, { value: quote.value, gasLimit: await jobGas(vault, "openCheckpoint", args, quote.value) });
  const receipt = await tx.wait();
  if (!receipt || receipt.status !== 1) throw new Error(`openCheckpoint failed: ${tx.hash}`);
  // The job's id, from the protocol's event.
  const protocol = new Contract(d.protocol, ABIS.protocol);
  for (const log of receipt.logs) {
    if (log.address.toLowerCase() !== d.protocol.toLowerCase()) continue;
    const parsed = protocol.interface.parseLog(log);
    if (parsed?.name === "JobOpened") return { jobId: parsed.args.jobId as bigint, txHash: tx.hash };
  }
  throw new Error(`no job opened in ${tx.hash}`);
}
