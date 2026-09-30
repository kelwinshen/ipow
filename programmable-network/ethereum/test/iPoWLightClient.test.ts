import { expect } from "chai";
import { network } from "hardhat";

import {
  BLOCK1_HEADER_HEX,
  GENESIS_HEADER_HEX,
} from "./fixtures/bitcoinHeaders.ts";
import { MAINNET_HEADERS, MAINNET_TX } from "./fixtures/mainnetHeaders.ts";
import {
  EPOCH_BLOCKS,
  concat,
  headerBits,
  headerHashLE,
  headerTime,
  mine,
  mineChain,
  retarget,
  reverseHex,
  targetFromBits,
} from "./helpers/bitcoin.ts";

const { ethers } = await network.create();

// Spec: docs/design/ipow-protocol.md, section 2. The D numbers are its decisions.

const HOUR = 3600;
const WEEK = 7 * 24 * HOUR;
const ZERO_HASH = "0x" + "00".repeat(32);

function range(from: number, to: number): string[] {
  const out: string[] = [];
  for (let h = from; h <= to; h++) out.push(MAINNET_HEADERS[h]);
  return out;
}

async function deploy() {
  return ethers.deployContract("iPoWLightClientHarness", [0]);
}

describe("iPoWLightClient: real Bitcoin blocks, real rules", function () {
  // Epoch 480 starts at height 967680.
  const E480 = 967680;
  const E479 = 965664;
  const es480 = range(E480, E480 + 5);
  const es479 = range(E479, E479 + 5);
  const es480Time = headerTime(MAINNET_HEADERS[E480]);
  const es479Time = headerTime(MAINNET_HEADERS[E479]);

  async function withEpochStart480() {
    const c = await deploy();
    await c.setNow(headerTime(MAINNET_HEADERS[E480 + 5]) + 600);
    await c.addEpochStart(concat(es480), E480);
    const esId = await c.nodeId(
      headerHashLE(MAINNET_HEADERS[E480]),
      E480,
      es480Time
    );
    return { c, esId };
  }

  describe("epoch start (D44)", function () {
    it("records the first 6 blocks of an epoch and stores each block", async function () {
      const { c, esId } = await withEpochStart480();

      const es = await c.getEpochStart(esId);
      expect(es.bits).to.equal(BigInt(headerBits(MAINNET_HEADERS[E480])));
      expect(es.height).to.equal(BigInt(E480));
      expect(es.firstTime).to.equal(BigInt(es480Time));

      for (let i = 0; i < 6; i++) {
        const id = await c.nodeId(
          headerHashLE(MAINNET_HEADERS[E480 + i]),
          E480 + i,
          es480Time
        );
        expect(await c.isStored(id)).to.equal(true);
      }
    });

    it("rejects a height that is not the first of an epoch", async function () {
      const c = await deploy();
      await c.setNow(es480Time + HOUR);
      await expect(
        c.addEpochStart(concat(es480), E480 + 1)
      ).to.be.revertedWithCustomError(c, "InvalidHeight");
    });

    it("rejects 5 blocks", async function () {
      const c = await deploy();
      await c.setNow(es480Time + HOUR);
      await expect(
        c.addEpochStart(concat(es480.slice(0, 5)), E480)
      ).to.be.revertedWithCustomError(c, "InvalidLength");
    });

    it("rejects blocks that do not name each other in order", async function () {
      const c = await deploy();
      await c.setNow(headerTime(MAINNET_HEADERS[E480 + 6]) + 600);
      const gap = [...es480.slice(0, 5), MAINNET_HEADERS[E480 + 6]];
      await expect(
        c.addEpochStart(concat(gap), E480)
      ).to.be.revertedWithCustomError(c, "NotLinked");
    });

    it("rejects blocks of two difficulties", async function () {
      const c = await deploy();
      await c.setNow(headerTime(MAINNET_HEADERS[E480 + 4]) + 600);
      // The last block of epoch 479 and the first 5 of epoch 480 name each
      // other in order, but the difficulty changes between them.
      const mixed = range(E480 - 1, E480 + 4);
      await expect(
        c.addEpochStart(concat(mixed), E480)
      ).to.be.revertedWithCustomError(c, "DifficultyMismatch");
    });

    it("changes nothing when the same epoch start is recorded again", async function () {
      const { c, esId } = await withEpochStart480();
      const before = await c.getEpochStart(esId);
      await c.setNow(headerTime(MAINNET_HEADERS[E480 + 5]) + 6000);
      const tx = await c.addEpochStart(concat(es480), E480);
      await expect(tx).to.not.emit(c, "EpochStartRecorded");
      await expect(tx).to.not.emit(c, "BlockStored");
      expect((await c.getEpochStart(esId)).recordedAt).to.equal(
        before.recordedAt
      );
    });

    it("keeps the first record when the same first block comes with other blocks after it", async function () {
      const c = await deploy();
      const EASY = 0x207fffff;
      const T = 1_800_000_000;
      await c.setLimits(targetFromBits(EASY), targetFromBits(EASY));
      await c.setNow(T + HOUR);

      const first = mine({ prevLE: ZERO_HASH, time: T, bits: EASY });
      const tail = (n: number) =>
        mineChain({
          prevLE: headerHashLE(first),
          firstTime: T + 600,
          bits: EASY,
          count: n,
        });
      const a = [first, ...tail(5)];
      const b = [first, ...tail(5)];

      await c.addEpochStart(concat(a), 0);
      const id = await c.nodeId(headerHashLE(first), 0, T);
      const before = await c.getEpochStart(id);

      const tx = await c.addEpochStart(concat(b), 0);
      await expect(tx).to.not.emit(c, "EpochStartRecorded");
      expect(await c.getEpochStart(id)).to.deep.equal(before);
      // The other 5 blocks are stored as blocks, on their own branch.
      expect(
        await c.isStored(await c.nodeId(headerHashLE(b[5]), 5, T))
      ).to.equal(true);
    });

    it("cannot check that the 6 blocks are the first of an epoch", async function () {
      // Documented limit: a block number cannot be proven. Any 6 real blocks
      // in a row pass, with any stated height that starts an epoch.
      const c = await deploy();
      await c.setNow(headerTime(MAINNET_HEADERS[E480 + 15]) + 600);
      await c.addEpochStart(concat(range(E480 + 10, E480 + 15)), E480);
    });

    it("rejects an epoch start that is too old to serve any anchor (D57)", async function () {
      const c = await deploy();
      await c.setNow(es480Time + 4 * WEEK + 2 * HOUR + 1);
      await expect(
        c.addEpochStart(concat(es480), E480)
      ).to.be.revertedWithCustomError(c, "EpochStartTooOld");
    });

    it("rejects a block whose time is more than 2 hours ahead", async function () {
      const c = await deploy();
      await c.setNow(es480Time - 2 * HOUR - 1);
      await expect(
        c.addEpochStart(concat(es480), E480)
      ).to.be.revertedWithCustomError(c, "TimeTooNew");
    });

    it("rejects blocks below the minimum difficulty of 2^45 (D38)", async function () {
      const c = await deploy();
      await c.setNow(headerTime(GENESIS_HEADER_HEX) + HOUR);
      // Real blocks of 2009, difficulty 1.
      const old = [
        GENESIS_HEADER_HEX,
        BLOCK1_HEADER_HEX,
        BLOCK1_HEADER_HEX,
        BLOCK1_HEADER_HEX,
        BLOCK1_HEADER_HEX,
        BLOCK1_HEADER_HEX,
      ];
      await expect(
        c.addEpochStart(concat(old), 0)
      ).to.be.revertedWithCustomError(c, "DifficultyTooLow");
    });
  });

  describe("jump (D38, D39, D44, D57)", function () {
    const ANCHOR = E480 + 20;
    const anchorHeader = MAINNET_HEADERS[ANCHOR];
    const anchorTime = headerTime(anchorHeader);

    it("accepts a fresh real block with the difficulty of its epoch start", async function () {
      const { c, esId } = await withEpochStart480();
      const [, operator] = await ethers.getSigners();
      await c.setNow(anchorTime + 300);

      const id = await c.nodeId(headerHashLE(anchorHeader), ANCHOR, es480Time);
      await expect(c.connect(operator).jump(anchorHeader, esId, ANCHOR))
        .to.emit(c, "Jumped")
        .withArgs(id, esId, operator.address);

      const node = await c.getNode(id);
      expect(node.height).to.equal(BigInt(ANCHOR));
      expect(node.anchoredAt).to.equal(BigInt(anchorTime + 300));
      expect(node.prevHash).to.equal(headerHashLE(MAINNET_HEADERS[ANCHOR - 1]));
    });

    it("rejects an anchor older than 2 hours (D39)", async function () {
      const { c, esId } = await withEpochStart480();
      await c.setNow(anchorTime + 2 * HOUR + 1);
      await expect(
        c.jump(anchorHeader, esId, ANCHOR)
      ).to.be.revertedWithCustomError(c, "AnchorTooOld");
    });

    it("accepts an anchor that is exactly 2 hours old", async function () {
      const { c, esId } = await withEpochStart480();
      await c.setNow(anchorTime + 2 * HOUR);
      await c.jump(anchorHeader, esId, ANCHOR);
    });

    it("rejects a block without enough work", async function () {
      const { c, esId } = await withEpochStart480();
      await c.setNow(anchorTime + 300);
      const tampered = anchorHeader.slice(0, -2) + "00";
      await expect(
        c.jump(tampered, esId, ANCHOR)
      ).to.be.revertedWithCustomError(c, "InsufficientWork");
    });

    it("rejects an anchor with another difficulty than the epoch start (D44)", async function () {
      const { c, esId } = await withEpochStart480();
      const other = MAINNET_HEADERS[E480 - 1];
      await c.setNow(headerTime(other) + 300);
      await expect(
        c.jump(other, esId, E480)
      ).to.be.revertedWithCustomError(c, "DifficultyMismatch");
    });

    it("rejects a stated height outside the epoch of the epoch start", async function () {
      const { c, esId } = await withEpochStart480();
      await c.setNow(anchorTime + 300);
      await expect(
        c.jump(anchorHeader, esId, E480 + EPOCH_BLOCKS)
      ).to.be.revertedWithCustomError(c, "InvalidHeight");
      await expect(
        c.jump(anchorHeader, esId, E480 - 1)
      ).to.be.revertedWithCustomError(c, "InvalidHeight");
    });

    it("rejects a jump to the first height of the epoch with another block (the epoch's time is its own)", async function () {
      const { c, esId } = await withEpochStart480();
      const other = MAINNET_HEADERS[E480 + 1];
      await c.setNow(headerTime(other) + 300);
      await expect(c.jump(other, esId, E480)).to.be.revertedWithCustomError(
        c,
        "EpochTimeMismatch"
      );
      await c.jump(MAINNET_HEADERS[E480], esId, E480);
    });

    it("rejects an unknown epoch start", async function () {
      const { c } = await withEpochStart480();
      await c.setNow(anchorTime + 300);
      await expect(
        c.jump(anchorHeader, ZERO_HASH, ANCHOR)
      ).to.be.revertedWithCustomError(c, "UnknownEpochStart");
    });

    it("needs no permission: any account can add blocks (D64)", async function () {
      const { c, esId } = await withEpochStart480();
      const [, , stranger] = await ethers.getSigners();
      await c.setNow(anchorTime + 300);
      await c.connect(stranger).jump(anchorHeader, esId, ANCHOR);
      await c
        .connect(stranger)
        .extend(MAINNET_HEADERS[ANCHOR + 1], ANCHOR, es480Time);
    });
  });

  describe("streaming (D48)", function () {
    const ANCHOR = E480 + 10;

    async function anchored() {
      const { c, esId } = await withEpochStart480();
      await c.setNow(headerTime(MAINNET_HEADERS[ANCHOR]) + 300);
      await c.jump(MAINNET_HEADERS[ANCHOR], esId, ANCHOR);
      await c.setNow(headerTime(MAINNET_HEADERS[E480 + 40]) + 300);
      return { c, esId };
    }

    it("stores 30 blocks after the anchor in one call and finds the anchor from the last", async function () {
      const { c } = await anchored();
      await c.extend(
        concat(range(ANCHOR + 1, ANCHOR + 30)),
        ANCHOR,
        es480Time
      );

      expect(
        await c.isAncestor(
          headerHashLE(MAINNET_HEADERS[ANCHOR]),
          ANCHOR,
          es480Time,
          headerHashLE(MAINNET_HEADERS[ANCHOR + 30]),
          ANCHOR + 30,
          es480Time,
          0
        )
      ).to.equal(true);
    });

    it("does not store a block twice", async function () {
      const { c } = await anchored();
      const first = await c.extend(
        MAINNET_HEADERS[ANCHOR + 1],
        ANCHOR,
        es480Time
      );
      await expect(first).to.emit(c, "BlockStored");

      const id = await c.nodeId(
        headerHashLE(MAINNET_HEADERS[ANCHOR + 1]),
        ANCHOR + 1,
        es480Time
      );
      const storedAt = (await c.getNode(id)).storedAt;

      await c.setNow(headerTime(MAINNET_HEADERS[E480 + 40]) + 900);
      const second = await c.extend(
        MAINNET_HEADERS[ANCHOR + 1],
        ANCHOR,
        es480Time
      );
      await expect(second).to.not.emit(c, "BlockStored");
      expect((await c.getNode(id)).storedAt).to.equal(storedAt);
    });

    it("rejects a block whose parent is not stored", async function () {
      const { c } = await anchored();
      await expect(
        c.extend(MAINNET_HEADERS[ANCHOR + 5], ANCHOR + 4, es480Time)
      ).to.be.revertedWithCustomError(c, "UnknownParent");
    });

    it("rejects a gap inside one call", async function () {
      const { c } = await anchored();
      const gap = [MAINNET_HEADERS[ANCHOR + 1], MAINNET_HEADERS[ANCHOR + 3]];
      await expect(
        c.extend(concat(gap), ANCHOR, es480Time)
      ).to.be.revertedWithCustomError(c, "NotLinked");
    });

    it("does not find an ancestor when a block on the way is missing", async function () {
      const { c, esId } = await anchored();
      // Blocks 11 to 13 after the epoch start, then a second anchor at 16.
      await c.extend(concat(range(ANCHOR + 1, ANCHOR + 3)), ANCHOR, es480Time);
      await c.setNow(headerTime(MAINNET_HEADERS[ANCHOR + 6]) + 300);
      await c.jump(MAINNET_HEADERS[ANCHOR + 6], esId, ANCHOR + 6);

      expect(
        await c.isAncestor(
          headerHashLE(MAINNET_HEADERS[ANCHOR]),
          ANCHOR,
          es480Time,
          headerHashLE(MAINNET_HEADERS[ANCHOR + 6]),
          ANCHOR + 6,
          es480Time,
          0
        )
      ).to.equal(false);
    });

    it("rejects input that is not whole headers, or more than 100 of them", async function () {
      const { c } = await anchored();
      await expect(
        c.extend("0x", ANCHOR, es480Time)
      ).to.be.revertedWithCustomError(c, "InvalidLength");
      await expect(
        c.extend(MAINNET_HEADERS[ANCHOR + 1] + "00", ANCHOR, es480Time)
      ).to.be.revertedWithCustomError(c, "InvalidLength");
      const many = new Array(101).fill(MAINNET_HEADERS[ANCHOR + 1]);
      await expect(
        c.extend(concat(many), ANCHOR, es480Time)
      ).to.be.revertedWithCustomError(c, "InvalidLength");
    });

    it("refuses a walk longer than the longest window (D70)", async function () {
      const { c } = await anchored();
      await expect(
        c.isAncestor(
          headerHashLE(MAINNET_HEADERS[ANCHOR]),
          ANCHOR,
          es480Time,
          headerHashLE(MAINNET_HEADERS[ANCHOR + 1]),
          ANCHOR + 101,
          es480Time,
          0
        )
      ).to.be.revertedWithCustomError(c, "WalkTooLong");
    });
  });

  describe("crossing into a new epoch (D72)", function () {
    const LAST = E480 - 1;

    it("calculates the difficulty of epoch 480 as Bitcoin did", async function () {
      const c = await deploy();
      expect(
        await c.retargetBits(
          headerBits(MAINNET_HEADERS[LAST]),
          es479Time,
          headerTime(MAINNET_HEADERS[LAST])
        )
      ).to.equal(BigInt(headerBits(MAINNET_HEADERS[E480])));
    });

    async function crossed() {
      const c = await deploy();
      await c.setNow(headerTime(MAINNET_HEADERS[E479 + 5]) + 600);
      await c.addEpochStart(concat(es479), E479);
      const esId = await c.nodeId(
        headerHashLE(MAINNET_HEADERS[E479]),
        E479,
        es479Time
      );

      await c.setNow(headerTime(MAINNET_HEADERS[LAST]) + 60);
      await c.jump(MAINNET_HEADERS[LAST], esId, LAST);

      await c.setNow(headerTime(MAINNET_HEADERS[E480 + 8]) + 60);
      await c.extend(concat(range(E480, E480 + 8)), LAST, es479Time);
      return c;
    }

    it("streams from the last block of epoch 479 into epoch 480", async function () {
      const c = await crossed();
      const first = await c.getNode(
        await c.nodeId(headerHashLE(MAINNET_HEADERS[E480]), E480, es480Time)
      );
      expect(first.bits).to.equal(BigInt(headerBits(MAINNET_HEADERS[E480])));
      expect(first.epochTime).to.equal(BigInt(es480Time));
    });

    it("finds the anchor in the old epoch from a block in the new one", async function () {
      const c = await crossed();
      const walk = (prevEpochTime: number) =>
        c.isAncestor(
          headerHashLE(MAINNET_HEADERS[LAST]),
          LAST,
          es479Time,
          headerHashLE(MAINNET_HEADERS[E480 + 8]),
          E480 + 8,
          es480Time,
          prevEpochTime
        );
      expect(await walk(es479Time)).to.equal(true);
      expect(await walk(es479Time + 1)).to.equal(false);
    });

    it("shares the blocks with an epoch start recorded by someone else", async function () {
      const c = await crossed();
      // The first 6 blocks of epoch 480 are stored already. Recording them as
      // an epoch start stores nothing new.
      const tx = await c.addEpochStart(concat(es480), E480);
      await expect(tx).to.not.emit(c, "BlockStored");
      await expect(tx).to.emit(c, "EpochStartRecorded");
    });
  });

  describe("the parent of a stored block (D64, D81)", function () {
    const ANCHOR = E480 + 20;

    async function jumped() {
      const { c, esId } = await withEpochStart480();
      await c.setNow(headerTime(MAINNET_HEADERS[ANCHOR]) + 300);
      await c.jump(MAINNET_HEADERS[ANCHOR], esId, ANCHOR);
      return c;
    }

    it("stores the real parent of an anchor, and the parent before it", async function () {
      const c = await jumped();
      await c.extendBack(
        MAINNET_HEADERS[ANCHOR - 1],
        headerHashLE(MAINNET_HEADERS[ANCHOR]),
        ANCHOR,
        es480Time,
        0
      );
      await c.extendBack(
        MAINNET_HEADERS[ANCHOR - 2],
        headerHashLE(MAINNET_HEADERS[ANCHOR - 1]),
        ANCHOR - 1,
        es480Time,
        0
      );
      expect(
        await c.isAncestor(
          headerHashLE(MAINNET_HEADERS[ANCHOR - 2]),
          ANCHOR - 2,
          es480Time,
          headerHashLE(MAINNET_HEADERS[ANCHOR]),
          ANCHOR,
          es480Time,
          0
        )
      ).to.equal(true);
    });

    it("rejects a block that is not the parent", async function () {
      const c = await jumped();
      await expect(
        c.extendBack(
          MAINNET_HEADERS[ANCHOR - 2],
          headerHashLE(MAINNET_HEADERS[ANCHOR]),
          ANCHOR,
          es480Time,
          0
        )
      ).to.be.revertedWithCustomError(c, "NotLinked");
    });

    it("rejects a child that is not stored", async function () {
      const c = await jumped();
      await expect(
        c.extendBack(
          MAINNET_HEADERS[ANCHOR],
          headerHashLE(MAINNET_HEADERS[ANCHOR + 1]),
          ANCHOR + 1,
          es480Time,
          0
        )
      ).to.be.revertedWithCustomError(c, "UnknownBlock");
    });

    it("goes back across the epoch change, with the difficulty Bitcoin's rule gives", async function () {
      const { c } = await withEpochStart480();
      const first = headerHashLE(MAINNET_HEADERS[E480]);
      // With a wrong time for the older epoch the difficulty does not fit.
      await expect(
        c.extendBack(MAINNET_HEADERS[E480 - 1], first, E480, es480Time, es479Time + 100_000)
      ).to.be.revertedWithCustomError(c, "DifficultyMismatch");

      await c.extendBack(MAINNET_HEADERS[E480 - 1], first, E480, es480Time, es479Time);
      expect(
        await c.isAncestor(
          headerHashLE(MAINNET_HEADERS[E480 - 1]),
          E480 - 1,
          es479Time,
          headerHashLE(MAINNET_HEADERS[E480 + 5]),
          E480 + 5,
          es480Time,
          es479Time
        )
      ).to.equal(true);
    });

    it("rejects a first block of an epoch whose time is not the epoch's time", async function () {
      // Block 967682 is jumped to with a height one too low: 967681. Its
      // parent would then be the first block of the epoch, but the epoch's
      // time is that of block 967680.
      const { c, esId } = await withEpochStart480();
      const header = MAINNET_HEADERS[E480 + 2];
      await c.setNow(headerTime(header) + 300);
      await c.jump(header, esId, E480 + 1);

      await expect(
        c.extendBack(
          MAINNET_HEADERS[E480 + 1],
          headerHashLE(header),
          E480 + 1,
          es480Time,
          0
        )
      ).to.be.revertedWithCustomError(c, "EpochTimeMismatch");
    });

    it("adds up the work of the blocks on the way", async function () {
      const c = await jumped();
      await c.setNow(headerTime(MAINNET_HEADERS[ANCHOR + 10]) + 300);
      await c.extend(concat(range(ANCHOR + 1, ANCHOR + 10)), ANCHOR, es480Time);
      const one = await c.workOf(headerBits(MAINNET_HEADERS[ANCHOR]));
      const [linked, work] = await c.walk(
        headerHashLE(MAINNET_HEADERS[ANCHOR]),
        ANCHOR,
        es480Time,
        headerHashLE(MAINNET_HEADERS[ANCHOR + 10]),
        ANCHOR + 10,
        es480Time,
        0
      );
      expect(linked).to.equal(true);
      expect(work).to.equal(one * 10n);

      const [unlinked, none] = await c.walk(
        headerHashLE(MAINNET_HEADERS[ANCHOR + 1]),
        ANCHOR,
        es480Time,
        headerHashLE(MAINNET_HEADERS[ANCHOR + 10]),
        ANCHOR + 10,
        es480Time,
        0
      );
      expect(unlinked).to.equal(false);
      expect(none).to.equal(0n);
    });
  });

  describe("the lowest block number (D93)", function () {
    it("refuses an epoch start and a parent below it", async function () {
      const c = await ethers.deployContract("iPoWLightClientHarness", [E480]);
      await c.setNow(headerTime(MAINNET_HEADERS[E479 + 5]) + 600);
      await expect(
        c.addEpochStart(concat(es479), E479)
      ).to.be.revertedWithCustomError(c, "BelowMinHeight");

      await c.setNow(headerTime(MAINNET_HEADERS[E480 + 5]) + 600);
      await c.addEpochStart(concat(es480), E480);
      // The block before 967,680 is below the lowest number.
      await expect(
        c.extendBack(
          MAINNET_HEADERS[E480 - 1],
          headerHashLE(MAINNET_HEADERS[E480]),
          E480,
          es480Time,
          es479Time
        )
      ).to.be.revertedWithCustomError(c, "BelowMinHeight");
      expect(await c.minHeight()).to.equal(BigInt(E480));
    });

    it("refuses a label of 0 for a real block", async function () {
      const c = await ethers.deployContract("iPoWLightClientHarness", [E480]);
      await c.setNow(headerTime(MAINNET_HEADERS[E480 + 5]) + 600);
      await expect(
        c.addEpochStart(concat(es480), 0)
      ).to.be.revertedWithCustomError(c, "BelowMinHeight");
    });
  });

  describe("which epoch start counts (D45)", function () {
    it("counts both real epoch starts, 2 weeks apart", async function () {
      const c = await deploy();
      await c.setNow(headerTime(MAINNET_HEADERS[E479 + 5]) + 600);
      await c.addEpochStart(concat(es479), E479);
      await c.setNow(headerTime(MAINNET_HEADERS[E480 + 5]) + 600);
      await c.addEpochStart(concat(es480), E480);

      const anchorTime = headerTime(MAINNET_HEADERS[E480 + 20]);
      await c.setNow(anchorTime + 300);
      const id479 = await c.nodeId(
        headerHashLE(MAINNET_HEADERS[E479]),
        E479,
        es479Time
      );
      const id480 = await c.nodeId(
        headerHashLE(MAINNET_HEADERS[E480]),
        E480,
        es480Time
      );
      expect(await c.epochStartCounts(id479, anchorTime)).to.equal(true);
      expect(await c.epochStartCounts(id480, anchorTime)).to.equal(true);
    });

    it("does not make a jump cost more when many epoch starts are recorded", async function () {
      const { c, esId } = await withEpochStart480();
      const anchor = MAINNET_HEADERS[E480 + 30];
      await c.setNow(headerTime(anchor) + 300);
      const before = await c.jump.estimateGas(anchor, esId, E480 + 30);

      // Someone records 40 more, for the price of gas only: real blocks, 6 in
      // a row, under many stated heights.
      for (let i = 1; i <= 40; i++) {
        await c.addEpochStart(
          concat(range(E480 + 10, E480 + 15)),
          E480 + i * EPOCH_BLOCKS
        );
      }
      const after = await c.jump.estimateGas(anchor, esId, E480 + 30);
      expect(after).to.equal(before);
    });

    it("rejects an unknown epoch start", async function () {
      const c = await deploy();
      await expect(
        c.epochStartCounts(ZERO_HASH, es480Time)
      ).to.be.revertedWithCustomError(c, "UnknownEpochStart");
    });
  });

  describe("a transaction in a block", function () {
    const siblings = MAINNET_TX.merkle.map(reverseHex);

    async function nodeOfTx(c: Awaited<ReturnType<typeof deploy>>) {
      return c.nodeId(
        headerHashLE(MAINNET_HEADERS[MAINNET_TX.height]),
        MAINNET_TX.height,
        es480Time
      );
    }

    it("finds a real transaction with its real proof", async function () {
      const { c } = await withEpochStart480();
      expect(
        await c.txInBlock(
          await nodeOfTx(c),
          MAINNET_TX.raw,
          siblings,
          MAINNET_TX.index
        )
      ).to.equal(true);
    });

    it("does not find it at another position or with a changed sibling", async function () {
      const { c } = await withEpochStart480();
      const id = await nodeOfTx(c);
      expect(
        await c.txInBlock(id, MAINNET_TX.raw, siblings, MAINNET_TX.index + 1)
      ).to.equal(false);

      const changed = [...siblings];
      changed[3] = ZERO_HASH;
      expect(
        await c.txInBlock(id, MAINNET_TX.raw, changed, MAINNET_TX.index)
      ).to.equal(false);
    });

    it("does not find a changed transaction", async function () {
      const { c } = await withEpochStart480();
      const changed = MAINNET_TX.raw.slice(0, -2) + "ff";
      expect(
        await c.txInBlock(
          await nodeOfTx(c),
          changed,
          siblings,
          MAINNET_TX.index
        )
      ).to.equal(false);
    });

    it("rejects 64 bytes, which could be an inner node of the tree", async function () {
      const { c } = await withEpochStart480();
      await expect(
        c.txInBlock(await nodeOfTx(c), "0x" + "11".repeat(64), siblings, 0)
      ).to.be.revertedWithCustomError(c, "InvalidTransaction");
    });

    it("rejects a position that does not fit the proof", async function () {
      const { c } = await withEpochStart480();
      await expect(
        c.txInBlock(
          await nodeOfTx(c),
          MAINNET_TX.raw,
          siblings,
          2 ** siblings.length
        )
      ).to.be.revertedWithCustomError(c, "InvalidIndex");
    });

    it("rejects a block that is not stored", async function () {
      const { c } = await withEpochStart480();
      await expect(
        c.txInBlock(ZERO_HASH, MAINNET_TX.raw, siblings, MAINNET_TX.index)
      ).to.be.revertedWithCustomError(c, "UnknownBlock");
    });
  });
});

describe("iPoWLightClient: blocks mined by the test, low difficulty", function () {
  // The real contract rejects every block here (D38). The harness lowers the
  // minimum difficulty so that the test can mine forks and fakes.
  const EASY = 0x207fffff;
  const EASY_TARGET = targetFromBits(EASY);
  const T0 = 1_800_000_000;

  async function deployEasy() {
    const c = await deploy();
    await c.setLimits(EASY_TARGET, EASY_TARGET);
    return c;
  }

  async function addEpochStart(
    c: Awaited<ReturnType<typeof deploy>>,
    opts: { bits: number; firstTime: number; height: number }
  ) {
    const headers = mineChain({
      prevLE: ZERO_HASH,
      firstTime: opts.firstTime,
      bits: opts.bits,
      count: 6,
    });
    await c.addEpochStart(concat(headers), opts.height);
    const id = await c.nodeId(
      headerHashLE(headers[0]),
      opts.height,
      opts.firstTime
    );
    return { id, headers };
  }

  it("the real contract rejects a low-difficulty block (D38)", async function () {
    const c = await deploy();
    await c.setNow(T0 + HOUR);
    const headers = mineChain({
      prevLE: ZERO_HASH,
      firstTime: T0,
      bits: EASY,
      count: 6,
    });
    await expect(c.addEpochStart(concat(headers), 0)).to.be.revertedWithCustomError(
      c,
      "DifficultyTooLow"
    );
  });

  describe("which epoch start counts (D45)", function () {
    // A target of 0x400000... is just under twice as hard as EASY, and
    // 0x3fffff... is just over.
    const WITHIN_HALF = 0x20400000;
    const BEYOND_HALF = 0x203fffff;

    it("counts while it has at least half of the highest difficulty", async function () {
      const c = await deployEasy();
      await c.setNow(T0 + HOUR);
      const weak = await addEpochStart(c, { bits: EASY, firstTime: T0, height: 0 });
      await addEpochStart(c, {
        bits: WITHIN_HALF,
        firstTime: T0,
        height: EPOCH_BLOCKS,
      });

      const anchor = mine({ prevLE: ZERO_HASH, time: T0 + HOUR, bits: EASY });
      await c.jump(anchor, weak.id, 10);
    });

    it("stops counting when a harder one is recorded", async function () {
      const c = await deployEasy();
      await c.setNow(T0 + HOUR);
      const weak = await addEpochStart(c, { bits: EASY, firstTime: T0, height: 0 });

      const before = mine({ prevLE: ZERO_HASH, time: T0 + HOUR, bits: EASY });
      await c.jump(before, weak.id, 10);

      const strong = await addEpochStart(c, {
        bits: BEYOND_HALF,
        firstTime: T0,
        height: EPOCH_BLOCKS,
      });

      const after = mine({ prevLE: ZERO_HASH, time: T0 + HOUR, bits: EASY });
      await expect(c.jump(after, weak.id, 11)).to.be.revertedWithCustomError(
        c,
        "EpochStartTooWeak"
      );

      // The harder one counts.
      const hard = mine({
        prevLE: ZERO_HASH,
        time: T0 + HOUR,
        bits: BEYOND_HALF,
      });
      await c.jump(hard, strong.id, EPOCH_BLOCKS + 10);
    });

    it("still compares an epoch start that is almost 4 weeks old", async function () {
      const c = await deployEasy();
      await c.setNow(T0 + HOUR);
      await addEpochStart(c, {
        bits: BEYOND_HALF,
        firstTime: T0,
        height: 0,
      });

      // One hour short of 4 weeks later, the harder one still counts against
      // a weak one.
      const later = T0 + 4 * WEEK - HOUR;
      await c.setNow(later);
      const weak = await addEpochStart(c, {
        bits: EASY,
        firstTime: later - HOUR,
        height: 2 * EPOCH_BLOCKS,
      });
      const anchor = mine({ prevLE: ZERO_HASH, time: later, bits: EASY });
      await expect(
        c.jump(anchor, weak.id, 2 * EPOCH_BLOCKS + 10)
      ).to.be.revertedWithCustomError(c, "EpochStartTooWeak");
    });

    it("compares by whole days: the first day of the 4 weeks counts in full", async function () {
      const c = await deployEasy();
      // T0 is 08:00 of its day. The harder epoch start is from 08:00.
      await c.setNow(T0 + HOUR);
      await addEpochStart(c, {
        bits: BEYOND_HALF,
        firstTime: T0,
        height: 0,
      });

      // 4 weeks and 4 hours later it is older than 4 weeks, but its day is
      // still the first day that is compared.
      const later = T0 + 4 * WEEK + 4 * HOUR;
      await c.setNow(later);
      const weak = await addEpochStart(c, {
        bits: EASY,
        firstTime: later - HOUR,
        height: 2 * EPOCH_BLOCKS,
      });
      const anchor = mine({ prevLE: ZERO_HASH, time: later, bits: EASY });
      await expect(
        c.jump(anchor, weak.id, 2 * EPOCH_BLOCKS + 10)
      ).to.be.revertedWithCustomError(c, "EpochStartTooWeak");
    });

    it("compares only epoch starts of the last 4 weeks", async function () {
      const c = await deployEasy();
      await c.setNow(T0 + HOUR);
      await addEpochStart(c, {
        bits: BEYOND_HALF,
        firstTime: T0,
        height: 0,
      });

      // More than 4 weeks later the difficulty has fallen to less than half.
      // The contract compares by whole days, so "later" is a day past the 4
      // weeks.
      const later = T0 + 4 * WEEK + 24 * HOUR;
      await c.setNow(later + HOUR);
      const weak = await addEpochStart(c, {
        bits: EASY,
        firstTime: later,
        height: 2 * EPOCH_BLOCKS,
      });

      const anchor = mine({
        prevLE: ZERO_HASH,
        time: later + HOUR,
        bits: EASY,
      });
      await c.jump(anchor, weak.id, 2 * EPOCH_BLOCKS + 10);
    });
  });

  describe("epoch start age (D57)", function () {
    it("rejects an anchor more than 4 weeks younger than its epoch start", async function () {
      const c = await deployEasy();
      await c.setNow(T0 + HOUR);
      const es = await addEpochStart(c, { bits: EASY, firstTime: T0, height: 0 });

      const anchorTime = T0 + 4 * WEEK + 1;
      await c.setNow(anchorTime);
      const anchor = mine({ prevLE: ZERO_HASH, time: anchorTime, bits: EASY });
      await expect(c.jump(anchor, es.id, 10)).to.be.revertedWithCustomError(
        c,
        "EpochStartOutOfRange"
      );
    });

    it("accepts an anchor exactly 4 weeks younger", async function () {
      const c = await deployEasy();
      await c.setNow(T0 + HOUR);
      const es = await addEpochStart(c, { bits: EASY, firstTime: T0, height: 0 });

      const anchorTime = T0 + 4 * WEEK;
      await c.setNow(anchorTime);
      const anchor = mine({ prevLE: ZERO_HASH, time: anchorTime, bits: EASY });
      await c.jump(anchor, es.id, 10);
    });

    it("rejects an anchor older than its epoch start", async function () {
      const c = await deployEasy();
      await c.setNow(T0 + HOUR);
      const es = await addEpochStart(c, { bits: EASY, firstTime: T0, height: 0 });

      const anchor = mine({ prevLE: ZERO_HASH, time: T0 - 1, bits: EASY });
      await expect(c.jump(anchor, es.id, 10)).to.be.revertedWithCustomError(
        c,
        "EpochStartOutOfRange"
      );
    });
  });

  describe("forks and other people's blocks (D48, D64)", function () {
    async function anchored() {
      const c = await deployEasy();
      await c.setNow(T0 + HOUR);
      const es = await addEpochStart(c, { bits: EASY, firstTime: T0, height: 0 });
      const anchor = mine({ prevLE: ZERO_HASH, time: T0 + HOUR, bits: EASY });
      await c.jump(anchor, es.id, 10);
      return { c, es, anchor };
    }

    it("stores two branches side by side", async function () {
      const { c, anchor } = await anchored();
      const a = mineChain({
        prevLE: headerHashLE(anchor),
        firstTime: T0 + HOUR + 600,
        bits: EASY,
        count: 3,
      });
      const b = mineChain({
        prevLE: headerHashLE(anchor),
        firstTime: T0 + HOUR + 600,
        bits: EASY,
        count: 2,
      });
      await c.extend(concat(a), 10, T0);
      await c.extend(concat(b), 10, T0);

      const reaches = (from: string, fromHeight: number, to: string, toHeight: number) =>
        c.isAncestor(
          headerHashLE(from),
          fromHeight,
          T0,
          headerHashLE(to),
          toHeight,
          T0,
          0
        );

      expect(await reaches(anchor, 10, a[2], 13)).to.equal(true);
      expect(await reaches(anchor, 10, b[1], 12)).to.equal(true);
      // A block of one branch is not an ancestor on the other.
      expect(await reaches(a[0], 11, b[1], 12)).to.equal(false);
      expect(await reaches(b[0], 11, a[2], 13)).to.equal(false);
    });

    it("keeps a block stated with a wrong height apart from the same block stated right", async function () {
      const { c, es, anchor } = await anchored();
      const [, , attacker] = await ethers.getSigners();

      const next = mine({
        prevLE: headerHashLE(anchor),
        time: T0 + HOUR + 600,
        bits: EASY,
      });

      // The attacker is first and states a wrong height for the same blocks.
      await c.connect(attacker).jump(anchor, es.id, 500);
      await c.connect(attacker).extend(next, 500, T0);

      // The honest operator streams the same block at the right height.
      await c.extend(next, 10, T0);

      const right = await c.nodeId(headerHashLE(next), 11, T0);
      const wrong = await c.nodeId(headerHashLE(next), 501, T0);
      expect(right).to.not.equal(wrong);
      expect((await c.getNode(right)).height).to.equal(11n);
      expect((await c.getNode(wrong)).height).to.equal(501n);
      expect(
        await c.isAncestor(
          headerHashLE(anchor),
          10,
          T0,
          headerHashLE(next),
          11,
          T0,
          0
        )
      ).to.equal(true);
    });

    it("does not walk through a block that was jumped onto a parent of another difficulty", async function () {
      // An honest epoch start and anchor at a higher difficulty.
      const HARD = 0x20400000;
      const c = await deployEasy();
      await c.setNow(T0 + HOUR);
      const honest = await addEpochStart(c, { bits: HARD, firstTime: T0, height: 0 });
      const anchor = mine({ prevLE: ZERO_HASH, time: T0 + HOUR, bits: HARD });
      await c.jump(anchor, honest.id, 10);

      // A second epoch start with the same first time and an easier
      // difficulty that still counts. A block of that difficulty names the
      // honest anchor as its parent. Streaming refuses it, a jump stores it.
      const easy = await addEpochStart(c, { bits: EASY, firstTime: T0, height: 0 });
      const f1 = mine({
        prevLE: headerHashLE(anchor),
        time: T0 + HOUR,
        bits: EASY,
      });
      await expect(c.extend(f1, 10, T0)).to.be.revertedWithCustomError(
        c,
        "DifficultyMismatch"
      );
      await c.jump(f1, easy.id, 11);
      const f2 = mine({
        prevLE: headerHashLE(f1),
        time: T0 + HOUR + 600,
        bits: EASY,
      });
      await c.extend(f2, 11, T0);

      expect(
        await c.isAncestor(
          headerHashLE(anchor),
          10,
          T0,
          headerHashLE(f2),
          12,
          T0,
          0
        )
      ).to.equal(false);
      // From the jumped block itself the walk is fine.
      expect(
        await c.isAncestor(
          headerHashLE(f1),
          11,
          T0,
          headerHashLE(f2),
          12,
          T0,
          0
        )
      ).to.equal(true);
    });

    it("rejects a block with another difficulty inside an epoch", async function () {
      const { c, anchor } = await anchored();
      const other = mine({
        prevLE: headerHashLE(anchor),
        time: T0 + HOUR + 600,
        bits: 0x20400000,
      });
      await expect(c.extend(other, 10, T0)).to.be.revertedWithCustomError(
        c,
        "DifficultyMismatch"
      );
    });

    it("rejects a streamed block whose time is more than 2 hours ahead", async function () {
      const { c, anchor } = await anchored();
      const early = mine({
        prevLE: headerHashLE(anchor),
        time: T0 + 3 * HOUR + 1,
        bits: EASY,
      });
      await expect(c.extend(early, 10, T0)).to.be.revertedWithCustomError(
        c,
        "TimeTooNew"
      );
    });
  });

  describe("crossing into a new epoch (D72)", function () {
    // The anchor is stated as the last block of its epoch, one week after the
    // epoch began. Bitcoin's rule then doubles the difficulty.
    const LAST = EPOCH_BLOCKS - 1;
    const anchorTime = T0 + WEEK;

    async function atTheEdge() {
      const c = await deployEasy();
      await c.setNow(T0 + HOUR);
      const es = await addEpochStart(c, { bits: EASY, firstTime: T0, height: 0 });
      await c.setNow(anchorTime);
      const anchor = mine({ prevLE: ZERO_HASH, time: anchorTime, bits: EASY });
      await c.jump(anchor, es.id, LAST);
      return { c, anchor };
    }

    it("accepts the next block only with the new difficulty", async function () {
      const { c, anchor } = await atTheEdge();
      const newBits = retarget(EASY, T0, anchorTime, EASY_TARGET);
      // Half of the target, cut to the 3 bytes the encoding keeps.
      expect(newBits).to.equal(0x203fffff);
      expect(await c.retargetBits(EASY, T0, anchorTime)).to.equal(
        BigInt(newBits)
      );

      const oldDifficulty = mine({
        prevLE: headerHashLE(anchor),
        time: anchorTime + 600,
        bits: EASY,
      });
      await expect(
        c.extend(oldDifficulty, LAST, T0)
      ).to.be.revertedWithCustomError(c, "DifficultyMismatch");

      const first = mine({
        prevLE: headerHashLE(anchor),
        time: anchorTime + 600,
        bits: newBits,
      });
      await c.extend(first, LAST, T0);

      // Blocks after it keep the new difficulty and belong to the new epoch.
      const second = mine({
        prevLE: headerHashLE(first),
        time: anchorTime + 1200,
        bits: newBits,
      });
      await c.extend(second, EPOCH_BLOCKS, anchorTime + 600);

      const node = await c.getNode(
        await c.nodeId(
          headerHashLE(second),
          EPOCH_BLOCKS + 1,
          anchorTime + 600
        )
      );
      expect(node.bits).to.equal(BigInt(newBits));
    });

    it("does not walk across the epoch change when the difficulty is not the one Bitcoin's rule gives", async function () {
      const { c, anchor } = await atTheEdge();
      // A block that names the anchor but keeps the old difficulty. Streaming
      // refuses it. A jump stores it as the first block of the new epoch.
      const time = anchorTime + 600;
      await c.setNow(time);
      const es2 = await addEpochStart(c, {
        bits: EASY,
        firstTime: time,
        height: EPOCH_BLOCKS,
      });
      const wrong = mine({ prevLE: headerHashLE(anchor), time, bits: EASY });
      await c.jump(wrong, es2.id, EPOCH_BLOCKS);

      expect(
        await c.isAncestor(
          headerHashLE(anchor),
          LAST,
          T0,
          headerHashLE(wrong),
          EPOCH_BLOCKS,
          time,
          T0
        )
      ).to.equal(false);
    });

    it("never lets the difficulty fall below the minimum (D38)", async function () {
      const c = await deployEasy();
      // Twice as hard as the minimum of this test.
      const HARD = 0x20400000;
      await c.setLimits(targetFromBits(HARD), EASY_TARGET);
      await c.setNow(T0 + HOUR);
      const es = await addEpochStart(c, { bits: HARD, firstTime: T0, height: 0 });

      // 4 weeks make Bitcoin's rule halve the difficulty.
      const time = T0 + 4 * WEEK;
      await c.setNow(time);
      const anchor = mine({ prevLE: ZERO_HASH, time, bits: HARD });
      await c.jump(anchor, es.id, LAST);

      const easier = retarget(HARD, T0, time, EASY_TARGET);
      const next = mine({
        prevLE: headerHashLE(anchor),
        time: time + 600,
        bits: easier,
      });
      await expect(c.extend(next, LAST, T0)).to.be.revertedWithCustomError(
        c,
        "DifficultyTooLow"
      );
    });
  });

  describe("compact difficulty encoding", function () {
    it("round-trips real and test values", async function () {
      const c = await deploy();
      for (const bits of [0x17021ec5, 0x1702355e, 0x1d00ffff, EASY, 0x20400000]) {
        const target = await c.targetFromBits(bits);
        expect(target).to.equal(targetFromBits(bits));
        expect(await c.bitsFromTarget(target)).to.equal(BigInt(bits));
      }
    });

    it("moves a mantissa that would use the sign bit", async function () {
      const c = await deploy();
      // 0x80 << 16 needs the sign bit in 3 bytes, so it is written with 4.
      expect(await c.bitsFromTarget(0x800000n)).to.equal(0x04008000n);
    });

    it("rejects a negative, a zero and an oversized value", async function () {
      const c = await deploy();
      await expect(c.targetFromBits(0x1d80ffff)).to.be.revertedWithCustomError(
        c,
        "InvalidBits"
      );
      await expect(c.targetFromBits(0x1d000000)).to.be.revertedWithCustomError(
        c,
        "InvalidBits"
      );
      await expect(c.targetFromBits(0x2300ffff)).to.be.revertedWithCustomError(
        c,
        "InvalidBits"
      );
    });

    it("gives one block of epoch 480 a work of about 2^78.9", async function () {
      const c = await deploy();
      const work = await c.workOf(0x17021ec5);
      expect(work > 2n ** 78n && work < 2n ** 79n).to.equal(true);
    });
  });
});
