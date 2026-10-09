// Redeploys BetaBaskets alone on one EVM test network: for a change to
// BETA only (the creator's naming of a basket's token and its metadata
// URI, E4 in docs/specs/ipow-beta-app.md). Its baskets' record changed, so
// it is a new contract, not an upgrade; the old one keeps its baskets,
// which go on there by its own rules. BetaBaskets takes nothing at
// deployment. The key and endpoint are the network package's
// (networks/<network>/.env, evm/.env for Ethereum). Run from evm
// after `npx hardhat compile`:
//
//   node scripts/redeploy-beta.ts <network> [--dry]
//
// It reads the new contract's constants back, then writes the new address
// to deployments/<network>-testnet.json, keeping the old one as
// `previousBetaBaskets`, and `sources.BetaBaskets` = the commit it is built from (deploy.ts
// `contractsSource`: the contracts must be committed), so that
// verify-deployments.ts checks it against that commit's build. Then run
// verify-deployments.ts and sdk's scripts/sync.ts. Not Tempo: its
// transactions are its own type (deploy/networks.ts `sender`), so its
// BetaBaskets is redeployed by a script of the Tempo package, not written yet.

import { Contract, ContractFactory, FetchRequest, JsonRpcProvider, Wallet, getAddress } from "ethers";
import { readFileSync, writeFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

import { contractsSource } from "../deploy/deploy.ts";
import { NETWORKS, envFile } from "../deploy/networks.ts";

const here = dirname(fileURLToPath(import.meta.url));
const network = process.argv.slice(2).find((a) => !a.startsWith("--"));
const dry = process.argv.includes("--dry");
const s = network ? NETWORKS[network] : undefined;
if (!s || !s.rpcEnv.testnet) throw new Error("a network from deploy/networks.ts that has a test network");
if (s.sender) throw new Error(`${network} deploys through its own sender (${s.sender}); not built here`);
/** The commit the contract is built from; refuses uncommitted contracts. */
const source = contractsSource();
// HyperEVM puts contract deployments in its big blocks: the deployer's
// address must be switched to them first (scripts/hyperliquid-big-blocks.ts).
if (s.bigBlocks && !dry && !process.argv.includes("--big-blocks")) throw new Error(`${network}: deploying needs big blocks; switch the deployer's address to them, then pass --big-blocks`);

const vars: Record<string, string> = {};
for (const line of readFileSync(envFile(network!), "utf8").split("\n")) {
  const m = line.match(/^\s*([A-Z0-9_]+)\s*=\s*"?([^"\n]*)"?/);
  if (m) vars[m[1]] = m[2].trim();
}
const request = new FetchRequest(vars[s.rpcEnv.testnet]);
request.setHeader("user-agent", "ipow-redeploy-beta");
const provider = new JsonRpcProvider(request, undefined, { staticNetwork: true });
const key = vars[s.keyEnv.testnet];
const wallet = new Wallet(key.startsWith("0x") ? key : "0x" + key, provider);
const chainId = Number((await provider.getNetwork()).chainId);
if (chainId !== s.chainId.testnet) throw new Error(`chain ${chainId} is not ${network}'s test network (${s.chainId.testnet})`);

const file = join(here, "..", "deployments", `${network}-testnet.json`);
const d = JSON.parse(readFileSync(file, "utf8"));
const artifact = JSON.parse(readFileSync(join(here, "..", "artifacts", "contracts", "applications", "beta", "BetaBaskets.sol", "BetaBaskets.json"), "utf8"));
console.log(`${network} (chain ${chainId}): old BetaBaskets ${d.betaBaskets}${dry ? " (dry run)" : ""}`);
console.log("  note: the old contract's baskets stay there, under its own rules; apps that remember them keep opening them there");
if (dry) process.exit(0);

/** Fees on a big-block network, capped at twice the big-block price now. */
const fees = async () => {
  if (!s.bigBlocks) return {};
  const price = BigInt(await provider.send("eth_bigBlockGasPrice", []));
  return { maxFeePerGas: price * 2n, maxPriorityFeePerGas: 0n };
};
const factory = new ContractFactory(artifact.abi, artifact.bytecode, wallet);
const c = await factory.deploy(await fees());
await c.deploymentTransaction()!.wait(1, 180_000);
const address = getAddress(await c.getAddress());
console.log(`BetaBaskets deployed at ${address} (${c.deploymentTransaction()?.hash})`);

// Read back what is on chain, not what the transaction said: the constants
// of the build, and that no name is left for a creation to read.
const fresh = new Contract(address, artifact.abi, provider);
for (let i = 0; (await provider.getCode(address)) === "0x"; i++) {
  if (i === 30) throw new Error(`no code at ${address} after a minute`);
  await new Promise((r) => setTimeout(r, 2_000));
}
const checks: [string, unknown, unknown][] = [
  ["MAX_PARTS", await fresh.MAX_PARTS(), 8n],
  ["MAX_FEE_BPS", await fresh.MAX_FEE_BPS(), 100n],
  ["MAX_NAME", await fresh.MAX_NAME(), 64n],
  ["MAX_SYMBOL", await fresh.MAX_SYMBOL(), 16n],
  ["MAX_URI", await fresh.MAX_URI(), 2048n],
  ["creatingName", await fresh.creatingName(), ""],
  ["creatingSymbol", await fresh.creatingSymbol(), ""],
];
for (const [name, got, want] of checks) if (got !== want) throw new Error(`the new BetaBaskets' ${name} reads ${got}, not ${want}`);
console.log("read back: the limits and an empty naming match");

d.previousBetaBaskets = d.betaBaskets;
d.betaBaskets = address;
d.sources = { ...(d.sources ?? {}), BetaBaskets: source };
writeFileSync(file, JSON.stringify(d, null, 2) + "\n");
console.log(`wrote ${file}`);
