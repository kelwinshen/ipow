import { expect } from "chai";
import { network } from "hardhat";

import {
  COIN_SCRIPT,
  buildTx,
  headerHashLE,
  merkle,
  mine,
  targetFromBits,
  txidLE,
  concat,
} from "./helpers/bitcoin.ts";

const { ethers } = await network.create();

// Conversion, the first application on the protocol. Design:
// docs/drafts/ipow-conversion-app.md. The Bitcoin blocks here are mined by
// the test at a low difficulty, which only the test light client allows.

const MINUTE = 60;
const HOUR = 3600;
const DAY = 24 * HOUR;
const ETH = 10n ** 18n;
const EASY = 0x207fffff;
const ZERO_HASH = "0x" + "00".repeat(32);
const FEES = ETH / 10n;
// A gas estimate runs at a base fee of zero, where the fee paths cost less
// (V14): the swaps are sent with a stated limit.
const GAS = 2_000_000n;
const USER_SCRIPT = "0x0014" + "22".repeat(20);
const OPERATOR_SCRIPT = "0x0014" + "33".repeat(20);
const MAX_SATS = 10_000_000n; // 0.1 BTC

type Ref = { hash: string; height: number; epochTime: number };
type Tx = { raw: string; txid: string };

async function latestTime() {
  return (await ethers.provider.getBlock("latest"))!.timestamp;
}

async function mineAt(t: number) {
  await ethers.provider.send("evm_setNextBlockTimestamp", [t]);
  await ethers.provider.send("evm_mine", []);
}

function tagPayload(tag: string): string {
  return ethers.sha256(ethers.concat([ethers.toUtf8Bytes("iPoW job"), tag]));
}

/** A Bitcoin chain the test mines and streams to the light client. */
class TestChain {
  tip!: Ref;
  private blocks = new Map<string, string[]>();
  private salt = 0;
  constructor(private lightClient: any) {}

  async start(now: number) {
    const epochTime = now - HOUR;
    const headers: string[] = [];
    let prevLE = ZERO_HASH;
    for (let i = 0; i < 6; i++) {
      const h = mine({ prevLE, time: epochTime + i * 10 * MINUTE, bits: EASY });
      headers.push(h);
      prevLE = headerHashLE(h);
    }
    await this.lightClient.addEpochStart(concat(headers), 0);
    this.tip = { hash: prevLE, height: 5, epochTime };
  }

  async add(txs: Tx[] = [], time?: number): Promise<Ref> {
    const on = this.tip;
    const coinbase = buildTx({
      inputs: [{ txidLE: ZERO_HASH, vout: 0xffffffff }],
      outputs: [{ value: BigInt(++this.salt), script: COIN_SCRIPT }],
    });
    const txids = [txidLE(coinbase), ...txs.map((t) => t.txid)];
    const header = mine({ prevLE: on.hash, time: time ?? (await latestTime()) + 1, bits: EASY, merkleRootLE: merkle(txids, 0).rootLE });
    await this.lightClient.extend(header, on.height, on.epochTime);
    this.tip = { hash: headerHashLE(header), height: on.height + 1, epochTime: on.epochTime };
    this.blocks.set(this.tip.hash, txids);
    return this.tip;
  }

  proofOf(block: Ref, tx: Tx) {
    const txids = this.blocks.get(block.hash)!;
    const i = txids.indexOf(tx.txid);
    return { siblings: merkle(txids, i).siblings, txIndex: i };
  }
}

function tx(inputs: { txidLE: string; vout: number }[], outputs: { value: bigint; script: string }[]): Tx {
  const raw = buildTx({ inputs, outputs });
  return { raw, txid: txidLE(raw) };
}

async function deploy() {
  const lightClient = await ethers.deployContract("iPoWLightClientHarness", [0]);
  await lightClient.setLimits(targetFromBits(EASY), targetFromBits(EASY));
  const protocol = await ethers.deployContract("iPoWProtocolHarness", [await lightClient.getAddress()]);
  const conversion = await ethers.deployContract("Conversion", [await protocol.getAddress(), MAX_SATS, ethers.ZeroAddress]);
  const token = await ethers.deployContract("MockERC20", ["Test", "TST"]);
  const [, user, operator, guardian, stranger] = await ethers.getSigners();

  const now = Math.ceil(((await latestTime()) + DAY) / DAY) * DAY;
  await mineAt(now);
  const chain = new TestChain(lightClient);
  await chain.start(now);

  // The operator's bond and first chain head.
  await protocol.connect(operator).lockBond(3n * ETH, { value: 3n * ETH });
  const first = tx([{ txidLE: ethers.id("funding"), vout: 0 }], [
    { value: 546n, script: COIN_SCRIPT },
    { value: 0n, script: "0x6a20" + (await protocol.chainHeadCommitment(operator.address)).slice(2) },
  ]);
  const block = await chain.add([first]);
  const { siblings, txIndex } = chain.proofOf(block, first);
  await protocol.connect(operator).registerChainHead(block, first.raw, siblings, txIndex, 0, 1);

  return { lightClient, protocol, conversion, token, chain, user, operator, guardian, stranger };
}

type Ctx = Awaited<ReturnType<typeof deploy>>;

/** The operator wins the swap's job and is locked in. */
async function win(ctx: Ctx, swapId: bigint) {
  const { protocol, conversion, operator } = ctx;
  const jobId = (await conversion.getSwap(swapId)).jobId;
  await protocol.connect(operator).bid(jobId, await protocol.minimumBidOf(jobId));
  await mineAt((await latestTime()) + 61);
  return jobId;
}

/**
 * The operator's duty: anchors at a new block, mines `before` blocks, then its
 * tagged transaction (with `extra` outputs, and spending `alsoSpends`), then
 * 5 blocks on top, and proves.
 */
async function duty(
  ctx: Ctx,
  swapId: bigint,
  opts: { extra?: { value: bigint; script: string }[]; alsoSpends?: { txidLE: string; vout: number }[]; before?: number; beforeTxs?: Tx[] } = {}
) {
  const { protocol, conversion, chain, operator } = ctx;
  const jobId = (await conversion.getSwap(swapId)).jobId;
  const duty0 = await protocol.getDuty(jobId);
  let anchor: Ref;
  if (duty0.anchoredAt !== 0n) {
    anchor = { hash: duty0.anchor.hash, height: Number(duty0.anchor.height), epochTime: Number(duty0.anchor.epochTime) };
  } else {
    anchor = await chain.add();
    await protocol.connect(operator).anchorJob(jobId, anchor);
  }
  const blocks: Ref[] = [];
  for (let i = 0; i < (opts.before ?? 0); i++) {
    blocks.push(await chain.add(i === 0 ? (opts.beforeTxs ?? []) : []));
  }
  const head = await protocol.chainHeadOf(operator.address);
  const tagged = tx([{ txidLE: head.txid, vout: Number(head.vout) }, ...(opts.alsoSpends ?? [])], [
    { value: 546n, script: COIN_SCRIPT },
    { value: 0n, script: "0x6a20" + tagPayload(await conversion.tagOf(swapId)).slice(2) },
    ...(opts.extra ?? []),
  ]);
  const proofBlock = await chain.add([tagged]);
  for (let i = 0; i < 5; i++) await chain.add();
  const { siblings, txIndex } = chain.proofOf(proofBlock, tagged);
  await protocol.connect(operator).proveJob(jobId, {
    proofBlock,
    tip: chain.tip,
    prevEpochTime: 0,
    rawTx: tagged.raw,
    siblings,
    txIndex,
    headIndex: 0,
    tagIndex: 1,
  });
  return { jobId, anchor, tagged, proofBlock, blocks };
}

async function afterLock(ctx: Ctx, jobId: bigint) {
  const lockEnd = Number((await ctx.protocol.getDuty(jobId)).lockEnd);
  await mineAt(lockEnd + 1);
}

describe("Conversion: sell, coin to BTC", function () {
  async function opened(ctx: Ctx, sats = 5_000_000n) {
    const { conversion, user } = ctx;
    await conversion.connect(user).sell(ethers.ZeroAddress, ETH, sats, USER_SCRIPT, 6, FEES, { value: ETH + FEES, gasLimit: GAS });
    return 1n;
  }

  it("pays the operator once its payment to the user is proven and the lock has ended", async function () {
    const ctx = await deploy();
    const { conversion, operator } = ctx;
    const swapId = await opened(ctx);
    await win(ctx, swapId);
    const { jobId, tagged } = await duty(ctx, swapId, { extra: [{ value: 5_000_000n, script: USER_SCRIPT }] });

    // Not before the lock ends: a proof on made-up blocks can still be shown false.
    await expect(conversion.completeSell(swapId, tagged.raw)).to.be.revertedWithCustomError(conversion, "LockNotEnded");
    await afterLock(ctx, jobId);
    await expect(conversion.completeSell(swapId, tagged.raw)).to.changeEtherBalance(ethers, operator, ETH);
    expect((await conversion.getSwap(swapId)).state).to.equal(3n); // Done
  });

  it("refunds the user when the proven transaction pays too little", async function () {
    const ctx = await deploy();
    const { conversion, user } = ctx;
    const swapId = await opened(ctx);
    await win(ctx, swapId);
    const { jobId, tagged } = await duty(ctx, swapId, { extra: [{ value: 4_999_999n, script: USER_SCRIPT }] });
    await afterLock(ctx, jobId);
    await expect(conversion.completeSell(swapId, tagged.raw)).to.be.revertedWithCustomError(conversion, "NotPaid");
    await expect(conversion.refundSell(swapId, tagged.raw)).to.changeEtherBalance(ethers, user, ETH);
  });

  it("refunds the user after the deadline with no proof, and passes on the slashed escrow's share", async function () {
    const ctx = await deploy();
    const { conversion, protocol, user, guardian } = ctx;
    const swapId = await opened(ctx);
    const jobId = await win(ctx, swapId);
    await expect(conversion.refundSell(swapId, "0x")).to.be.revertedWithCustomError(conversion, "NotRefundable");
    const deadline = Number(await protocol.deadlineOf(jobId));
    await mineAt(deadline + 1);
    // A guardian reports the missed duty (D53).
    const salt = ethers.id("salt");
    const note = await protocol.noteFor(guardian.address, jobId, ZERO_HASH, salt);
    await protocol.connect(guardian).sealNote(note);
    await protocol.connect(guardian).reportMissedDuty(jobId, salt);
    const escrow = (await protocol.getJob(jobId)).escrow;
    await expect(conversion.refundSell(swapId, "0x")).to.changeEtherBalance(ethers, user, ETH + (escrow * 8000n) / 10000n);
  });

  it("sells a token", async function () {
    const ctx = await deploy();
    const { conversion, token, user, operator } = ctx;
    await token.mint(user.address, 1000n);
    await token.connect(user).approve(await conversion.getAddress(), 1000n);
    await conversion.connect(user).sell(await token.getAddress(), 1000n, 5_000_000n, USER_SCRIPT, 6, FEES, { value: FEES, gasLimit: GAS });
    await win(ctx, 1n);
    const { jobId, tagged } = await duty(ctx, 1n, { extra: [{ value: 5_000_000n, script: USER_SCRIPT }] });
    await afterLock(ctx, jobId);
    await conversion.completeSell(1n, tagged.raw);
    expect(await token.balanceOf(operator.address)).to.equal(1000n);
  });

  it("asks the protocol's lowest escrow", async function () {
    const ctx = await deploy();
    const { conversion, protocol } = ctx;
    await opened(ctx);
    const job = await protocol.getJob((await conversion.getSwap(1n)).jobId);
    const block = await ethers.provider.getBlock("latest");
    expect(job.escrow).to.equal(5n * (await protocol.commitmentFeeAt(6, block!.baseFeePerGas!)));
  });
});

describe("Conversion: buy, BTC to coin", function () {
  /** The operator wins, anchors at a new block, and locks 1 ETH. */
  async function funded(ctx: Ctx, script = OPERATOR_SCRIPT) {
    const { conversion, protocol, chain, user, operator } = ctx;
    await conversion.connect(user).buy(ethers.ZeroAddress, ETH, 5_000_000n, "0x", 6, FEES, { value: FEES, gasLimit: GAS });
    const swapId = await conversion.swapCount();
    const jobId = await win(ctx, swapId);
    const parent = chain.tip;
    const anchor = await chain.add();
    await protocol.connect(operator).anchorJob(jobId, anchor);
    await conversion.connect(operator).fund(swapId, script, { value: ETH });
    return { swapId, jobId, anchor, parent };
  }

  /** The user's payment to the operator's script. */
  function payment(sats = 5_000_000n, seed = "user coin") {
    return tx([{ txidLE: ethers.id(seed), vout: 0 }], [{ value: sats, script: OPERATOR_SCRIPT }]);
  }

  async function slashMissed(ctx: Ctx, jobId: bigint) {
    const { protocol, guardian } = ctx;
    await mineAt(Number(await protocol.deadlineOf(jobId)) + 1);
    const salt = ethers.id("salt" + jobId);
    await protocol.connect(guardian).sealNote(await protocol.noteFor(guardian.address, jobId, ZERO_HASH, salt));
    await protocol.connect(guardian).reportMissedDuty(jobId, salt);
  }

  it("gives the user the coin once the operator's receipt spends the user's payment", async function () {
    const ctx = await deploy();
    const { conversion, user } = ctx;
    const { swapId } = await funded(ctx);
    const paid = payment();
    const { tagged } = await duty(ctx, swapId, { alsoSpends: [{ txidLE: paid.txid, vout: 0 }] });
    await expect(conversion.completeBuy(swapId, tagged.raw, paid.raw, 0)).to.changeEtherBalance(ethers, user, ETH);
    // Once only.
    await expect(conversion.completeBuy(swapId, tagged.raw, paid.raw, 0)).to.be.revertedWithCustomError(conversion, "WrongState");
  });

  it("gives the user the coin on their own proof, below the operator's close", async function () {
    const ctx = await deploy();
    const { conversion, chain, user } = ctx;
    const { swapId } = await funded(ctx);
    const paid = payment();
    // Paid in the first block after the anchor; the close comes after the
    // payment blocks.
    const { blocks, tagged } = await duty(ctx, swapId, { before: 13, beforeTxs: [paid] });
    const payBlock = blocks[0];
    await expect(conversion.completeBuy(swapId, tagged.raw, paid.raw, 0)).to.be.revertedWithCustomError(conversion, "WrongTransaction");
    const { siblings, txIndex } = chain.proofOf(payBlock, paid);
    await expect(
      conversion.proveMyPayment(swapId, paid.raw, 0, payBlock, siblings, txIndex, payBlock, 0)
    ).to.changeEtherBalance(ethers, user, ETH);
    await expect(conversion.reclaim(swapId)).to.be.revertedWithCustomError(conversion, "WrongState");
  });

  it("refuses a payment on another branch than the operator's close", async function () {
    const ctx = await deploy();
    const { conversion, chain } = ctx;
    const { swapId, anchor } = await funded(ctx);
    await duty(ctx, swapId, { before: 13 });
    // A branch of its own from the anchor, holding a payment, with 6 blocks.
    const real = chain.tip;
    chain.tip = anchor;
    const forged = payment(5_000_000n, "forged");
    const forgedBlock = await chain.add([forged]);
    for (let i = 0; i < 5; i++) await chain.add();
    const p = chain.proofOf(forgedBlock, forged);
    await expect(
      conversion.proveMyPayment(swapId, forged.raw, 0, forgedBlock, p.siblings, p.txIndex, chain.tip, 0)
    ).to.be.revertedWithCustomError(conversion, "NotLinked");
    chain.tip = real;
  });

  it("lets the user prove on top of the anchor's parent once the operator failed, even when the anchor was dropped", async function () {
    const ctx = await deploy();
    const { conversion, chain, user } = ctx;
    const { swapId, jobId, parent } = await funded(ctx);
    // Bitcoin dropped the anchor: the real chain grows from its parent.
    chain.tip = parent;
    await chain.add();
    const paid = payment();
    const payBlock = await chain.add([paid]);
    for (let i = 0; i < 5; i++) await chain.add();
    const { siblings, txIndex } = chain.proofOf(payBlock, paid);
    // Not while the operator can still prove.
    await expect(
      conversion.proveMyPayment(swapId, paid.raw, 0, payBlock, siblings, txIndex, chain.tip, 0)
    ).to.be.revertedWithCustomError(conversion, "NotLinked");
    await slashMissed(ctx, jobId);
    await expect(
      conversion.proveMyPayment(swapId, paid.raw, 0, payBlock, siblings, txIndex, chain.tip, 0)
    ).to.changeEtherBalance(ethers, user, ETH);
    // And the escrow's share, once.
    const escrow = (await ctx.protocol.getJob(jobId)).escrow;
    await expect(conversion.compensate(swapId)).to.changeEtherBalance(ethers, user, (escrow * 8000n) / 10000n);
    await expect(conversion.compensate(swapId)).to.changeEtherBalance(ethers, user, 0n);
  });

  it("gives the operator the coin back after its close, when the user did not pay", async function () {
    const ctx = await deploy();
    const { conversion, operator } = ctx;
    const { swapId, jobId } = await funded(ctx);
    await duty(ctx, swapId, { before: 13 });
    await expect(conversion.reclaim(swapId)).to.be.revertedWithCustomError(conversion, "NotReclaimable");
    await afterLock(ctx, jobId);
    await expect(conversion.reclaim(swapId)).to.changeEtherBalance(ethers, operator, ETH);
  });

  it("does not count a close mined within the payment blocks; the operator waits 36 hours after the deadline", async function () {
    const ctx = await deploy();
    const { conversion, protocol, operator } = ctx;
    const { swapId, jobId } = await funded(ctx);
    await duty(ctx, swapId);
    await afterLock(ctx, jobId);
    await expect(conversion.reclaim(swapId)).to.be.revertedWithCustomError(conversion, "NotReclaimable");
    const deadline = Number(await protocol.deadlineOf(jobId));
    await mineAt(deadline + 36 * HOUR);
    await expect(conversion.reclaim(swapId)).to.changeEtherBalance(ethers, operator, ETH);
  });

  it("rejects a payment outside the payment blocks or with too few confirmations", async function () {
    const ctx = await deploy();
    const { conversion, chain } = ctx;
    const { swapId, jobId } = await funded(ctx);
    const late = payment(5_000_001n, "late");
    for (let i = 0; i < 12; i++) await chain.add();
    const lateBlock = await chain.add([late]);
    await slashMissed(ctx, jobId);
    for (let i = 0; i < 5; i++) await chain.add();
    const p = chain.proofOf(lateBlock, late);
    await expect(
      conversion.proveMyPayment(swapId, late.raw, 0, lateBlock, p.siblings, p.txIndex, chain.tip, 0)
    ).to.be.revertedWithCustomError(conversion, "OutsidePaymentBlocks");
  });

  it("lets only the job's operator fund, after its anchor, in time, with a new script", async function () {
    const ctx = await deploy();
    const { conversion, protocol, chain, user, operator, stranger } = ctx;
    await conversion.connect(user).buy(ethers.ZeroAddress, ETH, 5_000_000n, "0x", 6, FEES, { value: FEES, gasLimit: GAS });
    const jobId = await win(ctx, 1n);
    await expect(conversion.connect(operator).fund(1n, OPERATOR_SCRIPT, { value: ETH })).to.be.revertedWithCustomError(conversion, "NotAnchored");
    const anchor = await chain.add();
    await protocol.connect(operator).anchorJob(jobId, anchor);
    await expect(conversion.connect(stranger).fund(1n, OPERATOR_SCRIPT, { value: ETH })).to.be.revertedWithCustomError(conversion, "NotOperator");
    await expect(conversion.connect(operator).fund(1n, OPERATOR_SCRIPT, { value: ETH - 1n })).to.be.revertedWithCustomError(conversion, "InvalidAmount");
    await conversion.connect(operator).fund(1n, OPERATOR_SCRIPT, { value: ETH });

    await conversion.connect(user).buy(ethers.ZeroAddress, ETH, 5_000_000n, "0x", 6, FEES, { value: FEES, gasLimit: GAS });
    const job2 = await win(ctx, 2n);
    // An anchor mined 40 minutes ago: the payment blocks are mostly over.
    const anchor2 = await chain.add([], (await latestTime()) - 40 * MINUTE);
    await protocol.connect(operator).anchorJob(job2, anchor2);
    await expect(conversion.connect(operator).fund(2n, "0x0014" + "44".repeat(20), { value: ETH })).to.be.revertedWithCustomError(conversion, "AnchorTooOld");
    await mineAt((await latestTime()) + 31 * MINUTE);
    await conversion.cancel(2n);
    expect((await conversion.getSwap(2n)).state).to.equal(5n); // Cancelled

    // A script named for an earlier swap is refused.
    await conversion.connect(user).buy(ethers.ZeroAddress, ETH, 5_000_000n, "0x", 6, FEES, { value: FEES, gasLimit: GAS });
    const job3 = await win(ctx, 3n);
    await protocol.connect(operator).anchorJob(job3, await chain.add());
    await expect(conversion.connect(operator).fund(3n, OPERATOR_SCRIPT, { value: ETH })).to.be.revertedWithCustomError(conversion, "ScriptUsed");
  });

  it("keeps the user's own Bitcoin script on a buy, as a note", async function () {
    const ctx = await deploy();
    const { conversion, user } = ctx;
    await conversion.connect(user).buy(ethers.ZeroAddress, ETH, 5_000_000n, USER_SCRIPT, 6, FEES, { value: FEES, gasLimit: GAS });
    expect((await conversion.getSwap(1n)).userScript).to.equal(USER_SCRIPT);
    // Too long for any standard script: refused. Empty: allowed.
    await expect(
      conversion.connect(user).buy(ethers.ZeroAddress, ETH, 5_000_000n, "0x" + "00".repeat(41), 6, FEES, { value: FEES, gasLimit: GAS })
    ).to.be.revertedWithCustomError(conversion, "InvalidScript");
    await conversion.connect(user).buy(ethers.ZeroAddress, ETH, 5_000_000n, "0x", 6, FEES, { value: FEES, gasLimit: GAS });
    expect((await conversion.getSwap(2n)).userScript).to.equal("0x");
  });

  it("cancels a swap nobody took, with the job's fees to the user's credit", async function () {
    const ctx = await deploy();
    const { conversion, protocol, user } = ctx;
    await conversion.connect(user).buy(ethers.ZeroAddress, ETH, 5_000_000n, "0x", 6, FEES, { value: FEES, gasLimit: GAS });
    await expect(conversion.cancel(1n)).to.be.revertedWithCustomError(conversion, "FundingTimeNotOver");
    await mineAt((await latestTime()) + 16 * MINUTE);
    const jobId = (await conversion.getSwap(1n)).jobId;
    expect(await protocol.credit(user.address)).to.equal(0n);
    await conversion.cancel(1n);
    expect((await conversion.getSwap(1n)).state).to.equal(5n);
    // The job expired with the swap: the fees are the user's to withdraw.
    expect((await protocol.getJob(jobId)).feesReturned).to.equal(true);
    expect(await protocol.credit(user.address)).to.equal(FEES);
    await expect(protocol.connect(user).withdrawCredit()).to.changeEtherBalance(ethers, user, FEES);
  });

  it("refunds a sell nobody took, with the job's fees to the user's credit", async function () {
    const ctx = await deploy();
    const { conversion, protocol, user } = ctx;
    await conversion.connect(user).sell(ethers.ZeroAddress, ETH, 5_000_000n, USER_SCRIPT, 6, FEES, { value: ETH + FEES, gasLimit: GAS });
    await mineAt((await latestTime()) + 16 * MINUTE);
    await expect(conversion.refundSell(1n, "0x")).to.changeEtherBalance(ethers, user, ETH);
    expect(await protocol.credit(user.address)).to.equal(FEES);
  });

  it("buys a token, locked exactly by the operator", async function () {
    const ctx = await deploy();
    const { conversion, protocol, token, chain, user, operator } = ctx;
    await conversion.connect(user).buy(await token.getAddress(), 1000n, 5_000_000n, "0x", 6, FEES, { value: FEES, gasLimit: GAS });
    const jobId = await win(ctx, 1n);
    await protocol.connect(operator).anchorJob(jobId, await chain.add());
    await token.mint(operator.address, 1000n);
    await token.connect(operator).approve(await conversion.getAddress(), 1000n);
    await conversion.connect(operator).fund(1n, OPERATOR_SCRIPT);
    const paid = payment();
    const { tagged } = await duty(ctx, 1n, { alsoSpends: [{ txidLE: paid.txid, vout: 0 }] });
    await conversion.completeBuy(1n, tagged.raw, paid.raw, 0);
    expect(await token.balanceOf(user.address)).to.equal(1000n);
  });

  it("refuses a swap above the size limit", async function () {
    const ctx = await deploy();
    const { conversion, user } = ctx;
    await expect(
      conversion.connect(user).buy(ethers.ZeroAddress, ETH, MAX_SATS + 1n, "0x", 6, FEES, { value: FEES, gasLimit: GAS })
    ).to.be.revertedWithCustomError(conversion, "TooLarge");
  });
});

describe("Conversion: compensation after a slash", function () {
  it("reaches the seller even when the slash comes after the refund", async function () {
    const ctx = await deploy();
    const { conversion, protocol, user, guardian } = ctx;
    await conversion.connect(user).sell(ethers.ZeroAddress, ETH, 5_000_000n, USER_SCRIPT, 6, FEES, { value: ETH + FEES, gasLimit: GAS });
    const jobId = await win(ctx, 1n);
    await mineAt(Number(await protocol.deadlineOf(jobId)) + 1);
    await conversion.refundSell(1n, "0x");
    await expect(conversion.compensate(1n)).to.be.revertedWithCustomError(conversion, "NotSlashed");
    const salt = ethers.id("late salt");
    await protocol.connect(guardian).sealNote(await protocol.noteFor(guardian.address, jobId, ZERO_HASH, salt));
    await protocol.connect(guardian).reportMissedDuty(jobId, salt);
    const escrow = (await protocol.getJob(jobId)).escrow;
    await expect(conversion.compensate(1n)).to.changeEtherBalance(ethers, user, (escrow * 8000n) / 10000n);
  });
});

describe("Conversion: tunnels between programmable networks (T1, T2)", function () {
  /** A sell paying OPERATOR_SCRIPT, whose payment counts only in Bitcoin blocks payFrom to payTo. */
  async function sellInWindow(ctx: Ctx, payFrom: number, payTo: number, sats = 5_000_000n) {
    const { conversion, user } = ctx;
    await conversion.connect(user).sellInWindow(ethers.ZeroAddress, ETH, sats, OPERATOR_SCRIPT, payFrom, payTo, 6, FEES, { value: ETH + FEES, gasLimit: GAS });
    return await conversion.swapCount();
  }

  it("completes a sell whose payment is mined inside its window", async function () {
    const ctx = await deploy();
    const { conversion, chain, operator } = ctx;
    const h = chain.tip.height;
    // The duty anchors at h + 1 and its transaction lands at h + 2.
    const swapId = await sellInWindow(ctx, h + 2, h + 13);
    await win(ctx, swapId);
    const { jobId, tagged, proofBlock } = await duty(ctx, swapId, { extra: [{ value: 5_000_000n, script: OPERATOR_SCRIPT }] });
    expect(proofBlock.height).to.equal(h + 2);
    await afterLock(ctx, jobId);
    await expect(conversion.completeSell(swapId, tagged.raw)).to.changeEtherBalance(ethers, operator, ETH);
  });

  it("refunds the user when the payment is mined outside the window, with no transaction needed", async function () {
    const ctx = await deploy();
    const { conversion, chain, user } = ctx;
    const h = chain.tip.height;
    const swapId = await sellInWindow(ctx, h + 2, h + 3);
    await win(ctx, swapId);
    // Three blocks before the transaction: it lands at h + 5, after the window.
    const { jobId, tagged } = await duty(ctx, swapId, { before: 3, extra: [{ value: 5_000_000n, script: OPERATOR_SCRIPT }] });
    await afterLock(ctx, jobId);
    await expect(conversion.completeSell(swapId, tagged.raw)).to.be.revertedWithCustomError(conversion, "PaidOutsideWindow");
    await expect(conversion.refundSell(swapId, "0x")).to.changeEtherBalance(ethers, user, ETH);
    expect((await conversion.getSwap(swapId)).state).to.equal(4n); // Refunded
  });

  it("counts a payment mined in the window's first or last block, and not one block either side", async function () {
    // The duty's transaction lands at h + 2 in each case.
    const cases: [number, number, boolean][] = [
      [2, 2, true], // first and last block of the window
      [1, 2, true], // last block
      [2, 9, true], // first block
      [3, 9, false], // one block before the window
      [1, 1, false], // one block after it
    ];
    for (const [from, to, inside] of cases) {
      const ctx = await deploy();
      const { conversion, chain, operator, user } = ctx;
      const h = chain.tip.height;
      const swapId = await sellInWindow(ctx, h + from, h + to);
      await win(ctx, swapId);
      const { jobId, tagged, proofBlock } = await duty(ctx, swapId, { extra: [{ value: 5_000_000n, script: OPERATOR_SCRIPT }] });
      expect(proofBlock.height).to.equal(h + 2);
      await afterLock(ctx, jobId);
      if (inside) {
        await expect(conversion.refundSell(swapId, tagged.raw)).to.be.revertedWithCustomError(conversion, "NotRefundable");
        await expect(conversion.completeSell(swapId, tagged.raw)).to.changeEtherBalance(ethers, operator, ETH);
      } else {
        await expect(conversion.completeSell(swapId, tagged.raw)).to.be.revertedWithCustomError(conversion, "PaidOutsideWindow");
        await expect(conversion.refundSell(swapId, "0x")).to.changeEtherBalance(ethers, user, ETH);
      }
    }
  });

  it("does not refund a sell paid in full inside its window, with or without its transaction", async function () {
    const ctx = await deploy();
    const { conversion, chain } = ctx;
    const h = chain.tip.height;
    const swapId = await sellInWindow(ctx, h + 2, h + 13);
    await win(ctx, swapId);
    const { jobId, tagged } = await duty(ctx, swapId, { extra: [{ value: 5_000_000n, script: OPERATOR_SCRIPT }] });
    // Neither while the lock runs, nor after it.
    await expect(conversion.refundSell(swapId, tagged.raw)).to.be.revertedWithCustomError(conversion, "NotRefundable");
    await afterLock(ctx, jobId);
    await expect(conversion.refundSell(swapId, tagged.raw)).to.be.revertedWithCustomError(conversion, "NotRefundable");
    await expect(conversion.refundSell(swapId, "0x")).to.be.revert(ethers);
  });

  it("refuses a window that starts at zero or ends before it starts", async function () {
    const ctx = await deploy();
    const { conversion, user } = ctx;
    for (const [from, to] of [[0, 10], [10, 9]]) {
      await expect(
        conversion.connect(user).sellInWindow(ethers.ZeroAddress, ETH, 5_000_000n, OPERATOR_SCRIPT, from, to, 6, FEES, { value: ETH + FEES, gasLimit: GAS })
      ).to.be.revertedWithCustomError(conversion, "BadWindow");
    }
  });

  it("opens a buy for a recipient: the coin goes to them, the opener pays the fees", async function () {
    const ctx = await deploy();
    const { conversion, user, operator, stranger } = ctx;
    await expect(
      conversion.connect(operator).buyFor(ethers.ZeroAddress, ethers.ZeroAddress, ETH, 5_000_000n, 6, FEES, { value: FEES, gasLimit: GAS })
    ).to.be.revertedWithCustomError(conversion, "ZeroRecipient");
    // A stranger opens it for the user; nobody takes it; the fees go back to the stranger.
    await expect(
      conversion.connect(stranger).buyFor(user.address, ethers.ZeroAddress, ETH, 5_000_000n, 6, FEES, { value: FEES, gasLimit: GAS })
    ).to.emit(conversion, "Bought");
    const s = await conversion.getSwap(1n);
    expect(s.user).to.equal(user.address);
    const job = await ctx.protocol.getJob(s.jobId);
    expect(job.payer).to.equal(stranger.address);
  });

  it("links a buy and a sell with one Bitcoin payment: both complete", async function () {
    const ctx = await deploy();
    const { conversion, protocol, chain, user, operator } = ctx;
    // The buy on the destination network: the operator opens it for the user,
    // wins it, anchors and locks 1 ETH at its new script.
    await conversion.connect(operator).buyFor(user.address, ethers.ZeroAddress, ETH, 5_000_000n, 6, FEES, { value: FEES, gasLimit: GAS });
    const buyId = await conversion.swapCount();
    const buyJob = await win(ctx, buyId);
    const anchor = await chain.add();
    await protocol.connect(operator).anchorJob(buyJob, anchor);
    await conversion.connect(operator).fund(buyId, OPERATOR_SCRIPT, { value: ETH });
    const buy = await conversion.getSwap(buyId);
    expect(buy.state).to.equal(2n); // Funded

    // The sell on the source network pays the buy's script, within the buy's payment blocks.
    const sellId = await sellInWindow(ctx, anchor.height + 1, anchor.height + 12);
    const sellJob = await win(ctx, sellId);
    const sold = await duty(ctx, sellId, { extra: [{ value: 5_000_000n, script: OPERATOR_SCRIPT }] });
    expect(sold.proofBlock.height).to.be.within(anchor.height + 1, anchor.height + 12);

    // The buy's receipt spends that payment (output 2 of the sell's transaction).
    const { tagged: receipt } = await duty(ctx, buyId, { alsoSpends: [{ txidLE: sold.tagged.txid, vout: 2 }] });
    await expect(conversion.completeBuy(buyId, receipt.raw, sold.tagged.raw, 2)).to.changeEtherBalance(ethers, user, ETH);

    // And the sell's operator is paid the user's ETH once its lock ends.
    await afterLock(ctx, sellJob);
    await expect(conversion.completeSell(sellId, sold.tagged.raw)).to.changeEtherBalance(ethers, operator, ETH);
  });
});
