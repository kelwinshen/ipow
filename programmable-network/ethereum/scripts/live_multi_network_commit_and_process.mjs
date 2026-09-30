// Commits the real Bitcoin block 968203 header to a given EVM chain's
// iPoW relay, then calls processAnchor for that chain's own MINT leg.
//
//   node live_multi_network_commit_and_process.mjs <network>
import { ethers } from "ethers";
import fs from "node:fs";

const HEADER_80 = "0x006015222fde37610400db26529178005dc8eb3c417b848fb479000000000000000000002125f4150a8cbd4e91c82c80a5bac6ae0b8b6dee102e9c8d8b4c29f6180184f9821db36ac51e0217889addaa";
const HEIGHT = 968203n;
const PARTY_ID = "0x" + "05".repeat(32);

// pos/merkle from blockstream.info's merkle-proof endpoint, verified
// offline against the block's real merkle_root before use. Each leaf
// position has its OWN distinct sibling path — an earlier version of this
// script mistakenly reused Ethereum's (pos 3817) proof for every leg,
// which only happened to work for Ethereum itself; reverted with
// InvalidMerkleBranch for Base (caught before it could do any real harm —
// the call reverts atomically, no partial state).
const MERKLE_BE = {
  ethereum: { pos: 3817, merkle: ["6ff10015798d53cd60b4df19e08d9f2b2132ea7b7a5cf964643a21d33cb5d7d7","b44d2cf6d0aaf0e572d54f1ea12e4184f0131f40fa06972b4b007fa0483a18bb","d9f0342e16bc42d470fa759233cdb2d79d43040f0331545d0b8e4def00c43fd2","01b64e019f6929d52b638d03e683acb22306df1b15238ca77e4cbeae9d7002cb","3efb6fc03f8fa3f44780cf541e2a40988cf1d26f2ba62a032b065d4b2347142e","b54f8220cde2e30720981984c92c5193f92bb15bc1a18cf0710f2f11e1b02902","a667714778a8369029c4153d514bfff3467252407c6bec96d5b03702ad5d7fae","338910268efdd0b830e5226be65f20f530c89336f4470c9cd96096497defa7a9","138373de3ee28a1dfa5a9d00515963d058343bab1f050821e4ca149cb65f167d","cb059f9afe94d3e59bccfe78523adca82535e7c9ecb868da32c5610972248d7c","1691fcd7f33a5c7ce2aff63fc5ceaabfe7e6fce856fe8d950ad83bc902ebf711","85d9bd5e092c4bc16085d9c3628b60b032200cb1614637e329c3cb9c251ecf96"] },
  base: { pos: 3819, merkle: ["5e7d2d11c0a16f83d11dc36c444b47c0ee8b355f065e7f6b08ba8111c48ba997","ed2b5454c8706f036ea170731b669146a69d0f01cffd4cd359e1afac847e435c","d9f0342e16bc42d470fa759233cdb2d79d43040f0331545d0b8e4def00c43fd2","01b64e019f6929d52b638d03e683acb22306df1b15238ca77e4cbeae9d7002cb","3efb6fc03f8fa3f44780cf541e2a40988cf1d26f2ba62a032b065d4b2347142e","b54f8220cde2e30720981984c92c5193f92bb15bc1a18cf0710f2f11e1b02902","a667714778a8369029c4153d514bfff3467252407c6bec96d5b03702ad5d7fae","338910268efdd0b830e5226be65f20f530c89336f4470c9cd96096497defa7a9","138373de3ee28a1dfa5a9d00515963d058343bab1f050821e4ca149cb65f167d","cb059f9afe94d3e59bccfe78523adca82535e7c9ecb868da32c5610972248d7c","1691fcd7f33a5c7ce2aff63fc5ceaabfe7e6fce856fe8d950ad83bc902ebf711","85d9bd5e092c4bc16085d9c3628b60b032200cb1614637e329c3cb9c251ecf96"] },
  robinhood: { pos: 3820, merkle: ["63fc4fdf587b95d1a89a28b01e2958c234c404933dd8b59ec885c1a6eec955dd","944b0d5dbb34d18811ad06d30b28ba0c49521979265935ae7cc1d95673a4944c","12b8ff865782178dc3b442891632294507d9d34cc290b519efa0475a28b9478a","01b64e019f6929d52b638d03e683acb22306df1b15238ca77e4cbeae9d7002cb","3efb6fc03f8fa3f44780cf541e2a40988cf1d26f2ba62a032b065d4b2347142e","b54f8220cde2e30720981984c92c5193f92bb15bc1a18cf0710f2f11e1b02902","a667714778a8369029c4153d514bfff3467252407c6bec96d5b03702ad5d7fae","338910268efdd0b830e5226be65f20f530c89336f4470c9cd96096497defa7a9","138373de3ee28a1dfa5a9d00515963d058343bab1f050821e4ca149cb65f167d","cb059f9afe94d3e59bccfe78523adca82535e7c9ecb868da32c5610972248d7c","1691fcd7f33a5c7ce2aff63fc5ceaabfe7e6fce856fe8d950ad83bc902ebf711","85d9bd5e092c4bc16085d9c3628b60b032200cb1614637e329c3cb9c251ecf96"] },
  hyperliquid: { pos: 3821, merkle: ["f781690c7bfe24f69c9819083dab3be6c55b18cb41ad9c0911f5c743d9013fe1","944b0d5dbb34d18811ad06d30b28ba0c49521979265935ae7cc1d95673a4944c","12b8ff865782178dc3b442891632294507d9d34cc290b519efa0475a28b9478a","01b64e019f6929d52b638d03e683acb22306df1b15238ca77e4cbeae9d7002cb","3efb6fc03f8fa3f44780cf541e2a40988cf1d26f2ba62a032b065d4b2347142e","b54f8220cde2e30720981984c92c5193f92bb15bc1a18cf0710f2f11e1b02902","a667714778a8369029c4153d514bfff3467252407c6bec96d5b03702ad5d7fae","338910268efdd0b830e5226be65f20f530c89336f4470c9cd96096497defa7a9","138373de3ee28a1dfa5a9d00515963d058343bab1f050821e4ca149cb65f167d","cb059f9afe94d3e59bccfe78523adca82535e7c9ecb868da32c5610972248d7c","1691fcd7f33a5c7ce2aff63fc5ceaabfe7e6fce856fe8d950ad83bc902ebf711","85d9bd5e092c4bc16085d9c3628b60b032200cb1614637e329c3cb9c251ecf96"] },
};

const NETWORKS = {
  ethereum: {
    envDir: "../", rpcKey: "SEPOLIA_RPC_URL", pkKey: "SEPOLIA_PRIVATE_KEY",
    ipow: "0xB8ab960D1121F33B48b4086aBFCDD8B750081588", betaVault: "0x30A386AEc5aD0afAcC6C8624b47ABbB719596a6b",
    ipowArtifact: "../artifacts/contracts/iPoW.sol/iPoW.json", vaultArtifact: "../artifacts/contracts/BetaVault.sol/BetaVault.json",
    pos: 3817,
    txRaw: "0x020000000290ac6e14edc401dfb799fd166d310733dbb6b5785ce69ca1217f1d9cfae471490000000000fdffffff90ac6e14edc401dfb799fd166d310733dbb6b5785ce69ca1217f1d9cfae471490200000000fdffffff032601000000000000160014f5c80ad661bd74b930bb7c84e5a01f6bd455f33b0000000000000000246a220101f3bcd2ae97c9d8e89da616d841526436f061393e40b652f9a79df4c8f8df02742a43000000000000160014f5c80ad661bd74b930bb7c84e5a01f6bd455f33b00000000",
    txidBE: "f2eaa23140d2f6d8565a440bf9f884eea9c3c48800d4518c188d02ba2df4fdc3",
    statement: "0x010000000000000002000000000000000002e0e72437e8caeddcb5368336c372742770ee2e3bb3b8e426a6d961e57a14f74a00000000000000020000000000000001000000006abc480a",
  },
  base: {
    envDir: "../../base", rpcKey: "BASE_SEPOLIA_RPC_URL", pkKey: "BASE_SEPOLIA_PRIVATE_KEY",
    ipow: "0xB7054E399E31A2cFE181c4fD59C7235562a6d45d", betaVault: "0x6354779b4Dbb564c712ea91c179eCF521C15BE73",
    ipowArtifact: "../../base/artifacts/contracts/iPoW.sol/iPoW.json", vaultArtifact: "../../base/artifacts/contracts/BetaVault.sol/BetaVault.json",
    pos: 3819,
    txRaw: "0x0200000002c3fdf42dba028d188c51d40088c4c3a9ee84f8f90b445a56d8f6d24031a2eaf20000000000fdffffffc3fdf42dba028d188c51d40088c4c3a9ee84f8f90b445a56d8f6d24031a2eaf20200000000fdffffff032601000000000000160014f5c80ad661bd74b930bb7c84e5a01f6bd455f33b0000000000000000246a220101a1853184562d64d272224108203d3d617cb6652bc2a331c5778c92331bef11949a41000000000000160014f5c80ad661bd74b930bb7c84e5a01f6bd455f33b00000000",
    txidBE: "83f3d77cefb7bc039c0f3ff9521c98f9c2ad5bde14e0f4624e0e2f68e94284a4",
    statement: "0x010000000000000002010000000000000001e0e72437e8caeddcb5368336c372742770ee2e3bb3b8e426a6d961e57a14f74a00000000000000020000000000000001000000006abc480a",
  },
  robinhood: {
    envDir: "../../robinhood", rpcKey: "ROBINHOOD_TESTNET_RPC_URL", pkKey: "ROBINHOOD_TESTNET_PRIVATE_KEY",
    ipow: "0x53e1291BdAff473694BbbB8DD257f9844e5f9F3c", betaVault: "0x35e564d74B90a3A5bfcA8Dec65b1325C83d2e822",
    ipowArtifact: "../../robinhood/artifacts/contracts/iPoW.sol/iPoW.json", vaultArtifact: "../../robinhood/artifacts/contracts/BetaVault.sol/BetaVault.json",
    pos: 3820,
    txRaw: "0x0200000002a48442e9682f0e4e62f4e014de5badc2f9981c52f93f0f9c03bcb7ef7cd7f3830000000000fdffffffa48442e9682f0e4e62f4e014de5badc2f9981c52f93f0f9c03bcb7ef7cd7f3830200000000fdffffff032601000000000000160014f5c80ad661bd74b930bb7c84e5a01f6bd455f33b0000000000000000246a220101b0eeae353876d153c3cd587cdcae7f239c5828f20877e5170ae86f884fae8cdf0a40000000000000160014f5c80ad661bd74b930bb7c84e5a01f6bd455f33b00000000",
    txidBE: "f781690c7bfe24f69c9819083dab3be6c55b18cb41ad9c0911f5c743d9013fe1",
    statement: "0x010000000000000002020000000000000001e0e72437e8caeddcb5368336c372742770ee2e3bb3b8e426a6d961e57a14f74a00000000000000020000000000000001000000006abc480a",
  },
  hyperliquid: {
    envDir: "../../hyperliquid", rpcKey: "HYPEREVM_TESTNET_RPC_URL", pkKey: "HYPEREVM_TESTNET_PRIVATE_KEY",
    ipow: "0xB7054E399E31A2cFE181c4fD59C7235562a6d45d", betaVault: "0x554Fe13e4a5d0931e7c8F7d3E74Dea8Ca04C244a",
    ipowArtifact: "../../hyperliquid/artifacts/contracts/iPoW.sol/iPoW.json", vaultArtifact: "../../hyperliquid/artifacts/contracts/BetaVault.sol/BetaVault.json",
    pos: 3821,
    txRaw: "0x0200000002e13f01d943c7f511099cad41cb185bc5e63bab3d0819989cf624fe7b0c6981f70000000000fdffffffe13f01d943c7f511099cad41cb185bc5e63bab3d0819989cf624fe7b0c6981f70200000000fdffffff032601000000000000160014f5c80ad661bd74b930bb7c84e5a01f6bd455f33b0000000000000000246a220101a03a2fd90e4fdbacf418b6b91fa92a778c35c68d93ec3f42fcc5e04339e658527a3e000000000000160014f5c80ad661bd74b930bb7c84e5a01f6bd455f33b00000000",
    txidBE: "63fc4fdf587b95d1a89a28b01e2958c234c404933dd8b59ec885c1a6eec955dd",
    statement: "0x010000000000000002030000000000000001e0e72437e8caeddcb5368336c372742770ee2e3bb3b8e426a6d961e57a14f74a00000000000000020000000000000001000000006abc480a",
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
  if (!cfg) throw new Error("usage: node live_multi_network_commit_and_process.mjs <ethereum|base|robinhood|hyperliquid>");
  const env = loadEnv(cfg.envDir);
  const provider = new ethers.JsonRpcProvider(env[cfg.rpcKey]);
  const wallet = new ethers.Wallet(env[cfg.pkKey], provider);
  const ipowArtifact = JSON.parse(fs.readFileSync(new URL(cfg.ipowArtifact, import.meta.url), "utf8"));
  const vaultArtifact = JSON.parse(fs.readFileSync(new URL(cfg.vaultArtifact, import.meta.url), "utf8"));
  const ipow = new ethers.Contract(cfg.ipow, ipowArtifact.abi, wallet);
  const vault = new ethers.Contract(cfg.betaVault, vaultArtifact.abi, wallet);

  const tip = await ipow.globalTipHeight();
  console.log(name, "current tip:", tip.toString());
  if (tip < HEIGHT) {
    const tx = await ipow.commitGlobalBitcoinHeader80(HEADER_80, HEIGHT);
    await tx.wait();
    console.log("committed header at", HEIGHT.toString(), "sig", tx.hash);
  } else {
    console.log("header already committed (or a later tip exists)");
  }

  const txidLE = "0x" + Buffer.from(cfg.txidBE, "hex").reverse().toString("hex");
  const anchor = await vault.anchors(txidLE);
  if (anchor.status != 0n) {
    console.log("anchor already processed, status:", anchor.status.toString());
  } else {
    const tx = await vault.processAnchor(PARTY_ID, cfg.statement, cfg.txRaw, HEIGHT, branchLE(MERKLE_BE[name].merkle), MERKLE_BE[name].pos);
    const receipt = await tx.wait();
    console.log("processAnchor sig", tx.hash, "status", receipt.status);
  }

  const lock = await vault.locks(name === "ethereum" ? 2 : 1);
  console.log("lock state after processing:", lock.state.toString(), "(2 = Final)");
}

main().catch((e) => {
  console.error(e);
  process.exit(1);
});
