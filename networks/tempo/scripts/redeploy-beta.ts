// Redeploys BetaBaskets alone on Tempo's testnet: Tempo's counterpart of
// ../../../evm/scripts/redeploy-beta.ts, which can't send Tempo's
// transactions (their own type: viem's Tempo support sends them here, in
// the ordinary nonce lane, fees in PathUSD). Tempo's BetaBaskets was still
// the first build (bc09e28), from before a basket's parts could be the
// vault's receipts (E5, docs/specs/ipow-beta-app.md); the SDK and apps call
// the current one. It is a new contract, not an upgrade: the old one keeps
// its baskets under its own rules. BetaBaskets takes nothing at
// deployment. Run from networks/tempo after `npx hardhat compile` in evm:
//
//   node scripts/redeploy-beta.ts [--dry]
//   node scripts/redeploy-beta.ts --at <address>
//
// --at sends nothing: it reads back a BetaBaskets this script already sent
// (one whose read-back stopped) and writes the record.
//
// Where it lands is worked out from the ordinary nonce and checked empty
// first (a Tempo receipt's contractAddress is not trusted: see this
// package's README); the code there is compared with the build and its
// constants read back. Then it writes the address to
// ../../evm/deployments/tempo-testnet.json, keeping the old one as
// `previousBetaBaskets`, with `sources.BetaBaskets` = the commit it is built
// from. Then run verify-deployments.ts in evm and sdk's scripts/sync.ts.

import fs from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { createClient, http, Account } from "viem/tempo";
import { getContractAddress, keccak256 } from "viem";
import { readContract } from "viem/actions";

import { contractsSource } from "../../../evm/deploy/deploy.ts";
import { NETWORKS } from "../../../evm/deploy/networks.ts";

const here = dirname(fileURLToPath(import.meta.url));
const DRY = process.argv.includes("--dry");
const AT = process.argv.includes("--at") ? process.argv[process.argv.indexOf("--at") + 1] : undefined;
if (process.argv.includes("--at") && !/^0x[0-9a-fA-F]{40}$/.test(AT ?? "")) throw new Error("--at needs an address");
if (AT && DRY) throw new Error("--at writes the record; it has no dry run");

const source = contractsSource();
const s = NETWORKS.tempo;
const env = Object.fromEntries(
  fs.readFileSync(join(here, "..", ".env"), "utf8").split("\n").filter((l) => l.includes("=") && !l.startsWith("#")).map((l) => {
    const i = l.indexOf("=");
    return [l.slice(0, i).trim(), l.slice(i + 1).trim().replace(/^"|"$/g, "")];
  })
);
const account = Account.fromSecp256k1(env[s.keyEnv.testnet]);
const client = createClient({ account, testnet: true, transport: http(env[s.rpcEnv.testnet]) });
const PATHUSD = s.coin.kind === "token" ? s.coin.token.testnet! : "";
if (!PATHUSD) throw new Error("Tempo's settings name no fee token");

const file = join(here, "..", "..", "..", "evm", "deployments", "tempo-testnet.json");
const d = JSON.parse(fs.readFileSync(file, "utf8"));
const artifact = JSON.parse(fs.readFileSync(join(here, "..", "..", "..", "evm", "artifacts", "contracts", "applications", "beta", "BetaBaskets.sol", "BetaBaskets.json"), "utf8"));

let expected: `0x${string}`;
if (AT) {
  expected = AT as `0x${string}`;
  console.log(`tempo: reading back ${expected} (sent before), in place of ${d.betaBaskets}`);
} else {
  const nonce = await client.getTransactionCount({ address: account.address });
  expected = getContractAddress({ from: account.address, opcode: "CREATE", nonce: BigInt(nonce) });
  const code = await client.getCode({ address: expected });
  if (code && code !== "0x") throw new Error(`the address of nonce ${nonce}, ${expected}, holds code already`);
  console.log(`tempo: old BetaBaskets ${d.betaBaskets}; deployer ${account.address}, ordinary nonce ${nonce}; the new one lands at ${expected}; built from ${source}`);
  console.log("  note: the old contract's baskets stay there, under its own rules");
  if (DRY) process.exit(0);

  const hash = await client.sendTransaction({ data: artifact.bytecode, nonce, nonceKey: 0n, feeToken: PATHUSD } as any);
  const receipt = await client.waitForTransactionReceipt({ hash });
  if (receipt.status !== "success") throw new Error(`the deployment reverted: ${hash}`);
  if (receipt.contractAddress?.toLowerCase() !== expected.toLowerCase()) {
    console.log(`  the receipt names ${receipt.contractAddress}; nonce ${nonce} gives ${expected}, whose code is read`);
  }
  console.log(`sent ${hash}`);
}

// Read back what is on chain, not what the receipt said: the build's
// runtime code, the constants, and that no name is left for a creation.
let onChain: string | undefined;
for (let i = 0; !(onChain = await client.getCode({ address: expected })) || onChain === "0x"; i++) {
  if (i === 30) throw new Error(`no code at ${expected} after a minute`);
  await new Promise((r) => setTimeout(r, 2_000));
}
if (keccak256(onChain as `0x${string}`) !== keccak256(artifact.deployedBytecode)) throw new Error(`the code at ${expected} is not the build's`);
const read = (functionName: string) => readContract(client, { address: expected, abi: artifact.abi, functionName });
const checks: [string, unknown][] = [
  ["MAX_PARTS", 8n],
  ["MAX_FEE_BPS", 100n],
  ["MAX_NAME", 64n],
  ["MAX_SYMBOL", 16n],
  ["MAX_URI", 2048n],
  ["creatingName", ""],
  ["creatingSymbol", ""],
];
for (const [name, want] of checks) {
  const got = await read(name);
  // viem reads a small integer type (MAX_FEE_BPS's uint16) as a number, a uint256 as a bigint.
  if (String(got) !== String(want)) throw new Error(`the new BetaBaskets' ${name} reads ${got}, not ${want}`);
}
console.log(`read back: the code is the build's, and the limits and an empty naming match, at ${expected}`);

if (d.betaBaskets.toLowerCase() === expected.toLowerCase()) throw new Error(`the record already names ${expected}`);
d.previousBetaBaskets = d.betaBaskets;
d.betaBaskets = expected;
d.sources = { ...(d.sources ?? {}), BetaBaskets: source };
fs.writeFileSync(file, JSON.stringify(d, null, 2) + "\n");
console.log(`wrote ${file}`);
