// Deploys the protocol on one EVM test network from its settings
// (deploy/networks.ts), with its vault paired with Solana, and writes the
// addresses read back to deployments/<network>-<env>.json.
//
//   node scripts/deploy-network.ts <network> <env> <minHeight> <maxSats> <solanaPairAccount> [--big-blocks]
//
// The key and endpoint come from the network package's own git-ignored
// .env (networks/<network>/.env, evm/.env for Ethereum); nothing secret is printed.
// <solanaPairAccount> is the Solana vault's pair account for this network,
// `["config", <network number>]` of the ipow-vault program, in base58.

import { JsonRpcProvider, Wallet, FetchRequest } from "ethers";
import { readFileSync, writeFileSync, mkdirSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

import { contractsSource, deployNetwork, solanaAccountBytes } from "../deploy/deploy.ts";
import { NETWORKS, type Env, envFile } from "../deploy/networks.ts";

const here = dirname(fileURLToPath(import.meta.url));
const [network, env, minHeight, maxSats, solanaPair] = process.argv.slice(2).filter((a) => !a.startsWith("--"));
// Only once the deployer's address is confirmed to use big blocks (HyperEVM).
const bigBlocks = process.argv.includes("--big-blocks");
if (!solanaPair) throw new Error("usage: deploy-network.ts <network> <env> <minHeight> <maxSats> <solanaPairAccount>");

function readEnv(file: string): Record<string, string> {
  const out: Record<string, string> = {};
  for (const line of readFileSync(file, "utf8").split("\n")) {
    const m = line.match(/^\s*([A-Z0-9_]+)\s*=\s*"?([^"\n]*)"?/);
    if (m) out[m[1]] = m[2].trim();
  }
  return out;
}


const source = contractsSource();
const settings = NETWORKS[network];
if (!settings) throw new Error(`unknown network ${network}`);
if (!Number.isInteger(Number(minHeight)) || Number(minHeight) <= 0 || !/^[0-9]+$/.test(maxSats)) throw new Error("minHeight and maxSats must be whole numbers");
const vars = readEnv(envFile(network));
const request = new FetchRequest(vars[settings.rpcEnv[env as Env]]);
// Some public endpoints refuse a request without one.
request.setHeader("user-agent", "ipow-deploy");
const provider = new JsonRpcProvider(request, undefined, { staticNetwork: true });
const key = vars[settings.keyEnv[env as Env]];
const signer = new Wallet(key.startsWith("0x") ? key : "0x" + key, provider);

// Hedera's relay estimates fees wrongly for typed transactions.
const overrides = network === "hedera" ? { type: 0, gasPrice: (await provider.getFeeData()).gasPrice } : {};

const deployment = await deployNetwork(signer, {
  network,
  env: env as Env,
  minHeight: Number(minHeight),
  maxSats: BigInt(maxSats),
  pairs: [{ peer: 2, peerVault: solanaAccountBytes(solanaPair) }],
  bigBlocks,
  overrides,
  log: (line) => console.log(line),
});

const out = join(here, "..", "deployments");
mkdirSync(out, { recursive: true });
const file = join(out, `${network}-${env}.json`);
writeFileSync(file, JSON.stringify({ ...deployment, source, deployer: signer.address, minHeight: Number(minHeight), maxSats, solanaPairAccount: solanaPair, at: new Date().toISOString() }, null, 2) + "\n");
console.log(`written ${file}`);
