import { expect } from "chai";
import { network } from "hardhat";

import {
  BLOCK1_HEADER_HEX,
  GENESIS_HEADER_HEX,
  GENESIS_HEIGHT,
  computeHeaderHashLE,
} from "./fixtures/bitcoinHeaders.ts";
import { BPS_DENOM, COMMIT_FEE_BPS, deployIPoWV1 } from "./helpers/deploy.ts";

const { ethers } = await network.create();

describe("iPoWV1: commitGlobalBitcoinHeader80", function () {
  it("reverts Unauthorized when called by a non-operator", async function () {
    const [operator, stranger] = await ethers.getSigners();
    const c = await deployIPoWV1(operator);

    await expect(
      c
        .connect(stranger)
        .commitGlobalBitcoinHeader80(GENESIS_HEADER_HEX, GENESIS_HEIGHT)
    ).to.be.revertedWithCustomError(c, "Unauthorized");
  });

  it("reverts InvalidHeader when the header isn't exactly 80 bytes", async function () {
    const [operator] = await ethers.getSigners();
    const c = await deployIPoWV1(operator);
    const tooShort = GENESIS_HEADER_HEX.slice(0, -2);

    await expect(
      c.connect(operator).commitGlobalBitcoinHeader80(tooShort, GENESIS_HEIGHT)
    ).to.be.revertedWithCustomError(c, "InvalidHeader");
  });

  it("reverts LowWork when the header's hash doesn't satisfy its own encoded target", async function () {
    const [operator] = await ethers.getSigners();
    const c = await deployIPoWV1(operator);
    // Flip the last nonce byte of the real genesis header — breaks the PoW solution
    // while keeping the same declared difficulty (bits), so LowWork is the only
    // possible revert reason (the header is still a well-formed 80 bytes).
    const tampered = GENESIS_HEADER_HEX.slice(0, -2) + "00";

    await expect(
      c.connect(operator).commitGlobalBitcoinHeader80(tampered, GENESIS_HEIGHT)
    ).to.be.revertedWithCustomError(c, "LowWork");
  });

  it("accepts the real genesis header, updates state, and emits GlobalHeaderAppended", async function () {
    const [operator] = await ethers.getSigners();
    const c = await deployIPoWV1(operator);
    const expectedHashLE = computeHeaderHashLE(GENESIS_HEADER_HEX);

    await expect(
      c
        .connect(operator)
        .commitGlobalBitcoinHeader80(GENESIS_HEADER_HEX, GENESIS_HEIGHT)
    ).to.emit(c, "GlobalHeaderAppended");

    expect(await c.globalHeightToHashLE(GENESIS_HEIGHT)).to.equal(
      expectedHashLE
    );
    expect(await c.globalTipHeight()).to.equal(GENESIS_HEIGHT);

    const meta = await c.globalHeaders(expectedHashLE);
    expect(meta.set).to.equal(true);
    expect(meta.nBits).to.equal(0x1d00ffffn);
    expect(meta.timestamp).to.equal(1231006505n);
  });

  it("allows idempotent resubmission of the identical header at the same height", async function () {
    const [operator] = await ethers.getSigners();
    const c = await deployIPoWV1(operator);

    await c
      .connect(operator)
      .commitGlobalBitcoinHeader80(GENESIS_HEADER_HEX, GENESIS_HEIGHT);
    // Resubmitting the exact same bytes at the same height must not revert
    // (HeightRewrite only fires for a *different* hash at an already-set height).
    await expect(
      c
        .connect(operator)
        .commitGlobalBitcoinHeader80(GENESIS_HEADER_HEX, GENESIS_HEIGHT)
    ).to.not.revert(ethers);
    expect(await c.globalTipHeight()).to.equal(GENESIS_HEIGHT);
  });
});

describe("iPoWV1: approveAndStartWithAnchorAndFirst (integration, using the genesis header)", function () {
  const bitcoinAmount = 100_000n;
  const nativeAmount = ethers.parseEther("1");
  const paradappProgram = "0x76a914" + "22".repeat(20) + "88ac";

  async function commitUserBitcoinToNative(c: any, user: any) {
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
    return txId;
  }

  it("opens the header window and reserves liquidity once a real header is available at the tip", async function () {
    const [operator, user] = await ethers.getSigners();
    const c = await deployIPoWV1(operator);

    // Fund the protocol so it can reserve the full native payout (100% RESERVE_MARGIN_BPS).
    await c.connect(operator).addNativeLiquidity({ value: nativeAmount });
    // A real header must exist at the tip before a window can be anchored.
    await c
      .connect(operator)
      .commitGlobalBitcoinHeader80(GENESIS_HEADER_HEX, GENESIS_HEIGHT);

    const txId = await commitUserBitcoinToNative(c, user);

    await expect(
      c
        .connect(operator)
        .approveAndStartWithAnchorAndFirst(txId, 3600n, paradappProgram)
    ).to.emit(c, "ConversionApproved");

    const stored = await c.conversions(txId);
    expect(stored.approved).to.equal(true);
    expect(stored.reservedNative).to.equal(nativeAmount);
    expect(await c.totalReservedNative()).to.equal(nativeAmount);

    const anchor = await c.anchorInfo(txId);
    expect(anchor.anchorHeight).to.equal(GENESIS_HEIGHT);
  });

  it("reverts NoHeadersYet-adjacent GlobalFirstHeaderMissing when no header has ever been submitted", async function () {
    const [operator, user] = await ethers.getSigners();
    const c = await deployIPoWV1(operator);
    await c.connect(operator).addNativeLiquidity({ value: nativeAmount });

    const txId = await commitUserBitcoinToNative(c, user);

    await expect(
      c
        .connect(operator)
        .approveAndStartWithAnchorAndFirst(txId, 3600n, paradappProgram)
    ).to.be.revertedWithCustomError(c, "GlobalFirstHeaderMissing");
  });

  it("reverts LowReserve when the protocol lacks sufficient liquidity to reserve the payout", async function () {
    const [operator, user] = await ethers.getSigners();
    const c = await deployIPoWV1(operator);
    await c
      .connect(operator)
      .commitGlobalBitcoinHeader80(GENESIS_HEADER_HEX, GENESIS_HEIGHT);
    // No addNativeLiquidity call — nothing available to reserve.

    const txId = await commitUserBitcoinToNative(c, user);

    await expect(
      c
        .connect(operator)
        .approveAndStartWithAnchorAndFirst(txId, 3600n, paradappProgram)
    ).to.be.revertedWithCustomError(c, "LowReserve");
  });
});

describe("iPoWV1: permissionless header fallback (§6.9)", function () {
  it("a stranger may extend only a stale tip, only by one, only with a linked header", async function () {
    const [operator, stranger] = await ethers.getSigners();
    const c = await deployIPoWV1(operator);
    await c.connect(operator).commitGlobalBitcoinHeader80(GENESIS_HEADER_HEX, GENESIS_HEIGHT);

    // Fresh tip: refused.
    await expect(c.connect(stranger).commitGlobalBitcoinHeader80(BLOCK1_HEADER_HEX, 1)).to.be.revertedWithCustomError(c, "HeaderNotStale");
    await ethers.provider.send("evm_increaseTime", [30 * 60 + 1]);
    await ethers.provider.send("evm_mine", []);
    // Stale tip but a jump: still operator-only.
    await expect(c.connect(stranger).commitGlobalBitcoinHeader80(BLOCK1_HEADER_HEX, 5)).to.be.revertedWithCustomError(c, "Unauthorized");
    // Stale tip, unlinked header (genesis again at height 1): refused.
    await expect(c.connect(stranger).commitGlobalBitcoinHeader80(GENESIS_HEADER_HEX, 1)).to.be.revertedWithCustomError(c, "PrevAndTipUnmatch");
    // Real block 1 links: accepted from a stranger.
    await expect(c.connect(stranger).commitGlobalBitcoinHeader80(BLOCK1_HEADER_HEX, 1)).to.emit(c, "GlobalHeaderAppended");
    expect(await c.globalTipHeight()).to.equal(1n);
    expect(await c.globalHeightToHashLE(1)).to.equal(computeHeaderHashLE(BLOCK1_HEADER_HEX));
  });

  it("the operator's own extensions must link to the tip too", async function () {
    const [operator] = await ethers.getSigners();
    const c = await deployIPoWV1(operator);
    await c.connect(operator).commitGlobalBitcoinHeader80(GENESIS_HEADER_HEX, GENESIS_HEIGHT);
    await expect(c.connect(operator).commitGlobalBitcoinHeader80(GENESIS_HEADER_HEX, 1)).to.be.revertedWithCustomError(c, "PrevAndTipUnmatch");
    await expect(c.connect(operator).commitGlobalBitcoinHeader80(BLOCK1_HEADER_HEX, 1)).to.emit(c, "GlobalHeaderAppended");
  });
});
