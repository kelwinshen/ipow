// Deploys the new protocol on Tempo's testnet (stage 8) from the one source
// in ../ethereum with Tempo's settings (D134, D136, D137): the token builds,
// PathUSD as the coin. Tempo's transactions are its own type, which ethers
// cannot send, so each deployment is sent with viem's Tempo support, and
// the shared deploy function (../ethereum/deploy/deploy.ts) does every
// check and read-back as on the other networks. Run from
// programmable-network/tempo:
//
//   node scripts/deploy-new-protocol.ts [--dry]
//   node scripts/deploy-new-protocol.ts --vaults-only --solana-pair <account> [--genesis-days N]
//
// --vaults-only keeps the protocol of deployments/tempo-testnet.json and
// deploys a new vault paired with Solana beside it, naming <account> (the
// Solana vault's ["config", 8]); with --genesis-days its receipts start in
// genesis for the deployer (docs/drafts/ipow-vault-genesis.md).
//
// Where a contract lands: read on 2026-10-02, the deployer's ordinary nonce
// was 12 and exactly the CREATE addresses of raw nonces 0 to 11 held code,
// though its lane 1 had reached 27, which suggests a contract lands at the
// address of the account's ordinary nonce whatever lane sent it. The
// implementation doc's Tempo section describes the bug differently (the
// lane's own nonce number); either way, each deployment here is sent in the
// ordinary lane, where the two agree, the address is worked out from that
// nonce and checked empty first, and the code there is then compared with
// the build; a receipt naming another address is reported, not trusted.
// With --dry it only reads.

import fs from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { createClient, http, Account } from "viem/tempo";
import { encodeDeployData, encodeFunctionData, getContractAddress } from "viem";

import { contractsSource, deployNetwork, readOnlySigner, solanaAccountBytes } from "../../ethereum/deploy/deploy.ts";
import { NETWORKS } from "../../ethereum/deploy/networks.ts";

const here = dirname(fileURLToPath(import.meta.url));
const DRY = process.argv.includes("--dry");
const argv = process.argv.slice(2);
const flag = (name: string) => (argv.includes(name) ? argv[argv.indexOf(name) + 1] : undefined);
const VAULTS_ONLY = argv.includes("--vaults-only");
const GENESIS_DAYS = Number(flag("--genesis-days") ?? 0);
if (flag("--genesis-days") && !(GENESIS_DAYS > 0 && GENESIS_DAYS <= 30)) throw new Error("--genesis-days: 1 to 30");
if (VAULTS_ONLY && !flag("--solana-pair")) throw new Error("--vaults-only needs --solana-pair");
/** D93, as on every other network. */
const MIN_HEIGHT = 965_567;
const MAX_SATS = 100_000n;
/** The Solana vault's pair account for Tempo, ["config", 8]. */
const SOLANA_PAIR = flag("--solana-pair") ?? "7wvWJdeTW9WynaP17Le7dnWcBkV7qpVz78sR9TrA977Q";
/** The same account as 32 bytes. */
const SOLANA_PAIR_BYTES = solanaAccountBytes(SOLANA_PAIR);

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

const empty = async (address: `0x${string}`) => {
  const code = await client.getCode({ address });
  return !code || code === "0x";
};
const createAddress = (nonce: number) => getContractAddress({ from: account.address, opcode: "CREATE", nonce: BigInt(nonce) });

const start = await client.getTransactionCount({ address: account.address });
console.log("deployer", account.address, "ordinary nonce", start, "contracts from", source);
// The deployment takes 9 transactions: 7 create the deployer's contracts,
// and 2 make the vault's parts at its factories' addresses. The deployer's
// addresses for all 9 nonces are checked free (2 of them stay unused).
for (let n = start; n < start + 9; n++) {
  if (!(await empty(createAddress(n)))) throw new Error(`the address of nonce ${n}, ${createAddress(n)}, holds code already`);
}
console.log(`the addresses of nonces ${start} to ${start + 8} are empty`);
if (DRY) {
  console.log("dry: nothing sent; the first contract would land at", createAddress(start));
  process.exit(0);
}

async function send(name: string, abi: unknown, bytecode: string, args: unknown[]) {
  const nonce = await client.getTransactionCount({ address: account.address });
  const expected = createAddress(nonce);
  if (!(await empty(expected))) throw new Error(`${name}: the address of nonce ${nonce} holds code already`);
  const data = encodeDeployData({ abi: abi as any, bytecode: bytecode as `0x${string}`, args });
  const hash = await client.sendTransaction({ data, nonce, nonceKey: 0n, feeToken: PATHUSD } as any);
  const receipt = await client.waitForTransactionReceipt({ hash });
  if (receipt.status !== "success") throw new Error(`${name}: reverted, ${hash}`);
  if (receipt.contractAddress?.toLowerCase() !== expected.toLowerCase()) {
    console.log(`  ${name}: the receipt names ${receipt.contractAddress}; nonce ${nonce} gives ${expected}, whose code is read`);
  }
  return expected;
}

async function call(to: string, abi: unknown, fn: string, args: unknown[]) {
  const nonce = await client.getTransactionCount({ address: account.address });
  const data = encodeFunctionData({ abi: abi as any, functionName: fn, args });
  const hash = await client.sendTransaction({ to: to as `0x${string}`, data, nonce, nonceKey: 0n, feeToken: PATHUSD } as any);
  const receipt = await client.waitForTransactionReceipt({ hash });
  if (receipt.status !== "success") throw new Error(`${fn}: reverted, ${hash}`);
}

async function nextCreate(offset: number) {
  return createAddress((await client.getTransactionCount({ address: account.address })) + offset);
}

const file = join(here, "..", "..", "ethereum", "deployments", "tempo-testnet.json");
const before = VAULTS_ONLY ? JSON.parse(fs.readFileSync(file, "utf8")) : null;
const genesis = GENESIS_DAYS ? { key: account.address, end: BigInt(Math.floor(Date.now() / 1000) + GENESIS_DAYS * 86_400) } : undefined;
const deployment = await deployNetwork(readOnlySigner(env[s.rpcEnv.testnet], account.address), {
  network: "tempo",
  env: "testnet",
  minHeight: MIN_HEIGHT,
  maxSats: MAX_SATS,
  pairs: [{ peer: 2, peerVault: SOLANA_PAIR_BYTES }],
  send,
  call,
  nextCreate,
  existing: before ? { lightClient: before.lightClient, dataFee: before.dataFee, protocol: before.protocol, conversion: before.conversion, betaBaskets: before.betaBaskets } : undefined,
  genesis,
  log: (line) => console.log(line),
});

if (before) {
  // The record before, kept beside it; then the new vault in place of the old.
  const replaced = join(here, "..", "..", "ethereum", "deployments", "replaced");
  fs.mkdirSync(replaced, { recursive: true });
  fs.copyFileSync(file, join(replaced, `tempo-testnet.${Date.now()}.json`));
  const vault = deployment.vaults[0];
  before.vaults = [
    { ...vault, homeFactory: deployment.homeFactory, receiptsFactory: deployment.receiptsFactory, source, ...(genesis ? { genesisEnd: Number(genesis.end) } : {}), at: new Date().toISOString() },
    ...before.vaults.filter((v: { peer: number }) => v.peer !== 2),
  ];
  Object.assign(before, { homeFactory: deployment.homeFactory, receiptsFactory: deployment.receiptsFactory, solanaPairAccount: SOLANA_PAIR });
  fs.writeFileSync(file, JSON.stringify(before, null, 2) + "\n");
} else {
  fs.writeFileSync(file, JSON.stringify({ ...deployment, source, deployer: account.address, minHeight: MIN_HEIGHT, maxSats: String(MAX_SATS), solanaPairAccount: SOLANA_PAIR, at: new Date().toISOString() }, null, 2) + "\n");
}
console.log("written", file);
