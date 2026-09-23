// Deploys BetaHubPathUSD — Tempo-only BetaHub variant using PathUSD for
// bonds/payouts instead of native value (mirrors BetaVaultPathUSD's own
// fix, see contracts/BetaHubPathUSD.sol's header comment). Same
// viem-native, fee-sponsored deploy path as scripts/deploy_viem_betahub.mjs;
// same "never trust the reported address" discipline as every other Tempo
// deploy script here — a confirmed, unfixed chain-level bug means the
// receipt's own contractAddress field can be wrong.
//
//   node scripts/deploy_betahub_pathusd.mjs
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

const artifact = JSON.parse(fs.readFileSync("artifacts/contracts/BetaHubPathUSD.sol/BetaHubPathUSD.json", "utf8"));

const NONCE_KEY = 1n;
let nonce = await client.nonce.getNonce({ account: account.address, nonceKey: NONCE_KEY });
console.log("starting nonce for lane", NONCE_KEY, ":", nonce);

const ipowAddr = "0x53e1291bdaff473694bbbb8dd257f9844e5f9f3c";
const PATH_USD = "0x20c0000000000000000000000000000000000000";
// Same time-based params as the plain BetaHub's own deploy (timing isn't
// PathUSD-specific); bond/slash/reward magnitudes rescaled to PathUSD's
// 6-decimal round-number convention, matching deploy_betavault_pathusd.mjs's
// own scale (5 PathUSD operator bond, 1 PathUSD slash unit, 0.5 PathUSD
// veto reward) rather than deriving a conversion from the native-ETH
// amounts, which have no principled ETH<->PathUSD exchange rate anyway.
const params = {
  tChallengeSecs: 7n * 86400n,
  tSkipSecs: 28800n,
  refundMarginSecs: 600n,
  unbondDelaySecs: 60n,
  minOperatorBond: 5_000_000n, // 5 PathUSD
  minAuditorBond: 3_000_000n, // 3 PathUSD
  slashWeiPerUnit: 1_000_000n, // 1 PathUSD
  vetoSlashWei: 1_000_000n, // 1 PathUSD
  vetoRewardWei: 500_000n, // 0.5 PathUSD
  bountyBps: 1000n,
};

const data = encodeDeployData({
  abi: artifact.abi,
  bytecode: artifact.bytecode,
  args: [account.address, ipowAddr, PATH_USD, 7n, params, "iBETA", "iBETA"],
});
const hash = await client.sendTransaction({ data, feePayer: true, nonceKey: NONCE_KEY, nonce: Number(nonce) });
console.log("deploy tx:", hash);
const receipt = await client.waitForTransactionReceipt({ hash });
console.log("reported address:", receipt.contractAddress, "status:", receipt.status);
console.log("used nonce:", nonce.toString());
console.log("\n(Reported address is NOT to be trusted on Tempo — independently scan nearby nonces for real bytecode next.)");
