import { expect } from "chai";
import { network } from "hardhat";

import {
  COIN_SCRIPT,
  buildTx,
  concat,
  headerHashLE,
  merkle,
  mine,
  tagScript,
  targetFromBits,
  txidLE,
} from "./helpers/bitcoin.ts";

const { ethers } = await network.create();

// Spec: docs/design/ipow-protocol.md, sections 3, 4.3, 4.4, 5 and 6. The D
// numbers are its decisions. The Bitcoin blocks here are mined by the test at
// a low difficulty, which only the test harness of the light client allows.

const MINUTE = 60;
const HOUR = 3600;
const DAY = 24 * HOUR;
const ETH = 10n ** 18n;
const GWEI = 10n ** 9n;
const GAS = 1_000_000n;

const EASY = 0x207fffff;
const ZERO_HASH = "0x" + "00".repeat(32);
const TAG = ethers.id("transfer-1");
const SALT = ethers.id("salt");

const FEE = ((110_000n * 30n + 520_000n) * GWEI * 3n) / 2n;
const ESCROW_FEE = ETH / 200n;

type Ref = { hash: string; height: number; epochTime: number };
type Tx = { raw: string; txid: string };

async function latestTime() {
  return (await ethers.provider.getBlock("latest"))!.timestamp;
}

async function at(timestamp: number) {
  await ethers.provider.send("evm_setNextBlockTimestamp", [timestamp]);
}

async function mineAt(timestamp: number) {
  await at(timestamp);
  await ethers.provider.send("evm_mine", []);
}

async function setPrice(price: bigint) {
  await ethers.provider.send("hardhat_setNextBlockBaseFeePerGas", [
    "0x" + price.toString(16),
  ]);
}

/** A Bitcoin chain the test mines and streams to the light client. */
class TestChain {
  tip!: Ref;
  epochTime!: number;
  epochStartId!: string;
  private blocks = new Map<string, string[]>();

  constructor(private lightClient: any) {}

  /** Records an epoch start whose last block is 10 minutes old. */
  async start(now: number) {
    this.epochTime = now - 60 * MINUTE;
    const headers: string[] = [];
    let prevLE = ZERO_HASH;
    for (let i = 0; i < 6; i++) {
      const header = mine({
        prevLE,
        time: this.epochTime + i * 10 * MINUTE,
        bits: EASY,
      });
      headers.push(header);
      prevLE = headerHashLE(header);
    }
    await this.lightClient.addEpochStart(concat(headers), 0);
    this.epochStartId = await this.lightClient.nodeId(
      headerHashLE(headers[0]),
      0,
      this.epochTime
    );
    this.tip = { hash: prevLE, height: 5, epochTime: this.epochTime };
  }

  /** Mines a block on `parent` (the tip unless stated) and streams it. */
  async add(time: number, txs: Tx[] = [], parent?: Ref): Promise<Ref> {
    const on = parent ?? this.tip;
    // The first transaction of a block is the miner's own.
    const coinbase = buildTx({
      inputs: [{ txidLE: ZERO_HASH, vout: 0xffffffff }],
      outputs: [{ value: 1n, script: COIN_SCRIPT }],
    });
    const txids = [txidLE(coinbase + time.toString(16)), ...txs.map((t) => t.txid)];
    const header = mine({
      prevLE: on.hash,
      time,
      bits: EASY,
      merkleRootLE: merkle(txids, 0).rootLE,
    });
    await this.lightClient.extend(header, on.height, on.epochTime);
    const ref = {
      hash: headerHashLE(header),
      height: on.height + 1,
      epochTime: on.epochTime,
    };
    this.blocks.set(ref.hash, txids);
    if (!parent) this.tip = ref;
    return ref;
  }

  /** Where a transaction is in a block, for a proof. */
  proofOf(block: Ref, tx: Tx) {
    const txids = this.blocks.get(block.hash)!;
    const index = txids.indexOf(tx.txid);
    return { siblings: merkle(txids, index).siblings, txIndex: index };
  }
}

/** What a tagged transaction carries for a job with this tag. */
function tagPayload(tag: string): string {
  return ethers.sha256(ethers.concat([ethers.toUtf8Bytes("iPoW job"), tag]));
}

/**
 * A transaction that spends one coin, and makes the next coin at output 0 and
 * an OP_RETURN at output 1. `tag` is an application's tag. `payload` is used
 * as it is.
 */
function makeTx(opts: {
  spends: { txid: string; vout: number };
  tag?: string;
  payload?: string;
}): Tx {
  const raw = buildTx({
    inputs: [{ txidLE: opts.spends.txid, vout: opts.spends.vout }],
    outputs: [
      { value: 546n, script: COIN_SCRIPT },
      {
        value: 0n,
        script: tagScript(opts.payload ?? tagPayload(opts.tag!)),
      },
    ],
  });
  return { raw, txid: txidLE(raw) };
}

async function deploy() {
  // The chain of these tests starts at block 0, so no lowest number.
  const lightClient = await ethers.deployContract("iPoWLightClientHarness", [0]);
  await lightClient.setLimits(targetFromBits(EASY), targetFromBits(EASY));
  // The harness keeps every rule of the real contract unless a test lowers the
  // limit of questions for a parent.
  const protocol = await ethers.deployContract("iPoWProtocolHarness", [
    await lightClient.getAddress(),
  ]);
  const [, application, user, operator, guardian, stranger, otherApplication] =
    await ethers.getSigners();
  await protocol.connect(application).registerApplication([7 * DAY]);
  await protocol.connect(otherApplication).registerApplication([]);

  // Round times make the arithmetic of the tests readable.
  const now = Math.ceil(((await latestTime()) + DAY) / DAY) * DAY;
  await mineAt(now);
  const chain = new TestChain(lightClient);
  await chain.start(now);

  return {
    lightClient,
    protocol,
    chain,
    application,
    otherApplication,
    user,
    operator,
    guardian,
    stranger,
    now,
  };
}

type Ctx = Awaited<ReturnType<typeof deploy>>;

/** The operator's first chain head: a coin made by a transaction that names it. */
async function registerChainHead(ctx: Ctx, who = ctx.operator) {
  const { protocol, chain } = ctx;
  const tx = makeTx({
    spends: { txid: ethers.id("funding-" + who.address), vout: 0 },
    payload: await protocol.chainHeadCommitment(who.address),
  });
  const block = await chain.add((await latestTime()) + 1, [tx]);
  const { siblings, txIndex } = chain.proofOf(block, tx);
  await protocol
    .connect(who)
    .registerChainHead(block, tx.raw, siblings, txIndex, 0, 1);
  return tx;
}

/** Opens a job, lets the operator win it, and waits until it is locked in. */
async function assignedJob(
  ctx: Ctx,
  opts: {
    tag?: string;
    confirmations?: number;
    claimKind?: number;
    bid?: bigint;
    application?: any;
  } = {}
) {
  const { protocol, user, operator } = ctx;
  const confirmations = opts.confirmations ?? 6;
  await setPrice(GWEI);
  const fee =
    ((110_000n * BigInt(24 + confirmations) + 520_000n) * GWEI * 3n) / 2n;
  const tx = await protocol
    .connect(opts.application ?? ctx.application)
    .openJob(
      opts.tag ?? TAG,
      ETH,
      50,
      confirmations,
      opts.claimKind ?? 0,
      user.address,
      fee + ESCROW_FEE, { value: fee + ESCROW_FEE, gasLimit: GAS }
    );
  const openedAt = (await ethers.provider.getBlock(
    (await tx.wait())!.blockNumber
  ))!.timestamp;
  const jobId = await protocol.jobCount();

  await at(openedAt + 1 * MINUTE);
  await protocol.connect(operator).bid(jobId, opts.bid ?? ETH);
  const lockedInAt = openedAt + 2 * MINUTE;
  await mineAt(lockedInAt);
  return { jobId, lockedInAt, fee };
}

/**
 * The usual duty: the operator names a fresh anchor, its tagged transaction is
 * in block `inBlock` of the window, and `onTop` blocks follow.
 */
async function duty(
  ctx: Ctx,
  jobId: bigint,
  opts: {
    tag?: string;
    payload?: string;
    anchor?: Ref;
    inBlock?: number;
    onTop?: number;
    spends?: { txid: string; vout: number };
  } = {}
) {
  const { protocol, chain, operator } = ctx;
  const head = await protocol.chainHeadOf(operator.address);

  let time = (await latestTime()) + 1;
  const anchor = opts.anchor ?? (await chain.add(time));
  chain.tip = anchor;
  await protocol.connect(operator).anchorJob(jobId, anchor);

  const tx = makeTx({
    spends: opts.spends ?? { txid: head.txid, vout: Number(head.vout) },
    tag: opts.tag ?? TAG,
    payload: opts.payload,
  });
  const inBlock = opts.inBlock ?? 1;
  for (let i = 1; i < inBlock; i++) await chain.add(++time + i);
  time = (await latestTime()) + 1;
  const proofBlock = await chain.add(time, [tx]);
  for (let i = 0; i < (opts.onTop ?? 5); i++) {
    await chain.add((await latestTime()) + 1);
  }

  const proof = {
    proofBlock,
    tip: chain.tip,
    prevEpochTime: 0,
    rawTx: tx.raw,
    ...chain.proofOf(proofBlock, tx),
    headIndex: 0,
    tagIndex: 1,
  };
  return { anchor, proofBlock, tx, proof };
}

async function readyOperator() {
  const ctx = await deploy();
  await ctx.protocol.connect(ctx.operator).lockBond(3n * ETH, { value: 3n * ETH });
  const genesis = await registerChainHead(ctx);
  return { ...ctx, genesis };
}

describe("iPoWProtocol: chain head (D30, D34)", function () {
  it("registers a coin made by a transaction that names the operator", async function () {
    const ctx = await deploy();
    const { protocol, operator } = ctx;
    const tx = await registerChainHead(ctx);
    const head = await protocol.chainHeadOf(operator.address);
    expect(head.set).to.equal(true);
    expect(head.txid).to.equal(tx.txid);
    expect(head.vout).to.equal(0n);
  });

  it("rejects a transaction that names another operator", async function () {
    const ctx = await deploy();
    const { protocol, chain, operator, stranger } = ctx;
    const tx = makeTx({
      spends: { txid: ethers.id("funding"), vout: 0 },
      payload: await protocol.chainHeadCommitment(operator.address),
    });
    const block = await chain.add((await latestTime()) + 1, [tx]);
    const { siblings, txIndex } = chain.proofOf(block, tx);
    await expect(
      protocol
        .connect(stranger)
        .registerChainHead(block, tx.raw, siblings, txIndex, 0, 1)
    ).to.be.revertedWithCustomError(protocol, "WrongTag");
  });

  it("rejects a transaction that is not in the block", async function () {
    const ctx = await deploy();
    const { protocol, chain, operator } = ctx;
    const tx = makeTx({
      spends: { txid: ethers.id("funding"), vout: 0 },
      payload: await protocol.chainHeadCommitment(operator.address),
    });
    const block = await chain.add((await latestTime()) + 1);
    await expect(
      protocol
        .connect(operator)
        .registerChainHead(block, tx.raw, [ZERO_HASH], 1, 0, 1)
    ).to.be.revertedWithCustomError(protocol, "NotInBlock");
  });

  it("rejects an output that is not a coin", async function () {
    const ctx = await deploy();
    const { protocol, chain, operator } = ctx;
    const tx = makeTx({
      spends: { txid: ethers.id("funding"), vout: 0 },
      payload: await protocol.chainHeadCommitment(operator.address),
    });
    const block = await chain.add((await latestTime()) + 1, [tx]);
    const { siblings, txIndex } = chain.proofOf(block, tx);
    // Output 1 is the OP_RETURN, output 2 does not exist.
    for (const coinIndex of [1, 2]) {
      await expect(
        protocol
          .connect(operator)
          .registerChainHead(block, tx.raw, siblings, txIndex, coinIndex, 1)
      ).to.be.revertedWithCustomError(protocol, "NoCoin");
    }
  });

  it("registers once", async function () {
    const ctx = await deploy();
    await registerChainHead(ctx);
    await expect(registerChainHead(ctx)).to.be.revertedWithCustomError(
      ctx.protocol,
      "ChainHeadExists"
    );
  });

  it("rejects a transaction with witness data, and one that is cut short", async function () {
    const ctx = await deploy();
    const { protocol, chain, operator } = ctx;
    const block = await chain.add((await latestTime()) + 1);
    const tx = makeTx({
      spends: { txid: ethers.id("funding"), vout: 0 },
      payload: await protocol.chainHeadCommitment(operator.address),
    });
    // Whatever is proven must first be in the block, so these are put in one.
    for (const [raw, error] of [
      [tx.raw.slice(0, 10) + "0001" + tx.raw.slice(10), "WitnessSerialization"],
      [tx.raw.slice(0, -2), "MalformedTx"],
      [tx.raw + "00", "MalformedTx"],
    ]) {
      const bad = { raw, txid: txidLE(raw) };
      const at = await chain.add((await latestTime()) + 1, [bad]);
      const { siblings, txIndex } = chain.proofOf(at, bad);
      await expect(
        protocol
          .connect(operator)
          .registerChainHead(at, bad.raw, siblings, txIndex, 0, 1)
      ).to.be.revertedWithCustomError(protocol, error);
    }
    expect(block.height).to.equal(6);
  });
});

describe("iPoWProtocol: a stranger cannot share an operator's chain head", function () {
  it("rejects a job's transaction as a registration, whatever tag the application chose", async function () {
    const ctx = await readyOperator();
    const { protocol, chain, operator, stranger } = ctx;
    // The stranger's application uses the stranger's commitment as its tag.
    const tag = await protocol.chainHeadCommitment(stranger.address);
    const { jobId } = await assignedJob(ctx, { tag });
    const { proof, tx, proofBlock } = await duty(ctx, jobId, { tag });

    // The OP_RETURN of the job's transaction is not the commitment.
    await expect(
      protocol
        .connect(stranger)
        .registerChainHead(proofBlock, tx.raw, proof.siblings, proof.txIndex, 0, 1)
    ).to.be.revertedWithCustomError(protocol, "WrongTag");

    await protocol.connect(operator).proveJob(jobId, proof);
    expect(chain.tip.height > proofBlock.height).to.equal(true);
  });

  it("marks the transaction of a registration as used, so it can never settle a job", async function () {
    const ctx = await deploy();
    const { protocol, chain, operator } = ctx;
    const tx = makeTx({
      spends: { txid: ethers.id("funding"), vout: 0 },
      payload: await protocol.chainHeadCommitment(operator.address),
    });
    const block = await chain.add((await latestTime()) + 1, [tx]);
    const { siblings, txIndex } = chain.proofOf(block, tx);
    await protocol
      .connect(operator)
      .registerChainHead(block, tx.raw, siblings, txIndex, 0, 1);
    expect(await protocol.usedTx(tx.txid)).to.equal(true);
  });

  it("does not accept the application's tag as it is, only marked as a job's tag", async function () {
    const ctx = await readyOperator();
    const { jobId } = await assignedJob(ctx);
    const { proof } = await duty(ctx, jobId, { payload: TAG } as any);
    await expect(
      ctx.protocol.connect(ctx.operator).proveJob(jobId, proof)
    ).to.be.revertedWithCustomError(ctx.protocol, "WrongTag");
  });
});

describe("iPoWProtocol: anchor of a job (D39)", function () {
  it("lets the operator name a fresh block as the anchor", async function () {
    const ctx = await readyOperator();
    const { protocol, chain, operator } = ctx;
    const { jobId } = await assignedJob(ctx);
    const anchor = await chain.add((await latestTime()) + 1);
    await expect(protocol.connect(operator).anchorJob(jobId, anchor))
      .to.emit(protocol, "JobAnchored")
      .withArgs(jobId, anchor.hash, anchor.height, anchor.epochTime);
    const recorded = (await protocol.getDuty(jobId)).anchor;
    expect(recorded.hash).to.equal(anchor.hash);
    expect(recorded.height).to.equal(BigInt(anchor.height));
  });

  it("rejects an anchor older than 2 hours, and accepts one exactly 2 hours old", async function () {
    const ctx = await readyOperator();
    const { protocol, chain, operator } = ctx;
    const { jobId } = await assignedJob(ctx);
    const time = (await latestTime()) + 1;
    const anchor = await chain.add(time);

    await mineAt(time + 2 * HOUR + 1);
    await expect(
      protocol.connect(operator).anchorJob(jobId, anchor)
    ).to.be.revertedWithCustomError(protocol, "AnchorTooOld");

    const fresh = await chain.add(time + 2 * HOUR + 2);
    await at(time + 4 * HOUR + 2);
    await protocol.connect(operator).anchorJob(jobId, fresh);
  });

  it("rejects anyone but the operator of the job", async function () {
    const ctx = await readyOperator();
    const { protocol, chain, stranger } = ctx;
    const { jobId } = await assignedJob(ctx);
    const anchor = await chain.add((await latestTime()) + 1);
    await expect(
      protocol.connect(stranger).anchorJob(jobId, anchor)
    ).to.be.revertedWithCustomError(protocol, "NotOperator");
  });

  it("rejects a block the light client does not hold", async function () {
    const ctx = await readyOperator();
    const { protocol, operator, chain } = ctx;
    const { jobId } = await assignedJob(ctx);
    await expect(
      protocol
        .connect(operator)
        .anchorJob(jobId, { ...chain.tip, height: chain.tip.height + 1 })
    ).to.be.revertedWithCustomError(protocol, "UnknownBlock");
  });

  it("names the anchor once", async function () {
    const ctx = await readyOperator();
    const { protocol, chain, operator } = ctx;
    const { jobId } = await assignedJob(ctx);
    const anchor = await chain.add((await latestTime()) + 1);
    await protocol.connect(operator).anchorJob(jobId, anchor);
    await expect(
      protocol.connect(operator).anchorJob(jobId, anchor)
    ).to.be.revertedWithCustomError(protocol, "AlreadyAnchored");
  });

  it("rejects an anchor while bidding is open", async function () {
    const ctx = await readyOperator();
    const { protocol, chain, operator, application, user } = ctx;
    await setPrice(GWEI);
    await protocol
      .connect(application)
      .openJob(TAG, ETH, 50, 6, 0, user.address, FEE + ESCROW_FEE, {
        value: FEE + ESCROW_FEE,
        gasLimit: GAS,
      });
    await protocol.connect(operator).bid(1, ETH);
    const anchor = await chain.add((await latestTime()) + 1);
    await expect(
      protocol.connect(operator).anchorJob(1, anchor)
    ).to.be.revertedWithCustomError(protocol, "NotAssigned");
  });
});

describe("iPoWProtocol: proof of the tagged transaction (D6, D9, D15, D27, D54)", function () {
  it("accepts a transaction in block 1 of the window with 5 blocks on top", async function () {
    const ctx = await readyOperator();
    const { protocol, operator } = ctx;
    const { jobId } = await assignedJob(ctx);
    const { proof, tx } = await duty(ctx, jobId);

    const sent = await protocol.connect(operator).proveJob(jobId, proof);
    const provenAt = (await ethers.provider.getBlock(
      (await sent.wait())!.blockNumber
    ))!.timestamp;
    await expect(sent)
      .to.emit(protocol, "JobProven")
      .withArgs(jobId, tx.txid, provenAt + 36 * HOUR);

    expect(await protocol.statusOf(jobId)).to.equal(4n);
    expect(await protocol.usedTx(tx.txid)).to.equal(true);
    // The output of the transaction is the operator's next chain head.
    const head = await protocol.chainHeadOf(operator.address);
    expect(head.txid).to.equal(tx.txid);
    expect(head.vout).to.equal(0n);
  });

  it("rejects 4 blocks on top: 6 confirmations are the block and 5 more (D15)", async function () {
    const ctx = await readyOperator();
    const { jobId } = await assignedJob(ctx);
    const { proof } = await duty(ctx, jobId, { onTop: 4 });
    await expect(
      ctx.protocol.connect(ctx.operator).proveJob(jobId, proof)
    ).to.be.revertedWithCustomError(ctx.protocol, "NotEnoughConfirmations");
  });

  it("accepts a transaction in block 25 and rejects one in block 26 (D15)", async function () {
    const ctx = await readyOperator();
    const first = await assignedJob(ctx, { tag: ethers.id("25") });
    const ok = await duty(ctx, first.jobId, { tag: ethers.id("25"), inBlock: 25 });
    await ctx.protocol.connect(ctx.operator).proveJob(first.jobId, ok.proof);

    const second = await assignedJob(ctx, { tag: ethers.id("26") });
    const late = await duty(ctx, second.jobId, {
      tag: ethers.id("26"),
      inBlock: 26,
    });
    await expect(
      ctx.protocol.connect(ctx.operator).proveJob(second.jobId, late.proof)
    ).to.be.revertedWithCustomError(ctx.protocol, "OutsideProofRange");
  });

  it("rejects the anchor itself as the block of the proof", async function () {
    const ctx = await readyOperator();
    const { jobId } = await assignedJob(ctx);
    const { proof, anchor } = await duty(ctx, jobId);
    await expect(
      ctx.protocol.connect(ctx.operator).proveJob(jobId, { ...proof, proofBlock: anchor })
    ).to.be.revertedWithCustomError(ctx.protocol, "OutsideProofRange");
  });

  it("needs one more block on top for each confirmation above 6 (D41, D68)", async function () {
    const ctx = await readyOperator();
    const { jobId } = await assignedJob(ctx, { confirmations: 8 });
    const { proof } = await duty(ctx, jobId, { onTop: 6 });
    await expect(
      ctx.protocol.connect(ctx.operator).proveJob(jobId, proof)
    ).to.be.revertedWithCustomError(ctx.protocol, "NotEnoughConfirmations");

    await ctx.chain.add((await latestTime()) + 1);
    await ctx.protocol.connect(ctx.operator).proveJob(jobId, { ...proof, tip: ctx.chain.tip });
  });

  it("rejects a transaction with another tag (D6)", async function () {
    const ctx = await readyOperator();
    const { jobId } = await assignedJob(ctx);
    const { proof } = await duty(ctx, jobId, { tag: ethers.id("other") });
    await expect(
      ctx.protocol.connect(ctx.operator).proveJob(jobId, proof)
    ).to.be.revertedWithCustomError(ctx.protocol, "WrongTag");
  });

  it("rejects a transaction that does not spend the operator's chain head (D30)", async function () {
    const ctx = await readyOperator();
    const { jobId } = await assignedJob(ctx);
    const { proof } = await duty(ctx, jobId, {
      spends: { txid: ethers.id("someone else's coin"), vout: 0 },
    });
    await expect(
      ctx.protocol.connect(ctx.operator).proveJob(jobId, proof)
    ).to.be.revertedWithCustomError(ctx.protocol, "WrongChainHead");
  });

  it("rejects the chain head at another output number", async function () {
    const ctx = await readyOperator();
    const { jobId } = await assignedJob(ctx);
    const { proof } = await duty(ctx, jobId, {
      spends: { txid: ctx.genesis.txid, vout: 1 },
    });
    await expect(
      ctx.protocol.connect(ctx.operator).proveJob(jobId, proof)
    ).to.be.revertedWithCustomError(ctx.protocol, "WrongChainHead");
  });

  it("rejects a proof after the deadline (D27)", async function () {
    const ctx = await readyOperator();
    const { jobId, lockedInAt } = await assignedJob(ctx);
    const { proof } = await duty(ctx, jobId);

    await at(lockedInAt + DAY + 1);
    await expect(
      ctx.protocol.connect(ctx.operator).proveJob(jobId, proof)
    ).to.be.revertedWithCustomError(ctx.protocol, "DeadlinePassed");
  });

  it("accepts a proof at the last second of the deadline", async function () {
    const ctx = await readyOperator();
    const { jobId, lockedInAt } = await assignedJob(ctx);
    const { proof } = await duty(ctx, jobId);
    await at(lockedInAt + DAY);
    await ctx.protocol.connect(ctx.operator).proveJob(jobId, proof);
  });

  it("rejects a proof for a job with no anchor", async function () {
    const ctx = await readyOperator();
    const first = await assignedJob(ctx, { tag: ethers.id("a") });
    const { proof } = await duty(ctx, first.jobId, { tag: ethers.id("b") });
    const second = await assignedJob(ctx, { tag: ethers.id("b") });
    await expect(
      ctx.protocol.connect(ctx.operator).proveJob(second.jobId, proof)
    ).to.be.revertedWithCustomError(ctx.protocol, "NotAnchored");
  });

  it("rejects a block that does not follow the anchor of the job", async function () {
    const ctx = await readyOperator();
    const { protocol, chain, operator } = ctx;
    const { jobId } = await assignedJob(ctx);

    const time = (await latestTime()) + 1;
    const parent = chain.tip;
    const anchor = await chain.add(time);
    await protocol.connect(operator).anchorJob(jobId, anchor);

    // A second branch from the block before the anchor.
    const head = await protocol.chainHeadOf(operator.address);
    const tx = makeTx({
      spends: { txid: head.txid, vout: Number(head.vout) },
      tag: TAG,
    });
    let on = await chain.add(time + 1, [], parent);
    const proofBlock = await chain.add(time + 2, [tx], on);
    on = proofBlock;
    for (let i = 0; i < 5; i++) on = await chain.add(time + 3 + i, [], on);

    await expect(
      protocol.connect(operator).proveJob(jobId, {
        proofBlock,
        tip: on,
        prevEpochTime: 0,
        rawTx: tx.raw,
        ...chain.proofOf(proofBlock, tx),
        headIndex: 0,
        tagIndex: 1,
      })
    ).to.be.revertedWithCustomError(protocol, "NotLinked");
  });

  it("rejects a transaction that is not in the block", async function () {
    const ctx = await readyOperator();
    const { jobId } = await assignedJob(ctx);
    const { proof, anchor } = await duty(ctx, jobId);

    // The block after the one that holds the transaction, with 5 on top.
    const next = await ctx.lightClient.getNode(
      await ctx.lightClient.nodeId(
        ctx.chain.tip.hash,
        ctx.chain.tip.height,
        ctx.chain.tip.epochTime
      )
    );
    expect(next.height).to.equal(BigInt(anchor.height + 6));
    const other = await ctx.chain.add((await latestTime()) + 1);
    const wrongBlock = {
      hash: (
        await ctx.lightClient.getNode(
          await ctx.lightClient.nodeId(
            other.hash,
            other.height,
            other.epochTime
          )
        )
      ).prevHash,
      height: other.height - 1,
      epochTime: other.epochTime,
    };
    for (let i = 0; i < 5; i++) await ctx.chain.add((await latestTime()) + 1);

    await expect(
      ctx.protocol.connect(ctx.operator).proveJob(jobId, {
        ...proof,
        proofBlock: wrongBlock,
        tip: ctx.chain.tip,
      })
    ).to.be.revertedWithCustomError(ctx.protocol, "NotInBlock");
  });

  it("uses a transaction for one job only (D80)", async function () {
    const ctx = await readyOperator();
    // Two applications open a job with the same tag, and the same operator
    // wins both and names the same anchor for both.
    const first = await assignedJob(ctx);
    const second = await assignedJob(ctx, {
      application: ctx.otherApplication,
    });
    const { proof, anchor } = await duty(ctx, first.jobId);
    await ctx.protocol.connect(ctx.operator).anchorJob(second.jobId, anchor);

    await ctx.protocol.connect(ctx.operator).proveJob(first.jobId, proof);
    await expect(
      ctx.protocol.connect(ctx.operator).proveJob(second.jobId, proof)
    ).to.be.revertedWithCustomError(ctx.protocol, "TransactionUsed");
  });

  it("proves the jobs of one operator in the order of its chain (D34)", async function () {
    const ctx = await readyOperator();
    const { protocol, operator } = ctx;
    const first = await assignedJob(ctx, { tag: ethers.id("1") });
    const one = await duty(ctx, first.jobId, { tag: ethers.id("1") });
    await protocol.connect(operator).proveJob(first.jobId, one.proof);

    const second = await assignedJob(ctx, { tag: ethers.id("2") });
    // The next transaction spends the output of the first.
    const two = await duty(ctx, second.jobId, { tag: ethers.id("2") });
    await protocol.connect(operator).proveJob(second.jobId, two.proof);
    expect((await protocol.chainHeadOf(operator.address)).txid).to.equal(
      two.tx.txid
    );
  });

  it("accepts the proof from the operator of the job only", async function () {
    const ctx = await readyOperator();
    const { jobId } = await assignedJob(ctx);
    const { proof } = await duty(ctx, jobId);
    await expect(
      ctx.protocol.connect(ctx.stranger).proveJob(jobId, proof)
    ).to.be.revertedWithCustomError(ctx.protocol, "NotOperator");
  });

  it("proves a job once", async function () {
    const ctx = await readyOperator();
    const { jobId } = await assignedJob(ctx);
    const { proof } = await duty(ctx, jobId);
    await ctx.protocol.connect(ctx.operator).proveJob(jobId, proof);
    await expect(
      ctx.protocol.connect(ctx.operator).proveJob(jobId, proof)
    ).to.be.revertedWithCustomError(ctx.protocol, "NotAssigned");
  });
});

describe("iPoWProtocol: moving the chain head past a transaction", function () {
  it("lets the operator move on after a transaction that came too late", async function () {
    const ctx = await readyOperator();
    const { protocol, chain, operator } = ctx;
    const { jobId, lockedInAt } = await assignedJob(ctx);
    const { proof, tx, proofBlock } = await duty(ctx, jobId);
    await mineAt(lockedInAt + DAY + 1);

    await protocol
      .connect(operator)
      .advanceChainHead(
        proofBlock,
        tx.raw,
        proof.siblings,
        proof.txIndex,
        0
      );
    expect((await protocol.chainHeadOf(operator.address)).txid).to.equal(
      tx.txid
    );
    expect(await protocol.usedTx(tx.txid)).to.equal(true);
    expect(chain.tip.height > proofBlock.height).to.equal(true);
  });

  it("does not let anyone else move an operator's chain head", async function () {
    const ctx = await readyOperator();
    const { protocol, stranger, operator } = ctx;
    const { jobId } = await assignedJob(ctx);
    const { proof, tx, proofBlock } = await duty(ctx, jobId);

    // A stranger with no chain head.
    await expect(
      protocol
        .connect(stranger)
        .advanceChainHead(proofBlock, tx.raw, proof.siblings, proof.txIndex, 0)
    ).to.be.revertedWithCustomError(protocol, "NoChainHead");

    // A stranger with a chain head of its own.
    await registerChainHead(ctx, stranger);
    await expect(
      protocol
        .connect(stranger)
        .advanceChainHead(proofBlock, tx.raw, proof.siblings, proof.txIndex, 0)
    ).to.be.revertedWithCustomError(protocol, "WrongChainHead");

    // The operator's proof still works.
    await protocol.connect(operator).proveJob(jobId, proof);
  });
});

describe("iPoWProtocol: the lock and the payment (D23, D50, D67, D71)", function () {
  async function proven(opts: Parameters<typeof assignedJob>[1] = {}) {
    const ctx = await readyOperator();
    const job = await assignedJob(ctx, opts);
    const { proof } = await duty(ctx, job.jobId, {
      onTop: (opts?.confirmations ?? 6) - 1,
    });
    const sent = await ctx.protocol.connect(ctx.operator).proveJob(job.jobId, proof);
    const provenAt = (await ethers.provider.getBlock(
      (await sent.wait())!.blockNumber
    ))!.timestamp;
    return { ...ctx, ...job, provenAt };
  }

  it("pays nothing before the lock has ended (D67)", async function () {
    const { protocol, jobId, provenAt } = await proven();
    await at(provenAt + 36 * HOUR - 1);
    await expect(protocol.settle(jobId)).to.be.revertedWithCustomError(
      protocol,
      "LockNotEnded"
    );
  });

  it("pays both fees to the operator and frees its bond after 36 hours (D23, D50)", async function () {
    const { protocol, operator, stranger, jobId, provenAt, fee } =
      await proven({ bid: 2n * ETH });
    expect(await protocol.bondOf(operator.address)).to.deep.equal([
      3n * ETH,
      2n * ETH,
    ]);

    await at(provenAt + 36 * HOUR);
    await expect(protocol.connect(stranger).settle(jobId))
      .to.emit(protocol, "JobSettled")
      .withArgs(jobId, operator.address, fee + ESCROW_FEE);

    expect(await protocol.statusOf(jobId)).to.equal(6n);
    expect(await protocol.bondOf(operator.address)).to.deep.equal([
      3n * ETH,
      0n,
    ]);
    expect(await protocol.credit(operator.address)).to.equal(fee + ESCROW_FEE);
    expect(await protocol.feesHeldOf(jobId)).to.equal(0n);
    await protocol.connect(operator).withdrawBond(3n * ETH);
  });

  it("pays once", async function () {
    const { protocol, jobId, provenAt } = await proven();
    await at(provenAt + 36 * HOUR);
    await protocol.settle(jobId);
    await expect(protocol.settle(jobId)).to.be.revertedWithCustomError(
      protocol,
      "NotProven"
    );
  });

  it("locks a job with a long deadline for 1.5 times its deadline (D71)", async function () {
    const { protocol, jobId, provenAt } = await proven({ confirmations: 76 });
    expect(await protocol.lockTimeOf(jobId)).to.equal(BigInt(120 * HOUR));
    expect((await protocol.getDuty(jobId)).lockEnd).to.equal(
      BigInt(provenAt + 120 * HOUR)
    );
  });

  it("locks a job that makes a claim for its whole challenge period, from the proof (D66, D74)", async function () {
    const { protocol, jobId, provenAt } = await proven({ claimKind: 1 });
    expect(await protocol.lockTimeOf(jobId)).to.equal(BigInt(7 * DAY));
    await at(provenAt + 7 * DAY - 1);
    await expect(protocol.settle(jobId)).to.be.revertedWithCustomError(
      protocol,
      "LockNotEnded"
    );
    await at(provenAt + 7 * DAY);
    await protocol.settle(jobId);
  });

  it("does not pay for a job that was not proven", async function () {
    const ctx = await readyOperator();
    const { jobId, lockedInAt } = await assignedJob(ctx);
    await mineAt(lockedInAt + 10 * DAY);
    await expect(ctx.protocol.settle(jobId)).to.be.revertedWithCustomError(
      ctx.protocol,
      "NotProven"
    );
  });
});

describe("iPoWProtocol: a missed duty (D7, D26, D35, D36, D47, D53, D62)", function () {
  async function missed(bid = ETH) {
    const ctx = await readyOperator();
    const job = await assignedJob(ctx, { bid });
    const note = await ctx.protocol.noteFor(
      ctx.guardian.address,
      job.jobId,
      ZERO_HASH,
      SALT
    );
    return { ...ctx, ...job, note };
  }

  it("slashes the full escrow: 80% to the application, 20% to the guardian, the fees to the user", async function () {
    const {
      protocol,
      application,
      user,
      operator,
      guardian,
      jobId,
      lockedInAt,
      note,
      fee,
    } = await missed();
    await protocol.connect(guardian).sealNote(note);

    await at(lockedInAt + DAY + 1);
    await expect(protocol.connect(guardian).reportMissedDuty(jobId, SALT))
      .to.emit(protocol, "JobSlashed")
      .withArgs(
        jobId,
        operator.address,
        guardian.address,
        (ETH * 8n) / 10n,
        (ETH * 2n) / 10n
      );

    expect(await protocol.statusOf(jobId)).to.equal(5n);
    expect(await protocol.credit(application.address)).to.equal(
      (ETH * 8n) / 10n
    );
    expect(await protocol.credit(guardian.address)).to.equal((ETH * 2n) / 10n);
    expect(await protocol.credit(user.address)).to.equal(fee + ESCROW_FEE);
    expect(await protocol.bondOf(operator.address)).to.deep.equal([
      2n * ETH,
      0n,
    ]);
  });

  it("takes x, not what the operator locked above x (D26)", async function () {
    const { protocol, operator, guardian, jobId, lockedInAt, note } =
      await missed(2n * ETH);
    await protocol.connect(guardian).sealNote(note);
    await at(lockedInAt + DAY + 1);
    await protocol.connect(guardian).reportMissedDuty(jobId, SALT);
    // 3 ETH of bond, 2 ETH locked for the job, 1 ETH slashed.
    expect(await protocol.bondOf(operator.address)).to.deep.equal([
      2n * ETH,
      0n,
    ]);
    await protocol.connect(operator).withdrawBond(2n * ETH);
  });

  it("does not slash before the deadline has passed", async function () {
    const { protocol, guardian, jobId, lockedInAt, note } = await missed();
    await protocol.connect(guardian).sealNote(note);
    await at(lockedInAt + DAY);
    await expect(
      protocol.connect(guardian).reportMissedDuty(jobId, SALT)
    ).to.be.revertedWithCustomError(protocol, "DeadlineNotPassed");
  });

  it("does not slash a job whose proof was accepted", async function () {
    const ctx = await missed();
    const { protocol, guardian, operator, jobId, lockedInAt, note } = ctx;
    await protocol.connect(guardian).sealNote(note);
    const { proof } = await duty(ctx, jobId);
    await protocol.connect(operator).proveJob(jobId, proof);

    await at(lockedInAt + DAY + 1);
    await expect(
      protocol.connect(guardian).reportMissedDuty(jobId, SALT)
    ).to.be.revertedWithCustomError(protocol, "NotAssigned");
  });

  it("needs the guardian's sealed note (D47)", async function () {
    const { protocol, guardian, jobId, lockedInAt } = await missed();
    await at(lockedInAt + DAY + 1);
    await expect(
      protocol.connect(guardian).reportMissedDuty(jobId, SALT)
    ).to.be.revertedWithCustomError(protocol, "NoNote");
  });

  it("does not let someone copy a guardian's report (D47)", async function () {
    const { protocol, guardian, stranger, jobId, lockedInAt, note } =
      await missed();
    await protocol.connect(guardian).sealNote(note);
    await at(lockedInAt + DAY + 1);
    // The copier sees the salt in the guardian's transaction. The note was
    // sealed for the guardian, not for the copier.
    await expect(
      protocol.connect(stranger).reportMissedDuty(jobId, SALT)
    ).to.be.revertedWithCustomError(protocol, "NoNote");
    await protocol.connect(guardian).reportMissedDuty(jobId, SALT);
  });

  it("needs the note to be sealed in an earlier block (D47)", async function () {
    const { protocol, guardian, jobId, lockedInAt, note } = await missed();
    await mineAt(lockedInAt + DAY + 1);

    await ethers.provider.send("evm_setAutomine", [false]);
    try {
      const seal = await protocol
        .connect(guardian)
        .sealNote(note, { gasLimit: GAS });
      const report = await protocol
        .connect(guardian)
        .reportMissedDuty(jobId, SALT, { gasLimit: GAS });
      await ethers.provider.send("evm_mine", []);

      expect((await ethers.provider.getTransactionReceipt(seal.hash))!.status)
        .to.equal(1);
      expect(
        (await ethers.provider.getTransactionReceipt(report.hash))!.status
      ).to.equal(0);
    } finally {
      await ethers.provider.send("evm_setAutomine", [true]);
    }
    expect(await protocol.statusOf(jobId)).to.equal(3n);

    // In the next block the same report is accepted.
    await protocol.connect(guardian).reportMissedDuty(jobId, SALT);
    expect(await protocol.statusOf(jobId)).to.equal(5n);
  });

  it("seals a note once and uses it once", async function () {
    const { protocol, guardian, jobId, lockedInAt, note } = await missed();
    await protocol.connect(guardian).sealNote(note);
    await expect(
      protocol.connect(guardian).sealNote(note)
    ).to.be.revertedWithCustomError(protocol, "NoteExists");

    await at(lockedInAt + DAY + 1);
    await protocol.connect(guardian).reportMissedDuty(jobId, SALT);
    await expect(
      protocol.connect(guardian).reportMissedDuty(jobId, SALT)
    ).to.be.revertedWithCustomError(protocol, "NotAssigned");
  });
});

describe("iPoWProtocol: what the contract holds", function () {
  it("is the bonds, the fees it has not paid or returned, and the credits", async function () {
    const ctx = await readyOperator();
    const { protocol, application, user, operator, guardian } = ctx;

    async function check(jobIds: bigint[]) {
      let expected = (await protocol.bondOf(operator.address))[0];
      for (const id of jobIds) expected += await protocol.feesHeldOf(id);
      for (const who of [application, user, operator, guardian]) {
        expected += await protocol.credit(who.address);
      }
      expect(
        await ethers.provider.getBalance(await protocol.getAddress())
      ).to.equal(expected);
    }

    // One job is done and paid, one is missed and slashed.
    const done = await assignedJob(ctx, { tag: ethers.id("done"), bid: 2n * ETH });
    const { proof } = await duty(ctx, done.jobId, { tag: ethers.id("done") });
    const sent = await protocol.connect(operator).proveJob(done.jobId, proof);
    const provenAt = (await ethers.provider.getBlock(
      (await sent.wait())!.blockNumber
    ))!.timestamp;
    const jobs = [done.jobId];
    await check(jobs);

    const missed = await assignedJob(ctx, { tag: ethers.id("missed") });
    jobs.push(missed.jobId);
    await protocol
      .connect(guardian)
      .sealNote(
        await protocol.noteFor(guardian.address, missed.jobId, ZERO_HASH, SALT)
      );
    await check(jobs);

    await at(provenAt + 36 * HOUR);
    await protocol.settle(done.jobId);
    await check(jobs);

    await protocol.connect(guardian).reportMissedDuty(missed.jobId, SALT);
    await check(jobs);

    for (const who of [application, user, operator, guardian]) {
      await protocol.connect(who).withdrawCredit();
    }
    await check(jobs);
    expect(await protocol.bondOf(operator.address)).to.deep.equal([
      2n * ETH,
      0n,
    ]);
  });
});

describe("iPoWProtocol: challenge of a proof (D49, D81)", function () {
  async function provenJob() {
    const ctx = await readyOperator();
    const job = await assignedJob(ctx);
    // The block before the anchor, where a competing branch can start.
    const parent = ctx.chain.tip;
    const d = await duty(ctx, job.jobId);
    const sent = await ctx.protocol
      .connect(ctx.operator)
      .proveJob(job.jobId, d.proof);
    const provenAt = (await ethers.provider.getBlock(
      (await sent.wait())!.blockNumber
    ))!.timestamp;
    const deposit = (await ctx.protocol.getJob(job.jobId)).commitmentFee;
    return { ...ctx, ...job, ...d, parent, provenAt, deposit };
  }

  async function seal(ctx: any, jobId: bigint, evidence: string) {
    await ctx.protocol
      .connect(ctx.guardian)
      .sealNote(
        await ctx.protocol.noteFor(ctx.guardian.address, jobId, evidence, SALT)
      );
  }

  /** Mines `count` blocks on `from` without moving the test chain's tip. */
  async function branch(ctx: any, from: Ref, count: number) {
    let on = from;
    const refs: Ref[] = [];
    for (let i = 0; i < count; i++) {
      on = await ctx.chain.add((await latestTime()) + 1, [], on);
      refs.push(on);
    }
    return refs;
  }

  describe("asking for the parent of the anchor", function () {
    it("fails when anyone shows the parent: the deposit goes to the operator", async function () {
      const ctx = await provenJob();
      const { protocol, guardian, operator, stranger, jobId, anchor, deposit } = ctx;
      await seal(ctx, jobId, await protocol.parentEvidence(anchor.hash));
      await protocol.connect(guardian).askParent(jobId, SALT, { value: deposit });
      const cid = await protocol.challengeCount();
      expect((await protocol.getChallenge(cid)).kind).to.equal(1n);
      expect(await protocol.openChallengesOf(jobId)).to.equal(1n);

      // The parent is a real block the light client holds already.
      await expect(protocol.connect(stranger).showParent(cid, 0))
        .to.emit(protocol, "ChallengeFailed")
        .withArgs(cid, guardian.address);
      expect(await protocol.credit(operator.address)).to.equal(deposit);
      const d = await protocol.getDuty(jobId);
      expect(d.deepest.hash).to.equal(ctx.parent.hash);
      expect(d.parentsShown).to.equal(1n);
      expect((await protocol.getChallenge(cid)).kind).to.equal(0n);
      expect(await protocol.openChallengesOf(jobId)).to.equal(0n);
    });

    it("makes the proof false when nobody shows the parent in 12 hours", async function () {
      const ctx = await readyOperator();
      const { protocol, lightClient, chain, application, user, operator, guardian } = ctx;
      const job = await assignedJob(ctx);

      // A forger's anchor: a block whose parent nobody has.
      const time = (await latestTime()) + 1;
      const header = mine({ prevLE: ethers.id("nowhere"), time, bits: EASY });
      await lightClient.jump(header, chain.epochStartId, 50);
      const anchor = { hash: headerHashLE(header), height: 50, epochTime: chain.epochTime };
      const d = await duty(ctx, job.jobId, { anchor });
      await protocol.connect(operator).proveJob(job.jobId, d.proof);
      const deposit = (await protocol.getJob(job.jobId)).commitmentFee;

      await seal(ctx, job.jobId, await protocol.parentEvidence(anchor.hash));
      const asked = await protocol
        .connect(guardian)
        .askParent(job.jobId, SALT, { value: deposit });
      const askedAt = (await ethers.provider.getBlock(
        (await asked.wait())!.blockNumber
      ))!.timestamp;
      const cid = await protocol.challengeCount();

      await expect(protocol.showParent(cid, 0)).to.be.revertedWithCustomError(
        protocol,
        "NoParent"
      );
      await at(askedAt + 12 * HOUR - 1);
      await expect(protocol.resolveChallenge(cid)).to.be.revertedWithCustomError(
        protocol,
        "ResponseTimeNotOver"
      );

      // At the last second the parent can no longer be shown, and nothing can
      // be paid out: the challenge is resolved first.
      await at(askedAt + 12 * HOUR);
      await expect(protocol.showParent(cid, 0)).to.be.revertedWithCustomError(
        protocol,
        "ResponseTimeOver"
      );
      await at(askedAt + 12 * HOUR + 1);
      await expect(protocol.resolveChallenge(cid))
        .to.emit(protocol, "JobSlashed")
        .withArgs(job.jobId, operator.address, guardian.address, (ETH * 8n) / 10n, (ETH * 2n) / 10n);
      expect(await protocol.statusOf(job.jobId)).to.equal(5n);
      expect(await protocol.credit(application.address)).to.equal((ETH * 8n) / 10n);
      // The reward and the deposit back.
      expect(await protocol.credit(guardian.address)).to.equal((ETH * 2n) / 10n + deposit);
      expect(await protocol.credit(user.address)).to.equal(job.fee + ESCROW_FEE);
    });

    it("lets a forger answer by mining one more parent, each time it is asked", async function () {
      const ctx = await readyOperator();
      const { protocol, lightClient, chain, operator, guardian } = ctx;
      const job = await assignedJob(ctx);

      const time = (await latestTime()) + 1;
      const p = mine({ prevLE: ethers.id("nowhere"), time, bits: EASY });
      const header = mine({ prevLE: headerHashLE(p), time: time + 1, bits: EASY });
      await lightClient.jump(header, chain.epochStartId, 50);
      const anchor = { hash: headerHashLE(header), height: 50, epochTime: chain.epochTime };
      const d = await duty(ctx, job.jobId, { anchor });
      await protocol.connect(operator).proveJob(job.jobId, d.proof);
      const deposit = (await protocol.getJob(job.jobId)).commitmentFee;

      await seal(ctx, job.jobId, await protocol.parentEvidence(anchor.hash));
      await protocol.connect(guardian).askParent(job.jobId, SALT, { value: deposit });
      await lightClient.extendBack(p, anchor.hash, 50, chain.epochTime, 0);
      await protocol.showParent(await protocol.challengeCount(), 0);

      // The next question is for the parent of the parent. It is question 2,
      // so it costs 2 deposits (D91).
      expect(await protocol.parentDepositOf(job.jobId)).to.equal(2n * deposit);
      const deeper = await protocol.parentEvidence(headerHashLE(p));
      await protocol
        .connect(guardian)
        .sealNote(await protocol.noteFor(guardian.address, job.jobId, deeper, ethers.id("2")));
      await expect(
        protocol.connect(guardian).askParent(job.jobId, ethers.id("2"), { value: deposit })
      ).to.be.revertedWithCustomError(protocol, "WrongValue");
      await protocol
        .connect(guardian)
        .askParent(job.jobId, ethers.id("2"), { value: 2n * deposit });
      const cid = await protocol.challengeCount();
      const c = await protocol.getChallenge(cid);
      expect(c.kind).to.equal(1n);
      expect(c.asked.hash).to.equal(headerHashLE(p));
      expect(c.deposit).to.equal(2n * deposit);
    });

    it("keeps the lock fixed, and opens the last challenge 12 hours before it ends (D99)", async function () {
      const ctx = await provenJob();
      const { protocol, guardian, stranger, anchor, jobId, provenAt, deposit } = ctx;
      const lockEnd = provenAt + 36 * HOUR;
      const evidence = await protocol.parentEvidence(anchor.hash);
      await seal(ctx, jobId, evidence);
      await protocol
        .connect(stranger)
        .sealNote(await protocol.noteFor(stranger.address, jobId, evidence, SALT));

      // Exactly 12 hours before the end is the last moment.
      await at(lockEnd - 12 * HOUR);
      await protocol.connect(guardian).askParent(jobId, SALT, { value: deposit });
      expect((await protocol.getDuty(jobId)).lockEnd).to.equal(BigInt(lockEnd));

      await at(lockEnd - 12 * HOUR + 1);
      await expect(
        protocol.connect(stranger).askParent(jobId, SALT, { value: deposit })
      ).to.be.revertedWithCustomError(protocol, "ChallengeWindowClosed");

      // The answer comes before the lock ends, and the operator is paid then.
      await at(lockEnd - 60);
      await protocol.showParent(await protocol.challengeCount(), 0);
      expect((await protocol.getDuty(jobId)).lockEnd).to.equal(BigInt(lockEnd));
      await at(lockEnd);
      await protocol.settle(jobId);
    });

    it("opens a competing-branch challenge up to exactly 12 hours before the end, not a second later (D99)", async function () {
      const ctx = await provenJob();
      const { protocol, guardian, anchor, parent, jobId, provenAt, deposit } = ctx;
      const lockEnd = provenAt + 36 * HOUR;
      const g = await branch(ctx, parent, 8);
      await seal(ctx, jobId, await protocol.forkEvidence(g[0].hash));
      const salt2 = ethers.id("2");
      await protocol
        .connect(guardian)
        .sealNote(await protocol.noteFor(guardian.address, jobId, await protocol.forkEvidence(g[0].hash), salt2));

      await at(lockEnd - 12 * HOUR + 1);
      await expect(
        protocol.connect(guardian).challengeFork(jobId, anchor, g[0], g[7], 0, SALT, { value: deposit })
      ).to.be.revertedWithCustomError(protocol, "ChallengeWindowClosed");

      const fresh = await provenJob();
      const g2 = await branch(fresh, fresh.parent, 8);
      await fresh.protocol
        .connect(fresh.guardian)
        .sealNote(
          await fresh.protocol.noteFor(fresh.guardian.address, fresh.jobId, await fresh.protocol.forkEvidence(g2[0].hash), SALT)
        );
      await at(fresh.provenAt + 24 * HOUR);
      await fresh.protocol
        .connect(fresh.guardian)
        .challengeFork(fresh.jobId, fresh.anchor, g2[0], g2[7], 0, SALT, { value: fresh.deposit });
    });

    it("accepts the answer to the last question until the second before the lock ends", async function () {
      const ctx = await provenJob();
      const { protocol, guardian, anchor, jobId, provenAt, deposit } = ctx;
      const lockEnd = provenAt + 36 * HOUR;
      await seal(ctx, jobId, await protocol.parentEvidence(anchor.hash));
      await at(lockEnd - 12 * HOUR);
      await protocol.connect(guardian).askParent(jobId, SALT, { value: deposit });
      await at(lockEnd - 1);
      await protocol.showParent(await protocol.challengeCount(), 0);
      await at(lockEnd);
      await protocol.settle(jobId);
    });

    it("does not pay the operator while a question is open at the end of the lock", async function () {
      const ctx = await provenJob();
      const { protocol, guardian, anchor, jobId, provenAt, deposit } = ctx;
      const lockEnd = provenAt + 36 * HOUR;
      await seal(ctx, jobId, await protocol.parentEvidence(anchor.hash));
      await at(lockEnd - 12 * HOUR);
      await protocol.connect(guardian).askParent(jobId, SALT, { value: deposit });

      // At the end the question is still open: it cannot be answered any more,
      // and the operator cannot be paid until it is resolved.
      await at(lockEnd);
      await expect(protocol.settle(jobId)).to.be.revertedWithCustomError(protocol, "ChallengeOpen");
      await protocol.resolveChallenge(await protocol.challengeCount());
      expect(await protocol.statusOf(jobId)).to.equal(5n);
    });

    it("rejects a wrong deposit, a missing note, and a challenge after the lock", async function () {
      const ctx = await provenJob();
      const { protocol, guardian, stranger, anchor, jobId, provenAt, deposit } = ctx;
      const evidence = await protocol.parentEvidence(anchor.hash);

      await expect(
        protocol.connect(guardian).askParent(jobId, SALT, { value: deposit })
      ).to.be.revertedWithCustomError(protocol, "NoNote");

      await seal(ctx, jobId, evidence);
      await expect(
        protocol.connect(guardian).askParent(jobId, SALT, { value: deposit - 1n })
      ).to.be.revertedWithCustomError(protocol, "WrongValue");

      await protocol.connect(guardian).askParent(jobId, SALT, { value: deposit });
      await protocol.showParent(await protocol.challengeCount(), 0);
      await at(provenAt + 24 * HOUR + 1);
      await protocol
        .connect(stranger)
        .sealNote(
          await protocol.noteFor(
            stranger.address,
            jobId,
            await protocol.parentEvidence(ctx.parent.hash),
            SALT
          )
        );
      await expect(
        protocol.connect(stranger).askParent(jobId, SALT, { value: 2n * deposit })
      ).to.be.revertedWithCustomError(protocol, "ChallengeWindowClosed");
    });

    it("lets several guardians challenge at once, so an operator cannot keep the place busy (D88)", async function () {
      const ctx = await provenJob();
      const { protocol, guardian, stranger, operator, anchor, jobId, deposit } = ctx;
      const evidence = await protocol.parentEvidence(anchor.hash);

      // The operator challenges itself from another address.
      await protocol
        .connect(stranger)
        .sealNote(await protocol.noteFor(stranger.address, jobId, evidence, SALT));
      await seal(ctx, jobId, evidence);
      await protocol.connect(stranger).askParent(jobId, SALT, { value: deposit });
      const own = await protocol.challengeCount();

      // A real guardian still gets in.
      await protocol.connect(guardian).askParent(jobId, SALT, { value: deposit });
      const real = await protocol.challengeCount();
      expect(real).to.equal(own + 1n);
      expect(await protocol.openChallengesOf(jobId)).to.equal(2n);

      // One answer moves the question on. Both challenges are answered.
      await protocol.showParent(own, 0);
      await protocol.showParent(real, 0);
      expect((await protocol.getDuty(jobId)).parentsShown).to.equal(1n);
      expect(await protocol.openChallengesOf(jobId)).to.equal(0n);
      expect(await protocol.credit(operator.address)).to.equal(2n * deposit);
    });

    it("gives the deposit back to a guardian whose challenge is open when another has slashed the job (D88)", async function () {
      const ctx = await readyOperator();
      const { protocol, lightClient, chain, operator, guardian, stranger } = ctx;
      const job = await assignedJob(ctx);
      const time = (await latestTime()) + 1;
      const header = mine({ prevLE: ethers.id("nowhere"), time, bits: EASY });
      await lightClient.jump(header, chain.epochStartId, 50);
      const anchor = { hash: headerHashLE(header), height: 50, epochTime: chain.epochTime };
      const d = await duty(ctx, job.jobId, { anchor });
      await protocol.connect(operator).proveJob(job.jobId, d.proof);
      const deposit = (await protocol.getJob(job.jobId)).commitmentFee;
      const evidence = await protocol.parentEvidence(anchor.hash);

      await seal(ctx, job.jobId, evidence);
      await protocol
        .connect(stranger)
        .sealNote(await protocol.noteFor(stranger.address, job.jobId, evidence, SALT));
      const first = await protocol.connect(guardian).askParent(job.jobId, SALT, { value: deposit });
      const firstAt = (await ethers.provider.getBlock((await first.wait())!.blockNumber))!.timestamp;
      const firstId = await protocol.challengeCount();
      await protocol.connect(stranger).askParent(job.jobId, SALT, { value: deposit });
      const secondId = await protocol.challengeCount();

      await at(firstAt + 12 * HOUR + 60);
      await protocol.resolveChallenge(firstId);
      expect(await protocol.statusOf(job.jobId)).to.equal(5n);

      await expect(protocol.resolveChallenge(secondId))
        .to.emit(protocol, "ChallengeRefunded")
        .withArgs(secondId, stranger.address);
      expect(await protocol.credit(stranger.address)).to.equal(deposit);
      expect(await protocol.openChallengesOf(job.jobId)).to.equal(0n);
    });

    it("stops after the limit of questions, with the deposit growing each time (D91, D92)", async function () {
      const ctx = await provenJob();
      const { protocol, lightClient, chain, guardian, operator, jobId, deposit } = ctx;
      expect(await protocol.MAX_PARENT_QUESTIONS()).to.equal(2016n);
      // 2,016 rounds would take too long here. The rule is the same with 3.
      await protocol.setMaxParentQuestions(3);

      let paid = 0n;
      for (let k = 1n; k <= 3n; k++) {
        const asked = (await protocol.getDuty(jobId)).deepest;
        const salt = ethers.id(`question ${k}`);
        await protocol
          .connect(guardian)
          .sealNote(
            await protocol.noteFor(
              guardian.address,
              jobId,
              await protocol.parentEvidence(asked.hash),
              salt
            )
          );
        expect(await protocol.parentDepositOf(jobId)).to.equal(k * deposit);
        await protocol.connect(guardian).askParent(jobId, salt, { value: k * deposit });
        paid += k * deposit;
        // The test chain holds every parent already.
        await protocol.showParent(await protocol.challengeCount(), 0);
        expect((await protocol.getDuty(jobId)).parentsShown).to.equal(k);
      }
      expect(await protocol.credit(operator.address)).to.equal(paid);

      const asked = (await protocol.getDuty(jobId)).deepest;
      const salt = ethers.id("question 4");
      await protocol
        .connect(guardian)
        .sealNote(
          await protocol.noteFor(
            guardian.address,
            jobId,
            await protocol.parentEvidence(asked.hash),
            salt
          )
        );
      await expect(
        protocol.connect(guardian).askParent(jobId, salt, { value: 4n * deposit })
      ).to.be.revertedWithCustomError(protocol, "TooManyQuestions");
    });

    it("makes the proof false when the anchor is stated at height 0, where no parent can be shown", async function () {
      const ctx = await readyOperator();
      const { protocol, lightClient, operator, guardian } = ctx;
      const job = await assignedJob(ctx);

      // A forger's own epoch start at height 0; its first block is the anchor.
      const time = (await latestTime()) - 60 * MINUTE;
      const headers: string[] = [];
      let prevLE = ethers.id("garbage");
      for (let i = 0; i < 6; i++) {
        const h = mine({ prevLE, time: time + i * 10 * MINUTE, bits: EASY });
        headers.push(h);
        prevLE = headerHashLE(h);
      }
      await lightClient.addEpochStart(concat(headers), 0);
      const esId = await lightClient.nodeId(headerHashLE(headers[0]), 0, time);
      await lightClient.jump(headers[0], esId, 0);
      const anchor = { hash: headerHashLE(headers[0]), height: 0, epochTime: time };
      await mineAt(time + 2 * HOUR - 60);
      const d = await duty(ctx, job.jobId, { anchor });
      await protocol.connect(operator).proveJob(job.jobId, d.proof);
      const deposit = (await protocol.getJob(job.jobId)).commitmentFee;

      await seal(ctx, job.jobId, await protocol.parentEvidence(anchor.hash));
      const asked = await protocol.connect(guardian).askParent(job.jobId, SALT, { value: deposit });
      const askedAt = (await ethers.provider.getBlock((await asked.wait())!.blockNumber))!.timestamp;
      const cid = await protocol.challengeCount();
      await expect(protocol.showParent(cid, 0)).to.be.revertedWithCustomError(protocol, "NoParent");

      await at(askedAt + 12 * HOUR);
      await expect(protocol.resolveChallenge(cid)).to.emit(protocol, "JobSlashed");
    });
  });

  describe("a competing branch", function () {
    it("fails against real blocks: the operator adds blocks and keeps the lead", async function () {
      const ctx = await provenJob();
      const { protocol, guardian, operator, stranger, chain, anchor, parent, jobId, provenAt, deposit } = ctx;

      // Operator's branch from the anchor: anchor, proof block, 5 on top = 7.
      // The guardian's branch from the same parent: 8.
      const g = await branch(ctx, parent, 8);
      await seal(ctx, jobId, await protocol.forkEvidence(g[0].hash));
      await protocol
        .connect(guardian)
        .challengeFork(jobId, anchor, g[0], g[7], 0, SALT, { value: deposit });
      const cid = await protocol.challengeCount();
      const c = await protocol.getChallenge(cid);
      expect(c.guardianWork > c.operatorWork).to.equal(true);

      // Two more blocks on the operator's branch.
      await chain.add((await latestTime()) + 1);
      await chain.add((await latestTime()) + 1);
      const extended = await protocol.connect(stranger).extendBranch(cid, false, ctx.proof.tip, chain.tip, 0);
      const extendedAt = (await ethers.provider.getBlock(
        (await extended.wait())!.blockNumber
      ))!.timestamp;
      const after = await protocol.getChallenge(cid);
      expect(after.operatorWork > after.guardianWork).to.equal(true);

      // The lock does not move when the lead changes (D99).
      const lockEnd = provenAt + 36 * HOUR;
      expect((await protocol.getDuty(jobId)).lockEnd).to.equal(BigInt(lockEnd));
      expect(extendedAt < lockEnd).to.equal(true);

      await at(lockEnd - 1);
      await expect(protocol.resolveChallenge(cid)).to.be.revertedWithCustomError(
        protocol,
        "LockNotEnded"
      );
      await at(lockEnd);
      await expect(protocol.resolveChallenge(cid))
        .to.emit(protocol, "ChallengeFailed")
        .withArgs(cid, guardian.address);
      expect(await protocol.credit(operator.address)).to.equal(deposit);
      await protocol.settle(jobId);
    });

    it("cannot be frozen by showing a block that goes nowhere on the operator's side", async function () {
      const ctx = await provenJob();
      const { protocol, guardian, stranger, chain, anchor, parent, jobId, deposit, proof } = ctx;
      const g = await branch(ctx, parent, 8);
      await seal(ctx, jobId, await protocol.forkEvidence(g[0].hash));
      await protocol
        .connect(guardian)
        .challengeFork(jobId, anchor, g[0], g[7], 0, SALT, { value: deposit });

      const cid = await protocol.challengeCount();

      // Someone puts a block on the operator's tip that no real block follows.
      const deadEnd = await chain.add((await latestTime()) + 1, [], proof.tip);
      await protocol.connect(stranger).extendBranch(cid, false, proof.tip, deadEnd, 0);

      // The real chain grows from the operator's tip, not from the dead end.
      const real = await branch(ctx, proof.tip, 2);
      await protocol.extendBranch(cid, false, proof.tip, real[1], 0);
      const c = await protocol.getChallenge(cid);
      expect(c.operatorWork > c.guardianWork).to.equal(true);

      // A block that was never shown on that side cannot be built on.
      await expect(
        protocol.extendBranch(cid, false, g[3], g[7], 0)
      ).to.be.revertedWithCustomError(protocol, "NotShown");
    });

    it("lets the guardian's side take the lead back, with the lock unchanged (D99)", async function () {
      const ctx = await provenJob();
      const { protocol, guardian, anchor, parent, jobId, deposit, proof, provenAt } = ctx;
      const g = await branch(ctx, parent, 8);
      await seal(ctx, jobId, await protocol.forkEvidence(g[0].hash));
      await protocol
        .connect(guardian)
        .challengeFork(jobId, anchor, g[0], g[7], 0, SALT, { value: deposit });

      const cid = await protocol.challengeCount();
      const o = await branch(ctx, proof.tip, 2);
      await protocol.extendBranch(cid, false, proof.tip, o[1], 0);
      const more = await branch(ctx, g[7], 2);
      const sent = await protocol.extendBranch(cid, true, g[7], more[1], 0);
      const sentAt = (await ethers.provider.getBlock(
        (await sent.wait())!.blockNumber
      ))!.timestamp;
      const c = await protocol.getChallenge(cid);
      expect(c.guardianWork > c.operatorWork).to.equal(true);
      expect(sentAt < provenAt + 36 * HOUR).to.equal(true);
      expect((await protocol.getDuty(jobId)).lockEnd).to.equal(BigInt(provenAt + 36 * HOUR));
    });

    it("makes the proof false when the guardian's branch still has more work when the lock ends", async function () {
      const ctx = await provenJob();
      const { protocol, guardian, operator, application, anchor, parent, jobId, deposit } = ctx;
      const g = await branch(ctx, parent, 8);
      await seal(ctx, jobId, await protocol.forkEvidence(g[0].hash));
      await protocol
        .connect(guardian)
        .challengeFork(jobId, anchor, g[0], g[7], 0, SALT, { value: deposit });

      await at(Number((await protocol.getDuty(jobId)).lockEnd));
      await expect(protocol.resolveChallenge(await protocol.challengeCount()))
        .to.emit(protocol, "JobSlashed")
        .withArgs(jobId, operator.address, guardian.address, (ETH * 8n) / 10n, (ETH * 2n) / 10n);
      expect(await protocol.credit(application.address)).to.equal((ETH * 8n) / 10n);
      expect(await protocol.credit(guardian.address)).to.equal((ETH * 2n) / 10n + deposit);
      await expect(protocol.settle(jobId)).to.be.revertedWithCustomError(protocol, "NotProven");
    });

    it("lets the operator's branch win when the work is equal", async function () {
      const ctx = await provenJob();
      const { protocol, guardian, operator, chain, anchor, parent, jobId, deposit } = ctx;
      const g = await branch(ctx, parent, 8);
      await seal(ctx, jobId, await protocol.forkEvidence(g[0].hash));
      await protocol
        .connect(guardian)
        .challengeFork(jobId, anchor, g[0], g[7], 0, SALT, { value: deposit });

      const cid = await protocol.challengeCount();
      await chain.add((await latestTime()) + 1);
      await protocol.extendBranch(cid, false, ctx.proof.tip, chain.tip, 0);
      const c = await protocol.getChallenge(cid);
      expect(c.operatorWork).to.equal(c.guardianWork);

      await at(Number((await protocol.getDuty(jobId)).lockEnd));
      await protocol.resolveChallenge(cid);
      expect(await protocol.credit(operator.address)).to.equal(deposit);
    });

    it("rejects a branch without more work, one that does not share the parent, and one that parts below the anchor", async function () {
      const ctx = await provenJob();
      const { protocol, guardian, chain, anchor, parent, jobId, deposit } = ctx;

      const short = await branch(ctx, parent, 7);
      await seal(ctx, jobId, await protocol.forkEvidence(short[0].hash));
      await expect(
        protocol
          .connect(guardian)
          .challengeFork(jobId, anchor, short[0], short[6], 0, SALT, { value: deposit })
      ).to.be.revertedWithCustomError(protocol, "NotHeavier");

      // A branch that starts one block lower does not share the anchor's parent.
      const grand = await ctx.lightClient.getNode(
        await ctx.lightClient.nodeId(parent.hash, parent.height, parent.epochTime)
      );
      const grandRef = { hash: grand.prevHash, height: parent.height - 1, epochTime: parent.epochTime };
      const low = await branch(ctx, grandRef, 10);
      const salt2 = ethers.id("2");
      await protocol
        .connect(guardian)
        .sealNote(await protocol.noteFor(guardian.address, jobId, await protocol.forkEvidence(low[0].hash), salt2));
      await expect(
        protocol
          .connect(guardian)
          .challengeFork(jobId, anchor, low[0], low[9], 0, salt2, { value: deposit })
      ).to.be.revertedWithCustomError(protocol, "NotCompeting");

      // Parting at the anchor's parent itself is below the anchor.
      const salt3 = ethers.id("3");
      await protocol
        .connect(guardian)
        .sealNote(await protocol.noteFor(guardian.address, jobId, await protocol.forkEvidence(low[0].hash), salt3));
      await expect(
        protocol
          .connect(guardian)
          .challengeFork(jobId, parent, low[0], low[9], 0, salt3, { value: deposit })
      ).to.be.revertedWithCustomError(protocol, "NotOnOperatorBranch");
      expect(chain.tip.height > anchor.height).to.equal(true);
    });

    it("stops adding blocks when the lock has ended", async function () {
      const ctx = await provenJob();
      const { protocol, guardian, chain, anchor, parent, jobId, deposit } = ctx;
      const g = await branch(ctx, parent, 8);
      await seal(ctx, jobId, await protocol.forkEvidence(g[0].hash));
      await protocol
        .connect(guardian)
        .challengeFork(jobId, anchor, g[0], g[7], 0, SALT, { value: deposit });
      await chain.add((await latestTime()) + 1);
      await at(Number((await protocol.getDuty(jobId)).lockEnd));
      await expect(
        protocol.extendBranch(await protocol.challengeCount(), false, ctx.proof.tip, chain.tip, 0)
      ).to.be.revertedWithCustomError(protocol, "LockEnded");
    });
  });
});

describe("iPoWProtocol: claims and attest (D11, D42, D73, D74, D94)", function () {
  async function provenClaim(bid = ETH) {
    const ctx = await readyOperator();
    const job = await assignedJob(ctx, { claimKind: 1, bid });
    const d = await duty(ctx, job.jobId);
    const sent = await ctx.protocol.connect(ctx.operator).proveJob(job.jobId, d.proof);
    const provenAt = (await ethers.provider.getBlock((await sent.wait())!.blockNumber))!.timestamp;
    await ctx.protocol.connect(ctx.stranger).lockBond(2n * ETH, { value: 2n * ETH });
    return { ...ctx, ...job, ...d, provenAt };
  }

  it("lets an attester take the operator's place: x locked from its bond, the operator's bond freed (D42, D73)", async function () {
    const { protocol, operator, stranger, jobId } = await provenClaim(2n * ETH);
    expect(await protocol.bondOf(operator.address)).to.deep.equal([3n * ETH, 2n * ETH]);

    await expect(protocol.connect(stranger).attest(jobId))
      .to.emit(protocol, "Attested")
      .withArgs(jobId, stranger.address, ETH);
    // The attester locks x, not the operator's larger bid.
    expect(await protocol.bondOf(stranger.address)).to.deep.equal([2n * ETH, ETH]);
    expect(await protocol.bondOf(operator.address)).to.deep.equal([3n * ETH, 0n]);
    await protocol.connect(operator).withdrawBond(3n * ETH);
  });

  it("pays the attester up to 40% of the escrow fee, for the time it kept x locked, and the operator the rest (D42, D96)", async function () {
    const { protocol, operator, stranger, jobId, provenAt, fee } = await provenClaim();
    const sent = await protocol.connect(stranger).attest(jobId);
    const attestedAt = (await ethers.provider.getBlock((await sent.wait())!.blockNumber))!.timestamp;

    const lockEnd = provenAt + 7 * DAY;
    await at(lockEnd);
    await protocol.settle(jobId);
    const attesterShare =
      (ESCROW_FEE * 4000n * BigInt(lockEnd - attestedAt)) / (10_000n * BigInt(lockEnd - provenAt));
    expect(attesterShare > (ESCROW_FEE * 3999n) / 10_000n).to.equal(true);
    expect(await protocol.credit(stranger.address)).to.equal(attesterShare);
    expect(await protocol.credit(operator.address)).to.equal(fee + ESCROW_FEE - attesterShare);
    expect(await protocol.bondOf(stranger.address)).to.deep.equal([2n * ETH, 0n]);
  });

  it("pays about half of the 40% to an attester that stepped in halfway through the lock (D96)", async function () {
    const { protocol, operator, stranger, jobId, provenAt, fee } = await provenClaim();
    const lockEnd = provenAt + 7 * DAY;
    const half = provenAt + (7 * DAY) / 2;
    await at(half);
    await protocol.connect(stranger).attest(jobId);
    await at(lockEnd);
    await protocol.settle(jobId);
    const share = (ESCROW_FEE * 4000n * BigInt(lockEnd - half)) / (10_000n * BigInt(7 * DAY));
    expect(share).to.equal((ESCROW_FEE * 2000n) / 10_000n);
    expect(await protocol.credit(stranger.address)).to.equal(share);
    expect(await protocol.credit(operator.address)).to.equal(fee + ESCROW_FEE - share);
  });

  it("lets the operator attest its own job (D95)", async function () {
    const { protocol, operator, jobId } = await provenClaim();
    await protocol.connect(operator).attest(jobId);
    expect((await protocol.getDuty(jobId)).attester).to.equal(operator.address);
    expect(await protocol.isOfficial(jobId)).to.equal(true);
    // Its bid is freed, and x is locked in its place.
    expect(await protocol.bondOf(operator.address)).to.deep.equal([3n * ETH, ETH]);
  });

  it("slashes the attester, not the operator, when the proof is proven fake (D42)", async function () {
    const ctx = await readyOperator();
    const { protocol, lightClient, chain, operator, guardian, stranger, application } = ctx;
    const job = await assignedJob(ctx, { claimKind: 1 });
    const time = (await latestTime()) + 1;
    const header = mine({ prevLE: ethers.id("nowhere"), time, bits: EASY });
    await lightClient.jump(header, chain.epochStartId, 50);
    const anchor = { hash: headerHashLE(header), height: 50, epochTime: chain.epochTime };
    const d = await duty(ctx, job.jobId, { anchor });
    await protocol.connect(operator).proveJob(job.jobId, d.proof);
    await protocol.connect(stranger).lockBond(2n * ETH, { value: 2n * ETH });
    await protocol.connect(stranger).attest(job.jobId);

    const deposit = (await protocol.getJob(job.jobId)).commitmentFee;
    await protocol
      .connect(guardian)
      .sealNote(await protocol.noteFor(guardian.address, job.jobId, await protocol.parentEvidence(anchor.hash), SALT));
    const asked = await protocol.connect(guardian).askParent(job.jobId, SALT, { value: deposit });
    const askedAt = (await ethers.provider.getBlock((await asked.wait())!.blockNumber))!.timestamp;
    await at(askedAt + 12 * HOUR);
    await protocol.resolveChallenge(await protocol.challengeCount());

    expect(await protocol.bondOf(stranger.address)).to.deep.equal([ETH, 0n]);
    expect(await protocol.bondOf(operator.address)).to.deep.equal([3n * ETH, 0n]);
    expect(await protocol.credit(application.address)).to.equal((ETH * 8n) / 10n);
    expect(await protocol.isOfficial(job.jobId)).to.equal(false);
  });

  it("accepts one attester, with enough free bond, while the lock runs", async function () {
    const { protocol, stranger, guardian, jobId, provenAt } = await provenClaim();
    await expect(protocol.connect(guardian).attest(jobId)).to.be.revertedWithCustomError(protocol, "BondNotFree");
    await protocol.connect(stranger).attest(jobId);
    await protocol.connect(guardian).lockBond(2n * ETH, { value: 2n * ETH });
    await expect(protocol.connect(guardian).attest(jobId)).to.be.revertedWithCustomError(protocol, "AlreadyAttested");

    const other = await provenClaim();
    await at(other.provenAt + 7 * DAY);
    await expect(other.protocol.connect(other.stranger).attest(other.jobId)).to.be.revertedWithCustomError(
      other.protocol,
      "LockEnded"
    );
    expect(provenAt > 0).to.equal(true);
  });

  it("rejects an attest before the proof", async function () {
    const ctx = await readyOperator();
    const { jobId } = await assignedJob(ctx, { claimKind: 1 });
    await ctx.protocol.connect(ctx.stranger).lockBond(2n * ETH, { value: 2n * ETH });
    await expect(ctx.protocol.connect(ctx.stranger).attest(jobId)).to.be.revertedWithCustomError(
      ctx.protocol,
      "NotProven"
    );
  });

  describe("when a message counts as official (D11)", function () {
    it("makes a settlement official when its proof is accepted", async function () {
      const ctx = await readyOperator();
      const { jobId } = await assignedJob(ctx);
      const { proof } = await duty(ctx, jobId);
      expect(await ctx.protocol.isOfficial(jobId)).to.equal(false);
      await ctx.protocol.connect(ctx.operator).proveJob(jobId, proof);
      expect(await ctx.protocol.isOfficial(jobId)).to.equal(true);
      expect(await ctx.protocol.challengePeriodOf(jobId)).to.equal(0n);
    });

    it("makes a claim official when its lock has ended: the challenge period, counted from the proof (D74, D97)", async function () {
      const { protocol, jobId, provenAt } = await provenClaim();
      expect(await protocol.challengePeriodOf(jobId)).to.equal(BigInt(7 * DAY));
      expect(await protocol.isOfficial(jobId)).to.equal(false);
      await mineAt(provenAt + 7 * DAY - 1);
      expect(await protocol.isOfficial(jobId)).to.equal(false);
      await mineAt(provenAt + 7 * DAY);
      expect(await protocol.isOfficial(jobId)).to.equal(true);
    });

    it("makes a claim official at once when it is attested", async function () {
      const { protocol, stranger, jobId } = await provenClaim();
      await protocol.connect(stranger).attest(jobId);
      expect(await protocol.isOfficial(jobId)).to.equal(true);
    });

    it("does not make a claim official while a challenge of its proof is open", async function () {
      const ctx = await provenClaim();
      const { protocol, guardian, anchor, jobId, provenAt } = ctx;
      const lockEnd = provenAt + 7 * DAY;
      const deposit = (await protocol.getJob(jobId)).commitmentFee;
      await protocol
        .connect(guardian)
        .sealNote(await protocol.noteFor(guardian.address, jobId, await protocol.parentEvidence(anchor.hash), SALT));
      await at(lockEnd - 12 * HOUR);
      await protocol.connect(guardian).askParent(jobId, SALT, { value: deposit });

      await mineAt(lockEnd);
      expect(await protocol.isOfficial(jobId)).to.equal(false);
      await protocol.resolveChallenge(await protocol.challengeCount());
      expect(await protocol.isOfficial(jobId)).to.equal(false);
    });

    it("makes a claim official at the end of its lock once its challenges are answered, and keeps it official (D97, D99)", async function () {
      const ctx = await provenClaim();
      const { protocol, guardian, anchor, jobId, provenAt } = ctx;
      const lockEnd = provenAt + 7 * DAY;
      const deposit = (await protocol.getJob(jobId)).commitmentFee;
      await protocol
        .connect(guardian)
        .sealNote(await protocol.noteFor(guardian.address, jobId, await protocol.parentEvidence(anchor.hash), SALT));
      await at(lockEnd - 12 * HOUR);
      await protocol.connect(guardian).askParent(jobId, SALT, { value: deposit });
      await at(lockEnd - 60);
      await protocol.showParent(await protocol.challengeCount(), 0);

      await mineAt(lockEnd);
      expect(await protocol.isOfficial(jobId)).to.equal(true);
      await expect(
        protocol.connect(guardian).askParent(jobId, SALT, { value: 2n * deposit })
      ).to.be.revertedWithCustomError(protocol, "ChallengeWindowClosed");
    });
  });
});
