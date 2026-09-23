// Tempo's "Moderato" testnet RPC fails eth_estimateGas whenever EIP-1559
// fee fields (maxFeePerGas/maxPriorityFeePerGas) are included in the call
// — confirmed directly against the RPC: the exact same call succeeds
// without those fields and fails with them. Hardhat Ignition always
// includes them internally, so this deploys the same three contracts
// (iPoWV1, BetaVault, MockERC20) via plain ethers with an explicit
// gasLimit, skipping Ignition's estimation entirely for this network only.
//
//   node scripts/deploy_raw.mjs
import fs from "node:fs";
import { ethers } from "ethers";

const env = Object.fromEntries(
  fs.readFileSync(".env", "utf8").split("\n").filter((l) => l.includes("=") && !l.startsWith("#")).map((l) => {
    const i = l.indexOf("=");
    return [l.slice(0, i).trim(), l.slice(i + 1).trim().replace(/^"|"$/g, "")];
  })
);
const provider = new ethers.JsonRpcProvider(env.TEMPO_TESTNET_RPC_URL);
const wallet = new ethers.Wallet(env.TEMPO_TESTNET_PRIVATE_KEY, provider);
const artifact = (n) => JSON.parse(fs.readFileSync(`artifacts/contracts/${n === "MockERC20" ? "test/MockERC20" : n}.sol/${n}.json`, "utf8"));

async function deploy(name, args, gasLimit) {
  const a = artifact(name);
  const factory = new ethers.ContractFactory(a.abi, a.bytecode, wallet);
  const tx = await factory.getDeployTransaction(...args);
  tx.gasLimit = gasLimit;
  const sent = await wallet.sendTransaction(tx);
  console.log(`${name} deploy tx`, sent.hash);
  const rc = await sent.wait();
  console.log(`${name} deployed at`, rc.contractAddress, "gasUsed", rc.gasUsed.toString());
  return rc.contractAddress;
}

const me = await wallet.getAddress();
const NATIVE_DECIMALS = 18n;
const SELF_NETWORK_ID = 7n; // Tempo, per the iPoW protocol network ID registry
const COMMIT_FEE_BPS = 50n;

const ipowAddr = await deploy("iPoWV1", [NATIVE_DECIMALS, SELF_NETWORK_ID, me, COMMIT_FEE_BPS], 30_000_000n);

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
const betaVaultAddr = await deploy("BetaVault", [me, ipowAddr, params], 10_000_000n);
const mockTokenAddr = await deploy("MockERC20", ["Mock BETA-leg USD", "mUSD"], 3_000_000n);

console.log("\niPoWV1:   ", ipowAddr);
console.log("BetaVault:", betaVaultAddr);
console.log("MockERC20:", mockTokenAddr);
