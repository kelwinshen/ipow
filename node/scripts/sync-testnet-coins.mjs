#!/usr/bin/env node
// Brings node.testnet.yml up to date with the test deployments: adds a
// network the node does not list yet (from
// evm/deployments/<network>-testnet.json) and, on
// each EVM network, a Conversion coin line for every mock RWA token of
// deployments/<network>-testnet-rwa.json that is not listed yet, at the
// stand-in price of PRICES. A line already there is never changed, so a
// price set by hand stays. Run after deploying, from node:
//
//   node scripts/sync-testnet-coins.mjs [--dry]
//
// Prices are sats per whole token at a hundredth of the market, as the
// comment on Sepolia's Conversion says: a stand-in rate, not a quote.

import { existsSync, readFileSync, writeFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const here = dirname(fileURLToPath(import.meta.url));
const SETTINGS = join(here, "..", "node.testnet.yml");
const DEPLOYMENTS = join(here, "..", "..", "evm", "deployments");
const dry = process.argv.includes("--dry");

/** The node's names of the EVM test networks, and their packages' .env variables. */
const NETWORKS = {
  ethereum: { name: "ethereum-sepolia", rpc: "SEPOLIA_RPC_URL", key: "SEPOLIA_PRIVATE_KEY" },
  base: { name: "base-sepolia", rpc: "BASE_SEPOLIA_RPC_URL", key: "BASE_SEPOLIA_PRIVATE_KEY" },
  hyperliquid: { name: "hyperliquid-testnet", rpc: "HYPEREVM_TESTNET_RPC_URL", key: "HYPEREVM_TESTNET_PRIVATE_KEY" },
  robinhood: { name: "robinhood-testnet", rpc: "ROBINHOOD_TESTNET_RPC_URL", key: "ROBINHOOD_TESTNET_PRIVATE_KEY" },
  arbitrum: { name: "arbitrum-sepolia", rpc: "ARBITRUM_SEPOLIA_RPC_URL", key: "ARBITRUM_SEPOLIA_PRIVATE_KEY" },
};

/** Sats per whole token: a hundredth of the market (about $1 = 10 sats). */
const PRICES = {
  // Greatwall's index (2026-10-06).
  cbBTC: 1_000_000, // one bitcoin
  VTIon: 3_200,
  IEMGon: 650,
  SPYx: 6_500,
  GLDx: 3_700,
  // Arbitrum's set (2026-10-06).
  rINTC: 350,
  rMSTR: 3_300,
  rMU: 1_800,
  rMRNA: 280,
  rOKLO: 1_000,
  PGOLD: 40_000, // priced as XAUT, an ounce of gold
  rDRAM: 300,
  rSPY: 6_500,
  rSNDK: 1_200,
  rTSM: 2_900,
};

let yml = readFileSync(SETTINGS, "utf8");
const changes = [];

/** The block of one network in `networks:`, as [start, end) offsets. */
function block(name) {
  const start = yml.indexOf(`\n  - name: ${name}\n`);
  if (start < 0) return null;
  const rest = yml.slice(start + 1);
  const next = rest.search(/\n  - name: |\n\S/);
  return [start + 1, next < 0 ? yml.length : start + 1 + next + 1];
}

for (const [network, n] of Object.entries(NETWORKS)) {
  const file = join(DEPLOYMENTS, `${network}-testnet.json`);
  if (!existsSync(file)) {
    console.log(`${network}: not deployed yet, skipped`);
    continue;
  }
  const d = JSON.parse(readFileSync(file, "utf8"));

  // The network itself, after the last EVM network (before Solana's).
  if (!block(n.name)) {
    const entry = [
      `  - name: ${n.name}`,
      `    kind: evm`,
      `    rpc_env: ${n.rpc}`,
      `    key_env: ${n.key}`,
      `    light_client: "${d.lightClient}"`,
      `    protocol: "${d.protocol}"`,
      `    # A challenge's deposit, at most 0.01 ETH.`,
      `    guardian: { max_deposit: "10000000000000000" }`,
      `    # At most 0.01 ETH of bond locked for one job, as on the other rollups.`,
      `    operator: { max_bid: "10000000000000000" }`,
      `    # Conversion (added by scripts/sync-testnet-coins.mjs): the coin and`,
      `    # Greatwall's mock RWA tokens of deployments/${network}-testnet-rwa.json.`,
      `    conversion:`,
      `      address: "${d.conversion}"`,
      `      coins:`,
      `        - { token: native, symbol: ETH, decimals: 18, pay_sats: 300000, ask_sats: 300000 }`,
      ``,
    ].join("\n");
    const at = yml.indexOf("\n  - name: solana-devnet\n");
    if (at < 0) throw new Error("node.testnet.yml has no solana-devnet network to add before");
    yml = yml.slice(0, at + 1) + entry + "\n" + yml.slice(at + 1);
    changes.push(`${n.name}: added (light client ${d.lightClient}, protocol ${d.protocol}, Conversion ${d.conversion})`);
  }

  // Its tokens.
  const rwaFile = join(DEPLOYMENTS, `${network}-testnet-rwa.json`);
  if (!existsSync(rwaFile)) continue;
  const rwa = JSON.parse(readFileSync(rwaFile, "utf8"));
  for (const [symbol, t] of Object.entries(rwa.tokens)) {
    const [start, end] = block(n.name);
    const section = yml.slice(start, end);
    if (section.toLowerCase().includes(t.address.toLowerCase())) continue;
    const price = PRICES[symbol];
    if (!price) {
      console.log(`${network} ${symbol}: no price in PRICES, not added`);
      continue;
    }
    // After the last coin line of the network.
    const lines = [...section.matchAll(/^        - \{ token: .*\}\n/gm)];
    if (!lines.length) throw new Error(`${n.name}: no coins list to add ${symbol} to`);
    const last = lines[lines.length - 1];
    const at = start + last.index + last[0].length;
    const line = `        - { token: "${t.address}", symbol: ${symbol}, decimals: ${t.decimals}, pay_sats: ${price}, ask_sats: ${price} }\n`;
    yml = yml.slice(0, at) + line + yml.slice(at);
    changes.push(`${n.name}: ${symbol} at ${t.address}, ${price} sats`);
  }
}

if (!changes.length) console.log("node.testnet.yml is up to date");
for (const c of changes) console.log((dry ? "would add " : "added ") + c);
if (changes.length && !dry) writeFileSync(SETTINGS, yml);
