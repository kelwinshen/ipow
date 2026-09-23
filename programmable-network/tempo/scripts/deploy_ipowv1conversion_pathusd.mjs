// Deploys iPoWV1ConversionPathUSD — Tempo-only iPoWV1Conversion variant
// using PathUSD for the commit fee, auction stake, and bounty instead of
// native value (see contracts/iPoWV1ConversionPathUSD.sol's header
// comment). Same viem-native, fee-sponsored deploy path and same
// never-trust-the-reported-address discipline as every other Tempo
// deploy script here.
//
//   node scripts/deploy_ipowv1conversion_pathusd.mjs
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

const artifact = JSON.parse(
  fs.readFileSync("artifacts/contracts/iPoWV1ConversionPathUSD.sol/iPoWV1ConversionPathUSD.json", "utf8"),
);

const NONCE_KEY = 1n;
let nonce = await client.nonce.getNonce({ account: account.address, nonceKey: NONCE_KEY });
console.log("starting nonce for lane", NONCE_KEY, ":", nonce);

const ipowAddr = "0x53e1291bdaff473694bbbb8dd257f9844e5f9f3c";
const PATH_USD = "0x20c0000000000000000000000000000000000000";
const COMMIT_FEE_BPS = 50n;

const data = encodeDeployData({
  abi: artifact.abi,
  bytecode: artifact.bytecode,
  args: [account.address, ipowAddr, PATH_USD, COMMIT_FEE_BPS],
});
const hash = await client.sendTransaction({ data, feePayer: true, nonceKey: NONCE_KEY, nonce: Number(nonce) });
console.log("deploy tx:", hash);
const receipt = await client.waitForTransactionReceipt({ hash });
console.log("reported address:", receipt.contractAddress, "status:", receipt.status);
console.log("used nonce:", nonce.toString());
console.log("\n(Reported address is NOT to be trusted on Tempo — independently scan nearby nonces for real bytecode next.)");
