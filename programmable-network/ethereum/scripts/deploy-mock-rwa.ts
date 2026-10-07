// Deploys Greatwall.finance's test stand-ins for tokenized assets (MockRWA,
// named "<asset> (<test network> Greatwall)" after the real tokens they
// stand in for, see TOKENS) on one EVM test network, registers each with
// every vault of the network (one per pair, D132: the pair with Solana and
// each EVM pair), so it can be locked toward any peer and named as a
// basket's part there, mints the operator's inventory for swaps, and reads
// it all back. Run it again after a pair is added: it registers the listed
// tokens with the new pair's vault. A token in the record but no longer
// listed in TOKENS (Sepolia's first four) keeps only the vaults it has. Tokens already in deployments/<network>-testnet-rwa.json are
// kept and skipped, so the list can grow between runs. A logo is not on
// chain: Greatwall serves each token's image (public/assets/rwa/<symbol>.png
// for Sepolia, public/assets/rwa/<network>/<symbol>.png for the others)
// in its token list. The key and endpoint are the network package's
// (programmable-network/<network>/.env), as in deploy-network.ts. Run from
// programmable-network/ethereum, after `npx hardhat compile`:
//
//   node scripts/deploy-mock-rwa.ts <network> [--operator <address>] [--inventory <whole tokens>] [--big-blocks] [--dry]
//
// The operator defaults to the deployer's address; the inventory to 1,000 of
// each. With --dry it only prints what it would do. It refuses a network
// that is not a test network, and writes deployments/<network>-testnet-rwa.json.

import { Contract, ContractFactory, FetchRequest, JsonRpcProvider, NonceManager, Wallet, getAddress, parseUnits } from "ethers";
import { existsSync, readFileSync, writeFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

import { NETWORKS } from "../deploy/networks.ts";

/** The tokens of each network: the real tokenized asset each stands in for
 *  (rwa.xyz), and what the faucet gives at a time: the 2026-10-05 sets
 *  Greatwall lists. Sepolia's first four (AAPL, TSLA, NVDA, GOLD) were the
 *  2026-10-04 set, still deployed and in its record, not listed here. */
const TOKENS: Record<string, { symbol: string; name: string; faucet: string }[]> = {
  ethereum: [
    { symbol: "XAUT", name: "Tether Gold (Sepolia Greatwall)", faucet: "1" },
    { symbol: "STRCx", name: "Strategy PP Variable xStock (Sepolia Greatwall)", faucet: "5" },
    { symbol: "IVVon", name: "iShares Core S&P 500 ETF (Ondo) (Sepolia Greatwall)", faucet: "5" },
    { symbol: "QQQon", name: "Invesco QQQ (Ondo) (Sepolia Greatwall)", faucet: "5" },
    { symbol: "NVDAon", name: "NVIDIA (Ondo) (Sepolia Greatwall)", faucet: "5" },
    { symbol: "SLVon", name: "iShares Silver Trust (Ondo) (Sepolia Greatwall)", faucet: "5" },
    { symbol: "IEFAon", name: "iShares Core MSCI EAFE ETF (Ondo) (Sepolia Greatwall)", faucet: "5" },
    { symbol: "AMDon", name: "AMD (Ondo) (Sepolia Greatwall)", faucet: "5" },
    { symbol: "SOFIon", name: "SoFi Technologies (Ondo) (Sepolia Greatwall)", faucet: "5" },
    { symbol: "HIMSon", name: "Hims & Hers Health (Ondo) (Sepolia Greatwall)", faucet: "5" },
    // 2026-10-06, for Greatwall's own index.
    { symbol: "VTIon", name: "Vanguard Total Stock Market ETF (Ondo) (Sepolia Greatwall)", faucet: "5" },
    { symbol: "IEMGon", name: "iShares Core MSCI Emerging Markets ETF (Ondo) (Sepolia Greatwall)", faucet: "5" },
  ],
  base: [
    { symbol: "GLDY", name: "Streamex GLDY (Base Sepolia Greatwall)", faucet: "5" },
    { symbol: "JTRSY", name: "Janus Henderson Treasury Fund (Base Sepolia Greatwall)", faucet: "100" },
    { symbol: "AUDD", name: "Australian Digital Dollar (Base Sepolia Greatwall)", faucet: "100" },
    { symbol: "JSPXA", name: "Janus Henderson S&P500 Fund (Base Sepolia Greatwall)", faucet: "5" },
    { symbol: "JSPX", name: "S&P 500 DeFi (Centrifuge) (Base Sepolia Greatwall)", faucet: "5" },
    { symbol: "bTSLA", name: "Backed Tesla Inc (Base Sepolia Greatwall)", faucet: "5" },
    { symbol: "USTBL", name: "Spiko US T-Bills Money Market Fund (Base Sepolia Greatwall)", faucet: "100" },
    { symbol: "wtNKE", name: "Wrapped NIKE, Inc. ST0x (Base Sepolia Greatwall)", faucet: "5" },
    { symbol: "AMZN.d", name: "Amazon.com, Inc. Common Stock (Base Sepolia Greatwall)", faucet: "5" },
    { symbol: "wtMCD", name: "Wrapped McDonald's Corporation ST0x (Base Sepolia Greatwall)", faucet: "5" },
    // 2026-10-06, for Greatwall's own index.
    { symbol: "GLDx", name: "Gold xStock (Base Sepolia Greatwall)", faucet: "5" },
  ],
  // Robinhood's stock tokens carry no ticker on rwa.xyz; the symbol here is
  // the stock's ticker with an r, the name Robinhood's.
  robinhood: [
    { symbol: "USDG", name: "Global Dollar (Robinhood Testnet Greatwall)", faucet: "100" },
    { symbol: "SPYr", name: "SPDR S&P 500 ETF Trust Robinhood Token (Robinhood Testnet Greatwall)", faucet: "5" },
    { symbol: "NVDAr", name: "NVIDIA Robinhood Token (Robinhood Testnet Greatwall)", faucet: "5" },
    { symbol: "SPCXr", name: "SpaceX Class A Common Stock Robinhood Token (Robinhood Testnet Greatwall)", faucet: "5" },
    { symbol: "METAr", name: "Meta Platforms Robinhood Token (Robinhood Testnet Greatwall)", faucet: "5" },
    { symbol: "COINr", name: "Coinbase Robinhood Token (Robinhood Testnet Greatwall)", faucet: "5" },
    { symbol: "USOr", name: "United States Oil Fund Robinhood Token (Robinhood Testnet Greatwall)", faucet: "5" },
    { symbol: "GMEr", name: "GameStop Robinhood Token (Robinhood Testnet Greatwall)", faucet: "5" },
    { symbol: "PLTRr", name: "Palantir Technologies Robinhood Token (Robinhood Testnet Greatwall)", faucet: "5" },
    { symbol: "SGOVr", name: "iShares 0-3 Month Treasury Bond Robinhood Token (Robinhood Testnet Greatwall)", faucet: "100" },
  ],
  hyperliquid: [
    { symbol: "thBILL", name: "Theo Short Duration US Treasury Fund (HyperEVM Testnet Greatwall)", faucet: "100" },
    { symbol: "USDH", name: "USDH (HyperEVM Testnet Greatwall)", faucet: "100" },
    { symbol: "GMEx", name: "Gamestop xStock (HyperEVM Testnet Greatwall)", faucet: "5" },
    { symbol: "MUx", name: "Micron Technology xStock (HyperEVM Testnet Greatwall)", faucet: "5" },
    { symbol: "USDHL", name: "Hyper USD (HyperEVM Testnet Greatwall)", faucet: "100" },
    { symbol: "SKHYx", name: "SK hynix xStock (HyperEVM Testnet Greatwall)", faucet: "5" },
    { symbol: "NBISx", name: "Nebius xStock (HyperEVM Testnet Greatwall)", faucet: "5" },
    { symbol: "IWMx", name: "Russell 2000 xStock (HyperEVM Testnet Greatwall)", faucet: "5" },
    { symbol: "CRWVx", name: "CoreWeave xStock (HyperEVM Testnet Greatwall)", faucet: "5" },
    // 2026-10-06, for Greatwall's own index.
    { symbol: "SPYx", name: "SP500 xStock (HyperEVM Testnet Greatwall)", faucet: "5" },
  ],
  // Arbitrum (D141): Reality's stock tokens, Pleasing Gold, and Coinbase
  // Wrapped BTC for Greatwall's own index (2026-10-06).
  arbitrum: [
    { symbol: "rINTC", name: "Intel Reality (Arbitrum Sepolia Greatwall)", faucet: "5" },
    { symbol: "rMSTR", name: "MicroStrategy Reality (Arbitrum Sepolia Greatwall)", faucet: "5" },
    { symbol: "rMU", name: "Micron Technology Reality (Arbitrum Sepolia Greatwall)", faucet: "5" },
    { symbol: "rMRNA", name: "Moderna Reality (Arbitrum Sepolia Greatwall)", faucet: "5" },
    { symbol: "rOKLO", name: "Oklo Inc Reality (Arbitrum Sepolia Greatwall)", faucet: "5" },
    { symbol: "PGOLD", name: "Pleasing Gold (Arbitrum Sepolia Greatwall)", faucet: "1" },
    { symbol: "rDRAM", name: "Roundhill Memory ETF Reality (Arbitrum Sepolia Greatwall)", faucet: "5" },
    { symbol: "rSPY", name: "SPDR S&P 500 ETF Reality (Arbitrum Sepolia Greatwall)", faucet: "5" },
    { symbol: "rSNDK", name: "SanDisk Reality (Arbitrum Sepolia Greatwall)", faucet: "5" },
    { symbol: "rTSM", name: "Taiwan Semiconductor Reality (Arbitrum Sepolia Greatwall)", faucet: "5" },
    { symbol: "cbBTC", name: "Coinbase Wrapped BTC (Arbitrum Sepolia Greatwall)", faucet: "0.01" },
  ],
};

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
const tokens = TOKENS[network!];
if (!tokens) throw new Error(`no tokens listed for ${network}`);
// HyperEVM puts contract deployments in its big blocks: the deployer's
// address must be switched to them first (as for deploy-network.ts).
if (s.bigBlocks && !dry && !args.includes("--big-blocks")) throw new Error(`${network}: deploying needs big blocks; switch the deployer's address to them, then pass --big-blocks`);

const vars: Record<string, string> = {};
for (const line of readFileSync(join(here, "..", "..", network!, ".env"), "utf8").split("\n")) {
  const m = line.match(/^\s*([A-Z0-9_]+)\s*=\s*"?([^"\n]*)"?/);
  if (m) vars[m[1]] = m[2].trim();
}
const request = new FetchRequest(vars[s.rpcEnv.testnet]);
request.setHeader("user-agent", "ipow-mock-rwa");
const provider = new JsonRpcProvider(request, undefined, { staticNetwork: true });
const key = vars[s.keyEnv.testnet];
// The nonce is counted here, not asked of the endpoint before each send:
// a node behind the pool can be a block behind and answer with a used one.
const deployer = new Wallet(key.startsWith("0x") ? key : "0x" + key, provider);
const wallet = new NonceManager(deployer);

// A test network only: the chain must be the one deploy/networks.ts names for it.
const chainId = Number((await provider.getNetwork()).chainId);
if (chainId !== s.chainId.testnet) throw new Error(`chain ${chainId} is not ${network}'s test network (${s.chainId.testnet})`);

const d = JSON.parse(readFileSync(join(here, "..", "deployments", `${network}-testnet.json`), "utf8"));
const homeAddress: string = d.vaults[0].home;
/** Every vault of the network: one per pair (D132). The first, paired with
 *  Solana, gives a token's `vaultAsset`; each pair's number is kept by peer. */
const HOME_ABI = ["function registerAsset(address) returns (uint32)", "function assetOfToken(address) view returns (uint32)", "event AssetRegistered(uint32 indexed asset, address indexed token, uint8 decimals, uint8 recordDecimals)"];
const homes: { peer: number; home: Contract }[] = d.vaults.map((v: any) => ({ peer: Number(v.peer), home: new Contract(v.home, HOME_ABI, wallet) }));
const operator = getAddress(flag("--operator") ?? deployer.address);
const inventory = flag("--inventory") ?? "1000";
const artifact = JSON.parse(readFileSync(join(here, "..", "artifacts", "contracts", "testnet", "MockRWA.sol", "MockRWA.json"), "utf8"));
const home = homes[0].home;

const out = join(here, "..", "deployments", `${network}-testnet-rwa.json`);
const existing = existsSync(out) ? JSON.parse(readFileSync(out, "utf8")) : { tokens: {} };
console.log(`${network} test network (chain ${chainId}): deployer ${deployer.address}, operator ${operator}, vaults with ${homes.map((h) => h.peer).join(", ")} (first ${homeAddress})${dry ? " (dry run)" : ""}`);

/** The record so far: written after every token, so a failure part-way
 *  (HyperEVM's big-block fee spiked 5,000× on 2026-10-05, after eight of
 *  nine) loses nothing and a re-run continues. */
const save = () =>
  writeFileSync(out, JSON.stringify({ network: `${network}-testnet`, chainId, minter: deployer.address, operator, tokens: existing.tokens }, null, 2) + "\n");
/** Fees on a big-block network: capped at twice the big-block price now,
 *  rather than the node's estimate, which spiked to 500 gwei once. */
let lastBigPrice: bigint | null = null;
const fees = async () => {
  if (!s.bigBlocks) return {};
  // HyperEVM's endpoint sometimes answers "method not found" for the price
  // between answers (2026-10-06): asked again, then the last price read.
  for (let i = 0; i < 5; i++) {
    try {
      lastBigPrice = BigInt(await provider.send("eth_bigBlockGasPrice", []));
      break;
    } catch (e) {
      if (i === 4 && lastBigPrice === null) throw e;
      await new Promise((r) => setTimeout(r, 3_000));
    }
  }
  return { maxFeePerGas: lastBigPrice! * 2n, maxPriorityFeePerGas: 0n };
};

/** The transactions' gas, set rather than estimated: on Base Sepolia the
 *  estimate for a mint came from a node that had not seen the deploy yet
 *  (22,946 gas; the mint ran out and reverted, 2026-10-05). */
const GAS = { mint: 120_000, register: 300_000 };
/** On an Arbitrum chain gas also pays for posting the data to Ethereum, at
 *  a price that moves: the estimate and half again, at least the set gas. */
const gasOf = async (estimate: () => Promise<bigint>, set = 0) =>
  s.dataFee === "arb" ? { gasLimit: [BigInt(set), ((await estimate()) * 3n) / 2n].reduce((a, b) => (a > b ? a : b)) } : set ? { gasLimit: set } : {};
/** How long a transaction may take to land before the run fails loudly: a
 *  capped fee can be left behind by a price that rises after it was read. */
const WAIT_MS = 180_000;

for (const t of tokens) {
  let address: string | undefined = existing.tokens[t.symbol]?.address;
  if (address && (await provider.getCode(address)) !== "0x") {
    console.log(`${t.symbol}: deployed already at ${address}`);
  } else if (dry) {
    console.log(`${t.symbol}: would deploy "${t.name}", faucet ${t.faucet}, mint ${inventory} to the operator, register with the vaults of pairs ${homes.map((h) => h.peer).join(", ")}`);
    continue;
  } else {
    const factory = new ContractFactory(artifact.abi, artifact.bytecode, wallet);
    const c = await factory.deploy(t.name, t.symbol, parseUnits(t.faucet, 18), {
      ...(await fees()),
      ...(await gasOf(async () => wallet.estimateGas(await factory.getDeployTransaction(t.name, t.symbol, parseUnits(t.faucet, 18))))),
    });
    await c.deploymentTransaction()!.wait(1, WAIT_MS);
    address = await c.getAddress();
    console.log(`${t.symbol}: deployed at ${address} (${c.deploymentTransaction()?.hash})`);
    // Recorded at once, before anything else can fail.
    existing.tokens[t.symbol] = { address, name: t.name, decimals: 18, faucet: t.faucet };
    save();
    // Wait until the code is visible at all (the mint's gas is set, not
    // estimated, so a node that still lags cannot starve it).
    for (let i = 0; (await provider.getCode(address)) === "0x"; i++) {
      if (i === 30) throw new Error(`${t.symbol}: no code at ${address} after a minute`);
      await new Promise((r) => setTimeout(r, 2_000));
    }
  }
  if (dry) continue;
  // The operator's inventory, minted once: on a fresh deployment, or after
  // a run that stopped before it.
  const token = new Contract(address, artifact.abi, wallet);
  if ((await token.balanceOf(operator)) === 0n) {
    await (await token.mint(operator, parseUnits(inventory, 18), { ...(await gasOf(() => token.mint.estimateGas(operator, parseUnits(inventory, 18)), GAS.mint)), ...(await fees()) })).wait(1, WAIT_MS);
    console.log(`${t.symbol}: minted ${inventory} to ${operator}`);
  }
  // Registered with each pair's vault once: asset numbers start at 1 for
  // tokens there.
  const pairAssets: Record<string, number> = { ...(existing.tokens[t.symbol]?.pairAssets ?? {}) };
  for (const { peer, home: h } of homes) {
    let assetPlusOne = Number(await h.assetOfToken(address));
    if (assetPlusOne === 0) {
      await (await h.registerAsset(address, { ...(await gasOf(() => h.registerAsset.estimateGas(address), GAS.register)), ...(await fees()) })).wait(1, WAIT_MS);
      // Read until a node behind the endpoint shows it (Base Sepolia's gave
      // 0 right after the receipt, 2026-10-05).
      for (let i = 0; (assetPlusOne = Number(await h.assetOfToken(address))) === 0; i++) {
        if (i === 30) throw new Error(`${t.symbol}: not registered with the vault of pair ${peer} after a minute`);
        await new Promise((r) => setTimeout(r, 2_000));
      }
      console.log(`${t.symbol}: registered with the vault of pair ${peer} as asset ${assetPlusOne - 1}`);
    }
    pairAssets[peer] = assetPlusOne - 1;
  }
  existing.tokens[t.symbol] = { address, name: t.name, decimals: 18, faucet: t.faucet, vaultAsset: pairAssets[homes[0].peer], pairAssets };
  save();
}

if (!dry) {
  // Read back what is on chain, not what the transactions said.
  for (const [symbol, t] of Object.entries<any>(existing.tokens)) {
    const token = new Contract(t.address, artifact.abi, provider);
    const [name, sym, minter, faucet, held, asset] = await Promise.all([
      token.name(),
      token.symbol(),
      token.minter(),
      token.faucetAmount(),
      token.balanceOf(operator),
      home.assetOfToken(t.address),
    ]);
    if (name !== t.name || sym !== symbol || getAddress(minter) !== deployer.address || faucet !== parseUnits(t.faucet, 18) || Number(asset) - 1 !== t.vaultAsset)
      throw new Error(`${symbol} at ${t.address} does not read back as recorded: name "${name}", symbol ${sym}, minter ${minter}, faucet ${faucet}, vault asset ${Number(asset) - 1} (record: "${t.name}", ${symbol}, ${deployer.address}, ${parseUnits(t.faucet, 18)}, ${t.vaultAsset})`);
    for (const [peer, n] of Object.entries<number>(t.pairAssets ?? {})) {
      const h = homes.find((x) => String(x.peer) === peer);
      const got = h ? Number(await h.home.assetOfToken(t.address)) - 1 : -1;
      if (got !== n) throw new Error(`${symbol}: the vault of pair ${peer} reads asset ${got}, recorded ${n}`);
    }
    console.log(`checked ${symbol}: ${t.address}, vault asset ${t.vaultAsset}${t.pairAssets ? `, by pair ${JSON.stringify(t.pairAssets)}` : ""}, operator holds ${held / 10n ** 18n}`);
  }
  save();
  console.log(`wrote ${out}`);
}
