// Redeploys one EVM test network's vault paired with Solana, beside the
// network's protocol as already deployed: new factories, the parts, and the
// vault naming the Solana vault's pair account. With --genesis-days, its
// receipts start in genesis for the deployer, ending N days from now at the
// latest (docs/specs/ipow-vault-genesis.md). The record's vault paired with
// Solana is replaced; the protocol and the other pairs are left as they are.
// Run from evm after the contracts are committed:
//
//   node scripts/redeploy-solana-vault.ts <network> <solanaPairAccount> [--genesis-days N] [--big-blocks] [--rpc <url>]
//
// --rpc sends through another endpoint than the .env's: HyperEVM's own
// (https://rpc.hyperliquid-testnet.xyz/evm), whose gas estimate knows the
// deployer's big blocks, where Alchemy's estimates against small blocks.
//
// <solanaPairAccount> is ["config", <network number>] of the Solana vault
// program the pair is set up on (for genesis, the new program; its init
// script prints every peer's). The key and endpoint come from the network
// package's own git-ignored .env; nothing secret is printed. Tempo deploys
// through its own sender: tempo/scripts/deploy-new-protocol.ts --vaults-only.

import { FetchRequest, JsonRpcProvider, Wallet } from "ethers";
import { copyFileSync, mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

import { contractsSource, deployNetwork, solanaAccountBytes } from "../deploy/deploy.ts";
import { RetryProvider } from "../deploy/retry-provider.ts";
import { NETWORKS, envFile } from "../deploy/networks.ts";

const here = dirname(fileURLToPath(import.meta.url));
const args = process.argv.slice(2);
const at = args.indexOf("--genesis-days");
const genesisDays = at >= 0 ? Number(args[at + 1]) : 0;
const [network, solanaPair] = args.filter((a, i) => !a.startsWith("--") && args[i - 1] !== "--genesis-days" && args[i - 1] !== "--rpc");
const rpcOverride = args.includes("--rpc") ? args[args.indexOf("--rpc") + 1] : undefined;
if (!network || !solanaPair) throw new Error("usage: redeploy-solana-vault.ts <network> <solanaPairAccount> [--genesis-days N] [--big-blocks]");
if (at >= 0 && !(genesisDays > 0 && genesisDays <= 30)) throw new Error("--genesis-days: 1 to 30");
const bigBlocks = args.includes("--big-blocks");

const settings = NETWORKS[network];
if (!settings) throw new Error(`unknown network ${network}`);
if (settings.sender) throw new Error(`${network} deploys through its own sender: use its own script`);

function readEnv(file: string): Record<string, string> {
  const out: Record<string, string> = {};
  for (const line of readFileSync(file, "utf8").split("\n")) {
    const m = line.match(/^\s*([A-Z0-9_]+)\s*=\s*"?([^"\n]*)"?/);
    if (m) out[m[1]] = m[2].trim();
  }
  return out;
}

const source = contractsSource();
const file = join(here, "..", "deployments", `${network}-testnet.json`);
const record = JSON.parse(readFileSync(file, "utf8"));
const vars = readEnv(envFile(network));
const provider = new RetryProvider(rpcOverride ?? vars[settings.rpcEnv.testnet!]);
const key = vars[settings.keyEnv.testnet!];
const signer = new Wallet(key.startsWith("0x") ? key : "0x" + key, provider);
if (signer.address.toLowerCase() !== String(record.deployer).toLowerCase()) {
  throw new Error(`${network}: the key is ${signer.address}, the record's deployer ${record.deployer}`);
}
// Hedera's relay estimates fees wrongly for typed transactions.
const overrides = network === "hedera" ? { type: 0, gasPrice: (await provider.getFeeData()).gasPrice } : {};
const genesis = genesisDays ? { key: signer.address, end: BigInt(Math.floor(Date.now() / 1000) + genesisDays * 86_400) } : undefined;

console.log(`${network}: a new vault paired with Solana (${solanaPair}), beside protocol ${record.protocol}${genesis ? `, in genesis for ${genesis.key} until ${new Date(Number(genesis.end) * 1000).toISOString()}` : ""}`);
const deployment = await deployNetwork(signer, {
  network,
  env: "testnet",
  minHeight: record.minHeight,
  maxSats: BigInt(record.maxSats),
  pairs: [{ peer: 2, peerVault: solanaAccountBytes(solanaPair) }],
  existing: { lightClient: record.lightClient, dataFee: record.dataFee, protocol: record.protocol, conversion: record.conversion, betaBaskets: record.betaBaskets },
  genesis,
  bigBlocks,
  overrides,
  log: (line) => console.log(line),
});

// The record before, kept beside it; then the new vault in place of the old.
mkdirSync(join(here, "..", "deployments", "replaced"), { recursive: true });
copyFileSync(file, join(here, "..", "deployments", "replaced", `${network}-testnet.${Date.now()}.json`));
const vault = deployment.vaults[0];
record.vaults = [
  { ...vault, homeFactory: deployment.homeFactory, receiptsFactory: deployment.receiptsFactory, source, ...(genesis ? { genesisEnd: Number(genesis.end) } : {}), at: new Date().toISOString() },
  ...record.vaults.filter((v: { peer: number }) => v.peer !== 2),
];
record.homeFactory = deployment.homeFactory;
record.receiptsFactory = deployment.receiptsFactory;
// The network's factories are now the new vault's: name their source, as
// the record's own `source` is the protocol's (verify-deployments.ts).
record.sources = { ...(record.sources ?? {}), VaultHomeFactory: source, VaultReceiptsFactory: source };
record.solanaPairAccount = solanaPair;
writeFileSync(file, JSON.stringify(record, null, 2) + "\n");
console.log(`written ${file}: vault ${vault.vault} names ${solanaPair}`);
