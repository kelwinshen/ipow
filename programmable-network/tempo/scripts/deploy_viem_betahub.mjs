// BetaHub (design/ipow-implementation.md §8.18) on Tempo — same viem/Tempo-native deploy
// path as scripts/deploy_viem.mjs (which deployed iPoW + BetaVault +
// MockERC20 here already); see that file's own comments for why plain
// Hardhat/ethers can't speak Tempo's fee-sponsored, 2D-nonce transaction
// model. Reuses the already-deployed iPoW relay and NONCE_KEY=1n lane
// (continuing that lane's nonce, not restarting it — a fresh lane's first
// nonce=0 slot would collide with this sender's very first contract
// creation, per the confirmed nonce-lane address-derivation bug).
//
// NEW VARIANT OF THAT BUG, confirmed running this script for BetaHub: the
// transaction receipt's own contractAddress field was wrong — it reported
// nonce 2's address (already occupied by an old MockERC20 from an earlier
// deploy; that address's code never actually changed), while the contract
// genuinely landed at nonce 6's address instead (verified by reading live
// state: governance()/ipowHeaders()/SELF_NETWORK_ID() all correct, and its
// HubToken's minter() correctly points back to it). So on Tempo, NEVER
// trust a deploy script's reported address — always independently derive
// the candidate CREATE address for several nearby nonces
// (ethers.getCreateAddress({from, nonce})) and check which one actually
// holds code matching the expected deployed-bytecode length.
//
//   node scripts/deploy_viem_betahub.mjs
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

const artifact = (n, dir) => JSON.parse(fs.readFileSync(`artifacts/contracts/${dir ?? n}.sol/${n}.json`, "utf8"));

const NONCE_KEY = 1n;
let nonce = await client.nonce.getNonce({ account: account.address, nonceKey: NONCE_KEY });
console.log("starting nonce for lane", NONCE_KEY, ":", nonce);

async function deploy(name, abi, bytecode, args, dir) {
  const data = encodeDeployData({ abi, bytecode, args });
  const hash = await client.sendTransaction({ data, feePayer: true, nonceKey: NONCE_KEY, nonce: Number(nonce) });
  nonce += 1n;
  console.log(`${name} deploy tx`, hash);
  const receipt = await client.waitForTransactionReceipt({ hash });
  console.log(`${name} deployed at`, receipt.contractAddress, "status", receipt.status);
  if (receipt.status !== "success") throw new Error(`${name} deployment reverted`);
  const code = await client.getCode({ address: receipt.contractAddress });
  const expectedLen = (bytecode.length - 2) / 2;
  console.log(`  code length: ${(code.length - 2) / 2} bytes (creation bytecode was ${expectedLen})`);
  if (!code || code === "0x") throw new Error(`${name}: no code at reported address`);
  return receipt.contractAddress;
}

const ipowAddr = "0x53e1291bdaff473694bbbb8dd257f9844e5f9f3c";
console.log("reusing already-deployed iPoW:", ipowAddr);

const hubArt = artifact("BetaHub");
const params = {
  tChallengeSecs: 7n * 86400n,
  tSkipSecs: 28800n,
  refundMarginSecs: 600n,
  unbondDelaySecs: 60n,
  minOperatorBond: 3_000_000_000_000_000n,
  minAuditorBond: 2_000_000_000_000_000n,
  slashWeiPerUnit: 1_000_000_000_000_000n,
  vetoSlashWei: 1_000_000_000_000_000n,
  vetoRewardWei: 500_000_000_000_000n,
  bountyBps: 1000n,
};
const betaHubAddr = await deploy("BetaHub", hubArt.abi, hubArt.bytecode, [account.address, ipowAddr, 7n, params, "iBETA", "iBETA"]);

console.log("\niPoW: ", ipowAddr);
console.log("BetaHub:", betaHubAddr);
