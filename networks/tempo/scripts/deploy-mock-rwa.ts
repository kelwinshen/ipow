// Deploys Greatwall.finance's test stand-ins for tokenized assets on Tempo's
// testnet (MockRWA from ../../evm, named "<asset> (Tempo Testnet
// Greatwall)", see TOKENS), mints the operator's inventory, registers each
// with the vault's pair with Solana, and reads it all back: what
// ../../evm/scripts/deploy-mock-rwa.ts does on the other EVM networks.
// Tempo's transactions are its own type, which ethers cannot send, so each
// is sent with viem's Tempo support in the ordinary lane with PathUSD as the
// fee token, as deploy-new-protocol.ts does; a contract's address is worked
// out from the deployer's ordinary nonce and its code read there, since a
// receipt can name another address (networks/tempo/README.md).
// Run from networks/tempo, after `npx hardhat compile` in
// ../../evm:
//
//   node scripts/deploy-mock-rwa.ts [--operator <address>] [--inventory <whole tokens>] [--dry]
//
// Writes ../../evm/deployments/tempo-testnet-rwa.json after every token;
// every step is skipped when done, so a stopped run continues.

import fs from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { createClient, http, Account } from "viem/tempo";
import { encodeDeployData, encodeFunctionData, decodeFunctionResult, getAddress, getContractAddress, parseUnits } from "viem";

import { NETWORKS } from "../../../evm/deploy/networks.ts";

/** The tokens: the real tokenized asset each stands in for (rwa.xyz), and
 *  what the faucet gives at a time; all dollar-like, so 100 a claim. */
const TOKENS = [
  { symbol: "EURC", name: "EURC (Tempo Testnet Greatwall)", faucet: "100" },
  { symbol: "USDY", name: "Ondo U.S. Dollar Yield (Tempo Testnet Greatwall)", faucet: "100" },
  { symbol: "syrupUSDC", name: "Syrup USDC (Tempo Testnet Greatwall)", faucet: "100" },
];

const here = dirname(fileURLToPath(import.meta.url));
const args = process.argv.slice(2);
const DRY = args.includes("--dry");
const flag = (name: string) => (args.includes(name) ? args[args.indexOf(name) + 1] : undefined);

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
const chainId = await client.getChainId();
if (chainId !== s.chainId.testnet) throw new Error(`chain ${chainId} is not Tempo's testnet (${s.chainId.testnet})`);

const artifact = JSON.parse(fs.readFileSync(join(here, "..", "..", "..", "evm", "artifacts", "contracts", "testnet", "MockRWA.sol", "MockRWA.json"), "utf8"));
const HOME_ABI = [
  { type: "function", name: "registerAsset", inputs: [{ type: "address" }], outputs: [{ type: "uint32" }], stateMutability: "nonpayable" },
  { type: "function", name: "assetOfToken", inputs: [{ type: "address" }], outputs: [{ type: "uint32" }], stateMutability: "view" },
];
const d = JSON.parse(fs.readFileSync(join(here, "..", "..", "..", "evm", "deployments", "tempo-testnet.json"), "utf8"));
const home: `0x${string}` = d.vaults[0].home;
const operator = getAddress(flag("--operator") ?? account.address);
const inventory = flag("--inventory") ?? "1000";

const out = join(here, "..", "..", "..", "evm", "deployments", "tempo-testnet-rwa.json");
const existing = fs.existsSync(out) ? JSON.parse(fs.readFileSync(out, "utf8")) : { tokens: {} };
const save = () => fs.writeFileSync(out, JSON.stringify({ network: "tempo-testnet", chainId, minter: account.address, operator, tokens: existing.tokens }, null, 2) + "\n");

const empty = async (address: `0x${string}`) => {
  const code = await client.getCode({ address });
  return !code || code === "0x";
};
const nonce = () => client.getTransactionCount({ address: account.address });
const read = async (to: `0x${string}`, abi: any, functionName: string, fnArgs: unknown[] = []) => {
  const data = encodeFunctionData({ abi, functionName, args: fnArgs });
  const { data: result } = await client.call({ to, data });
  if (!result) throw new Error(`${functionName} at ${to} returned nothing`);
  return decodeFunctionResult({ abi, functionName, data: result });
};
/** Sends in the ordinary lane, fees in PathUSD, and waits for success. */
async function send(what: string, tx: { to?: `0x${string}`; data: `0x${string}` }, n?: number) {
  n ??= await nonce();
  const hash = await client.sendTransaction({ ...tx, nonce: n, nonceKey: 0n, feeToken: PATHUSD } as any);
  const receipt = await client.waitForTransactionReceipt({ hash });
  if (receipt.status !== "success") throw new Error(`${what}: reverted, ${hash}`);
  return { hash, receipt, nonce: n };
}

console.log(`Tempo testnet (chain ${chainId}): deployer ${account.address}, operator ${operator}, vault home ${home}${DRY ? " (dry run)" : ""}`);

for (const t of TOKENS) {
  let address: `0x${string}` | undefined = existing.tokens[t.symbol]?.address;
  if (address && !(await empty(address))) {
    console.log(`${t.symbol}: deployed already at ${address}`);
  } else if (DRY) {
    console.log(`${t.symbol}: would deploy "${t.name}", faucet ${t.faucet}, mint ${inventory} to the operator, register with the vault`);
    continue;
  } else {
    // The address of the ordinary nonce, checked free, is where the code is read.
    const n = await nonce();
    const expected = getContractAddress({ from: account.address, opcode: "CREATE", nonce: BigInt(n) });
    if (!(await empty(expected))) throw new Error(`${t.symbol}: the address of the next nonce, ${expected}, holds code already`);
    const data = encodeDeployData({ abi: artifact.abi, bytecode: artifact.bytecode, args: [t.name, t.symbol, parseUnits(t.faucet, 18)] });
    const { hash, receipt } = await send(`${t.symbol} deploy`, { data }, n);
    if (receipt.contractAddress?.toLowerCase() !== expected.toLowerCase()) console.log(`${t.symbol}: the receipt names ${receipt.contractAddress}; the nonce gives ${expected}, whose code is read`);
    if (await empty(expected)) throw new Error(`${t.symbol}: no code at ${expected} after ${hash}`);
    address = expected;
    console.log(`${t.symbol}: deployed at ${address} (${hash})`);
    existing.tokens[t.symbol] = { address, name: t.name, decimals: 18, faucet: t.faucet };
    save();
  }
  if (DRY) continue;
  if ((await read(address, artifact.abi, "balanceOf", [operator])) === 0n) {
    await send(`${t.symbol} mint`, { to: address, data: encodeFunctionData({ abi: artifact.abi, functionName: "mint", args: [operator, parseUnits(inventory, 18)] }) });
    console.log(`${t.symbol}: minted ${inventory} to ${operator}`);
  }
  // Registered with the vault once: asset numbers start at 1 for tokens.
  let assetPlusOne = Number(await read(home, HOME_ABI, "assetOfToken", [address]));
  if (assetPlusOne === 0) {
    await send(`${t.symbol} register`, { to: home, data: encodeFunctionData({ abi: HOME_ABI, functionName: "registerAsset", args: [address] }) });
    assetPlusOne = Number(await read(home, HOME_ABI, "assetOfToken", [address]));
    if (assetPlusOne === 0) throw new Error(`${t.symbol}: not registered after the transaction`);
    console.log(`${t.symbol}: registered with the vault as asset ${assetPlusOne - 1}`);
  }
  existing.tokens[t.symbol] = { address, name: t.name, decimals: 18, faucet: t.faucet, vaultAsset: assetPlusOne - 1 };
  save();
}

if (!DRY) {
  // Read back what is on chain, not what the transactions said.
  for (const [symbol, t] of Object.entries<any>(existing.tokens)) {
    const a = t.address as `0x${string}`;
    const [name, sym, minter, faucet, held, asset] = await Promise.all([
      read(a, artifact.abi, "name"),
      read(a, artifact.abi, "symbol"),
      read(a, artifact.abi, "minter"),
      read(a, artifact.abi, "faucetAmount"),
      read(a, artifact.abi, "balanceOf", [operator]),
      read(home, HOME_ABI, "assetOfToken", [a]),
    ]);
    if (name !== t.name || sym !== symbol || getAddress(minter as string) !== account.address || faucet !== parseUnits(t.faucet, 18) || Number(asset) - 1 !== t.vaultAsset)
      throw new Error(`${symbol} at ${a} does not read back as recorded: name "${name}", symbol ${sym}, minter ${minter}, faucet ${faucet}, vault asset ${Number(asset) - 1}`);
    console.log(`checked ${symbol}: ${a}, vault asset ${t.vaultAsset}, operator holds ${(held as bigint) / 10n ** 18n}`);
  }
  save();
  console.log(`wrote ${out}`);
}
