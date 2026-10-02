// Deploys the protocol on one EVM network from its settings (networks.ts,
// D134): the light client, the protocol's build for the network's coin with
// the reader of a rollup's data price, Conversion, BETA, the vault's
// factories and one vault per pair. Each address is read back from the
// network before it is used: a receipt alone is not trusted (Tempo's can
// name the wrong address).

import { ContractFactory, ZeroAddress, getAddress, type Signer } from "ethers";
import { readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

import { NETWORKS, type Env, type NetworkSettings } from "./networks.ts";

const SOLANA = 2;

const ARTIFACTS = join(dirname(fileURLToPath(import.meta.url)), "..", "artifacts", "contracts");

function artifact(file: string, name: string) {
  return JSON.parse(readFileSync(join(ARTIFACTS, file, `${name}.json`), "utf8"));
}

export type Pair = {
  /** The peer's network number (D133). */
  peer: number;
  /** The peer's vault in 32 bytes: on Solana its pair's config account; on
   *  an EVM network its address, predicted when it is deployed second. */
  peerVault: string;
};

export type DeployOptions = {
  network: string;
  env: Env;
  /** The light client's lowest Bitcoin height. */
  minHeight: number;
  /** Conversion's largest swap, in satoshis. */
  maxSats: bigint;
  pairs: Pair[];
  /** Only for a local test chain (31337): skips the chain id check and
   *  stands in for the coin's token. */
  local?: { coin?: string };
  /** The deployer's address already uses the network's big blocks, where
   *  the settings say deploying needs them (HyperEVM). */
  bigBlocks?: boolean;
};

export type Deployment = {
  network: string;
  env: Env;
  chainId: number;
  lightClient: string;
  dataFee: string | null;
  protocol: string;
  conversion: string;
  betaBaskets: string;
  homeFactory: string;
  receiptsFactory: string;
  vaults: { peer: number; peerVault: string; vault: string; home: string; receipts: string }[];
};

/** What stops a deployment of `name` in `env`, if anything. */
export function refusal(name: string, env: Env, local = false): string | null {
  const s: NetworkSettings | undefined = NETWORKS[name];
  if (!s) return `unknown network ${name}`;
  if (s.blocked) return `${name}: ${s.blocked}`;
  if (!s.vault[env]) return `${name} ${env}: the vault's amounts are not set`;
  if (!local && s.chainId[env] === null) return `${name} ${env}: the chain id is not set`;
  if (!local && s.coin.kind === "token" && !s.coin.token[env]) return `${name} ${env}: the coin's address is not set`;
  if (!local && s.sender === "viem-tempo") return `${name}: its transactions need a Tempo sender (viem), which this does not have yet`;
  return settingsError(s);
}

/** Settings that no build serves. */
export function settingsError(s: NetworkSettings): string | null {
  if (s.coin.kind === "native" && (s.coin.price === "gasPrice" || s.scaledBuild) && s.dataFee !== "none") {
    return "the gas-price and scaled builds read no price of data";
  }
  if (s.scaledBuild && (s.coin.kind !== "native" || s.coin.price !== "baseFee")) {
    return "a scaled build is only for a native coin priced by the base fee";
  }
  return null;
}

/** Whether code read from the network is the artifact's, apart from the
 *  values of its immutables, which only the deployment fills in. */
export function codeMatches(onChain: string, deployedBytecode: string, immutableReferences: Record<string, { start: number; length: number }[]>): boolean {
  if (onChain.length !== deployedBytecode.length) return false;
  const chars = onChain.toLowerCase().split("");
  for (const refs of Object.values(immutableReferences ?? {})) {
    for (const { start, length } of refs) {
      for (let i = 2 + start * 2; i < 2 + (start + length) * 2; i++) chars[i] = "0";
    }
  }
  return chars.join("") === deployedBytecode.toLowerCase();
}

export async function deployNetwork(signer: Signer, o: DeployOptions): Promise<Deployment> {
  const why = refusal(o.network, o.env, !!o.local);
  if (why) throw new Error(why);
  const s = NETWORKS[o.network];
  const provider = signer.provider!;
  const chainId = Number((await provider.getNetwork()).chainId);
  if (o.local && chainId !== 31337) throw new Error(`a local deployment on chain ${chainId}, not a local test chain`);
  if (!o.local && chainId !== s.chainId[o.env]) {
    throw new Error(`${o.network} ${o.env}: connected to chain ${chainId}, expected ${s.chainId[o.env]}`);
  }
  if (!o.local && s.bigBlocks && !o.bigBlocks) {
    throw new Error(`${o.network}: deploying needs big blocks; switch the deployer's address to them first`);
  }
  const coin = s.coin.kind === "token" ? (o.local ? o.local.coin : s.coin.token[o.env]) : ZeroAddress;
  if (!coin) throw new Error(`${o.network} ${o.env}: no coin`);
  for (const pair of o.pairs) {
    if (pair.peer < 1 || pair.peer > 8 || pair.peer === s.number) throw new Error(`peer network ${pair.peer}`);
    // An EVM peer's vault is an address: 20 bytes in 32 (section 11.3).
    if (pair.peer !== SOLANA && (!/^0x0{24}[0-9a-fA-F]{40}$/.test(pair.peerVault) || /^0x0{64}$/.test(pair.peerVault))) {
      throw new Error(`peer ${pair.peer}: its vault must be an address in 32 bytes`);
    }
  }
  const amounts = s.vault[o.env]!;

  /** Deploys, then reads the code at the address from the network. */
  async function deploy(file: string, name: string, args: unknown[]) {
    const a = artifact(file, name);
    const c = await new ContractFactory(a.abi, a.bytecode, signer).deploy(...args);
    await c.waitForDeployment();
    const address = getAddress(await c.getAddress());
    const code = await provider.getCode(address);
    if (!codeMatches(code, a.deployedBytecode, a.immutableReferences)) {
      throw new Error(`${name}: the code at ${address} is not the artifact's`);
    }
    return { address, contract: c as any };
  }

  const lightClient = await deploy("protocol/iPoWLightClient.sol", "iPoWLightClient", [o.minHeight]);
  await expectEqual("light client's lowest height", await lightClient.contract.minHeight(), o.minHeight);

  let dataFee: { address: string } | null = null;
  if (s.dataFee === "op") dataFee = await deploy("protocol/DataFee.sol", "OpDataFee", []);
  if (s.dataFee === "arb") dataFee = await deploy("protocol/DataFee.sol", "ArbDataFee", []);

  const protocol =
    s.coin.kind === "token"
      ? await deploy("protocol/iPoWProtocolToken.sol", "iPoWProtocolToken", [lightClient.address, coin, s.coin.priceScale])
      : s.coin.price === "gasPrice"
        ? await deploy("protocol/iPoWProtocol.sol", "iPoWProtocolGasPrice", [lightClient.address])
        : s.scaledBuild
          ? await deploy("protocol/iPoWProtocol.sol", s.scaledBuild.name, [lightClient.address])
          : await deploy("protocol/iPoWProtocol.sol", "iPoWProtocolNative", [lightClient.address, dataFee?.address ?? ZeroAddress]);
  if (s.scaledBuild) await expectEqual("protocol's work scale", await protocol.contract.WORK_SCALE(), s.scaledBuild.workScale);
  await expectEqual("protocol's light client", await protocol.contract.lightClient(), lightClient.address);
  if (s.coin.kind === "native") {
    await expectEqual("protocol's data fee", await protocol.contract.dataFee(), dataFee?.address ?? ZeroAddress);
  }
  else {
    await expectEqual("protocol's coin", await protocol.contract.coin(), coin);
    await expectEqual("protocol's price scale", await protocol.contract.priceScale(), (s.coin as { priceScale: bigint }).priceScale);
  }

  const conversion = await deploy("apps/Conversion.sol", "Conversion", [protocol.address, o.maxSats, coin]);
  await expectEqual("Conversion's protocol", await conversion.contract.protocol(), protocol.address);
  await expectEqual("Conversion's largest swap", await conversion.contract.maxSats(), o.maxSats);
  await expectEqual("Conversion's coin", await conversion.contract.coin(), coin);
  const betaBaskets = await deploy("apps/BetaBaskets.sol", "BetaBaskets", []);
  // A token coin's decimals are read from the token; this is the native's.
  const nativeDecimals = s.coin.kind === "native" ? s.coin.decimals : 18;
  const homeFactory = await deploy("protocol/vault/VaultFactories.sol", "VaultHomeFactory", [nativeDecimals]);
  await expectEqual("home factory's native decimals", await homeFactory.contract.nativeDecimals(), nativeDecimals);
  const receiptsFactory = await deploy("protocol/vault/VaultFactories.sol", "VaultReceiptsFactory", []);

  const vaults: Deployment["vaults"] = [];
  for (const pair of o.pairs) {
    const common = [protocol.address, s.number, pair.peer, pair.peerVault, amounts.deposit, amounts.minCertifyingEscrow, homeFactory.address, receiptsFactory.address];
    const vault =
      s.coin.kind === "native"
        ? await deploy("protocol/iPoWVault.sol", "iPoWVaultNative", common)
        : await deploy("protocol/iPoWVaultToken.sol", "iPoWVaultToken", [...common, coin]);
    const v = vault.contract;
    await expectEqual("vault's networks", `${await v.here()},${await v.peer()}`, `${s.number},${pair.peer}`);
    await expectEqual("vault's peer vault", (await v.peerVault()).toLowerCase(), pair.peerVault.toLowerCase());
    await expectEqual("vault's protocol", await v.protocol(), protocol.address);
    await expectEqual("vault's amounts", `${await v.deposit()},${await v.minCertifyingEscrow()}`, `${amounts.deposit},${amounts.minCertifyingEscrow}`);
    await expectEqual("vault's factories", `${await v.homeFactory()},${await v.receiptsFactory()}`, `${homeFactory.address},${receiptsFactory.address}`);
    const home = getAddress(await v.home());
    const receipts = getAddress(await v.receipts());
    for (const [part, file, name] of [[home, "protocol/vault/VaultHome.sol", "VaultHome"], [receipts, "protocol/vault/VaultReceipts.sol", "VaultReceipts"]]) {
      const a = artifact(file, name);
      if (!codeMatches(await provider.getCode(part), a.deployedBytecode, a.immutableReferences)) {
        throw new Error(`${name}: the code at ${part} is not the artifact's`);
      }
    }
    vaults.push({ peer: pair.peer, peerVault: pair.peerVault, vault: vault.address, home, receipts });
  }

  return {
    network: o.network,
    env: o.env,
    chainId,
    lightClient: lightClient.address,
    dataFee: dataFee?.address ?? null,
    protocol: protocol.address,
    conversion: conversion.address,
    betaBaskets: betaBaskets.address,
    homeFactory: homeFactory.address,
    receiptsFactory: receiptsFactory.address,
    vaults,
  };
}

async function expectEqual(what: string, read: unknown, want: unknown) {
  const norm = (x: unknown) => String(x).toLowerCase();
  if (norm(read) !== norm(want)) throw new Error(`${what}: read ${read}, expected ${want}`);
}
