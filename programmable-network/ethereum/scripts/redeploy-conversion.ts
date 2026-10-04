// Redeploys Conversion alone on one EVM test network, against the
// protocol already there: for a change to Conversion only (the tunnel's T1
// and T2, docs/drafts/ipow-conversion-tunnel.md). Its swaps' record changed,
// so it is a new contract, not an upgrade; the old one keeps its swaps,
// which end there by its own rules. The key and endpoint are the network
// package's (programmable-network/<network>/.env). Run from
// programmable-network/ethereum after `npx hardhat compile`:
//
//   node scripts/redeploy-conversion.ts <network> [--dry]
//
// It reads back the new contract's settings, then writes the new address to
// deployments/<network>-testnet.json, keeping the old one as
// `previousConversion`, and `sources.Conversion` = "current" so that
// verify-deployments.ts checks it against today's build. Then run
// verify-deployments.ts, packages/sdk's scripts/sync.ts, and point the
// node's settings at the new address.

import { Contract, ContractFactory, FetchRequest, JsonRpcProvider, Wallet, ZeroAddress, getAddress } from "ethers";
import { readFileSync, writeFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

import { NETWORKS } from "../deploy/networks.ts";

const here = dirname(fileURLToPath(import.meta.url));
const network = process.argv.slice(2).find((a) => !a.startsWith("--"));
const dry = process.argv.includes("--dry");
const s = network ? NETWORKS[network] : undefined;
if (!s || !s.rpcEnv.testnet) throw new Error("a network from deploy/networks.ts that has a test network");
if (s.sender) throw new Error(`${network} deploys through its own sender (${s.sender}); not built here`);

const vars: Record<string, string> = {};
for (const line of readFileSync(join(here, "..", "..", network!, ".env"), "utf8").split("\n")) {
  const m = line.match(/^\s*([A-Z0-9_]+)\s*=\s*"?([^"\n]*)"?/);
  if (m) vars[m[1]] = m[2].trim();
}
const request = new FetchRequest(vars[s.rpcEnv.testnet]);
request.setHeader("user-agent", "ipow-redeploy-conversion");
const provider = new JsonRpcProvider(request, undefined, { staticNetwork: true });
const key = vars[s.keyEnv.testnet];
const wallet = new Wallet(key.startsWith("0x") ? key : "0x" + key, provider);
const chainId = Number((await provider.getNetwork()).chainId);
if (chainId !== s.chainId.testnet) throw new Error(`chain ${chainId} is not ${network}'s test network (${s.chainId.testnet})`);

const file = join(here, "..", "deployments", `${network}-testnet.json`);
const d = JSON.parse(readFileSync(file, "utf8"));
const artifact = JSON.parse(readFileSync(join(here, "..", "artifacts", "contracts", "apps", "Conversion.sol", "Conversion.json"), "utf8"));
const old = new Contract(d.conversion, artifact.abi, provider);
// The coin and the cap, as the old one was made: the same settings.
const [coin, maxSats] = await Promise.all([old.coin() as Promise<string>, old.maxSats() as Promise<bigint>]);
const openSwaps = Number(await old.swapCount());
console.log(`${network} (chain ${chainId}): protocol ${d.protocol}, old Conversion ${d.conversion} with ${openSwaps} swaps, coin ${coin === ZeroAddress ? "native" : coin}, cap ${maxSats} sats${dry ? " (dry run)" : ""}`);
if (openSwaps > 0) console.log(`  note: the old contract's ${openSwaps} swaps stay there; a node watching it must keep doing so until they end`);
if (dry) process.exit(0);

const factory = new ContractFactory(artifact.abi, artifact.bytecode, wallet);
const c = await factory.deploy(d.protocol, maxSats, coin);
await c.waitForDeployment();
const address = getAddress(await c.getAddress());
console.log(`Conversion deployed at ${address} (${c.deploymentTransaction()?.hash})`);

// Read back what is on chain, not what the transaction said.
const fresh = new Contract(address, artifact.abi, provider);
let protocolOf = "";
for (let i = 0; i < 10 && !protocolOf; i++) {
  try {
    protocolOf = getAddress(await fresh.protocol());
  } catch {
    await new Promise((r) => setTimeout(r, 3000));
  }
}
const checks: [string, unknown, unknown][] = [
  ["protocol", protocolOf, getAddress(d.protocol)],
  ["maxSats", await fresh.maxSats(), maxSats],
  ["coin", getAddress(await fresh.coin()), getAddress(coin)],
  ["swapCount", await fresh.swapCount(), 0n],
];
for (const [name, got, want] of checks) if (got !== want) throw new Error(`the new Conversion's ${name} reads ${got}, not ${want}`);
console.log("read back: protocol, cap, coin and no swaps match");

d.previousConversion = d.conversion;
d.conversion = address;
d.sources = { ...(d.sources ?? {}), Conversion: "current" };
writeFileSync(file, JSON.stringify(d, null, 2) + "\n");
console.log(`wrote ${file}`);
