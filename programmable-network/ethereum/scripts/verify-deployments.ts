// Reads every test network's deployment back, independently of the run that
// made it (CLAUDE.md: verify live state, not receipts): each contract's
// code against the build, apart from its immutables, and how the contracts
// name each other. Read-only. Run from programmable-network/ethereum:
//
//   node scripts/verify-deployments.ts

import { Contract, FetchRequest, JsonRpcProvider, getAddress } from "ethers";
import { existsSync, readFileSync, readdirSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

import { codeMatches } from "../deploy/deploy.ts";
import { NETWORKS } from "../deploy/networks.ts";

const here = dirname(fileURLToPath(import.meta.url));
const artifact = (file: string, name: string) => JSON.parse(readFileSync(join(here, "..", "artifacts", "contracts", file, `${name}.json`), "utf8"));
/** A contract's code as built from the commit a deployment was made from,
 *  where it differs from today's: kept in deployments/source-<commit>/. */
function builtFrom(source: string | undefined, file: string, name: string) {
  const kept = join(here, "..", "deployments", `source-${source}`, `${name}.json`);
  return source && existsSync(kept) ? JSON.parse(readFileSync(kept, "utf8")) : artifact(file, name);
}
function readEnv(network: string): Record<string, string> {
  const out: Record<string, string> = {};
  for (const line of readFileSync(join(here, "..", "..", network, ".env"), "utf8").split("\n")) {
    const m = line.match(/^\s*([A-Z0-9_]+)\s*=\s*"?([^"\n]*)"?/);
    if (m) out[m[1]] = m[2].trim();
  }
  return out;
}

let failures = 0;
for (const file of readdirSync(join(here, "..", "deployments")).filter((f) => f.endsWith("-testnet.json"))) {
  const d = JSON.parse(readFileSync(join(here, "..", "deployments", file), "utf8"));
  const s = NETWORKS[d.network];
  const request = new FetchRequest(readEnv(d.network)[s.rpcEnv.testnet]);
  request.setHeader("user-agent", "ipow-verify");
  const p = new JsonRpcProvider(request, undefined, { staticNetwork: true });
  const problems: string[] = [];
  const chainId = Number(await p.send("eth_chainId", []));
  if (chainId !== d.chainId) problems.push(`chain ${chainId}`);

  const protocolName =
    s.coin.kind === "token" ? ["protocol/iPoWProtocolToken.sol", "iPoWProtocolToken"]
    : s.coin.price === "gasPrice" ? ["protocol/iPoWProtocol.sol", "iPoWProtocolGasPrice"]
    : s.scaledBuild ? ["protocol/iPoWProtocol.sol", s.scaledBuild.name]
    : ["protocol/iPoWProtocol.sol", "iPoWProtocolNative"];
  const vaultName = s.coin.kind === "token" ? ["protocol/iPoWVaultToken.sol", "iPoWVaultToken"] : ["protocol/iPoWVault.sol", "iPoWVaultNative"];
  const parts: [string, string, string][] = [
    [d.lightClient, "protocol/iPoWLightClient.sol", "iPoWLightClient"],
    [d.protocol, protocolName[0], protocolName[1]],
    [d.conversion, "apps/Conversion.sol", "Conversion"],
    [d.betaBaskets, "apps/BetaBaskets.sol", "BetaBaskets"],
    [d.homeFactory, "protocol/vault/VaultFactories.sol", "VaultHomeFactory"],
    [d.receiptsFactory, "protocol/vault/VaultFactories.sol", "VaultReceiptsFactory"],
  ];
  for (const v of d.vaults) {
    parts.push([v.vault, vaultName[0], vaultName[1]], [v.home, "protocol/vault/VaultHome.sol", "VaultHome"], [v.receipts, "protocol/vault/VaultReceipts.sol", "VaultReceipts"]);
  }
  if (d.dataFee) parts.push([d.dataFee, "protocol/DataFee.sol", s.dataFee === "op" ? "OpDataFee" : "ArbDataFee"]);
  for (const [address, f, name] of parts) {
    // A contract redeployed alone names its own source (redeploy-conversion.ts).
    const a = builtFrom(d.sources?.[name] ?? d.source, f, name);
    if (!codeMatches(await p.getCode(address), a.deployedBytecode, a.immutableReferences)) problems.push(`${name} at ${address}: code is not the build's`);
  }

  const at = (address: string, f: string, name: string) => new Contract(address, artifact(f, name).abi, p);
  const protocol = at(d.protocol, protocolName[0], protocolName[1]);
  const conversion = at(d.conversion, "apps/Conversion.sol", "Conversion");
  const coin = s.coin.kind === "token" ? s.coin.token.testnet : "0x0000000000000000000000000000000000000000";
  const checks: [string, unknown, unknown][] = [
    ["protocol's light client", await protocol.lightClient(), d.lightClient],
    ["light client's lowest height", await at(d.lightClient, "protocol/iPoWLightClient.sol", "iPoWLightClient").minHeight(), d.minHeight],
    ["Conversion's protocol", await conversion.protocol(), d.protocol],
    ["Conversion's largest swap", await conversion.maxSats(), d.maxSats],
    ["Conversion's coin", await conversion.coin(), coin],
    ["home factory's native decimals", await at(d.homeFactory, "protocol/vault/VaultFactories.sol", "VaultHomeFactory").nativeDecimals(), s.coin.kind === "native" ? s.coin.decimals : 18],
  ];
  if (s.coin.kind === "native") checks.push(["protocol's data fee", await protocol.dataFee(), d.dataFee ?? "0x0000000000000000000000000000000000000000"]);
  if (s.scaledBuild) checks.push(["protocol's work scale", await protocol.WORK_SCALE(), s.scaledBuild.workScale]);
  // Parts made before their vault (since 2026-10-03): the factories
  // recorded making them.
  const homeFactory = at(d.homeFactory, "protocol/vault/VaultFactories.sol", "VaultHomeFactory");
  const receiptsFactory = at(d.receiptsFactory, "protocol/vault/VaultFactories.sol", "VaultReceiptsFactory");
  for (const v of d.vaults) {
    if (d.vaultParts !== "made by the vault") {
      checks.push(["parts made by the factories", `${await homeFactory.made(v.home)},${await receiptsFactory.made(v.receipts)}`, "true,true"]);
    }
    const vault = at(v.vault, vaultName[0], vaultName[1]);
    checks.push(
      ["vault's protocol", await vault.protocol(), d.protocol],
      ["vault's networks", `${await vault.here()},${await vault.peer()}`, `${s.number},${v.peer}`],
      ["vault's peer vault", (await vault.peerVault()).toLowerCase(), v.peerVault.toLowerCase()],
      ["vault's parts", `${getAddress(await vault.home())},${getAddress(await vault.receipts())}`, `${v.home},${v.receipts}`],
      ["vault's factories", `${await vault.homeFactory()},${await vault.receiptsFactory()}`, `${d.homeFactory},${d.receiptsFactory}`],
      ["vault's amounts", `${await vault.deposit()},${await vault.minCertifyingEscrow()}`, `${s.vault.testnet!.deposit},${s.vault.testnet!.minCertifyingEscrow}`],
    );
  }
  for (const [what, got, want] of checks) if (String(got).toLowerCase() !== String(want).toLowerCase()) problems.push(`${what}: ${got}, expected ${want}`);

  failures += problems.length;
  console.log(`${d.network.padEnd(12)} chain ${chainId}  ${parts.length} contracts, built from ${d.source ?? "?"}  ${problems.length ? "PROBLEMS: " + problems.join("; ") : "all match"}`);
}
if (failures) process.exit(1);
