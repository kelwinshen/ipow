// Opens a checkpoint job through a test network's vault (D116): the
// simplest job, which an operator takes by writing its tagged Bitcoin
// transaction and proving it. The sender pays the job's fees, priced at the
// network's base fee now; they come back if nobody takes the job, and what
// is paid above them goes to the operator (D79). Run from
// programmable-network/ethereum:
//
//   node scripts/open-checkpoint.ts <network> [--check]
//
// With --check it only prices the job.

import { Contract, FetchRequest, JsonRpcProvider, Wallet, formatEther } from "ethers";
import { readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

import { NETWORKS } from "../deploy/networks.ts";

/** Six confirmations: a window of 30 blocks (D15). */
const CONFIRMATIONS = 6;
const here = dirname(fileURLToPath(import.meta.url));
const network = process.argv.slice(2).find((a) => !a.startsWith("--"))!;
const s = NETWORKS[network];
if (!s || s.coin.kind !== "native") throw new Error("a network with a native coin, from deploy/networks.ts");
const vars: Record<string, string> = {};
for (const line of readFileSync(join(here, "..", "..", network, ".env"), "utf8").split("\n")) {
  const m = line.match(/^\s*([A-Z0-9_]+)\s*=\s*"?([^"\n]*)"?/);
  if (m) vars[m[1]] = m[2].trim();
}
const request = new FetchRequest(vars[s.rpcEnv.testnet]);
request.setHeader("user-agent", "ipow-checkpoint");
const provider = new JsonRpcProvider(request, undefined, { staticNetwork: true });
const key = vars[s.keyEnv.testnet];
const wallet = new Wallet(key.startsWith("0x") ? key : "0x" + key, provider);
const d = JSON.parse(readFileSync(join(here, "..", "deployments", `${network}-testnet.json`), "utf8"));
const protocol = new Contract(d.protocol, [
  "function commitmentFeeAt(uint16, uint256) view returns (uint256)",
  "function MIN_ESCROW_MULTIPLE() view returns (uint256)",
  "function DEFAULT_ESCROW_FEE_BPS() view returns (uint16)",
  "function jobCount() view returns (uint256)",
], provider);
const vault = new Contract(d.vaults[0].vault, ["function openCheckpoint(uint16, uint256) payable returns (uint256)", "function minCertifyingEscrow() view returns (uint256)"], wallet);

const baseFee = (await provider.getBlock("latest"))!.baseFeePerGas!;
const fee = await protocol.commitmentFeeAt(CONFIRMATIONS, baseFee);
let escrow = (await protocol.MIN_ESCROW_MULTIPLE()) * fee;
const least = await vault.minCertifyingEscrow();
if (escrow < least) escrow = least;
const escrowFee = (escrow * BigInt(await protocol.DEFAULT_ESCROW_FEE_BPS())) / 10_000n;
// Twice the commitment fee: the base fee may rise before the job opens.
const paid = 2n * fee + escrowFee;
console.log(`${network}: base fee ${baseFee}, commitment fee ${formatEther(fee)}, escrow ${formatEther(escrow)}, escrow fee ${formatEther(escrowFee)}; pays ${formatEther(paid)}`);
if (process.argv.includes("--check")) process.exit(0);
const tx = await vault.openCheckpoint(CONFIRMATIONS, paid, { value: paid });
console.log("openCheckpoint", tx.hash, "status", (await tx.wait())?.status, "job", (await protocol.jobCount()).toString());
