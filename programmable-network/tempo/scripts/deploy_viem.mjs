// Tempo's Moderato testnet doesn't use ordinary native-ETH gas — every
// transaction needs a feeToken and is paid for by a feePayer (a sponsor),
// via a Tempo-specific transaction type. Plain Hardhat/ethers assumes
// standard EIP-1559 gas and can't speak this model (see scripts/deploy_raw.mjs
// and its "insufficient funds"/gas-estimation failures for why that path
// doesn't work here). This uses viem's native Tempo support instead,
// routing every deploy through Tempo's public testnet sponsor endpoint
// (https://sponsor.moderato.tempo.xyz, no API key needed) so no real gas
// token is required at all.
//
// IMPORTANT nonce gotcha: setting `feePayer: true` with no explicit
// `nonceKey` makes viem silently pick Tempo's "expiring nonce" lane
// (nonceKey = maxUint256, nonce = 0) — meant for single-use,
// order-independent sponsored calls (see node_modules/viem/tempo/
// chainConfig.ts's `useExpiringNonce` logic). Every transaction sent that
// way reuses the identical (nonceKey, nonce) pair, and since a created
// contract's address is derived from it, every deploy landed at the
// SAME address, only the first one really taking effect (confirmed by
// checking deployed bytecode directly — the other two silently no-opped
// despite reporting status: success). Fix: use a real, incrementing
// nonce lane (nonceKey = 1n here) explicitly for every deploy.
//
//   node scripts/deploy_viem.mjs
import fs from "node:fs";
import { createClient, http, withRelay, Account } from "viem/tempo";
import { encodeDeployData } from "viem";

const env = Object.fromEntries(
  fs.readFileSync(".env", "utf8").split("\n").filter((l) => l.includes("=") && !l.startsWith("#")).map((l) => {
    const i = l.indexOf("=");
    return [l.slice(0, i).trim(), l.slice(i + 1).trim().replace(/^"|"$/g, "")];
  })
);

const account = Account.fromSecp256k1(env.TEMPO_TESTNET_PRIVATE_KEY);
const client = createClient({
  account,
  testnet: true,
  transport: withRelay(http(env.TEMPO_TESTNET_RPC_URL), http("https://sponsor.moderato.tempo.xyz")),
});

const artifact = (n) => JSON.parse(fs.readFileSync(`artifacts/contracts/${n === "MockERC20" ? "test/MockERC20" : n}.sol/${n}.json`, "utf8"));

const NONCE_KEY = 1n;
let nonce = await client.nonce.getNonce({ account: account.address, nonceKey: NONCE_KEY });
console.log("starting nonce for lane", NONCE_KEY, ":", nonce);

async function deploy(name, abi, bytecode, args) {
  const data = encodeDeployData({ abi, bytecode, args });
  // sendTransaction's `nonce` is a plain number, not the bigint getNonce
  // returns — a bigint/number mismatch here trips viem's internal
  // "filled transaction nonce does not match the requested nonce" check
  // even when they print identically.
  const hash = await client.sendTransaction({ data, feePayer: true, nonceKey: NONCE_KEY, nonce: Number(nonce) });
  nonce += 1n;
  console.log(`${name} deploy tx`, hash);
  const receipt = await client.waitForTransactionReceipt({ hash });
  console.log(`${name} deployed at`, receipt.contractAddress, "status", receipt.status);
  if (receipt.status !== "success") throw new Error(`${name} deployment reverted`);
  // Confirm real bytecode landed there — don't trust contractAddress alone
  // after the identical-address bug above.
  const code = await client.getCode({ address: receipt.contractAddress });
  const expectedLen = (bytecode.length - 2) / 2; // rough upper bound (creation vs runtime differ)
  console.log(`  code length: ${(code.length - 2) / 2} bytes (creation bytecode was ${expectedLen})`);
  if (!code || code === "0x") throw new Error(`${name}: no code at reported address`);
  return receipt.contractAddress;
}

// iPoWV1 already genuinely deployed in the prior run (verified by bytecode
// match) — reuse it rather than spend the sponsor's funds redeploying.
const ipowAddr = "0x53e1291bdaff473694bbbb8dd257f9844e5f9f3c";
console.log("reusing already-deployed iPoWV1:", ipowAddr);

const vaultArt = artifact("BetaVault");
const params = {
  ethWeiPerUnit: 1_000_000_000_000_000n,
  tFinSecs: 14400n,
  tChallengeSecs: 7n * 86400n,
  tSkipSecs: 28800n,
  refundMarginSecs: 600n,
  unbondDelaySecs: 60n,
  minOperatorBond: 3_000_000_000_000_000n,
  minAuditorBond: 2_000_000_000_000_000n,
  vetoSlashWei: 1_000_000_000_000_000n,
  vetoRewardWei: 500_000_000_000_000n,
  bountyBps: 1000n,
};
const betaVaultAddr = await deploy("BetaVault", vaultArt.abi, vaultArt.bytecode, [account.address, ipowAddr, params]);

const mockArt = artifact("MockERC20");
const mockTokenAddr = await deploy("MockERC20", mockArt.abi, mockArt.bytecode, ["Mock BETA-leg USD", "mUSD"]);

console.log("\niPoWV1:   ", ipowAddr);
console.log("BetaVault:", betaVaultAddr);
console.log("MockERC20:", mockTokenAddr);
