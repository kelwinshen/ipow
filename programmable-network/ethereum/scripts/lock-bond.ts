// Locks an operator's bond in the protocol on one EVM test network: the
// node never locks a bond itself. The key and endpoint are the network
// package's (programmable-network/<network>/.env); the protocol is the one
// in deployments/<network>-testnet.json. Run from programmable-network/ethereum:
//
//   node scripts/lock-bond.ts <network> <amount in the coin's smallest unit> [--check]
//
// With --check it only reads the bond.

import { Contract, FetchRequest, JsonRpcProvider, Wallet, formatEther } from "ethers";
import { readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

import { NETWORKS } from "../deploy/networks.ts";

const here = dirname(fileURLToPath(import.meta.url));
const [network, amountArg] = process.argv.slice(2).filter((a) => !a.startsWith("--"));
const s = NETWORKS[network];
if (!s || s.coin.kind !== "native") throw new Error("a network with a native coin, from deploy/networks.ts");
const vars: Record<string, string> = {};
for (const line of readFileSync(join(here, "..", "..", network, ".env"), "utf8").split("\n")) {
  const m = line.match(/^\s*([A-Z0-9_]+)\s*=\s*"?([^"\n]*)"?/);
  if (m) vars[m[1]] = m[2].trim();
}
const request = new FetchRequest(vars[s.rpcEnv.testnet]);
request.setHeader("user-agent", "ipow-bond");
const provider = new JsonRpcProvider(request, undefined, { staticNetwork: true });
const key = vars[s.keyEnv.testnet];
const wallet = new Wallet(key.startsWith("0x") ? key : "0x" + key, provider);
const d = JSON.parse(readFileSync(join(here, "..", "deployments", `${network}-testnet.json`), "utf8"));
const protocol = new Contract(d.protocol, ["function lockBond(uint256) payable", "function bondOf(address) view returns (uint256 bond, uint256 locked)"], wallet);

const show = async () => {
  const [bond, locked] = await protocol.bondOf(wallet.address);
  console.log(`${network}: ${wallet.address} bond ${formatEther(bond)}, locked in jobs ${formatEther(locked)}`);
};
await show();
if (process.argv.includes("--check")) process.exit(0);
const amount = BigInt(amountArg);
if (amount <= 0n) throw new Error("an amount above zero");
const tx = await protocol.lockBond(amount, { value: amount });
console.log("lockBond", tx.hash, "status", (await tx.wait())?.status);
await show();
