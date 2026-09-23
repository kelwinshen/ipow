// Fix for the chained-statement mistake: Base/Robinhood/Hyperliquid's
// existing party 0x05 was registered pointing at the ORIGINAL shared chain
// head (4971e4fa...), but the MINT statement anchored for each of those
// legs actually spends an intermediate output further down the single
// shared Bitcoin chain (Ethereum's/Base's/Robinhood's own new head) — each
// EVM BetaVault only ever needs to see ITS OWN leg's link, and can't skip
// ahead past links it never processed. Fix: register a NEW party (0x06,
// independent per-chain storage, no collision with 0x05 or with any other
// chain's own 0x06) pointed at the CORRECT outpoint that leg's actual
// statement transaction spends, then process that same anchor under the
// new party id. Ethereum (already succeeded under 0x05) and Solana (which
// processes the whole chain sequentially through every link) are
// unaffected — this only touches Base/Robinhood/Hyperliquid.
//
//   node live_multi_network_fix_party.mjs <network>
import { ethers } from "ethers";
import fs from "node:fs";

const NEW_PARTY_ID = "0x" + "06".repeat(32);
const HEIGHT = 968203n;

const NETWORKS = {
  base: {
    envDir: "../../base", rpcKey: "BASE_SEPOLIA_RPC_URL", pkKey: "BASE_SEPOLIA_PRIVATE_KEY",
    betaVault: "0x6354779b4Dbb564c712ea91c179eCF521C15BE73",
    vaultArtifact: "../../base/artifacts/contracts/BetaVault.sol/BetaVault.json",
    startTxidBE: "f2eaa23140d2f6d8565a440bf9f884eea9c3c48800d4518c188d02ba2df4fdc3", // Ethereum's new head — what Base's MINT tx actually spends
    startVout: 0,
    pos: 3819,
    merkle: ["5e7d2d11c0a16f83d11dc36c444b47c0ee8b355f065e7f6b08ba8111c48ba997","ed2b5454c8706f036ea170731b669146a69d0f01cffd4cd359e1afac847e435c","d9f0342e16bc42d470fa759233cdb2d79d43040f0331545d0b8e4def00c43fd2","01b64e019f6929d52b638d03e683acb22306df1b15238ca77e4cbeae9d7002cb","3efb6fc03f8fa3f44780cf541e2a40988cf1d26f2ba62a032b065d4b2347142e","b54f8220cde2e30720981984c92c5193f92bb15bc1a18cf0710f2f11e1b02902","a667714778a8369029c4153d514bfff3467252407c6bec96d5b03702ad5d7fae","338910268efdd0b830e5226be65f20f530c89336f4470c9cd96096497defa7a9","138373de3ee28a1dfa5a9d00515963d058343bab1f050821e4ca149cb65f167d","cb059f9afe94d3e59bccfe78523adca82535e7c9ecb868da32c5610972248d7c","1691fcd7f33a5c7ce2aff63fc5ceaabfe7e6fce856fe8d950ad83bc902ebf711","85d9bd5e092c4bc16085d9c3628b60b032200cb1614637e329c3cb9c251ecf96"],
    txRaw: "0x0200000002c3fdf42dba028d188c51d40088c4c3a9ee84f8f90b445a56d8f6d24031a2eaf20000000000fdffffffc3fdf42dba028d188c51d40088c4c3a9ee84f8f90b445a56d8f6d24031a2eaf20200000000fdffffff032601000000000000160014f5c80ad661bd74b930bb7c84e5a01f6bd455f33b0000000000000000246a220101a1853184562d64d272224108203d3d617cb6652bc2a331c5778c92331bef11949a41000000000000160014f5c80ad661bd74b930bb7c84e5a01f6bd455f33b00000000",
    statement: "0x010000000000000002010000000000000001e0e72437e8caeddcb5368336c372742770ee2e3bb3b8e426a6d961e57a14f74a00000000000000020000000000000001000000006abc480a",
    lockId: 1,
  },
  robinhood: {
    envDir: "../../robinhood", rpcKey: "ROBINHOOD_TESTNET_RPC_URL", pkKey: "ROBINHOOD_TESTNET_PRIVATE_KEY",
    betaVault: "0x35e564d74B90a3A5bfcA8Dec65b1325C83d2e822",
    vaultArtifact: "../../robinhood/artifacts/contracts/BetaVault.sol/BetaVault.json",
    startTxidBE: "83f3d77cefb7bc039c0f3ff9521c98f9c2ad5bde14e0f4624e0e2f68e94284a4", // Base's new head
    startVout: 0,
    pos: 3820,
    merkle: ["63fc4fdf587b95d1a89a28b01e2958c234c404933dd8b59ec885c1a6eec955dd","944b0d5dbb34d18811ad06d30b28ba0c49521979265935ae7cc1d95673a4944c","12b8ff865782178dc3b442891632294507d9d34cc290b519efa0475a28b9478a","01b64e019f6929d52b638d03e683acb22306df1b15238ca77e4cbeae9d7002cb","3efb6fc03f8fa3f44780cf541e2a40988cf1d26f2ba62a032b065d4b2347142e","b54f8220cde2e30720981984c92c5193f92bb15bc1a18cf0710f2f11e1b02902","a667714778a8369029c4153d514bfff3467252407c6bec96d5b03702ad5d7fae","338910268efdd0b830e5226be65f20f530c89336f4470c9cd96096497defa7a9","138373de3ee28a1dfa5a9d00515963d058343bab1f050821e4ca149cb65f167d","cb059f9afe94d3e59bccfe78523adca82535e7c9ecb868da32c5610972248d7c","1691fcd7f33a5c7ce2aff63fc5ceaabfe7e6fce856fe8d950ad83bc902ebf711","85d9bd5e092c4bc16085d9c3628b60b032200cb1614637e329c3cb9c251ecf96"],
    txRaw: "0x0200000002a48442e9682f0e4e62f4e014de5badc2f9981c52f93f0f9c03bcb7ef7cd7f3830000000000fdffffffa48442e9682f0e4e62f4e014de5badc2f9981c52f93f0f9c03bcb7ef7cd7f3830200000000fdffffff032601000000000000160014f5c80ad661bd74b930bb7c84e5a01f6bd455f33b0000000000000000246a220101b0eeae353876d153c3cd587cdcae7f239c5828f20877e5170ae86f884fae8cdf0a40000000000000160014f5c80ad661bd74b930bb7c84e5a01f6bd455f33b00000000",
    statement: "0x010000000000000002020000000000000001e0e72437e8caeddcb5368336c372742770ee2e3bb3b8e426a6d961e57a14f74a00000000000000020000000000000001000000006abc480a",
    lockId: 1,
  },
  hyperliquid: {
    envDir: "../../hyperliquid", rpcKey: "HYPEREVM_TESTNET_RPC_URL", pkKey: "HYPEREVM_TESTNET_PRIVATE_KEY",
    betaVault: "0x554Fe13e4a5d0931e7c8F7d3E74Dea8Ca04C244a",
    vaultArtifact: "../../hyperliquid/artifacts/contracts/BetaVault.sol/BetaVault.json",
    startTxidBE: "f781690c7bfe24f69c9819083dab3be6c55b18cb41ad9c0911f5c743d9013fe1", // Robinhood's new head
    startVout: 0,
    pos: 3821,
    merkle: ["f781690c7bfe24f69c9819083dab3be6c55b18cb41ad9c0911f5c743d9013fe1","944b0d5dbb34d18811ad06d30b28ba0c49521979265935ae7cc1d95673a4944c","12b8ff865782178dc3b442891632294507d9d34cc290b519efa0475a28b9478a","01b64e019f6929d52b638d03e683acb22306df1b15238ca77e4cbeae9d7002cb","3efb6fc03f8fa3f44780cf541e2a40988cf1d26f2ba62a032b065d4b2347142e","b54f8220cde2e30720981984c92c5193f92bb15bc1a18cf0710f2f11e1b02902","a667714778a8369029c4153d514bfff3467252407c6bec96d5b03702ad5d7fae","338910268efdd0b830e5226be65f20f530c89336f4470c9cd96096497defa7a9","138373de3ee28a1dfa5a9d00515963d058343bab1f050821e4ca149cb65f167d","cb059f9afe94d3e59bccfe78523adca82535e7c9ecb868da32c5610972248d7c","1691fcd7f33a5c7ce2aff63fc5ceaabfe7e6fce856fe8d950ad83bc902ebf711","85d9bd5e092c4bc16085d9c3628b60b032200cb1614637e329c3cb9c251ecf96"],
    txRaw: "0x0200000002e13f01d943c7f511099cad41cb185bc5e63bab3d0819989cf624fe7b0c6981f70000000000fdffffffe13f01d943c7f511099cad41cb185bc5e63bab3d0819989cf624fe7b0c6981f70200000000fdffffff032601000000000000160014f5c80ad661bd74b930bb7c84e5a01f6bd455f33b0000000000000000246a220101a03a2fd90e4fdbacf418b6b91fa92a778c35c68d93ec3f42fcc5e04339e658527a3e000000000000160014f5c80ad661bd74b930bb7c84e5a01f6bd455f33b00000000",
    statement: "0x010000000000000002030000000000000001e0e72437e8caeddcb5368336c372742770ee2e3bb3b8e426a6d961e57a14f74a00000000000000020000000000000001000000006abc480a",
    lockId: 1,
  },
};

function loadEnv(dir) {
  const text = fs.readFileSync(new URL(dir + "/.env", import.meta.url), "utf8");
  return Object.fromEntries(
    text.split("\n").filter((l) => l.includes("=") && !l.startsWith("#")).map((l) => {
      const i = l.indexOf("=");
      return [l.slice(0, i).trim(), l.slice(i + 1).trim().replace(/^"|"$/g, "")];
    })
  );
}
function branchLE(merkleBE) {
  return merkleBE.map((h) => "0x" + Buffer.from(h, "hex").reverse().toString("hex"));
}

async function main() {
  const name = process.argv[2];
  const cfg = NETWORKS[name];
  if (!cfg) throw new Error("usage: node live_multi_network_fix_party.mjs <base|robinhood|hyperliquid>");
  const env = loadEnv(cfg.envDir);
  const provider = new ethers.JsonRpcProvider(env[cfg.rpcKey]);
  const wallet = new ethers.Wallet(env[cfg.pkKey], provider);
  const vaultArtifact = JSON.parse(fs.readFileSync(new URL(cfg.vaultArtifact, import.meta.url), "utf8"));
  const vault = new ethers.Contract(cfg.betaVault, vaultArtifact.abi, wallet);

  const startTxidLE = "0x" + Buffer.from(cfg.startTxidBE, "hex").reverse().toString("hex");

  const existing = await vault.parties(NEW_PARTY_ID);
  if (!existing.exists) {
    const approveTx = await vault.approveOperator(NEW_PARTY_ID, wallet.address);
    await approveTx.wait();
    console.log("approveOperator sig", approveTx.hash);
    const registerTx = await vault.registerParty(NEW_PARTY_ID, 0, startTxidLE, cfg.startVout, { value: ethers.parseEther("0.01") });
    await registerTx.wait();
    console.log("registerParty (fixed pointer) sig", registerTx.hash);
  } else {
    console.log("party 0x06 already registered here, skipping");
  }

  const tx = await vault.processAnchor(NEW_PARTY_ID, cfg.statement, cfg.txRaw, HEIGHT, branchLE(cfg.merkle), cfg.pos);
  const receipt = await tx.wait();
  console.log("processAnchor sig", tx.hash, "status", receipt.status);

  const lock = await vault.locks(cfg.lockId);
  console.log("lock state after processing:", lock.state.toString(), "(2 = Final)");
}

main().catch((e) => {
  console.error(e);
  process.exit(1);
});
