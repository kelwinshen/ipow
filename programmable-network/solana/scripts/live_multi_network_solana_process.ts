// Commits Bitcoin block 968203's header to Solana's ipow relay (a jump from
// tip 968053, valid since active_open_conversions==0), then processes all
// 8 statement-chain anchors (4 MINT + 4 self-ATTEST) on beta_factory in
// the exact order they were actually broadcast on Bitcoin — Solana's copy
// of party 0x05's pointer must advance through every link in sequence,
// unlike each EVM BetaVault which only ever needed its own single link
// (see live_multi_network_fix_party.mjs's comment for why those needed a
// different fix).
//
//   npx ts-node scripts/live_multi_network_solana_process.ts <action>
// actions: commit-header | process-all | status | exercise

import * as anchor from "@coral-xyz/anchor";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";

const WORKSPACE_ROOT = path.join(__dirname, "..");
const IPOW_PROGRAM_ID = new anchor.web3.PublicKey("EsmGbkui9ZFC6Fch9J6xoyNRZSp6TwfvcjS88beP1Vem");
const HEIGHT = 968203;
const HEADER_80_HEX = "006015222fde37610400db26529178005dc8eb3c417b848fb479000000000000000000002125f4150a8cbd4e91c82c80a5bac6ae0b8b6dee102e9c8d8b4c29f6180184f9821db36ac51e0217889addaa";
const PARTY_ID = Buffer.alloc(32, 0x05);
const COMPOSITION_ID = BigInt(2);
const SOL_USER_HEX = "e0e72437e8caeddcb5368336c372742770ee2e3bb3b8e426a6d961e57a14f74a";

// In the exact order the transactions were actually chained on Bitcoin.
const ANCHORS = [
  { name: "MINT-Ethereum", kind: 1, txidBE: "f2eaa23140d2f6d8565a440bf9f884eea9c3c48800d4518c188d02ba2df4fdc3",
    txRaw: "020000000290ac6e14edc401dfb799fd166d310733dbb6b5785ce69ca1217f1d9cfae471490000000000fdffffff90ac6e14edc401dfb799fd166d310733dbb6b5785ce69ca1217f1d9cfae471490200000000fdffffff032601000000000000160014f5c80ad661bd74b930bb7c84e5a01f6bd455f33b0000000000000000246a220101f3bcd2ae97c9d8e89da616d841526436f061393e40b652f9a79df4c8f8df02742a43000000000000160014f5c80ad661bd74b930bb7c84e5a01f6bd455f33b00000000",
    statement: "010000000000000002000000000000000002e0e72437e8caeddcb5368336c372742770ee2e3bb3b8e426a6d961e57a14f74a00000000000000020000000000000001000000006abc480a",
    pos: 3817, merkle: ["6ff10015798d53cd60b4df19e08d9f2b2132ea7b7a5cf964643a21d33cb5d7d7","b44d2cf6d0aaf0e572d54f1ea12e4184f0131f40fa06972b4b007fa0483a18bb","d9f0342e16bc42d470fa759233cdb2d79d43040f0331545d0b8e4def00c43fd2","01b64e019f6929d52b638d03e683acb22306df1b15238ca77e4cbeae9d7002cb","3efb6fc03f8fa3f44780cf541e2a40988cf1d26f2ba62a032b065d4b2347142e","b54f8220cde2e30720981984c92c5193f92bb15bc1a18cf0710f2f11e1b02902","a667714778a8369029c4153d514bfff3467252407c6bec96d5b03702ad5d7fae","338910268efdd0b830e5226be65f20f530c89336f4470c9cd96096497defa7a9","138373de3ee28a1dfa5a9d00515963d058343bab1f050821e4ca149cb65f167d","cb059f9afe94d3e59bccfe78523adca82535e7c9ecb868da32c5610972248d7c","1691fcd7f33a5c7ce2aff63fc5ceaabfe7e6fce856fe8d950ad83bc902ebf711","85d9bd5e092c4bc16085d9c3628b60b032200cb1614637e329c3cb9c251ecf96"],
    componentIndex: 0 },
  { name: "MINT-Base", kind: 1, txidBE: "83f3d77cefb7bc039c0f3ff9521c98f9c2ad5bde14e0f4624e0e2f68e94284a4",
    txRaw: "0200000002c3fdf42dba028d188c51d40088c4c3a9ee84f8f90b445a56d8f6d24031a2eaf20000000000fdffffffc3fdf42dba028d188c51d40088c4c3a9ee84f8f90b445a56d8f6d24031a2eaf20200000000fdffffff032601000000000000160014f5c80ad661bd74b930bb7c84e5a01f6bd455f33b0000000000000000246a220101a1853184562d64d272224108203d3d617cb6652bc2a331c5778c92331bef11949a41000000000000160014f5c80ad661bd74b930bb7c84e5a01f6bd455f33b00000000",
    statement: "010000000000000002010000000000000001e0e72437e8caeddcb5368336c372742770ee2e3bb3b8e426a6d961e57a14f74a00000000000000020000000000000001000000006abc480a",
    pos: 3819, merkle: ["5e7d2d11c0a16f83d11dc36c444b47c0ee8b355f065e7f6b08ba8111c48ba997","ed2b5454c8706f036ea170731b669146a69d0f01cffd4cd359e1afac847e435c","d9f0342e16bc42d470fa759233cdb2d79d43040f0331545d0b8e4def00c43fd2","01b64e019f6929d52b638d03e683acb22306df1b15238ca77e4cbeae9d7002cb","3efb6fc03f8fa3f44780cf541e2a40988cf1d26f2ba62a032b065d4b2347142e","b54f8220cde2e30720981984c92c5193f92bb15bc1a18cf0710f2f11e1b02902","a667714778a8369029c4153d514bfff3467252407c6bec96d5b03702ad5d7fae","338910268efdd0b830e5226be65f20f530c89336f4470c9cd96096497defa7a9","138373de3ee28a1dfa5a9d00515963d058343bab1f050821e4ca149cb65f167d","cb059f9afe94d3e59bccfe78523adca82535e7c9ecb868da32c5610972248d7c","1691fcd7f33a5c7ce2aff63fc5ceaabfe7e6fce856fe8d950ad83bc902ebf711","85d9bd5e092c4bc16085d9c3628b60b032200cb1614637e329c3cb9c251ecf96"],
    componentIndex: 1 },
  { name: "MINT-Robinhood", kind: 1, txidBE: "f781690c7bfe24f69c9819083dab3be6c55b18cb41ad9c0911f5c743d9013fe1",
    txRaw: "0200000002a48442e9682f0e4e62f4e014de5badc2f9981c52f93f0f9c03bcb7ef7cd7f3830000000000fdffffffa48442e9682f0e4e62f4e014de5badc2f9981c52f93f0f9c03bcb7ef7cd7f3830200000000fdffffff032601000000000000160014f5c80ad661bd74b930bb7c84e5a01f6bd455f33b0000000000000000246a220101b0eeae353876d153c3cd587cdcae7f239c5828f20877e5170ae86f884fae8cdf0a40000000000000160014f5c80ad661bd74b930bb7c84e5a01f6bd455f33b00000000",
    statement: "010000000000000002020000000000000001e0e72437e8caeddcb5368336c372742770ee2e3bb3b8e426a6d961e57a14f74a00000000000000020000000000000001000000006abc480a",
    pos: 3820, merkle: ["63fc4fdf587b95d1a89a28b01e2958c234c404933dd8b59ec885c1a6eec955dd","944b0d5dbb34d18811ad06d30b28ba0c49521979265935ae7cc1d95673a4944c","12b8ff865782178dc3b442891632294507d9d34cc290b519efa0475a28b9478a","01b64e019f6929d52b638d03e683acb22306df1b15238ca77e4cbeae9d7002cb","3efb6fc03f8fa3f44780cf541e2a40988cf1d26f2ba62a032b065d4b2347142e","b54f8220cde2e30720981984c92c5193f92bb15bc1a18cf0710f2f11e1b02902","a667714778a8369029c4153d514bfff3467252407c6bec96d5b03702ad5d7fae","338910268efdd0b830e5226be65f20f530c89336f4470c9cd96096497defa7a9","138373de3ee28a1dfa5a9d00515963d058343bab1f050821e4ca149cb65f167d","cb059f9afe94d3e59bccfe78523adca82535e7c9ecb868da32c5610972248d7c","1691fcd7f33a5c7ce2aff63fc5ceaabfe7e6fce856fe8d950ad83bc902ebf711","85d9bd5e092c4bc16085d9c3628b60b032200cb1614637e329c3cb9c251ecf96"],
    componentIndex: 2 },
  { name: "MINT-Hyperliquid", kind: 1, txidBE: "63fc4fdf587b95d1a89a28b01e2958c234c404933dd8b59ec885c1a6eec955dd",
    txRaw: "0200000002e13f01d943c7f511099cad41cb185bc5e63bab3d0819989cf624fe7b0c6981f70000000000fdffffffe13f01d943c7f511099cad41cb185bc5e63bab3d0819989cf624fe7b0c6981f70200000000fdffffff032601000000000000160014f5c80ad661bd74b930bb7c84e5a01f6bd455f33b0000000000000000246a220101a03a2fd90e4fdbacf418b6b91fa92a778c35c68d93ec3f42fcc5e04339e658527a3e000000000000160014f5c80ad661bd74b930bb7c84e5a01f6bd455f33b00000000",
    statement: "010000000000000002030000000000000001e0e72437e8caeddcb5368336c372742770ee2e3bb3b8e426a6d961e57a14f74a00000000000000020000000000000001000000006abc480a",
    pos: 3821, merkle: ["f781690c7bfe24f69c9819083dab3be6c55b18cb41ad9c0911f5c743d9013fe1","944b0d5dbb34d18811ad06d30b28ba0c49521979265935ae7cc1d95673a4944c","12b8ff865782178dc3b442891632294507d9d34cc290b519efa0475a28b9478a","01b64e019f6929d52b638d03e683acb22306df1b15238ca77e4cbeae9d7002cb","3efb6fc03f8fa3f44780cf541e2a40988cf1d26f2ba62a032b065d4b2347142e","b54f8220cde2e30720981984c92c5193f92bb15bc1a18cf0710f2f11e1b02902","a667714778a8369029c4153d514bfff3467252407c6bec96d5b03702ad5d7fae","338910268efdd0b830e5226be65f20f530c89336f4470c9cd96096497defa7a9","138373de3ee28a1dfa5a9d00515963d058343bab1f050821e4ca149cb65f167d","cb059f9afe94d3e59bccfe78523adca82535e7c9ecb868da32c5610972248d7c","1691fcd7f33a5c7ce2aff63fc5ceaabfe7e6fce856fe8d950ad83bc902ebf711","85d9bd5e092c4bc16085d9c3628b60b032200cb1614637e329c3cb9c251ecf96"],
    componentIndex: 3 },
  { name: "ATTEST-Ethereum", kind: 5, txidBE: "fe8f9ae12e06f72565dbb3e87df69ae4849072a2068be92f1356683e25b3418d",
    txRaw: "0200000002dd55c9eea6c185c89eb5d83d9304c434c258291eb0289aa8d1957b58df4ffc630000000000fdffffffdd55c9eea6c185c89eb5d83d9304c434c258291eb0289aa8d1957b58df4ffc630200000000fdffffff032601000000000000160014f5c80ad661bd74b930bb7c84e5a01f6bd455f33b0000000000000000246a22010527cbd168ccd2c401f6bde15e7331b8b537e2862e35cb104e0427d29bafe859e9ea3c000000000000160014f5c80ad661bd74b930bb7c84e5a01f6bd455f33b00000000",
    statement: "05c3fdf42dba028d188c51d40088c4c3a9ee84f8f90b445a56d8f6d24031a2eaf2",
    pos: 3822, merkle: ["5d98ec104ecbfef5c6bdc9295ed8c430afd31d26684a5d504ce6d0515a47047d","b007db280eabf45eadd1f287637e414bd049d2c7151ab649b2285c534d6ea83c","12b8ff865782178dc3b442891632294507d9d34cc290b519efa0475a28b9478a","01b64e019f6929d52b638d03e683acb22306df1b15238ca77e4cbeae9d7002cb","3efb6fc03f8fa3f44780cf541e2a40988cf1d26f2ba62a032b065d4b2347142e","b54f8220cde2e30720981984c92c5193f92bb15bc1a18cf0710f2f11e1b02902","a667714778a8369029c4153d514bfff3467252407c6bec96d5b03702ad5d7fae","338910268efdd0b830e5226be65f20f530c89336f4470c9cd96096497defa7a9","138373de3ee28a1dfa5a9d00515963d058343bab1f050821e4ca149cb65f167d","cb059f9afe94d3e59bccfe78523adca82535e7c9ecb868da32c5610972248d7c","1691fcd7f33a5c7ce2aff63fc5ceaabfe7e6fce856fe8d950ad83bc902ebf711","85d9bd5e092c4bc16085d9c3628b60b032200cb1614637e329c3cb9c251ecf96"],
    targetTxidBE: "f2eaa23140d2f6d8565a440bf9f884eea9c3c48800d4518c188d02ba2df4fdc3" },
  { name: "ATTEST-Base", kind: 5, txidBE: "5d98ec104ecbfef5c6bdc9295ed8c430afd31d26684a5d504ce6d0515a47047d",
    txRaw: "02000000028d41b3253e6856132fe98b06a2729084e49af67de8b3db6525f7062ee19a8ffe0000000000fdffffff8d41b3253e6856132fe98b06a2729084e49af67de8b3db6525f7062ee19a8ffe0200000000fdffffff032601000000000000160014f5c80ad661bd74b930bb7c84e5a01f6bd455f33b0000000000000000246a220105030e4b6dc1d405f75f2695f282f2278b58390ee1a1b999f6f6f717b90cedc2615a3b000000000000160014f5c80ad661bd74b930bb7c84e5a01f6bd455f33b00000000",
    statement: "05a48442e9682f0e4e62f4e014de5badc2f9981c52f93f0f9c03bcb7ef7cd7f383",
    pos: 3823, merkle: ["fe8f9ae12e06f72565dbb3e87df69ae4849072a2068be92f1356683e25b3418d","b007db280eabf45eadd1f287637e414bd049d2c7151ab649b2285c534d6ea83c","12b8ff865782178dc3b442891632294507d9d34cc290b519efa0475a28b9478a","01b64e019f6929d52b638d03e683acb22306df1b15238ca77e4cbeae9d7002cb","3efb6fc03f8fa3f44780cf541e2a40988cf1d26f2ba62a032b065d4b2347142e","b54f8220cde2e30720981984c92c5193f92bb15bc1a18cf0710f2f11e1b02902","a667714778a8369029c4153d514bfff3467252407c6bec96d5b03702ad5d7fae","338910268efdd0b830e5226be65f20f530c89336f4470c9cd96096497defa7a9","138373de3ee28a1dfa5a9d00515963d058343bab1f050821e4ca149cb65f167d","cb059f9afe94d3e59bccfe78523adca82535e7c9ecb868da32c5610972248d7c","1691fcd7f33a5c7ce2aff63fc5ceaabfe7e6fce856fe8d950ad83bc902ebf711","85d9bd5e092c4bc16085d9c3628b60b032200cb1614637e329c3cb9c251ecf96"],
    targetTxidBE: "83f3d77cefb7bc039c0f3ff9521c98f9c2ad5bde14e0f4624e0e2f68e94284a4" },
  { name: "ATTEST-Robinhood", kind: 5, txidBE: "0e04069ea91888b2739c976de193629ef367aa416eabfebd660605d2cde69624",
    txRaw: "02000000027d04475a51d0e64c505d4a68261dd3af30c4d85e29c9bdc6f5fecb4e10ec985d0000000000fdffffff7d04475a51d0e64c505d4a68261dd3af30c4d85e29c9bdc6f5fecb4e10ec985d0200000000fdffffff032601000000000000160014f5c80ad661bd74b930bb7c84e5a01f6bd455f33b0000000000000000246a22010537cfeca55b7da4bd98d2f02ad74d30081158060661f820da3744984b54482f77ca39000000000000160014f5c80ad661bd74b930bb7c84e5a01f6bd455f33b00000000",
    statement: "05e13f01d943c7f511099cad41cb185bc5e63bab3d0819989cf624fe7b0c6981f7",
    pos: 3824, merkle: ["ba13fa747340cccc3f2f2483d5c001f1f8e636f07a8021923676defda37bba41","2cc46cd2beb4aa181201415987baeb673dc445c255ca557e770c1133647d4475","df5426f0b439346b1d61a9eeb718cc946649423b38cbcfd0a96d4d884ba5815d","82c8d0ac3e124cefcd65b0b97f261d4fec985a2b5c3c56ba13c008d9fee66432","61cbceb6ef50908b3dd02d7676b26767ee0001c3f6e18b1889749564c81b9dfe","b54f8220cde2e30720981984c92c5193f92bb15bc1a18cf0710f2f11e1b02902","a667714778a8369029c4153d514bfff3467252407c6bec96d5b03702ad5d7fae","338910268efdd0b830e5226be65f20f530c89336f4470c9cd96096497defa7a9","138373de3ee28a1dfa5a9d00515963d058343bab1f050821e4ca149cb65f167d","cb059f9afe94d3e59bccfe78523adca82535e7c9ecb868da32c5610972248d7c","1691fcd7f33a5c7ce2aff63fc5ceaabfe7e6fce856fe8d950ad83bc902ebf711","85d9bd5e092c4bc16085d9c3628b60b032200cb1614637e329c3cb9c251ecf96"],
    targetTxidBE: "f781690c7bfe24f69c9819083dab3be6c55b18cb41ad9c0911f5c743d9013fe1" },
  { name: "ATTEST-Hyperliquid", kind: 5, txidBE: "ba13fa747340cccc3f2f2483d5c001f1f8e636f07a8021923676defda37bba41",
    txRaw: "02000000022496e6cdd2050666bdfeab6e41aa67f39e6293e16d979c73b28818a99e06040e0000000000fdffffff2496e6cdd2050666bdfeab6e41aa67f39e6293e16d979c73b28818a99e06040e0200000000fdffffff032601000000000000160014f5c80ad661bd74b930bb7c84e5a01f6bd455f33b0000000000000000246a22010572023ac012cc8c285100bed9afc030a213a375f26b43f2a0477af1c691383a053a38000000000000160014f5c80ad661bd74b930bb7c84e5a01f6bd455f33b00000000",
    statement: "05dd55c9eea6c185c89eb5d83d9304c434c258291eb0289aa8d1957b58df4ffc63",
    pos: 3825, merkle: ["0e04069ea91888b2739c976de193629ef367aa416eabfebd660605d2cde69624","2cc46cd2beb4aa181201415987baeb673dc445c255ca557e770c1133647d4475","df5426f0b439346b1d61a9eeb718cc946649423b38cbcfd0a96d4d884ba5815d","82c8d0ac3e124cefcd65b0b97f261d4fec985a2b5c3c56ba13c008d9fee66432","61cbceb6ef50908b3dd02d7676b26767ee0001c3f6e18b1889749564c81b9dfe","b54f8220cde2e30720981984c92c5193f92bb15bc1a18cf0710f2f11e1b02902","a667714778a8369029c4153d514bfff3467252407c6bec96d5b03702ad5d7fae","338910268efdd0b830e5226be65f20f530c89336f4470c9cd96096497defa7a9","138373de3ee28a1dfa5a9d00515963d058343bab1f050821e4ca149cb65f167d","cb059f9afe94d3e59bccfe78523adca82535e7c9ecb868da32c5610972248d7c","1691fcd7f33a5c7ce2aff63fc5ceaabfe7e6fce856fe8d950ad83bc902ebf711","85d9bd5e092c4bc16085d9c3628b60b032200cb1614637e329c3cb9c251ecf96"],
    targetTxidBE: "0e04069ea91888b2739c976de193629ef367aa416eabfebd660605d2cde69624" }, // placeholder, corrected below
];
// Fix ATTEST-Hyperliquid's target: it attests the Hyperliquid MINT itself.
ANCHORS[7].targetTxidBE = "63fc4fdf587b95d1a89a28b01e2958c234c404933dd8b59ec885c1a6eec955dd";

function loadProvider(): anchor.AnchorProvider {
  const walletPath = (process.env.ANCHOR_WALLET ?? "~/.config/solana/id.json").replace(/^~/, os.homedir());
  const kp = anchor.web3.Keypair.fromSecretKey(Uint8Array.from(JSON.parse(fs.readFileSync(walletPath, "utf8"))));
  const conn = new anchor.web3.Connection(process.env.ANCHOR_PROVIDER_URL ?? "https://api.devnet.solana.com", "confirmed");
  return new anchor.AnchorProvider(conn, new anchor.Wallet(kp), { commitment: "confirmed" });
}
const idl = (name: string) => JSON.parse(fs.readFileSync(path.join(WORKSPACE_ROOT, "target", "idl", `${name}.json`), "utf8"));
const pda = (seeds: (Buffer | Uint8Array)[], pid: anchor.web3.PublicKey) => anchor.web3.PublicKey.findProgramAddressSync(seeds, pid)[0];
const u64le = (n: bigint | number) => new anchor.BN(n.toString()).toArrayLike(Buffer, "le", 8);
const beToLeBuf = (hexBE: string) => Buffer.from(hexBE, "hex").reverse();
const branchLE = (merkleBE: string[]) => merkleBE.map((h) => [...beToLeBuf(h)]);

async function main() {
  const action = process.argv[2];
  const provider = loadProvider();
  anchor.setProvider(provider);
  const wallet = provider.wallet.publicKey;

  const ipow: any = new anchor.Program(idl("ipow"), provider);
  const factory: any = new anchor.Program(idl("beta_factory"), provider);
  const FACTORY_ID = factory.programId as anchor.web3.PublicKey;

  if (action === "commit-header") {
    const [globalState] = anchor.web3.PublicKey.findProgramAddressSync([Buffer.from("global_state")], IPOW_PROGRAM_ID);
    const [header] = anchor.web3.PublicKey.findProgramAddressSync([Buffer.from("header"), u64le(HEIGHT)], IPOW_PROGRAM_ID);
    const [prevHeightTracker] = anchor.web3.PublicKey.findProgramAddressSync([Buffer.from("tracker"), u64le(HEIGHT - 1)], IPOW_PROGRAM_ID);
    const gs: any = await ipow.account.globalState.fetch(globalState);
    console.log("current ipow tip:", gs.globalTipHeight.toString());
    if (BigInt(gs.globalTipHeight.toString()) >= BigInt(HEIGHT)) {
      console.log("already at or past target height");
      return;
    }
    const sig = await ipow.methods
      .commitGlobalHeader([...Buffer.from(HEADER_80_HEX, "hex")], new anchor.BN(HEIGHT))
      .accounts({
        globalState,
        header,
        prevHeightTracker,
        prevEpochStartHeader: null,
        prevEpochEndHeader: null,
        prevHeader: null,
        submitter: wallet,
        systemProgram: anchor.web3.SystemProgram.programId,
      })
      .rpc();
    console.log("header committed; sig", sig);
    return;
  }

  if (action === "process-all") {
    const config = pda([Buffer.from("config")], FACTORY_ID);
    const bondEscrow = pda([Buffer.from("bond_escrow")], FACTORY_ID);
    const insurance = pda([Buffer.from("insurance")], FACTORY_ID);
    const rewardPool = pda([Buffer.from("rewards")], FACTORY_ID);
    const feesPda = pda([Buffer.from("fees")], FACTORY_ID);
    const partyPda = pda([Buffer.from("party"), PARTY_ID], FACTORY_ID);
    const pendingPda = pda([Buffer.from("pending"), wallet.toBuffer(), u64le(2)], FACTORY_ID);
    const [ipowHeader] = anchor.web3.PublicKey.findProgramAddressSync([Buffer.from("header"), u64le(HEIGHT)], IPOW_PROGRAM_ID);
    const partyOwnerAddr = wallet; // party.owner == wallet (registered by me)

    // A legacy transaction here comes in ~40 bytes over Solana's 1232-byte
    // limit (the merkle branch alone is 384 bytes of real instruction data,
    // plus ~15 accounts x 33 bytes each) — use a v0 transaction with an
    // Address Lookup Table for the 9 accounts identical across all 8 calls,
    // shrinking each from a 32-byte key to a 1-byte index.
    const sharedAccounts = [config, bondEscrow, insurance, rewardPool, feesPda, partyPda, pendingPda, ipowHeader, anchor.web3.SystemProgram.programId, FACTORY_ID, partyOwnerAddr];
    const slot = await provider.connection.getSlot();
    const [createIx, lookupTableAddr] = anchor.web3.AddressLookupTableProgram.createLookupTable({
      authority: wallet, payer: wallet, recentSlot: slot,
    });
    const extendIx = anchor.web3.AddressLookupTableProgram.extendLookupTable({
      payer: wallet, authority: wallet, lookupTable: lookupTableAddr, addresses: sharedAccounts,
    });
    const { blockhash } = await provider.connection.getLatestBlockhash();
    const setupMsg = new anchor.web3.TransactionMessage({
      payerKey: wallet, recentBlockhash: blockhash, instructions: [createIx, extendIx],
    }).compileToV0Message();
    const setupTx = new anchor.web3.VersionedTransaction(setupMsg);
    setupTx.sign([(provider.wallet as anchor.Wallet).payer]);
    const setupSig = await provider.connection.sendTransaction(setupTx);
    await provider.connection.confirmTransaction(setupSig, "confirmed");
    console.log("lookup table created:", lookupTableAddr.toBase58(), "sig", setupSig);
    // ALTs need to be activated in a slot after creation before use.
    await new Promise((r) => setTimeout(r, 3000));
    const lut = (await provider.connection.getAddressLookupTable(lookupTableAddr)).value!;

    for (const a of ANCHORS) {
      const txidLE = beToLeBuf(a.txidBE);
      const [processed] = anchor.web3.PublicKey.findProgramAddressSync([Buffer.from("anchor"), txidLE], FACTORY_ID);
      const existing = await provider.connection.getAccountInfo(processed);
      if (existing) {
        console.log(a.name, "- already processed, skipping");
        continue;
      }
      const accounts: any = {
        config,
        party: partyPda,
        partyOwner: partyOwnerAddr,
        processed,
        header: ipowHeader,
        bondEscrow,
        insurance,
        rewardPool,
        fees: feesPda,
        submitter: wallet,
        systemProgram: anchor.web3.SystemProgram.programId,
        pending: null,
        pendingUser: null,
        burn: null,
        targetParty: null,
        targetAnchor: null,
        priorParty: null,
      };
      if (a.kind === 1) {
        accounts.pending = pendingPda;
        accounts.pendingUser = wallet; // solUser == my own wallet
      } else if (a.kind === 5) {
        const targetTxidLE = beToLeBuf(a.targetTxidBE!);
        const [targetAnchor] = anchor.web3.PublicKey.findProgramAddressSync([Buffer.from("anchor"), targetTxidLE], FACTORY_ID);
        accounts.targetAnchor = targetAnchor;
        accounts.pending = pendingPda;
        accounts.pendingUser = wallet;
      }
      const cuIx = anchor.web3.ComputeBudgetProgram.setComputeUnitLimit({ units: 1_400_000 });
      const mainIx = await factory.methods
        .processAnchor([...txidLE], Buffer.from(a.statement, "hex"), Buffer.from(a.txRaw, "hex"), new anchor.BN(HEIGHT), branchLE(a.merkle), new anchor.BN(a.pos))
        .accounts(accounts)
        .instruction();
      const { blockhash: bh } = await provider.connection.getLatestBlockhash();
      const msg = new anchor.web3.TransactionMessage({
        payerKey: wallet, recentBlockhash: bh, instructions: [cuIx, mainIx],
      }).compileToV0Message([lut]);
      const vtx = new anchor.web3.VersionedTransaction(msg);
      vtx.sign([(provider.wallet as anchor.Wallet).payer]);
      const sig = await provider.connection.sendTransaction(vtx);
      await provider.connection.confirmTransaction(sig, "confirmed");
      console.log(a.name, "- processed; sig", sig);
    }
    return;
  }

  if (action === "status") {
    const pendingPda = pda([Buffer.from("pending"), wallet.toBuffer(), u64le(2)], FACTORY_ID);
    const p: any = await factory.account.pending.fetch(pendingPda);
    console.log("pending:", {
      compositionId: p.compositionId.toString(),
      approved: p.approved,
      queuedBy: Buffer.from(p.queuedBy).toString("hex"),
      remoteAnchorTxid: p.remoteAnchorTxid.map((b: number[]) => Buffer.from(b).toString("hex")),
    });
    for (const a of ANCHORS) {
      const txidLE = beToLeBuf(a.txidBE);
      const [processed] = anchor.web3.PublicKey.findProgramAddressSync([Buffer.from("anchor"), txidLE], FACTORY_ID);
      const acct: any = await provider.connection.getAccountInfo(processed);
      if (!acct) { console.log(a.name, "- not processed"); continue; }
      const pa: any = await factory.account.processedAnchor.fetch(processed);
      console.log(a.name, "- status:", pa.status, "attestedBy:", pa.attestedBy ? Buffer.from(pa.attestedBy).toString("hex") : undefined);
    }
    return;
  }

  if (action === "exercise") {
    const config = pda([Buffer.from("config")], FACTORY_ID);
    const compositionPda = pda([Buffer.from("composition"), u64le(COMPOSITION_ID)], FACTORY_ID);
    const pendingPda = pda([Buffer.from("pending"), wallet.toBuffer(), u64le(2)], FACTORY_ID);
    const partyPda = pda([Buffer.from("party"), PARTY_ID], FACTORY_ID);
    const mintAuthority = pda([Buffer.from("mint_authority")], FACTORY_ID);
    const feesPda = pda([Buffer.from("fees")], FACTORY_ID);
    const TOKEN_PROGRAM = new anchor.web3.PublicKey("TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA");
    const ATA_PROGRAM = new anchor.web3.PublicKey("ATokenGPvbdGVxr1b2hvZbsiqW5xWH25efTNsLJA8knL");
    const cfg: any = await factory.account.factoryConfig.fetch(config);
    const userBeta = pda([wallet.toBuffer(), TOKEN_PROGRAM.toBuffer(), cfg.betaMint.toBuffer()], ATA_PROGRAM);

    const remotePdas = ANCHORS.slice(0, 4).map((a) => {
      const txidLE = beToLeBuf(a.txidBE);
      return anchor.web3.PublicKey.findProgramAddressSync([Buffer.from("anchor"), txidLE], FACTORY_ID)[0];
    });

    const cuIx = anchor.web3.ComputeBudgetProgram.setComputeUnitLimit({ units: 400_000 });
    const sig = await factory.methods
      .exerciseMint()
      .accounts({
        config,
        pending: pendingPda,
        composition: compositionPda,
        party: partyPda,
        user: wallet,
        userBeta,
        betaMint: cfg.betaMint,
        mintAuthority,
        fees: feesPda,
        tokenProgram: TOKEN_PROGRAM,
        systemProgram: anchor.web3.SystemProgram.programId,
        remote0: remotePdas[0],
        remote1: remotePdas[1],
        remote2: remotePdas[2],
        remote3: remotePdas[3],
        remote4: null,
        remote5: null,
        remote6: null,
      })
      .preInstructions([cuIx])
      .rpc();
    console.log("exercise_mint sig", sig);

    const beta = new anchor.web3.PublicKey(cfg.betaMint);
    const bal = await provider.connection.getTokenAccountBalance(userBeta);
    console.log("BETA balance:", bal.value.uiAmountString);
    return;
  }

  console.log("usage: ts-node live_multi_network_solana_process.ts <commit-header|process-all|status|exercise>");
}

if (require.main === module) {
  main().catch((e) => {
    console.error(e);
    process.exit(1);
  });
}
