import { expect } from "chai";
import { network } from "hardhat";

import {
  GENESIS_HEADER_HEX,
  GENESIS_HEIGHT,
} from "./fixtures/bitcoinHeaders.ts";
import { BPS_DENOM, COMMIT_FEE_BPS, deployIPoWV1Split } from "./helpers/deploy.ts";

const { ethers } = await network.create();

// This file always deploys the split (router+facets) version directly —
// unlike every other test file here, which deploys whichever version
// IPOW_SPLIT_DEPLOY selects. It exists specifically to exercise the one
// piece of logic that's genuinely new in the split (not a byte-for-byte
// copy of the monolithic contract's own code): the Admin facet's header
// relay reaching into the Conversion Settlement facet's
// `_tryFinalizeProof` via an external self-call through the router,
// since an `internal` function can't cross a `delegatecall` boundary —
// see `contracts/iPoWV1Router.sol` and both facets' contract-level
// comments (DESIGN_V2.md §8.16).
//
// A full "a real Bitcoin header arrives and auto-finalizes an already-
// pending proof" integration test isn't included here: it would need a
// second genuinely-mined valid-PoW Bitcoin header chained onto genesis,
// and this suite deliberately only uses the real, independently-verified
// genesis header (see fixtures/bitcoinHeaders.ts's own comment) rather
// than fabricate one. That gap is pre-existing — no test anywhere in
// this repo exercises iPoWV1's own `submitBitcoinMerkleProofWithTx` +
// header-arrival retry path for the *monolithic* contract either. What's
// covered below instead is everything achievable without new PoW: the
// cross-facet entry point is reachable and correctly routed, is safe to
// call on a nonexistent proof, and a proof submitted for a height that
// hasn't arrived yet genuinely goes "pending" rather than any of the
// three facets mishandling it.
describe("iPoWV1Router: cross-facet dispatch (split-only)", function () {
  it("routes tryFinalizeProof to the Conversion Settlement facet and it safely no-ops with nothing pending", async function () {
    const [operator] = await ethers.getSigners();
    const c = await deployIPoWV1Split(operator);

    // txId 0 never existed — `_tryFinalizeProof`'s own guard (`!p.set`)
    // makes this a safe no-op, same as the monolithic contract's
    // internal call would be. The meaningful thing being checked here
    // is that the call *reaches* the Conversion Settlement facet at all
    // (an unrecognized selector reverts with `Unauthorized` per the
    // router's `fallback`) rather than what it does once there.
    const iface = new ethers.Interface(["function tryFinalizeProof(uint256)"]);
    const data = iface.encodeFunctionData("tryFinalizeProof", [0n]);
    await expect(operator.sendTransaction({ to: await c.getAddress(), data })).to.not.revert(ethers);
  });

  it("an unrecognized selector reverts, proving the router doesn't silently accept anything", async function () {
    const [operator] = await ethers.getSigners();
    const c = await deployIPoWV1Split(operator);

    const data = "0xdeadbeef"; // not a real selector on either facet
    await expect(operator.sendTransaction({ to: await c.getAddress(), data })).to.revert(ethers);
  });

  it("a proof submitted for a height that hasn't arrived yet is left genuinely pending, not finalized or reverted", async function () {
    const [operator, user] = await ethers.getSigners();
    const c = await deployIPoWV1Split(operator);

    const bitcoinAmount = 100_000n;
    const nativeAmount = ethers.parseEther("1");
    const paradappProgram = "0x76a914" + "22".repeat(20) + "88ac";

    await c.connect(operator).addNativeLiquidity({ value: nativeAmount });
    await c.connect(operator).commitGlobalBitcoinHeader80(GENESIS_HEADER_HEX, GENESIS_HEIGHT);

    const fee = (nativeAmount * COMMIT_FEE_BPS) / BPS_DENOM;
    const txId = await c.nextTxId();
    await c
      .connect(user)
      .commitBitcoinToNative(
        bitcoinAmount,
        nativeAmount,
        0n,
        "0x76a914" + "11".repeat(20) + "88ac",
        ethers.ZeroAddress,
        "0x",
        0n,
        "0x",
        0n,
        { value: fee }
      );
    await c.connect(operator).approveAndStartWithAnchorAndFirst(txId, 3600n, paradappProgram);

    // A well-formed but not-yet-mineable-to-verify raw tx paying the
    // watched script — real enough to parse, not claimed to be really
    // Merkle-included anywhere. Same construction as BitcoinPrimitives.
    // test.ts's "parses the value and scriptPubKey of a simple
    // one-output transaction" (a dummy single input is required: a
    // 0x00 inCount followed by a 0x01 outCount is indistinguishable
    // from SegWit marker+flag framing).
    const txRaw = ethers.concat([
      "0x01000000", // version = 1
      "0x01", // 1 input
      ethers.ZeroHash, // outpoint txid
      "0x00000000", // outpoint vout index
      "0x00", // scriptSig length = 0
      "0xffffffff", // sequence
      "0x01", // 1 output
      "0xa086010000000000", // value = 100000 sats, LE
      "0x19", // script length = 25 (0x19), matching paradappProgram's real length
      paradappProgram,
      "0x00000000", // locktime
    ]);

    // blockHeight = GENESIS_HEIGHT + 1: within the proof window, but no
    // header exists there yet — so this must go pending, not finalize,
    // and must not revert (the monolithic contract's `_tryFinalizeProof`
    // is deliberately a "soft" verifier that never reverts on an
    // unresolved proof; this split's copy in the Settlement facet keeps
    // that exactly).
    await expect(
      c.connect(user).submitBitcoinMerkleProofWithTx(txId, txRaw, 0n, ethers.ZeroHash, GENESIS_HEIGHT + 1, [], 0n)
    ).to.not.revert(ethers);

    const info = await c.proofInfo(txId);
    expect(info.set).to.equal(true);
    expect(info.verified).to.equal(false);
    expect(info.invalid).to.equal(false); // not yet judged either way — still pending

    const stored = await c.conversions(txId);
    expect(stored.completed).to.equal(false);
  });
});
