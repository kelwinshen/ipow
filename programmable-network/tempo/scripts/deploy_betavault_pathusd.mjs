// Deploys BetaVaultPathUSD (DESIGN_V2.md §8.21) — Tempo-only BetaVault
// variant using PathUSD for bonds/payouts instead of native value, which
// Tempo's transaction type unconditionally rejects. Same viem-native,
// fee-sponsored deploy path as scripts/deploy_viem.mjs; same "never trust
// the reported address" discipline as scripts/deploy_viem_betahub.mjs.
//
//   node scripts/deploy_betavault_pathusd.mjs
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

const artifact = JSON.parse(fs.readFileSync("artifacts/contracts/BetaVaultPathUSD.sol/BetaVaultPathUSD.json", "utf8"));

const NONCE_KEY = 1n;
let nonce = await client.nonce.getNonce({ account: account.address, nonceKey: NONCE_KEY });
console.log("starting nonce for lane", NONCE_KEY, ":", nonce);

const ipowAddr = "0x53e1291bdaff473694bbbb8dd257f9844e5f9f3c";
const PATH_USD = "0x20c0000000000000000000000000000000000000";
const params = {
  ethWeiPerUnit: 1_000_000n, // 1.0 PathUSD per unit (6 decimals)
  tFinSecs: 14400n,
  tChallengeSecs: 7n * 86400n,
  tSkipSecs: 28800n,
  refundMarginSecs: 600n,
  unbondDelaySecs: 60n,
  minOperatorBond: 5_000_000n, // 5 PathUSD
  minAuditorBond: 3_000_000n,
  vetoSlashWei: 1_000_000n,
  vetoRewardWei: 500_000n,
  bountyBps: 1000n,
};

const data = encodeDeployData({
  abi: artifact.abi,
  bytecode: artifact.bytecode,
  args: [account.address, ipowAddr, PATH_USD, params],
});
const hash = await client.sendTransaction({ data, feePayer: true, nonceKey: NONCE_KEY, nonce: Number(nonce) });
console.log("deploy tx:", hash);
const receipt = await client.waitForTransactionReceipt({ hash });
console.log("reported address:", receipt.contractAddress, "status:", receipt.status);
console.log("\n(Reported address is NOT to be trusted on Tempo — independently scan nearby nonces for real bytecode next.)");
