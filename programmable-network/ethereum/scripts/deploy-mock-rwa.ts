// Deploys Greatwall.finance's test stand-ins for tokenized assets (MockRWA:
// AAPL, TSLA, NVDA, GOLD) on one EVM test network, registers each with the
// vault so it can be locked, mints the operator's inventory for swaps, and
// reads it all back. The key and endpoint are the network package's
// (programmable-network/<network>/.env), as in deploy-network.ts. Run from
// programmable-network/ethereum, after `npx hardhat compile`:
//
//   node scripts/deploy-mock-rwa.ts <network> [--operator <address>] [--inventory <whole tokens>] [--dry]
//
// The operator defaults to the deployer's address; the inventory to 1,000 of
// each. With --dry it only prints what it would do. It refuses a network
// that is not a test network, and writes deployments/<network>-testnet-rwa.json.

import { Contract, ContractFactory, FetchRequest, JsonRpcProvider, Wallet, getAddress, parseUnits } from "ethers";
import { existsSync, readFileSync, writeFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

import { NETWORKS } from "../deploy/networks.ts";

/** The tokens: what each is, and what the faucet gives at a time. */
const TOKENS = [
  { symbol: "AAPL", name: "Apple (test)", faucet: "5" },
  { symbol: "TSLA", name: "Tesla (test)", faucet: "5" },
  { symbol: "NVDA", name: "NVIDIA (test)", faucet: "5" },
  { symbol: "GOLD", name: "Gold, 1 troy ounce (test)", faucet: "1" },
];

const here = dirname(fileURLToPath(import.meta.url));
const args = process.argv.slice(2);
const flag = (name: string) => {
  const i = args.indexOf(name);
  return i >= 0 ? args[i + 1] : undefined;
};
const network = args.find((a) => !a.startsWith("--") && args[args.indexOf(a) - 1]?.startsWith("--") !== true);
const dry = args.includes("--dry");
const s = network ? NETWORKS[network] : undefined;
if (!s || !s.rpcEnv.testnet) throw new Error("a network from deploy/networks.ts that has a test network");

const vars: Record<string, string> = {};
for (const line of readFileSync(join(here, "..", "..", network!, ".env"), "utf8").split("\n")) {
  const m = line.match(/^\s*([A-Z0-9_]+)\s*=\s*"?([^"\n]*)"?/);
  if (m) vars[m[1]] = m[2].trim();
}
const request = new FetchRequest(vars[s.rpcEnv.testnet]);
request.setHeader("user-agent", "ipow-mock-rwa");
const provider = new JsonRpcProvider(request, undefined, { staticNetwork: true });
const key = vars[s.keyEnv.testnet];
const wallet = new Wallet(key.startsWith("0x") ? key : "0x" + key, provider);

// A test network only: the chain must be the one deploy/networks.ts names for it.
const chainId = Number((await provider.getNetwork()).chainId);
if (chainId !== s.chainId.testnet) throw new Error(`chain ${chainId} is not ${network}'s test network (${s.chainId.testnet})`);

const d = JSON.parse(readFileSync(join(here, "..", "deployments", `${network}-testnet.json`), "utf8"));
const homeAddress: string = d.vaults[0].home;
const operator = getAddress(flag("--operator") ?? wallet.address);
const inventory = flag("--inventory") ?? "1000";
const artifact = JSON.parse(readFileSync(join(here, "..", "artifacts", "contracts", "testnet", "MockRWA.sol", "MockRWA.json"), "utf8"));
const home = new Contract(
  homeAddress,
  ["function registerAsset(address) returns (uint32)", "function assetOfToken(address) view returns (uint32)", "event AssetRegistered(uint32 indexed asset, address indexed token, uint8 decimals, uint8 recordDecimals)"],
  wallet
);

const out = join(here, "..", "deployments", `${network}-testnet-rwa.json`);
const existing = existsSync(out) ? JSON.parse(readFileSync(out, "utf8")) : { tokens: {} };
console.log(`${network} test network (chain ${chainId}): deployer ${wallet.address}, operator ${operator}, vault home ${homeAddress}${dry ? " (dry run)" : ""}`);

for (const t of TOKENS) {
  let address: string | undefined = existing.tokens[t.symbol]?.address;
  if (address && (await provider.getCode(address)) !== "0x") {
    console.log(`${t.symbol}: deployed already at ${address}`);
  } else if (dry) {
    console.log(`${t.symbol}: would deploy "${t.name}", faucet ${t.faucet}, mint ${inventory} to the operator, register with the vault`);
    continue;
  } else {
    const factory = new ContractFactory(artifact.abi, artifact.bytecode, wallet);
    const c = await factory.deploy(t.name, t.symbol, parseUnits(t.faucet, 18));
    await c.waitForDeployment();
    address = await c.getAddress();
    console.log(`${t.symbol}: deployed at ${address} (${c.deploymentTransaction()?.hash})`);
    const token = new Contract(address, artifact.abi, wallet);
    await (await token.mint(operator, parseUnits(inventory, 18))).wait();
    console.log(`${t.symbol}: minted ${inventory} to ${operator}`);
  }
  if (dry) continue;
  // Registered with the vault once: asset numbers start at 1 for tokens.
  let assetPlusOne = Number(await home.assetOfToken(address));
  if (assetPlusOne === 0) {
    await (await home.registerAsset(address)).wait();
    assetPlusOne = Number(await home.assetOfToken(address));
    console.log(`${t.symbol}: registered with the vault as asset ${assetPlusOne - 1}`);
  }
  existing.tokens[t.symbol] = { address, name: t.name, decimals: 18, faucet: t.faucet, vaultAsset: assetPlusOne - 1 };
}

if (!dry) {
  // Read back what is on chain, not what the transactions said.
  for (const [symbol, t] of Object.entries<any>(existing.tokens)) {
    const token = new Contract(t.address, artifact.abi, provider);
    const [sym, minter, faucet, held, asset] = await Promise.all([
      token.symbol(),
      token.minter(),
      token.faucetAmount(),
      token.balanceOf(operator),
      home.assetOfToken(t.address),
    ]);
    if (sym !== symbol || getAddress(minter) !== wallet.address || faucet !== parseUnits(t.faucet, 18) || Number(asset) - 1 !== t.vaultAsset)
      throw new Error(`${symbol} at ${t.address} does not read back as deployed`);
    console.log(`checked ${symbol}: ${t.address}, vault asset ${t.vaultAsset}, operator holds ${held / 10n ** 18n}`);
  }
  writeFileSync(out, JSON.stringify({ network: `${network}-testnet`, chainId, minter: wallet.address, operator, tokens: existing.tokens }, null, 2) + "\n");
  console.log(`wrote ${out}`);
}
