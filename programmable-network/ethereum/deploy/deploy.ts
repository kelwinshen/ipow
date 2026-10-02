// Deploys the protocol on one EVM network from its settings (networks.ts,
// D134): the light client, the protocol's build for the network's coin with
// the reader of a rollup's data price, Conversion, BETA, the vault's
// factories and one vault per pair. Each address is read back from the
// network before it is used: a receipt alone is not trusted (Tempo's can
// name the wrong address).

import { Contract, ContractFactory, FetchRequest, JsonRpcProvider, VoidSigner, ZeroAddress, getAddress, getCreateAddress, type Signer } from "ethers";
import { execSync } from "node:child_process";
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
  /** Fields set on every transaction, such as a legacy gas price where a
   *  network's fee estimate is wrong (Hedera's relay). */
  overrides?: Record<string, unknown>;
  /** Called after each contract is deployed and read back. */
  log?: (line: string) => void;
  /** Sends a contract's deployment where ethers cannot (Tempo's own
   *  transactions), returning the address it was created at, worked out
   *  by the sender; `signer` then only reads. */
  send?: (name: string, abi: unknown, bytecode: string, args: unknown[]) => Promise<string>;
  /** With `send`: sends a call to a contract, and gives the address the
   *  deployer's contract `offset` deployments on will be created at. */
  call?: (to: string, abi: unknown, fn: string, args: unknown[]) => Promise<void>;
  nextCreate?: (offset: number) => Promise<string>;
};

/** The commit the contracts are built from; a deployment is only made
 *  from committed contracts, so that its record names their source. */
export function contractsSource(): string {
  const root = join(dirname(fileURLToPath(import.meta.url)), "..");
  const dirty = execSync("git status --porcelain contracts hardhat.config.ts", { cwd: root }).toString().trim();
  if (dirty) throw new Error(`the contracts have uncommitted changes:\n${dirty}`);
  // The artifacts the deployment compares with are built from that tree.
  execSync("npx hardhat compile --quiet", { cwd: root, stdio: "ignore" });
  return execSync("git log -1 --format=%H -- contracts hardhat.config.ts", { cwd: root }).toString().trim();
}

/** A Solana account in base58 as 32 bytes; anything else is refused. */
export function solanaAccountBytes(s: string): string {
  const A = "123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz";
  let n = 0n;
  for (const c of s) {
    const d = A.indexOf(c);
    if (d < 0) throw new Error(`not base58: ${s}`);
    n = n * 58n + BigInt(d);
  }
  const zeros = s.length - s.replace(/^1+/, "").length;
  const hex = n === 0n ? "" : n.toString(16).padStart(Math.ceil(n.toString(16).length / 2) * 2, "0");
  if (zeros + hex.length / 2 !== 32) throw new Error(`not a 32-byte account: ${s}`);
  return "0x" + hex.padStart(64, "0");
}

/** A signer that only reads, for a deployment sent by `send`. */
export function readOnlySigner(url: string, address: string): Signer {
  const request = new FetchRequest(url);
  request.setHeader("user-agent", "ipow-deploy");
  return new VoidSigner(address, new JsonRpcProvider(request, undefined, { staticNetwork: true }));
}

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
export function refusal(name: string, env: Env, local = false, ownSender = false): string | null {
  const s: NetworkSettings | undefined = NETWORKS[name];
  if (!s) return `unknown network ${name}`;
  if (s.blocked) return `${name}: ${s.blocked}`;
  if (!s.vault[env]) return `${name} ${env}: the vault's amounts are not set`;
  if (!local && s.chainId[env] === null) return `${name} ${env}: the chain id is not set`;
  if (!local && s.coin.kind === "token" && !s.coin.token[env]) return `${name} ${env}: the coin's address is not set`;
  if (!local && !ownSender && s.sender === "viem-tempo") return `${name}: its transactions need a Tempo sender (viem), which this does not have yet`;
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
  const why = refusal(o.network, o.env, !!o.local, !!o.send);
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
    let address: string;
    let c: any;
    if (o.send) {
      address = getAddress(await o.send(name, a.abi, a.bytecode, args));
      c = new Contract(address, a.abi, signer);
    } else {
      c = await new ContractFactory(a.abi, a.bytecode, signer).deploy(...args, { ...(o.overrides ?? {}) });
      await c.waitForDeployment();
      address = getAddress(await c.getAddress());
    }
    const code = await settle(() => provider.getCode(address), (c) => codeMatches(c, a.deployedBytecode, a.immutableReferences));
    if (!codeMatches(code, a.deployedBytecode, a.immutableReferences)) {
      throw new Error(`${name}: the code at ${address} is not the artifact's`);
    }
    o.log?.(`${name} ${address}`);
    return { address, contract: c as any };
  }

  const lightClient = await deploy("protocol/iPoWLightClient.sol", "iPoWLightClient", [o.minHeight]);
  await expectEqual("light client's lowest height", async () => await lightClient.contract.minHeight(), o.minHeight);

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
  if (s.scaledBuild) await expectEqual("protocol's work scale", async () => await protocol.contract.WORK_SCALE(), s.scaledBuild.workScale);
  await expectEqual("protocol's light client", async () => await protocol.contract.lightClient(), lightClient.address);
  if (s.coin.kind === "native") {
    await expectEqual("protocol's data fee", async () => await protocol.contract.dataFee(), dataFee?.address ?? ZeroAddress);
  }
  else {
    await expectEqual("protocol's coin", async () => await protocol.contract.coin(), coin);
    await expectEqual("protocol's price scale", async () => await protocol.contract.priceScale(), (s.coin as { priceScale: bigint }).priceScale);
  }

  const conversion = await deploy("apps/Conversion.sol", "Conversion", [protocol.address, o.maxSats, coin]);
  await expectEqual("Conversion's protocol", async () => await conversion.contract.protocol(), protocol.address);
  await expectEqual("Conversion's largest swap", async () => await conversion.contract.maxSats(), o.maxSats);
  await expectEqual("Conversion's coin", async () => await conversion.contract.coin(), coin);
  const betaBaskets = await deploy("apps/BetaBaskets.sol", "BetaBaskets", []);
  // A token coin's decimals are read from the token; this is the native's.
  const nativeDecimals = s.coin.kind === "native" ? s.coin.decimals : 18;
  const homeFactory = await deploy("protocol/vault/VaultFactories.sol", "VaultHomeFactory", [nativeDecimals]);
  await expectEqual("home factory's native decimals", async () => await homeFactory.contract.nativeDecimals(), nativeDecimals);
  const receiptsFactory = await deploy("protocol/vault/VaultFactories.sol", "VaultReceiptsFactory", []);

  /** A call that changes state, through the network's own sender if it has one. */
  async function call(to: string, abi: unknown, fn: string, args: unknown[]) {
    if (o.send) {
      if (!o.call) throw new Error("a sender needs its call too");
      return o.call(to, abi, fn, args);
    }
    await (await (new Contract(to, abi as any, signer) as any)[fn](...args, { ...(o.overrides ?? {}) })).wait();
  }
  /** Where the deployer's contract `offset` transactions on will be created. */
  async function nextCreate(offset: number): Promise<string> {
    if (o.send) {
      if (!o.nextCreate) throw new Error("a sender needs its nextCreate too");
      return getAddress(await o.nextCreate(offset));
    }
    const from = await signer.getAddress();
    return getCreateAddress({ from, nonce: (await provider.getTransactionCount(from, "pending")) + offset });
  }

  const vaults: Deployment["vaults"] = [];
  for (const pair of o.pairs) {
    // The parts first, each in its own transaction, made by the factories
    // for the vault deployed right after them: no transaction creates more
    // than one contract, as Tempo's limit of 50 million gas asks.
    const core = await nextCreate(2);
    const madeHome = getAddress(await homeFactory.contract.make.staticCall(core, s.number, pair.peer, coin));
    await call(homeFactory.address, homeFactory.contract.interface.fragments, "make", [core, s.number, pair.peer, coin]);
    // Anyone may make a part, so the one made is read back: the address a
    // simulation gave before the transaction could be another caller's.
    await expectEqual("home part made", async () => await homeFactory.contract.made(madeHome), true);
    const homePart = new Contract(madeHome, artifact("protocol/vault/VaultHome.sol", "VaultHome").abi, signer);
    await expectEqual("home part's vault and pair", async () => `${await homePart.core()},${await homePart.here()},${await homePart.peer()},${(await homePart.getAsset(0)).token}`, `${core},${s.number},${pair.peer},${coin}`);
    const madeReceipts = getAddress(await receiptsFactory.contract.make.staticCall(core, s.number, pair.peer));
    await call(receiptsFactory.address, receiptsFactory.contract.interface.fragments, "make", [core, s.number, pair.peer]);
    await expectEqual("receipts part made", async () => await receiptsFactory.contract.made(madeReceipts), true);
    const receiptsPart = new Contract(madeReceipts, artifact("protocol/vault/VaultReceipts.sol", "VaultReceipts").abi, signer);
    await expectEqual("receipts part's vault and pair", async () => `${await receiptsPart.core()},${await receiptsPart.here()},${await receiptsPart.peer()}`, `${core},${s.number},${pair.peer}`);
    o.log?.(`VaultHome ${madeHome}`);
    o.log?.(`VaultReceipts ${madeReceipts}`);
    const common = [protocol.address, s.number, pair.peer, pair.peerVault, amounts.deposit, amounts.minCertifyingEscrow, homeFactory.address, receiptsFactory.address, madeHome, madeReceipts];
    const vault =
      s.coin.kind === "native"
        ? await deploy("protocol/iPoWVault.sol", "iPoWVaultNative", common)
        : await deploy("protocol/iPoWVaultToken.sol", "iPoWVaultToken", [...common, coin]);
    if (vault.address !== core) throw new Error(`the vault is at ${vault.address}, its parts were made for ${core}`);
    const v = vault.contract;
    await expectEqual("vault's networks", async () => `${await v.here()},${await v.peer()}`, `${s.number},${pair.peer}`);
    await expectEqual("vault's peer vault", async () => (await v.peerVault()).toLowerCase(), pair.peerVault.toLowerCase());
    await expectEqual("vault's protocol", async () => await v.protocol(), protocol.address);
    await expectEqual("vault's amounts", async () => `${await v.deposit()},${await v.minCertifyingEscrow()}`, `${amounts.deposit},${amounts.minCertifyingEscrow}`);
    await expectEqual("vault's factories", async () => `${await v.homeFactory()},${await v.receiptsFactory()}`, `${homeFactory.address},${receiptsFactory.address}`);
    await expectEqual("vault's parts", async () => `${getAddress(await v.home())},${getAddress(await v.receipts())}`, `${madeHome},${madeReceipts}`);
    const home = madeHome;
    const receipts = madeReceipts;
    for (const [part, file, name] of [[home, "protocol/vault/VaultHome.sol", "VaultHome"], [receipts, "protocol/vault/VaultReceipts.sol", "VaultReceipts"]]) {
      const a = artifact(file, name);
      const code = await settle(() => provider.getCode(part), (c) => codeMatches(c, a.deployedBytecode, a.immutableReferences));
      if (!codeMatches(code, a.deployedBytecode, a.immutableReferences)) {
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

/** How long a read-back waits for an endpoint that has not yet seen the
 *  latest block: several nodes behind one endpoint can answer a read from
 *  before the deployment (seen on Base Sepolia). */
const SETTLE_TRIES = 15;
const SETTLE_MS = 2_000;

async function settle<T>(read: () => Promise<T>, ok: (v: T) => boolean): Promise<T> {
  let v!: T;
  let read_ = false;
  let error: unknown;
  for (let i = 0; i < SETTLE_TRIES; i++) {
    try {
      v = await read();
      read_ = true;
      if (ok(v)) return v;
    } catch (e) {
      // An endpoint behind the deployment may not know the contract yet.
      error = e;
    }
    await new Promise((r) => setTimeout(r, SETTLE_MS));
  }
  if (!read_) throw error;
  return v;
}

async function expectEqual(what: string, read: () => Promise<unknown>, want: unknown) {
  const norm = (x: unknown) => String(x).toLowerCase();
  const got = await settle(read, (v) => norm(v) === norm(want));
  if (norm(got) !== norm(want)) throw new Error(`${what}: read ${got}, expected ${want}`);
}
