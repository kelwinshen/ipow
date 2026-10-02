import { expect } from "chai";
import { network } from "hardhat";

const { ethers } = await network.create();

// Spec: docs/design/ipow-protocol.md, sections 3 to 5. The D numbers are its
// decisions. Built so far: bond, registration, jobs, fees, auction.

const MINUTE = 60;
const HOUR = 3600;
const DAY = 24 * HOUR;
const ETH = 10n ** 18n;
const GWEI = 10n ** 9n;

const WORK_PER_BLOCK = 80_000n;
const WORK_FIXED = 520_000n;
const TAG = ethers.id("transfer-1");
// The simulated network estimates gas at a price of zero, where the
// commitment fee is zero and storing it costs less. The tests state the gas.
const GAS = 1_000_000n;

async function deploy() {
  const lightClient = await ethers.deployContract("iPoWLightClient", [0]);
  const protocol = await ethers.deployContract("iPoWProtocolNative", [
    await lightClient.getAddress(),
    ethers.ZeroAddress,
  ]);
  const [, application, user, operatorA, operatorB, stranger] =
    await ethers.getSigners();
  await protocol.connect(application).registerApplication([]);
  return { protocol, lightClient, application, user, operatorA, operatorB, stranger };
}

async function timeOf(tx: { blockNumber: number | null }) {
  const block = await ethers.provider.getBlock(tx.blockNumber!);
  return block!.timestamp;
}

async function at(timestamp: number) {
  await ethers.provider.send("evm_setNextBlockTimestamp", [timestamp]);
}

async function setPrice(price: bigint) {
  await ethers.provider.send("hardhat_setNextBlockBaseFeePerGas", [
    "0x" + price.toString(16),
  ]);
}

async function mineAt(timestamp: number) {
  await at(timestamp);
  await ethers.provider.send("evm_mine", []);
}

/** amount of work x price x 1.5 */
function feeFor(price: bigint, confirmations: number) {
  const window = BigInt(24 + confirmations);
  const work = WORK_PER_BLOCK * window + WORK_FIXED;
  return (work * price * 3n) / 2n;
}

/** The same, at the price of the block a call ran in. */
async function feeAt(blockNumber: number, confirmations: number) {
  const block = await ethers.provider.getBlock(blockNumber);
  return feeFor(block!.baseFeePerGas!, confirmations);
}

type Ctx = Awaited<ReturnType<typeof deploy>>;

/** Opens a job with 1 ETH of escrow and returns what the contract recorded. */
async function openJob(
  ctx: Ctx,
  opts: {
    tag?: string;
    escrow?: bigint;
    bps?: number;
    confirmations?: number;
    claimKind?: number;
    value?: bigint;
    extra?: bigint;
    price?: bigint;
  } = {}
) {
  // Unless a test says otherwise, the job is sent exactly its two fees.
  const exact =
    feeFor(opts.price ?? GWEI, opts.confirmations ?? 6) +
    ((opts.escrow ?? ETH) * BigInt(opts.bps ?? 50)) / 10_000n;
  // Every test states the price of the network, 1 gwei unless it says
  // otherwise, so that no test depends on the one before it.
  await setPrice(opts.price ?? GWEI);
  const tx = await ctx.protocol
    .connect(ctx.application)
    .openJob(
      opts.tag ?? TAG,
      opts.escrow ?? ETH,
      opts.bps ?? 50,
      opts.confirmations ?? 6,
      opts.claimKind ?? 0,
      ctx.user.address,
      opts.value ?? exact + (opts.extra ?? 0n), { value: opts.value ?? exact + (opts.extra ?? 0n), gasLimit: GAS }
    );
  const receipt = await tx.wait();
  const jobId = await ctx.protocol.jobCount();
  return {
    jobId,
    openedAt: await timeOf(receipt!),
    blockNumber: receipt!.blockNumber,
    job: await ctx.protocol.getJob(jobId),
  };
}

describe("iPoWProtocol: operators (D12, D31, D43)", function () {
  it("makes anyone an operator by locking a bond, with no minimum", async function () {
    const { protocol, stranger } = await deploy();
    await expect(protocol.connect(stranger).lockBond(1n, { value: 1n }))
      .to.emit(protocol, "BondLocked")
      .withArgs(stranger.address, 1n);
    expect(await protocol.bondOf(stranger.address)).to.deep.equal([1n, 0n]);
  });

  it("rejects an empty bond", async function () {
    const { protocol, stranger } = await deploy();
    await expect(
      protocol.connect(stranger).lockBond(0n, { value: 0n })
    ).to.be.revertedWithCustomError(protocol, "ZeroAmount");
  });

  it("lets an operator withdraw free bond", async function () {
    const { protocol, operatorA } = await deploy();
    await protocol.connect(operatorA).lockBond(3n * ETH, { value: 3n * ETH });
    await expect(
      protocol.connect(operatorA).withdrawBond(2n * ETH)
    ).to.changeEtherBalances(
      ethers,
      [operatorA, protocol],
      [2n * ETH, -2n * ETH]
    );
    expect(await protocol.bondOf(operatorA.address)).to.deep.equal([ETH, 0n]);
  });

  it("does not let an operator withdraw more than it has", async function () {
    const { protocol, operatorA } = await deploy();
    await protocol.connect(operatorA).lockBond(ETH, { value: ETH });
    await expect(
      protocol.connect(operatorA).withdrawBond(ETH + 1n)
    ).to.be.revertedWithCustomError(protocol, "BondNotFree");
  });

  it("keeps bond that is locked for a job", async function () {
    const ctx = await deploy();
    const { protocol, operatorA } = ctx;
    await protocol.connect(operatorA).lockBond(3n * ETH, { value: 3n * ETH });
    const { jobId } = await openJob(ctx);
    await protocol.connect(operatorA).bid(jobId, 2n * ETH);

    expect(await protocol.bondOf(operatorA.address)).to.deep.equal([
      3n * ETH,
      2n * ETH,
    ]);
    await expect(
      protocol.connect(operatorA).withdrawBond(ETH + 1n)
    ).to.be.revertedWithCustomError(protocol, "BondNotFree");
    await protocol.connect(operatorA).withdrawBond(ETH);
  });
});

describe("iPoWProtocol: applications (D17, D28, D63)", function () {
  it("lets anyone register, with the challenge period of each kind of claim", async function () {
    const { protocol, stranger } = await deploy();
    await protocol
      .connect(stranger)
      .registerApplication([36 * HOUR, 7 * DAY]);
    expect(await protocol.isRegistered(stranger.address)).to.equal(true);
    expect(await protocol.challengePeriodsOf(stranger.address)).to.deep.equal([
      BigInt(36 * HOUR),
      BigInt(7 * DAY),
    ]);
  });

  it("does not let an application register again, so its periods never change (D17)", async function () {
    const { protocol, application } = await deploy();
    await expect(
      protocol.connect(application).registerApplication([7 * DAY])
    ).to.be.revertedWithCustomError(protocol, "AlreadyRegistered");
  });

  it("rejects a challenge period below 36 hours or above 7 days (D28)", async function () {
    const { protocol, stranger } = await deploy();
    await expect(
      protocol.connect(stranger).registerApplication([36 * HOUR - 1])
    ).to.be.revertedWithCustomError(protocol, "ChallengePeriodOutOfRange");
    await expect(
      protocol.connect(stranger).registerApplication([7 * DAY + 1])
    ).to.be.revertedWithCustomError(protocol, "ChallengePeriodOutOfRange");
  });

  it("rejects more than 32 kinds of claim", async function () {
    const { protocol, stranger } = await deploy();
    await expect(
      protocol
        .connect(stranger)
        .registerApplication(new Array(33).fill(7 * DAY))
    ).to.be.revertedWithCustomError(protocol, "TooManyClaimKinds");
  });
});

describe("iPoWProtocol: window, deadline and fees (D14, D24, D68 to D70)", function () {
  it("gives a job with 6 confirmations a window of 30 blocks and 1 day", async function () {
    const { protocol } = await deploy();
    expect(await protocol.windowOf(6)).to.equal(30n);
    expect(await protocol.dutyTimeFor(6)).to.equal(BigInt(DAY));
  });

  it("grows the window by one block and the deadline by 48 minutes for each confirmation (D68, D69)", async function () {
    const { protocol } = await deploy();
    expect(await protocol.windowOf(7)).to.equal(31n);
    expect(await protocol.dutyTimeFor(7)).to.equal(BigInt(DAY + 48 * MINUTE));
    expect(await protocol.windowOf(76)).to.equal(100n);
    expect(await protocol.dutyTimeFor(76)).to.equal(BigInt(80 * HOUR));
  });

  it("rejects fewer than 6 confirmations and a window above 100 blocks (D41, D70)", async function () {
    const { protocol } = await deploy();
    await expect(protocol.windowOf(5)).to.be.revertedWithCustomError(
      protocol,
      "ConfirmationsOutOfRange"
    );
    await expect(protocol.windowOf(77)).to.be.revertedWithCustomError(
      protocol,
      "ConfirmationsOutOfRange"
    );
  });

  it("calculates the commitment fee as work x price x 1.5 (D24)", async function () {
    const ctx = await deploy();
    const { job, blockNumber } = await openJob(ctx);
    expect(job.commitmentFee).to.equal(await feeAt(blockNumber, 6));
    expect(job.commitmentFee > 0n).to.equal(true);
  });

  it("charges more for a longer window (D69)", async function () {
    const ctx = await deploy();
    const short = await openJob(ctx, { tag: ethers.id("a"), confirmations: 6 });
    const long = await openJob(ctx, { tag: ethers.id("b"), confirmations: 76 });
    expect(long.job.commitmentFee).to.equal(await feeAt(long.blockNumber, 76));
    expect(long.job.commitmentFee > short.job.commitmentFee).to.equal(true);
  });

  it("follows the price of the network (D58)", async function () {
    const ctx = await deploy();
    const { job, blockNumber } = await openJob(ctx, { price: 40n * GWEI });
    const block = await ethers.provider.getBlock(blockNumber);
    expect(block!.baseFeePerGas).to.equal(40n * GWEI);
    // 30 blocks x 80,000 + 520,000 = 2,920,000 gas, x 40 gwei, x 1.5.
    expect(job.commitmentFee).to.equal((2_920_000n * 40n * GWEI * 3n) / 2n);
  });
});

describe("iPoWProtocol: opening a job (D19, D40, D55, D56, D65)", function () {
  it("records the job and takes the fees", async function () {
    const ctx = await deploy();
    const { protocol, application, user } = ctx;
    const { jobId, job, openedAt } = await openJob(ctx);

    expect(job.application).to.equal(application.address);
    expect(job.payer).to.equal(user.address);
    expect(job.tag).to.equal(TAG);
    expect(job.escrow).to.equal(ETH);
    expect(job.confirmations).to.equal(6n);
    expect(job.openedAt).to.equal(BigInt(openedAt));
    expect(await protocol.jobOfTag(application.address, TAG)).to.equal(jobId);
    expect(await protocol.statusOf(jobId)).to.equal(1n);
  });

  it("calculates the escrow fee as 0.5% of x by default (D25, D40)", async function () {
    const ctx = await deploy();
    const { job } = await openJob(ctx);
    expect(job.escrowFee).to.equal(ETH / 200n);
  });

  it("lets a job request a lower or a higher escrow fee (D25)", async function () {
    const ctx = await deploy();
    const low = await openJob(ctx, { tag: ethers.id("low"), bps: 10 });
    const high = await openJob(ctx, { tag: ethers.id("high"), bps: 200 });
    expect(low.job.escrowFee).to.equal(ETH / 1000n);
    expect(high.job.escrowFee).to.equal(ETH / 50n);
  });

  it("rejects an escrow fee above 100%", async function () {
    const ctx = await deploy();
    await expect(
      openJob(ctx, { bps: 10_001 })
    ).to.be.revertedWithCustomError(ctx.protocol, "EscrowFeeOutOfRange");
  });

  it("keeps what was sent above the fees for the operator, as commitment fee (D79)", async function () {
    const ctx = await deploy();
    const { job, jobId } = await openJob(ctx, { extra: ETH / 100n });
    expect(job.commitmentFee).to.equal(feeFor(GWEI, 6) + ETH / 100n);
    expect(job.escrowFee).to.equal(ETH / 200n);
    expect(await ctx.protocol.feesHeldOf(jobId)).to.equal(
      feeFor(GWEI, 6) + ETH / 100n + ETH / 200n
    );
    expect(await ctx.protocol.credit(ctx.user.address)).to.equal(0n);
  });

  it("calculates the minimum escrow on the fee of the moment, not on what was sent (D56, D79)", async function () {
    const ctx = await deploy();
    const { job } = await openJob(ctx, {
      escrow: 5n * feeFor(GWEI, 6),
      extra: ETH,
    });
    expect(job.escrow).to.equal(5n * feeFor(GWEI, 6));
  });

  it("rejects a job that does not pay its fees (D20)", async function () {
    const ctx = await deploy();
    await expect(
      openJob(ctx, { value: ETH / 200n })
    ).to.be.revertedWithCustomError(ctx.protocol, "FeesNotPaid");
  });

  it("rejects a job with no escrow (D55)", async function () {
    const ctx = await deploy();
    await expect(openJob(ctx, { escrow: 0n })).to.be.revertedWithCustomError(
      ctx.protocol,
      "EscrowTooLow"
    );
  });

  it("rejects an escrow below 5 times the commitment fee and accepts exactly 5 times (D56)", async function () {
    const ctx = await deploy();
    const fee = (2_920_000n * GWEI * 3n) / 2n;

    await expect(
      openJob(ctx, { escrow: 5n * fee - 1n })
    ).to.be.revertedWithCustomError(ctx.protocol, "EscrowTooLow");

    const { job } = await openJob(ctx, { escrow: 5n * fee });
    expect(job.escrow).to.equal(5n * job.commitmentFee);
  });

  it("rejects an application that is not registered (D63)", async function () {
    const { protocol, stranger, user } = await deploy();
    await expect(
      protocol
        .connect(stranger)
        .openJob(TAG, ETH, 50, 6, 0, user.address, ETH / 10n, {
        value: ETH / 10n,
        gasLimit: GAS,
      })
    ).to.be.revertedWithCustomError(protocol, "NotRegistered");
  });

  it("lets an application use a tag once", async function () {
    const ctx = await deploy();
    await openJob(ctx);
    await expect(openJob(ctx)).to.be.revertedWithCustomError(
      ctx.protocol,
      "TagUsed"
    );
  });

  it("lets two applications use the same tag", async function () {
    const ctx = await deploy();
    const { protocol, stranger, user } = ctx;
    await openJob(ctx);
    await protocol.connect(stranger).registerApplication([]);
    await protocol
      .connect(stranger)
      .openJob(TAG, ETH, 50, 6, 0, user.address, ETH / 10n, {
        value: ETH / 10n,
        gasLimit: GAS,
      });
    expect(await protocol.jobCount()).to.equal(2n);
  });

  it("accepts an escrow fee of 0% and of 100%", async function () {
    const ctx = await deploy();
    const none = await openJob(ctx, { tag: ethers.id("none"), bps: 0 });
    const all = await openJob(ctx, { tag: ethers.id("all"), bps: 10_000 });
    expect(none.job.escrowFee).to.equal(0n);
    expect(all.job.escrowFee).to.equal(ETH);
  });

  it("rejects a job with no payer", async function () {
    const { protocol, application } = await deploy();
    await expect(
      protocol
        .connect(application)
        .openJob(TAG, ETH, 50, 6, 0, ethers.ZeroAddress, ETH / 10n, {
          value: ETH / 10n,
          gasLimit: GAS,
        })
    ).to.be.revertedWithCustomError(protocol, "ZeroAddress");
  });

  it("opens a job for a kind of claim the application registered (D17)", async function () {
    const ctx = await deploy();
    const { protocol, stranger, user } = ctx;
    await protocol
      .connect(stranger)
      .registerApplication(new Array(32).fill(7 * DAY));
    await setPrice(GWEI);
    await protocol
      .connect(stranger)
      .openJob(TAG, ETH, 50, 6, 32, user.address, ETH / 10n, {
        value: ETH / 10n,
        gasLimit: GAS,
      });
    expect((await protocol.getJob(1)).claimKind).to.equal(32n);
    await setPrice(GWEI);
    await expect(
      protocol
        .connect(stranger)
        .openJob(ethers.id("x"), ETH, 50, 6, 33, user.address, ETH / 10n, {
          value: ETH / 10n,
          gasLimit: GAS,
        })
    ).to.be.revertedWithCustomError(protocol, "UnknownClaimKind");
  });

  it("tells the fee at a given price, for a caller that cannot read the price in a call", async function () {
    const ctx = await deploy();
    const { job } = await openJob(ctx, { price: 30n * GWEI });
    expect(await ctx.protocol.commitmentFeeAt(6, 30n * GWEI)).to.equal(
      job.commitmentFee
    );
  });

  it("rejects a kind of claim the application did not register (D17)", async function () {
    const ctx = await deploy();
    await expect(openJob(ctx, { claimKind: 1 })).to.be.revertedWithCustomError(
      ctx.protocol,
      "UnknownClaimKind"
    );
  });

  it("rejects confirmations out of range (D41, D70)", async function () {
    const ctx = await deploy();
    await expect(
      openJob(ctx, { confirmations: 5 })
    ).to.be.revertedWithCustomError(ctx.protocol, "ConfirmationsOutOfRange");
    await expect(
      openJob(ctx, { confirmations: 77 })
    ).to.be.revertedWithCustomError(ctx.protocol, "ConfirmationsOutOfRange");
  });
});

describe("iPoWProtocol: auction (D29, D32, D33, D37, D60)", function () {
  async function withJob() {
    const ctx = await deploy();
    await ctx.protocol.connect(ctx.operatorA).lockBond(3n * ETH, { value: 3n * ETH });
    await ctx.protocol.connect(ctx.operatorB).lockBond(3n * ETH, { value: 3n * ETH });
    const opened = await openJob(ctx);
    return { ...ctx, ...opened };
  }

  it("accepts a bid of exactly x and locks it from the bond (D19, D32)", async function () {
    const { protocol, operatorA, jobId } = await withJob();
    await expect(protocol.connect(operatorA).bid(jobId, ETH))
      .to.emit(protocol, "BidPlaced")
      .withArgs(jobId, operatorA.address, ETH);
    expect(await protocol.bondOf(operatorA.address)).to.deep.equal([
      3n * ETH,
      ETH,
    ]);
  });

  it("rejects a bid below x (D32)", async function () {
    const { protocol, operatorA, jobId } = await withJob();
    await expect(
      protocol.connect(operatorA).bid(jobId, ETH - 1n)
    ).to.be.revertedWithCustomError(protocol, "BidTooLow");
  });

  it("rejects a bid above the operator's free bond (D19)", async function () {
    const { protocol, operatorA, jobId } = await withJob();
    await expect(
      protocol.connect(operatorA).bid(jobId, 3n * ETH + 1n)
    ).to.be.revertedWithCustomError(protocol, "BondNotFree");
  });

  it("rejects a bid from someone with no bond", async function () {
    const { protocol, stranger, jobId } = await withJob();
    await expect(
      protocol.connect(stranger).bid(jobId, ETH)
    ).to.be.revertedWithCustomError(protocol, "BondNotFree");
  });

  it("frees the bond of the operator that is outbid (D32)", async function () {
    const { protocol, operatorA, operatorB, jobId } = await withJob();
    await protocol.connect(operatorA).bid(jobId, ETH);
    await protocol.connect(operatorB).bid(jobId, (3n * ETH) / 2n);

    expect(await protocol.bondOf(operatorA.address)).to.deep.equal([
      3n * ETH,
      0n,
    ]);
    expect(await protocol.bondOf(operatorB.address)).to.deep.equal([
      3n * ETH,
      (3n * ETH) / 2n,
    ]);
    expect((await protocol.getJob(jobId)).operator).to.equal(operatorB.address);
  });

  it("rejects a bid that is not better than the best one", async function () {
    const { protocol, operatorA, operatorB, jobId } = await withJob();
    await protocol.connect(operatorA).bid(jobId, 2n * ETH);
    await expect(
      protocol.connect(operatorB).bid(jobId, 2n * ETH)
    ).to.be.revertedWithCustomError(protocol, "BidTooLow");
  });

  it("lets an operator raise its own bid with the bond of its first bid", async function () {
    const { protocol, operatorA, jobId } = await withJob();
    await protocol.connect(operatorA).bid(jobId, 2n * ETH);
    await protocol.connect(operatorA).bid(jobId, 3n * ETH);
    expect(await protocol.bondOf(operatorA.address)).to.deep.equal([
      3n * ETH,
      3n * ETH,
    ]);
  });

  it("keeps the escrow fee on x when the winner locked more (D33)", async function () {
    const { protocol, operatorA, jobId } = await withJob();
    await protocol.connect(operatorA).bid(jobId, 3n * ETH);
    expect((await protocol.getJob(jobId)).escrowFee).to.equal(ETH / 200n);
  });

  it("locks the winner in 1 minute after the last bid (D37)", async function () {
    const { protocol, operatorA, operatorB, jobId, openedAt } = await withJob();
    await at(openedAt + 3 * MINUTE);
    await protocol.connect(operatorA).bid(jobId, ETH);
    expect(await protocol.auctionEndOf(jobId)).to.equal(
      BigInt(openedAt + 4 * MINUTE)
    );

    // One second before the minute is over, a better bid still counts.
    await at(openedAt + 4 * MINUTE - 1);
    await protocol.connect(operatorB).bid(jobId, 2n * ETH);
    expect(await protocol.auctionEndOf(jobId)).to.equal(
      BigInt(openedAt + 5 * MINUTE - 1)
    );

    await at(openedAt + 5 * MINUTE - 1);
    await expect(
      protocol.connect(operatorA).bid(jobId, 3n * ETH)
    ).to.be.revertedWithCustomError(protocol, "AuctionClosed");
    await mineAt(openedAt + 5 * MINUTE - 1);
    expect(await protocol.statusOf(jobId)).to.equal(3n);
    expect((await protocol.getJob(jobId)).operator).to.equal(operatorB.address);
    // The bid that came too late locked nothing.
    expect(await protocol.bondOf(operatorA.address)).to.deep.equal([
      3n * ETH,
      0n,
    ]);
  });

  it("closes 15 minutes after the job was opened, even after a late bid (D37)", async function () {
    const { protocol, operatorA, operatorB, jobId, openedAt } = await withJob();
    await at(openedAt + 15 * MINUTE - 10);
    await protocol.connect(operatorA).bid(jobId, ETH);
    expect(await protocol.auctionEndOf(jobId)).to.equal(
      BigInt(openedAt + 15 * MINUTE)
    );

    await at(openedAt + 15 * MINUTE);
    await expect(
      protocol.connect(operatorB).bid(jobId, 2n * ETH)
    ).to.be.revertedWithCustomError(protocol, "AuctionClosed");
  });

  it("starts the deadline when the winner is locked in (D60, D69)", async function () {
    const { protocol, operatorA, jobId, openedAt } = await withJob();
    expect(await protocol.deadlineOf(jobId)).to.equal(0n);

    await at(openedAt + 2 * MINUTE);
    await protocol.connect(operatorA).bid(jobId, ETH);
    // While a better bid can still come, there is no deadline yet.
    expect(await protocol.deadlineOf(jobId)).to.equal(0n);

    await mineAt(openedAt + 3 * MINUTE);
    expect(await protocol.deadlineOf(jobId)).to.equal(
      BigInt(openedAt + 3 * MINUTE + DAY)
    );
  });

  it("gives a job with 76 confirmations a deadline of 80 hours (D69, D70)", async function () {
    const ctx = await deploy();
    await ctx.protocol.connect(ctx.operatorA).lockBond(3n * ETH, { value: 3n * ETH });
    const { jobId, openedAt } = await openJob(ctx, { confirmations: 76 });
    await at(openedAt + MINUTE);
    await ctx.protocol.connect(ctx.operatorA).bid(jobId, ETH);
    await mineAt(openedAt + 2 * MINUTE);
    expect(await ctx.protocol.deadlineOf(jobId)).to.equal(
      BigInt(openedAt + 2 * MINUTE + 80 * HOUR)
    );
  });

  it("rejects a job that does not exist", async function () {
    const { protocol, operatorA } = await withJob();
    await expect(
      protocol.connect(operatorA).bid(99, ETH)
    ).to.be.revertedWithCustomError(protocol, "UnknownJob");
    for (const call of [
      () => protocol.expire(99),
      () => protocol.auctionEndOf(99),
      () => protocol.deadlineOf(99),
      () => protocol.feesHeldOf(99),
      () => protocol.minimumBidOf(99),
    ]) {
      await expect(call()).to.be.revertedWithCustomError(
        protocol,
        "UnknownJob"
      );
    }
  });

  it("lets the operator that was outbid withdraw its whole bond", async function () {
    const { protocol, operatorA, operatorB, jobId } = await withJob();
    await protocol.connect(operatorA).bid(jobId, 2n * ETH);
    await protocol.connect(operatorB).bid(jobId, 3n * ETH);
    await expect(
      protocol.connect(operatorA).withdrawBond(3n * ETH)
    ).to.changeEtherBalances(ethers, [operatorA], [3n * ETH]);
  });

  it("locks one bond for one job only", async function () {
    const ctx = await withJob();
    const { protocol, operatorA, jobId } = ctx;
    const second = await openJob(ctx, { tag: ethers.id("second") });
    await protocol.connect(operatorA).bid(jobId, 2n * ETH);
    await expect(
      protocol.connect(operatorA).bid(second.jobId, 2n * ETH)
    ).to.be.revertedWithCustomError(protocol, "BondNotFree");
    await protocol.connect(operatorA).bid(second.jobId, ETH);
    expect(await protocol.bondOf(operatorA.address)).to.deep.equal([
      3n * ETH,
      3n * ETH,
    ]);
  });

  it("needs a better bid to be at least 0.1% above the best one (D76)", async function () {
    const { protocol, operatorA, operatorB, jobId } = await withJob();
    expect(await protocol.minimumBidOf(jobId)).to.equal(ETH);
    await protocol.connect(operatorA).bid(jobId, ETH);

    const step = ETH / 1000n;
    expect(await protocol.minimumBidOf(jobId)).to.equal(ETH + step);
    await expect(
      protocol.connect(operatorB).bid(jobId, ETH + 1n)
    ).to.be.revertedWithCustomError(protocol, "BidTooLow");
    await expect(
      protocol.connect(operatorB).bid(jobId, ETH + step - 1n)
    ).to.be.revertedWithCustomError(protocol, "BidTooLow");

    await protocol.connect(operatorB).bid(jobId, ETH + step);
    expect((await protocol.getJob(jobId)).operator).to.equal(operatorB.address);
  });

  it("applies the 0.1% to an operator that raises its own bid", async function () {
    const { protocol, operatorA, jobId } = await withJob();
    await protocol.connect(operatorA).bid(jobId, ETH);
    await expect(
      protocol.connect(operatorA).bid(jobId, ETH + 1n)
    ).to.be.revertedWithCustomError(protocol, "BidTooLow");
  });
});

describe("iPoWProtocol: a job nobody takes (D61)", function () {
  it("expires after 15 minutes and returns both fees to the payer", async function () {
    const ctx = await deploy();
    const { protocol, user, stranger } = ctx;
    // The job was sent 0.01 ETH more than its fees.
    const { jobId, job, openedAt } = await openJob(ctx, { extra: ETH / 100n });
    const sent = feeFor(GWEI, 6) + ETH / 200n + ETH / 100n;
    expect(job.commitmentFee + job.escrowFee).to.equal(sent);
    expect(await protocol.credit(user.address)).to.equal(0n);

    await at(openedAt + 15 * MINUTE);
    await expect(protocol.connect(stranger).expire(jobId))
      .to.emit(protocol, "JobExpired")
      .withArgs(jobId);
    expect(await protocol.statusOf(jobId)).to.equal(2n);
    // Everything that was sent returns: no operator did the job.
    expect(await protocol.credit(user.address)).to.equal(sent);

    await expect(
      protocol.connect(user).withdrawCredit()
    ).to.changeEtherBalances(ethers, [user, protocol], [sent, -sent]);
    expect(await protocol.credit(user.address)).to.equal(0n);
  });

  it("does not expire while bidding is open", async function () {
    const ctx = await deploy();
    const { jobId, openedAt } = await openJob(ctx);
    await at(openedAt + 15 * MINUTE - 1);
    await expect(ctx.protocol.expire(jobId)).to.be.revertedWithCustomError(
      ctx.protocol,
      "AuctionOpen"
    );
  });

  it("does not expire a job that has a winner", async function () {
    const ctx = await deploy();
    await ctx.protocol.connect(ctx.operatorA).lockBond(ETH, { value: ETH });
    const { jobId, openedAt } = await openJob(ctx);
    await ctx.protocol.connect(ctx.operatorA).bid(jobId, ETH);
    await at(openedAt + 15 * MINUTE);
    await expect(ctx.protocol.expire(jobId)).to.be.revertedWithCustomError(
      ctx.protocol,
      "JobHasBid"
    );
  });

  it("returns the fees once", async function () {
    const ctx = await deploy();
    const { jobId, openedAt } = await openJob(ctx);
    await at(openedAt + 15 * MINUTE);
    await ctx.protocol.expire(jobId);
    await expect(ctx.protocol.expire(jobId)).to.be.revertedWithCustomError(
      ctx.protocol,
      "FeesAlreadyReturned"
    );
  });

  it("rejects a bid after the job expired", async function () {
    const ctx = await deploy();
    await ctx.protocol.connect(ctx.operatorA).lockBond(ETH, { value: ETH });
    const { jobId, openedAt } = await openJob(ctx);
    await mineAt(openedAt + 15 * MINUTE);
    await expect(
      ctx.protocol.connect(ctx.operatorA).bid(jobId, ETH)
    ).to.be.revertedWithCustomError(ctx.protocol, "AuctionClosed");
  });

  it("has nothing to withdraw for someone with no credit", async function () {
    const { protocol, stranger } = await deploy();
    await expect(
      protocol.connect(stranger).withdrawCredit()
    ).to.be.revertedWithCustomError(protocol, "ZeroAmount");
  });
});

describe("iPoWProtocol: no person in control (D59)", function () {
  it("cannot be deployed without a light client", async function () {
    const factory = await ethers.getContractFactory("iPoWProtocolNative");
    await expect(
      factory.deploy(ethers.ZeroAddress, ethers.ZeroAddress)
    ).to.be.revertedWithCustomError(factory, "ZeroAddress");
  });

  it("holds exactly the bonds, the fees of open jobs and the credits", async function () {
    const ctx = await deploy();
    const { protocol, operatorA, operatorB, user } = ctx;

    async function check(jobIds: bigint[]) {
      let expected = 0n;
      for (const who of [operatorA, operatorB]) {
        expected += (await protocol.bondOf(who.address))[0];
      }
      for (const id of jobIds) expected += await protocol.feesHeldOf(id);
      expected += await protocol.credit(user.address);
      expect(
        await ethers.provider.getBalance(await protocol.getAddress())
      ).to.equal(expected);
    }

    await protocol.connect(operatorA).lockBond(3n * ETH, { value: 3n * ETH });
    await protocol.connect(operatorB).lockBond(3n * ETH, { value: 3n * ETH });
    const first = await openJob(ctx, { tag: ethers.id("1"), value: ETH });
    const second = await openJob(ctx, { tag: ethers.id("2"), value: ETH });
    const jobs = [first.jobId, second.jobId];
    await check(jobs);

    await protocol.connect(operatorA).bid(first.jobId, ETH);
    await protocol.connect(operatorB).bid(first.jobId, 2n * ETH);
    await check(jobs);

    await protocol.connect(operatorA).withdrawBond(3n * ETH);
    await check(jobs);

    await mineAt(second.openedAt + 15 * MINUTE);
    await protocol.expire(second.jobId);
    await check(jobs);

    await protocol.connect(user).withdrawCredit();
    await check(jobs);
  });
});

describe("iPoWProtocol: an operator that is a contract", function () {
  async function withActor() {
    const ctx = await deploy();
    const actor = await ethers.deployContract("ProtocolActor", [
      await ctx.protocol.getAddress(),
    ]);
    await actor.lockBond({ value: 3n * ETH });
    return { ...ctx, actor };
  }

  it("cannot withdraw twice by calling back while it is paid", async function () {
    const { protocol, actor } = await withActor();
    await actor.setMode(2);
    await expect(actor.withdrawBond(ETH)).to.be.revertedWithCustomError(
      protocol,
      "TransferFailed"
    );
    expect(await protocol.bondOf(await actor.getAddress())).to.deep.equal([
      3n * ETH,
      0n,
    ]);
  });

  it("cannot reach its credit by calling back while it is paid", async function () {
    const { protocol, actor } = await withActor();
    await actor.setMode(3);
    await expect(actor.withdrawBond(ETH)).to.be.revertedWithCustomError(
      protocol,
      "TransferFailed"
    );
  });

  it("does not stop a better bid when it refuses money", async function () {
    const ctx = await withActor();
    const { protocol, actor, operatorA } = ctx;
    await protocol.connect(operatorA).lockBond(3n * ETH, { value: 3n * ETH });
    const { jobId } = await openJob(ctx);

    await actor.bid(jobId, ETH);
    await actor.setMode(1);
    await protocol.connect(operatorA).bid(jobId, 2n * ETH);

    expect((await protocol.getJob(jobId)).operator).to.equal(operatorA.address);
    expect(await protocol.bondOf(await actor.getAddress())).to.deep.equal([
      3n * ETH,
      0n,
    ]);
    // It harms only itself: its own withdrawal fails until it accepts money.
    await expect(actor.withdrawBond(ETH)).to.be.revertedWithCustomError(
      protocol,
      "TransferFailed"
    );
    await actor.setMode(0);
    await actor.withdrawBond(ETH);
  });

  it("rejects a withdrawal of nothing", async function () {
    const { protocol, operatorA } = await withActor();
    await expect(
      protocol.connect(operatorA).withdrawBond(0)
    ).to.be.revertedWithCustomError(protocol, "ZeroAmount");
  });
});
