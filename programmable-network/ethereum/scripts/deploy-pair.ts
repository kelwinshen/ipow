// Adds a vault pair between two EVM test networks that already run the
// protocol (D132): one vault on each, each naming the other's. The two
// vaults name each other at deployment, so the second's address is
// predicted from its deployer's nonce before the first is deployed (spec
// Build status, row 7), and read back once it is.
//
// Each side gets factories of today's build first: the live factories were
// deployed from a9493c7, before the parts were made ahead of their vault,
// and today's vault asks its factories whether they `made` its parts, which
// those cannot answer. The pair's vault records its own factories, and
// verify-deployments.ts checks it against them and against today's build
// (`source: "current"`; the next change to the vault needs a
// `deployments/source-<commit>/` snapshot for this pair, as a9493c7 has).
//
// The keys and endpoints are the network packages'
// (programmable-network/<network>/.env); the same deployer on both. Run from
// programmable-network/ethereum after `npx hardhat compile`:
//
//   node scripts/deploy-pair.ts <networkA> <networkB> [--big-blocks] [--dry] [--genesis-days N]
//
// With --genesis-days, both vaults' receipts start in genesis for the
// deployer, ending N days from now at the latest
// (docs/drafts/ipow-vault-genesis.md), and the pair's earlier vaults are
// replaced in the records rather than added to.
//
// Order: B's factories; B's vault address predicted from one settled nonce
// and both of B's parts simulated; A's factories, parts and vault naming
// the predicted B; B's nonce checked unmoved; B's parts and vault naming
// A's. Both records are written only when both vaults exist. Send nothing
// else from the deployer on B meanwhile: its nonce fixes B's address.

import { Contract, ContractFactory, FetchRequest, JsonRpcProvider, Wallet, ZeroAddress, formatEther, getAddress, getCreateAddress, parseEther, zeroPadValue } from "ethers";
import { readFileSync, writeFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

import { codeMatches } from "../deploy/deploy.ts";
import { RetryProvider } from "../deploy/retry-provider.ts";
import { NETWORKS, type NetworkSettings } from "../deploy/networks.ts";

const here = dirname(fileURLToPath(import.meta.url));
const args = process.argv.slice(2);
const [nameA, nameB] = args.filter((a, i) => !a.startsWith("--") && args[i - 1] !== "--genesis-days");
const dry = args.includes("--dry");
const bigBlocks = args.includes("--big-blocks");
const genesisAt = args.indexOf("--genesis-days");
const genesisDays = genesisAt >= 0 ? Number(args[genesisAt + 1]) : 0;
if (genesisAt >= 0 && !(genesisDays > 0 && genesisDays <= 30)) throw new Error("--genesis-days: 1 to 30");
/** Genesis for the deployer, ending this many days from now (the same end on both sides). */
const genesisEnd = genesisDays ? BigInt(Math.floor(Date.now() / 1000) + genesisDays * 86_400) : 0n;
if (!nameA || !nameB || nameA === nameB) throw new Error("usage: deploy-pair.ts <networkA> <networkB>");
/** The least a side should hold: the parts and the vault take about 12M gas. */
const MIN_BALANCE = parseEther("0.01");
const WAIT_MS = 180_000;
/** A network's own endpoint, used on big blocks. */
const OWN_RPC: Record<string, string> = { hyperliquid: "https://rpc.hyperliquid-testnet.xyz/evm" };

const artifact = (file: string, name: string) => JSON.parse(readFileSync(join(here, "..", "artifacts", "contracts", file, `${name}.json`), "utf8"));
const FACTORIES = "protocol/vault/VaultFactories.sol";
const VAULT = "protocol/iPoWVault.sol";

type Side = { name: string; s: NetworkSettings; provider: JsonRpcProvider; wallet: Wallet; file: string; d: any; coin: string };

async function side(name: string): Promise<Side> {
  const s = NETWORKS[name];
  if (!s || !s.rpcEnv.testnet) throw new Error(`${name}: a network from deploy/networks.ts that has a test network`);
  if (s.sender) throw new Error(`${name} deploys through its own sender (${s.sender}); not built here`);
  if (s.coin.kind !== "native") throw new Error(`${name}: a token-coin network's vault is not built here`);
  if (s.bigBlocks && !dry && !bigBlocks) throw new Error(`${name}: deploying needs big blocks; switch the deployer's address to them, then pass --big-blocks`);
  const vars: Record<string, string> = {};
  for (const line of readFileSync(join(here, "..", "..", name, ".env"), "utf8").split("\n")) {
    const m = line.match(/^\s*([A-Z0-9_]+)\s*=\s*"?([^"\n]*)"?/);
    if (m) vars[m[1]] = m[2].trim();
  }
  // On big blocks, the network's own endpoint: its gas estimate knows the
  // deployer's big blocks, where Alchemy's HyperEVM estimates against small
  // ones (3M gas) and refuses the parts (2026-10-08).
  const provider = new RetryProvider(s.bigBlocks && bigBlocks && OWN_RPC[name] ? OWN_RPC[name] : vars[s.rpcEnv.testnet], "ipow-deploy-pair");
  const key = vars[s.keyEnv.testnet];
  const wallet = new Wallet(key.startsWith("0x") ? key : "0x" + key, provider);
  const chainId = Number((await provider.getNetwork()).chainId);
  if (chainId !== s.chainId.testnet) throw new Error(`${name}: chain ${chainId} is not its test network (${s.chainId.testnet})`);
  const file = join(here, "..", "deployments", `${name}-testnet.json`);
  const d = JSON.parse(readFileSync(file, "utf8"));
  const balance = await provider.getBalance(wallet.address);
  if (!dry && balance < MIN_BALANCE) throw new Error(`${name}: the deployer holds ${formatEther(balance)} of the coin; at least ${formatEther(MIN_BALANCE)} is needed`);
  return { name, s, provider, wallet, file, d, coin: ZeroAddress };
}

const A = await side(nameA);
const B = await side(nameB);
if (A.wallet.address !== B.wallet.address) throw new Error("the two networks' deployers differ; this script predicts the second vault from one deployer's nonce");
for (const x of [A, B]) {
  const other = x === A ? B : A;
  const existing = x.d.vaults.find((v: any) => v.peer === other.s.number);
  // A genesis redeploy replaces a pair's vault, but never one already in genesis.
  if (existing && (!genesisDays || existing.genesisEnd)) throw new Error(`${x.name} already has a ${existing.genesisEnd ? "genesis " : ""}vault paired with ${other.name}`);
}

/** Fees on a big-block network, capped at twice the big-block price now. */
const lastBigPrice = new Map<string, bigint>();
const fees = async (x: Side) => {
  if (!x.s.bigBlocks) return {};
  // HyperEVM's endpoint sometimes answers "method not found" for the price
  // between answers (2026-10-06): asked again, then the last price read.
  for (let i = 0; i < 5; i++) {
    try {
      lastBigPrice.set(x.name, BigInt(await x.provider.send("eth_bigBlockGasPrice", [])));
      break;
    } catch (e) {
      if (i === 4 && !lastBigPrice.has(x.name)) throw e;
      await new Promise((r) => setTimeout(r, 3_000));
    }
  }
  return { maxFeePerGas: lastBigPrice.get(x.name)! * 2n, maxPriorityFeePerGas: 0n };
};
const asBytes32 = (address: string) => zeroPadValue(getAddress(address), 32);
/** A transaction's gas: on an Arbitrum chain the estimate includes posting
 *  its data to Ethereum, whose price moves between the estimate and the
 *  block (a factory ran out of gas at exactly its estimate on Arbitrum
 *  Sepolia, 2026-10-06), so half again is added there. */
const gasFor = (x: Side, estimate: bigint) => (x.s.dataFee === "arb" ? { gasLimit: (estimate * 3n) / 2n } : {});

/** Reads until an endpoint behind the deployment agrees, as deploy.ts does. */
async function settle<T>(read: () => Promise<T>, ok: (v: T) => boolean): Promise<T> {
  let v!: T;
  let error: unknown;
  let read_ = false;
  for (let i = 0; i < 15; i++) {
    try {
      v = await read();
      read_ = true;
      if (ok(v)) return v;
    } catch (e) {
      error = e;
    }
    await new Promise((r) => setTimeout(r, 2_000));
  }
  if (!read_) throw error;
  return v;
}
async function expectEqual(what: string, read: () => Promise<unknown>, want: unknown) {
  const norm = (x: unknown) => String(x).toLowerCase();
  const got = await settle(read, (v) => norm(v) === norm(want));
  if (norm(got) !== norm(want)) throw new Error(`${what}: read ${got}, expected ${want}`);
}

/** The deployer's nonce on `x`, settled: the same from the pending and the
 *  latest view, three reads in a row, since an endpoint's pool can lag. */
async function settledNonce(x: Side): Promise<number> {
  let last = -1;
  let agreed = 0;
  for (let i = 0; i < 30; i++) {
    const [pending, latest] = await Promise.all([x.provider.getTransactionCount(x.wallet.address, "pending"), x.provider.getTransactionCount(x.wallet.address, "latest")]);
    if (pending === latest && pending === last) agreed++;
    else agreed = 0;
    last = pending === latest ? pending : -1;
    if (agreed >= 2) return pending;
    await new Promise((r) => setTimeout(r, 1_500));
  }
  throw new Error(`${x.name}: the deployer's nonce would not settle (a transaction of its own still pending?)`);
}
const createAt = (x: Side, nonce: number) => getCreateAddress({ from: x.wallet.address, nonce });

/** Deploys, then reads the code at the address back from the network. */
async function deploy(x: Side, file: string, name: string, args: unknown[]) {
  const a = artifact(file, name);
  const factory = new ContractFactory(a.abi, a.bytecode, x.wallet);
  const estimate = x.s.dataFee === "arb" ? await x.wallet.estimateGas(await factory.getDeployTransaction(...args)) : 0n;
  const c = await factory.deploy(...args, { ...(await fees(x)), ...gasFor(x, estimate) });
  await c.deploymentTransaction()!.wait(1, WAIT_MS);
  const address = getAddress(await c.getAddress());
  const code = await settle(() => x.provider.getCode(address), (cd) => codeMatches(cd, a.deployedBytecode, a.immutableReferences));
  if (!codeMatches(code, a.deployedBytecode, a.immutableReferences)) throw new Error(`${x.name}: the code at ${address} is not the build's ${name}`);
  console.log(`${x.name}: ${name} ${address}`);
  return { address, contract: new Contract(address, a.abi, x.wallet) };
}

/** Factories of today's build on `x`, for this pair's vault. */
async function deployFactories(x: Side) {
  const home = await deploy(x, FACTORIES, "VaultHomeFactory", [(x.s.coin as { decimals: number }).decimals]);
  await expectEqual(`${x.name}: home factory's native decimals`, async () => await home.contract.nativeDecimals(), (x.s.coin as { decimals: number }).decimals);
  const receipts = await deploy(x, FACTORIES, "VaultReceiptsFactory", []);
  return { home, receipts };
}

type Factories = Awaited<ReturnType<typeof deployFactories>>;

/** Simulates both parts of `x`'s vault at `core` for `peer`: what the real
 *  calls will make, refused before anything is spent elsewhere. */
async function simulateParts(x: Side, f: Factories, peer: Side, core: string) {
  const madeHome = getAddress(await f.home.contract.make.staticCall(core, x.s.number, peer.s.number, x.coin));
  const madeReceipts = getAddress(await f.receipts.contract[receiptsMake].staticCall(...receiptsArgs(x, peer, core)));
  return { madeHome, madeReceipts };
}

/** The receipts part's factory call: in genesis for the deployer, or with none. */
const receiptsMake = genesisDays ? "makeGenesis" : "make";
function receiptsArgs(x: Side, peer: Side, core: string): unknown[] {
  return genesisDays ? [core, x.s.number, peer.s.number, x.wallet.address, genesisEnd] : [core, x.s.number, peer.s.number];
}

/** Makes the vault of `x` paired with `peer`, its parts first, at `core`
 *  (the deployer's nonce must be `nonce` now); the vault names `peerVault`. */
async function deployVault(x: Side, f: Factories, peer: Side, core: string, nonce: number, peerVault: string) {
  const now = await settledNonce(x);
  if (now !== nonce) throw new Error(`${x.name}: the deployer's nonce is ${now}, not ${nonce}: the vault would not land at ${core}`);
  const amounts = x.s.vault.testnet!;
  const { madeHome, madeReceipts } = await simulateParts(x, f, peer, core);
  const homeGas = x.s.dataFee === "arb" ? await f.home.contract.make.estimateGas(core, x.s.number, peer.s.number, x.coin) : 0n;
  await (await f.home.contract.make(core, x.s.number, peer.s.number, x.coin, { ...(await fees(x)), ...gasFor(x, homeGas) })).wait(1, WAIT_MS);
  // Anyone may make a part, so the one made is read back: the address a
  // simulation gave before the transaction could be another caller's.
  await expectEqual(`${x.name}: home part made`, async () => await f.home.contract.made(madeHome), true);
  const homePart = new Contract(madeHome, artifact("protocol/vault/VaultHome.sol", "VaultHome").abi, x.provider);
  await expectEqual(`${x.name}: home part's vault and pair`, async () => `${await homePart.core()},${await homePart.here()},${await homePart.peer()},${(await homePart.getAsset(0)).token}`, `${core},${x.s.number},${peer.s.number},${x.coin}`);
  const receiptsGas = x.s.dataFee === "arb" ? await f.receipts.contract[receiptsMake].estimateGas(...receiptsArgs(x, peer, core)) : 0n;
  await (await f.receipts.contract[receiptsMake](...receiptsArgs(x, peer, core), { ...(await fees(x)), ...gasFor(x, receiptsGas) })).wait(1, WAIT_MS);
  await expectEqual(`${x.name}: receipts part made`, async () => await f.receipts.contract.made(madeReceipts), true);
  const receiptsPart = new Contract(madeReceipts, artifact("protocol/vault/VaultReceipts.sol", "VaultReceipts").abi, x.provider);
  await expectEqual(`${x.name}: receipts part's vault and pair`, async () => `${await receiptsPart.core()},${await receiptsPart.here()},${await receiptsPart.peer()}`, `${core},${x.s.number},${peer.s.number}`);
  await expectEqual(`${x.name}: receipts part's genesis`, async () => `${(await receiptsPart.genesisKey()).toLowerCase()},${await receiptsPart.genesisEnd()}`, `${(genesisDays ? x.wallet.address : ZeroAddress).toLowerCase()},${genesisEnd}`);
  console.log(`${x.name}: VaultHome ${madeHome}, VaultReceipts ${madeReceipts}`);
  const common = [x.d.protocol, x.s.number, peer.s.number, peerVault, amounts.deposit, amounts.minCertifyingEscrow, f.home.address, f.receipts.address, madeHome, madeReceipts];
  const vault = await deploy(x, VAULT, "iPoWVaultNative", common);
  if (vault.address !== core) throw new Error(`${x.name}: the vault is at ${vault.address}, its parts were made for ${core}`);
  const v = vault.contract;
  await expectEqual(`${x.name}: vault's networks`, async () => `${await v.here()},${await v.peer()}`, `${x.s.number},${peer.s.number}`);
  await expectEqual(`${x.name}: vault's peer vault`, async () => (await v.peerVault()).toLowerCase(), peerVault.toLowerCase());
  await expectEqual(`${x.name}: vault's protocol`, async () => await v.protocol(), x.d.protocol);
  await expectEqual(`${x.name}: vault's amounts`, async () => `${await v.deposit()},${await v.minCertifyingEscrow()}`, `${amounts.deposit},${amounts.minCertifyingEscrow}`);
  await expectEqual(`${x.name}: vault's factories`, async () => `${await v.homeFactory()},${await v.receiptsFactory()}`, `${f.home.address},${f.receipts.address}`);
  await expectEqual(`${x.name}: vault's parts`, async () => `${getAddress(await v.home())},${getAddress(await v.receipts())}`, `${madeHome},${madeReceipts}`);
  for (const [part, file, name] of [[madeHome, "protocol/vault/VaultHome.sol", "VaultHome"], [madeReceipts, "protocol/vault/VaultReceipts.sol", "VaultReceipts"]] as const) {
    const a = artifact(file, name);
    const code = await settle(() => x.provider.getCode(part), (cd) => codeMatches(cd, a.deployedBytecode, a.immutableReferences));
    if (!codeMatches(code, a.deployedBytecode, a.immutableReferences)) throw new Error(`${x.name}: the code at ${part} is not the build's ${name}`);
  }
  console.log(`${x.name}: vault ${vault.address} read back`);
  return { peer: peer.s.number, peerVault, vault: vault.address, home: madeHome, receipts: madeReceipts, homeFactory: f.home.address, receiptsFactory: f.receipts.address, source: "current", ...(genesisDays ? { genesisEnd: Number(genesisEnd) } : {}) };
}

console.log(`${A.name} (chain ${A.s.chainId.testnet}) and ${B.name} (chain ${B.s.chainId.testnet}): deployer ${A.wallet.address}${dry ? " (dry run)" : ""}`);
if (dry) {
  const nA = await settledNonce(A);
  const nB = await settledNonce(B);
  console.log(`  with factories first on each side: ${A.name}'s vault at ${createAt(A, nA + 4)}, ${B.name}'s at ${createAt(B, nB + 4)}`);
  process.exit(0);
}

// B first: its factories, then its vault's address from one settled nonce,
// and both of its parts simulated, before anything is spent on A.
const fB = await deployFactories(B);
const nonceB = await settledNonce(B);
const coreB = createAt(B, nonceB + 2);
await simulateParts(B, fB, A, coreB);
console.log(`${B.name}: its vault will be ${coreB}; both parts simulate`);

// A: its factories, parts and vault, naming B's vault to come.
const fA = await deployFactories(A);
const nonceA = await settledNonce(A);
const coreA = createAt(A, nonceA + 2);
let recordA: Awaited<ReturnType<typeof deployVault>>;
try {
  recordA = await deployVault(A, fA, B, coreA, nonceA, asBytes32(coreB));
} catch (e) {
  throw new Error(`${A.name}: its vault was not deployed (${(e as Error).message}); nothing is recorded, ${B.name} only has new factories ${fB.home.address} and ${fB.receipts.address}`);
}

// B: its parts and vault at the predicted address, naming A's vault.
let recordB: Awaited<ReturnType<typeof deployVault>>;
try {
  recordB = await deployVault(B, fB, A, coreB, nonceB, asBytes32(recordA.vault));
} catch (e) {
  throw new Error(
    `${B.name}: its vault was not deployed at ${coreB} (${(e as Error).message}). ${A.name}'s vault ${recordA.vault} (parts ${recordA.home}, ${recordA.receipts}; factories ${recordA.homeFactory}, ${recordA.receiptsFactory}) names ${coreB}, which may now never hold a vault: it is not recorded, and a new run makes a new pair`
  );
}
// A genesis redeploy replaces the pair's vault on each side.
if (genesisDays) {
  A.d.vaults = A.d.vaults.filter((v: { peer: number }) => v.peer !== B.s.number);
  B.d.vaults = B.d.vaults.filter((v: { peer: number }) => v.peer !== A.s.number);
}
A.d.vaults.push({ ...recordA, at: new Date().toISOString() });
B.d.vaults.push({ ...recordB, at: new Date().toISOString() });
writeFileSync(A.file, JSON.stringify(A.d, null, 2) + "\n");
writeFileSync(B.file, JSON.stringify(B.d, null, 2) + "\n");
console.log(`wrote ${A.file} and ${B.file}`);
console.log(`pair ${A.name}–${B.name}: ${recordA.vault} names ${recordB.vault}, and back`);
