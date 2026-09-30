import { expect } from "chai";
import { network } from "hardhat";

import { COIN_SCRIPT, buildTx, concat, headerHashLE, merkle, mine, targetFromBits, txidLE } from "./helpers/bitcoin.ts";

const { ethers } = await network.create();

// The protocol's vault on Ethereum. Spec: docs/design/ipow-protocol.md,
// section 11. The Bitcoin blocks here are mined by the test at a low
// difficulty, which only the test light client allows.

const MINUTE = 60;
const HOUR = 3600;
const DAY = 24 * HOUR;
const WEEK = 7 * DAY;
const ETH = 10n ** 18n;
const GWEI = 10n ** 9n;
const EASY = 0x207fffff;
const ZERO_HASH = "0x" + "00".repeat(32);
const FEES = ETH / 10n;
const DEPOSIT = ETH / 100n;
const PEER_VAULT = ethers.id("solana vault");
const PEER_OPERATOR = ethers.id("carl on solana");
const RECIPIENT = ethers.id("alice on solana");
const GAS = 3_000_000n;
const MIN_CERTIFYING_ESCROW = ETH;

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

function opReturn(payload: string): string {
  return "0x6a20" + payload.slice(2);
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

  /** Mines a block on `on` (the tip by default) and stores it. */
  async add(txs: Tx[] = [], on?: Ref): Promise<Ref> {
    const parent = on ?? this.tip;
    const coinbase = buildTx({
      inputs: [{ txidLE: ZERO_HASH, vout: 0xffffffff }],
      outputs: [{ value: BigInt(++this.salt), script: COIN_SCRIPT }],
    });
    const txids = [txidLE(coinbase), ...txs.map((t) => t.txid)];
    const header = mine({ prevLE: parent.hash, time: (await latestTime()) + 1, bits: EASY, merkleRootLE: merkle(txids, 0).rootLE });
    await this.lightClient.extend(header, parent.height, parent.epochTime);
    const ref = { hash: headerHashLE(header), height: parent.height + 1, epochTime: parent.epochTime };
    if (!on) this.tip = ref;
    this.blocks.set(ref.hash, txids);
    return ref;
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

// Records, as section 11.5 writes them. Amounts in gwei, big-endian.
const u64 = (n: bigint) => ethers.toBeHex(n, 8);
const rec = {
  lock: (id: bigint, amount: bigint, recipient: string, fee: bigint) => ethers.concat(["0x01", u64(id), u64(amount), recipient, u64(fee)]),
  request: (id: bigint, amount: bigint, to: string, fee: bigint) => ethers.concat(["0x02", u64(id), u64(amount), to, u64(fee)]),
  cancel: (id: bigint) => ethers.concat(["0x03", u64(id)]),
  bond: (net: number, amount: bigint) => ethers.concat(["0x04", ethers.toBeHex(net, 1), u64(amount)]),
  exit: () => "0x05",
};
const ETHEREUM = 1;
const SOLANA = 2;

async function deploy() {
  const lightClient = await ethers.deployContract("iPoWLightClientHarness", [0]);
  await lightClient.setLimits(targetFromBits(EASY), targetFromBits(EASY));
  const protocol = await ethers.deployContract("iPoWProtocolHarness", [await lightClient.getAddress()]);
  const vault = await ethers.deployContract("iPoWVault", [await protocol.getAddress(), PEER_VAULT, DEPOSIT, MIN_CERTIFYING_ESCROW]);
  const [, user, operator, guardian, stranger] = await ethers.getSigners();

  const now = Math.ceil(((await latestTime()) + DAY) / DAY) * DAY;
  await mineAt(now);
  const chain = new TestChain(lightClient);
  await chain.start(now);

  // The operator's protocol bond and chain head, for checkpoint jobs.
  await protocol.connect(operator).lockBond({ value: 3n * ETH });
  const first = tx([{ txidLE: ethers.id("funding"), vout: 0 }], [
    { value: 546n, script: COIN_SCRIPT },
    { value: 0n, script: opReturn(await protocol.chainHeadCommitment(operator.address)) },
  ]);
  const block = await chain.add([first]);
  const { siblings, txIndex } = chain.proofOf(block, first);
  await protocol.connect(operator).registerChainHead(block, first.raw, siblings, txIndex, 0, 1);

  return { lightClient, protocol, vault, chain, user, operator, guardian, stranger };
}

type Ctx = Awaited<ReturnType<typeof deploy>>;

/**
 * D108, D116: a checkpoint job opened through the vault, proven by the
 * operator on top of the chain, its lock ended. Its proof block is real.
 */
async function checkpoint(ctx: Ctx): Promise<Ref> {
  const { protocol, vault, chain, operator, stranger } = ctx;
  await vault.connect(stranger).openCheckpoint(6, { value: FEES, gasLimit: GAS });
  const jobId = await protocol.jobCount();
  await protocol.connect(operator).bid(jobId, await protocol.minimumBidOf(jobId));
  await mineAt((await latestTime()) + 61);
  const anchor = await chain.add();
  await protocol.connect(operator).anchorJob(jobId, anchor);
  const head = await protocol.chainHeadOf(operator.address);
  const tagged = tx([{ txidLE: head.txid, vout: Number(head.vout) }], [
    { value: 546n, script: COIN_SCRIPT },
    { value: 0n, script: opReturn(tagPayload((await protocol.getJob(jobId)).tag)) },
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
  await expect(vault.recordRealFromJob(jobId)).to.be.revertedWithCustomError(vault, "NotCertified");
  await mineAt(Number((await protocol.getDuty(jobId)).lockEnd) + 1);
  await vault.recordRealFromJob(jobId);
  return proofBlock;
}

/** An operator's pair chain: its coin, and the messages it wrote. */
class PairChain {
  coin: { txidLE: string; vout: number };
  pending: { tx: Tx; block: Ref; batch: string }[] = [];
  constructor(public ctx: Ctx, public registration: { tx: Tx; block: Ref }) {
    this.coin = { txidLE: registration.tx.txid, vout: 0 };
  }

  /** Writes a message on Bitcoin, mined on `on` or on the tip. */
  async write(batch: string, on?: Ref) {
    const t = tx([this.coin], [
      { value: 546n, script: COIN_SCRIPT },
      { value: 0n, script: opReturn(await this.ctx.vault.messagePayload(batch)) },
    ]);
    const block = await this.ctx.chain.add([t], on);
    this.coin = { txidLE: t.txid, vout: 0 };
    const m = { tx: t, block, batch };
    this.pending.push(m);
    return m;
  }
}

function btcOf(ctx: Ctx, m: { tx: Tx; block: Ref }, real: Ref) {
  const { siblings, txIndex } = ctx.chain.proofOf(m.block, m.tx);
  return { block: m.block, rawTx: m.tx.raw, siblings, txIndex, real, prevEpochTime: 0 };
}

/** Writes the registration of the operator's pair chain on Bitcoin. */
async function writeRegistration(ctx: Ctx, who = ctx.operator) {
  const t = tx([{ txidLE: ethers.id("pair funding " + who.address), vout: 0 }], [
    { value: 546n, script: COIN_SCRIPT },
    { value: 0n, script: opReturn(await ctx.vault.pairCommitment(who.address, PEER_OPERATOR)) },
  ]);
  const block = await ctx.chain.add([t]);
  return new PairChain(ctx, { tx: t, block });
}

async function register(ctx: Ctx, pair: PairChain, real: Ref, who = ctx.operator) {
  await ctx.vault.connect(who).registerChain(PEER_OPERATOR, btcOf(ctx, pair.registration, real), 0, 1);
}

async function submit(ctx: Ctx, m: { tx: Tx; block: Ref; batch: string }, real: Ref, from = ctx.guardian, operator = ctx.operator) {
  return ctx.vault.connect(from).submitMessage(operator.address, btcOf(ctx, m, real), 0, 1, m.batch);
}

describe("iPoWVault: locks", function () {
  it("locks whole gwei, and keeps the amount as backing", async function () {
    const { vault, user } = await deploy();
    await vault.connect(user).lock(RECIPIENT, 10n * GWEI, { value: ETH + 10n * GWEI });
    const l = await vault.getLock(1n);
    expect(l.amount).to.equal(ETH / GWEI);
    expect(l.fee).to.equal(10n);
    expect(l.recipient).to.equal(RECIPIENT);
    expect(await vault.reserve()).to.equal(ETH);
    await expect(vault.connect(user).lock(RECIPIENT, 0, { value: ETH + 1n })).to.be.revertedWithCustomError(vault, "NotGwei");
    await expect(vault.connect(user).lock(RECIPIENT, ETH, { value: ETH })).to.be.revertedWithCustomError(vault, "ZeroAmount");
  });
});

describe("iPoWVault: real Bitcoin (D108)", function () {
  it("accepts a registration below a finished job's proof block, and a forged one never", async function () {
    const ctx = await deploy();
    const { vault, chain, operator, stranger } = ctx;
    const pair = await writeRegistration(ctx);
    // Before any real block exists, nothing counts.
    await expect(register(ctx, pair, pair.registration.block)).to.be.revertedWithCustomError(vault, "NotReal");
    const real = await checkpoint(ctx);
    await register(ctx, pair, real);
    const c = await vault.getChain(operator.address);
    expect(c.registered).to.equal(true);
    expect(c.peerOperator).to.equal(PEER_OPERATOR);

    // Mallory mines her own block off the real chain with a message "from"
    // the operator. It is not below any real block.
    const fork = await chain.add([], pair.registration.block);
    const forged = await pair.write(rec.exit(), fork);
    await expect(submit(ctx, forged, real, stranger)).to.be.revertedWithCustomError(vault, "NotReal");
    await expect(submit(ctx, forged, fork, stranger)).to.be.revertedWithCustomError(vault, "NotReal");
  });

  it("names both networks' operators and vaults in a registration, as section 11.3 writes it", async function () {
    const { vault, operator } = await deploy();
    const expected = ethers.sha256(
      ethers.concat([ethers.toUtf8Bytes("iPoW pair"), await vault.getAddress(), operator.address, PEER_VAULT, PEER_OPERATOR])
    );
    expect(await vault.pairCommitment(operator.address, PEER_OPERATOR)).to.equal(expected);
    expect(await vault.messagePayload("0x05")).to.equal(ethers.sha256(ethers.concat([ethers.toUtf8Bytes("iPoW vault"), "0x05"])));
  });

  it("records a block below a real block as real", async function () {
    const ctx = await deploy();
    const { vault, lightClient, chain } = ctx;
    const low = await chain.add();
    const real = await checkpoint(ctx);
    const id = await lightClient.nodeId(low.hash, low.height, low.epochTime);
    expect(await vault.isReal(id)).to.equal(false);
    await vault.recordReal(low, real, 0);
    expect(await vault.isReal(id)).to.equal(true);
  });

  it("refuses a registration that names someone else", async function () {
    const ctx = await deploy();
    const { vault, stranger } = ctx;
    const pair = await writeRegistration(ctx);
    const real = await checkpoint(ctx);
    await expect(register(ctx, pair, real, stranger)).to.be.revertedWithCustomError(vault, "WrongTag");
  });
});

/** A registered operator with `bond` in the vault; the messages written in `write` before the checkpoint. */
async function ready(ctx: Ctx, bond: bigint, write: (pair: PairChain) => Promise<void>) {
  const pair = await writeRegistration(ctx);
  await write(pair);
  const real = await checkpoint(ctx);
  await register(ctx, pair, real);
  await ctx.vault.connect(ctx.operator).addBond({ value: bond });
  await ctx.vault.connect(ctx.operator).addDeposits({ value: 10n * DEPOSIT });
  return { pair, real };
}

describe("iPoWVault: judging records about Ethereum (D109)", function () {
  it("pays the lock's fee for a true LOCK, once", async function () {
    const ctx = await deploy();
    const { vault, user, operator } = ctx;
    await vault.connect(user).lock(RECIPIENT, 5n * GWEI, { value: ETH + 5n * GWEI });
    const record = rec.lock(1n, ETH / GWEI, RECIPIENT, 5n);
    const { pair, real } = await ready(ctx, ETH, async (p) => {
      await p.write(record);
      await p.write(record);
    });
    await expect(submit(ctx, pair.pending[0], real)).to.emit(vault, "FeeEarned").withArgs(1n, operator.address, 5n * GWEI);
    await expect(submit(ctx, pair.pending[1], real)).to.not.emit(vault, "FeeEarned");
    expect(await vault.credit(operator.address)).to.equal(5n * GWEI);
    expect((await vault.getChain(operator.address)).messages).to.equal(2n);
  });

  it("slashes the whole bond for a false LOCK: 80% backs vETH, 20% to the submitter", async function () {
    const ctx = await deploy();
    const { vault, user, operator, guardian } = ctx;
    await vault.connect(user).lock(RECIPIENT, 0, { value: ETH });
    const { pair, real } = await ready(ctx, 5n * ETH, async (p) => {
      await p.write(rec.lock(1n, 2n * ETH / GWEI, RECIPIENT, 0n)); // wrong amount
    });
    await expect(submit(ctx, pair.pending[0], real, guardian))
      .to.emit(vault, "Slashed")
      .withArgs(operator.address, 5n * ETH, guardian.address);
    expect(await vault.reserve()).to.equal(ETH + 4n * ETH);
    expect(await vault.credit(guardian.address)).to.equal(ETH);
    const c = await vault.getChain(operator.address);
    expect(c.slashed).to.equal(true);
    expect(c.bond).to.equal(0n);
  });

  it("gives the whole slash to the backing when the operator submits its own lie", async function () {
    const ctx = await deploy();
    const { vault, operator } = ctx;
    const { pair, real } = await ready(ctx, 5n * ETH, async (p) => {
      await p.write(rec.lock(7n, 1n, RECIPIENT, 0n)); // no lock #7
    });
    await submit(ctx, pair.pending[0], real, operator);
    expect(await vault.reserve()).to.equal(5n * ETH);
    expect(await vault.credit(operator.address)).to.equal(0n);
  });

  it("treats a batch that does not parse as false", async function () {
    const ctx = await deploy();
    const { vault, operator } = ctx;
    const { pair, real } = await ready(ctx, ETH, async (p) => {
      await p.write("0x0901");
    });
    await submit(ctx, pair.pending[0], real);
    expect((await vault.getChain(operator.address)).slashed).to.equal(true);
  });

  it("processes a chain in order and never twice", async function () {
    const ctx = await deploy();
    const { vault } = ctx;
    const { pair, real } = await ready(ctx, ETH, async (p) => {
      await p.write(rec.bond(ETHEREUM, 1n));
      await p.write(rec.bond(ETHEREUM, 1n));
    });
    await expect(submit(ctx, pair.pending[1], real)).to.be.revertedWithCustomError(vault, "WrongCoin");
    await submit(ctx, pair.pending[0], real);
    await expect(submit(ctx, pair.pending[0], real)).to.be.revertedWithCustomError(vault, "WrongCoin");
  });

  it("keeps a stated BOND locked, and slashes a BOND above the bond", async function () {
    const ctx = await deploy();
    const { vault, operator } = ctx;
    const { pair, real } = await ready(ctx, 2n * ETH, async (p) => {
      await p.write(rec.bond(ETHEREUM, ETH / GWEI));
    });
    await submit(ctx, pair.pending[0], real);
    expect((await vault.getChain(operator.address)).stated).to.equal(ETH);
    expect(await vault.freeBond(operator.address)).to.equal(ETH);
    await expect(vault.connect(operator).withdrawBond(ETH + 1n)).to.be.revertedWithCustomError(vault, "BondNotFree");
    await vault.connect(operator).withdrawBond(ETH);

    const ctx2 = await deploy();
    const r2 = await ready(ctx2, ETH, async (p) => {
      await p.write(rec.bond(ETHEREUM, 2n * ETH / GWEI));
    });
    await submit(ctx2, r2.pair.pending[0], r2.real);
    expect((await ctx2.vault.getChain(ctx2.operator.address)).slashed).to.equal(true);
  });
});

describe("iPoWVault: claims from Solana (D110, D111)", function () {
  /** Operator with a vETH bond of `peer` on Solana made official, and 1 ETH locked by the user. */
  async function withPeerBond(ctx: Ctx, peer: bigint, more: (p: PairChain) => Promise<void>) {
    await ctx.vault.connect(ctx.user).lock(RECIPIENT, 0, { value: 2n * ETH });
    const r = await ready(ctx, ETH, async (p) => {
      await p.write(rec.bond(SOLANA, peer / GWEI));
      await more(p);
    });
    await submit(ctx, r.pair.pending[0], r.real);
    const claimId = await ctx.vault.claimCount();
    await mineAt((await latestTime()) + WEEK);
    await ctx.vault.decide(claimId);
    expect((await ctx.vault.getChain(ctx.operator.address)).peerBond).to.equal(peer);
    return r;
  }

  it("pays a REQUEST after 7 days with no objection", async function () {
    const ctx = await deploy();
    const { vault, stranger, operator } = ctx;
    const { pair, real } = await withPeerBond(ctx, 5n * ETH, async (p) => {
      await p.write(rec.request(7n, ETH / GWEI, stranger.address, 0n));
    });
    await expect(submit(ctx, pair.pending[1], real)).to.emit(vault, "ClaimBatch").withArgs(2n, pair.pending[1].batch);
    const claimId = await vault.claimCount();
    expect((await vault.getClaim(claimId)).openedBlock).to.equal(BigInt((await ethers.provider.getBlock("latest"))!.number));
    await expect(vault.payRequest(claimId, 7n)).to.be.revertedWithCustomError(vault, "NotAccepted");
    await expect(vault.decide(claimId)).to.be.revertedWithCustomError(vault, "WindowNotOver");
    await mineAt((await latestTime()) + WEEK);
    await vault.decide(claimId);
    await vault.payRequest(claimId, 7n);
    expect(await vault.credit(stranger.address)).to.equal(ETH);
    expect(await vault.reserve()).to.equal(ETH);
    await expect(vault.payRequest(claimId, 7n)).to.be.revertedWithCustomError(vault, "AlreadyDone");
    // The operator collects its deposit back.
    await vault.connect(operator).collect(claimId);
    expect(await vault.credit(operator.address)).to.equal(DEPOSIT);
    await expect(vault.connect(operator).collect(claimId)).to.be.revertedWithCustomError(vault, "NothingToCollect");
  });

  it("opens no claim past 80% of the bond on Solana", async function () {
    const ctx = await deploy();
    const { vault, stranger } = ctx;
    const { pair, real } = await withPeerBond(ctx, ETH, async (p) => {
      await p.write(rec.request(7n, (ETH * 8n) / 10n / GWEI + 1n, stranger.address, 0n));
    });
    const before = await vault.claimCount();
    await submit(ctx, pair.pending[1], real);
    expect(await vault.claimCount()).to.equal(before);
    expect((await vault.getRequest(before + 1n, 7n)).to).to.equal(ethers.ZeroAddress);
  });

  it("refuses a claim whose objection stood for 7 days, and pays the objector", async function () {
    const ctx = await deploy();
    const { vault, stranger, guardian } = ctx;
    const { pair, real } = await withPeerBond(ctx, 5n * ETH, async (p) => {
      await p.write(rec.request(7n, ETH / GWEI, stranger.address, 0n));
      await p.write(rec.request(8n, ETH / GWEI, stranger.address, 0n));
    });
    await submit(ctx, pair.pending[1], real);
    const claimId = await vault.claimCount();
    await expect(vault.connect(guardian).object(claimId, { value: 1n })).to.be.revertedWithCustomError(vault, "WrongDeposit");
    await vault.connect(guardian).object(claimId, { value: DEPOSIT });
    await expect(vault.connect(guardian).object(claimId, { value: DEPOSIT })).to.be.revertedWithCustomError(vault, "AlreadyHeld");
    await mineAt((await latestTime()) + WEEK);
    await vault.decide(claimId);
    expect((await vault.getClaim(claimId)).accepted).to.equal(false);
    await expect(vault.payRequest(claimId, 7n)).to.be.revertedWithCustomError(vault, "NotAccepted");
    // The objector gets its deposit back and the operator's.
    await vault.connect(guardian).collect(claimId);
    expect(await vault.credit(guardian.address)).to.equal(2n * DEPOSIT);
    // No later claim of the chain acts.
    const count = await vault.claimCount();
    await submit(ctx, pair.pending[2], real);
    expect(await vault.claimCount()).to.equal(count);
  });

  it("accepts a claim whose last answer stood for 7 days, and pays the answerer", async function () {
    const ctx = await deploy();
    const { vault, stranger, guardian, user } = ctx;
    const { pair, real } = await withPeerBond(ctx, 5n * ETH, async (p) => {
      await p.write(rec.request(7n, ETH / GWEI, stranger.address, 0n));
    });
    await submit(ctx, pair.pending[1], real);
    const claimId = await vault.claimCount();
    await mineAt((await latestTime()) + 6 * DAY);
    await vault.connect(guardian).object(claimId, { value: DEPOSIT });
    await mineAt((await latestTime()) + 6 * DAY);
    await expect(vault.connect(user).answer(claimId, { value: 1n })).to.be.revertedWithCustomError(vault, "WrongDeposit");
    await vault.connect(user).answer(claimId, { value: DEPOSIT });
    // The 7 days restart from the answer.
    await mineAt((await latestTime()) + 6 * DAY);
    await expect(vault.decide(claimId)).to.be.revertedWithCustomError(vault, "WindowNotOver");
    await mineAt((await latestTime()) + DAY);
    await vault.decide(claimId);
    expect((await vault.getClaim(claimId)).accepted).to.equal(true);
    // Two answers, the operator's and the user's, share the objector's deposit.
    await vault.connect(user).collect(claimId);
    expect(await vault.credit(user.address)).to.equal(DEPOSIT + DEPOSIT / 2n);
    await expect(vault.connect(guardian).collect(claimId)).to.be.revertedWithCustomError(vault, "NothingToCollect");
  });

  it("returns a lock whose CANCEL was accepted, with its unearned fee", async function () {
    const ctx = await deploy();
    const { vault, user } = ctx;
    await vault.connect(user).lock(RECIPIENT, 3n * GWEI, { value: ETH + 3n * GWEI });
    const { pair, real } = await withPeerBond(ctx, 5n * ETH, async (p) => {
      await p.write(rec.cancel(1n));
    });
    await submit(ctx, pair.pending[1], real);
    const claimId = await vault.claimCount();
    await mineAt((await latestTime()) + WEEK);
    await vault.decide(claimId);
    await vault.returnLock(claimId, 1n);
    expect(await vault.credit(user.address)).to.equal(ETH + 3n * GWEI);
    expect((await vault.getLock(1n)).returned).to.equal(true);
    await expect(vault.returnLock(claimId, 1n)).to.be.revertedWithCustomError(vault, "AlreadyDone");
    await expect(vault.returnLock(claimId, 2n)).to.be.revertedWithCustomError(vault, "NoCancel");
  });

  it("refuses the open claims of a chain proven false here", async function () {
    const ctx = await deploy();
    const { vault, stranger } = ctx;
    const { pair, real } = await withPeerBond(ctx, 5n * ETH, async (p) => {
      await p.write(rec.request(7n, ETH / GWEI, stranger.address, 0n));
      await p.write(rec.lock(99n, 1n, RECIPIENT, 0n)); // no lock #99
    });
    await submit(ctx, pair.pending[1], real);
    const claimId = await vault.claimCount();
    await submit(ctx, pair.pending[2], real);
    await mineAt((await latestTime()) + WEEK);
    await vault.decide(claimId);
    expect((await vault.getClaim(claimId)).accepted).to.equal(false);
  });
});

describe("iPoWVault: leaving (D112)", function () {
  it("ends the chain at EXIT, and frees the bond once its claims have ended", async function () {
    const ctx = await deploy();
    const { vault, operator } = ctx;
    const { pair, real } = await ready(ctx, 2n * ETH, async (p) => {
      await p.write(rec.bond(ETHEREUM, ETH / GWEI));
      await p.write(ethers.concat([rec.bond(SOLANA, ETH / GWEI), rec.exit()]));
      await p.write(rec.bond(ETHEREUM, 1n));
    });
    await submit(ctx, pair.pending[0], real);
    await submit(ctx, pair.pending[1], real);
    await expect(submit(ctx, pair.pending[2], real)).to.be.revertedWithCustomError(vault, "ChainEnded");
    // The BOND claim for Solana is still open.
    expect(await vault.freeBond(operator.address)).to.equal(ETH);
    await mineAt((await latestTime()) + WEEK);
    await vault.decide(await vault.claimCount());
    expect(await vault.freeBond(operator.address)).to.equal(2n * ETH);
  });

  it("treats a record after EXIT as false", async function () {
    const ctx = await deploy();
    const { vault, operator } = ctx;
    const { pair, real } = await ready(ctx, ETH, async (p) => {
      await p.write(ethers.concat([rec.exit(), rec.bond(ETHEREUM, 1n)]));
    });
    await submit(ctx, pair.pending[0], real);
    expect((await vault.getChain(operator.address)).slashed).to.equal(true);
  });
});

/** What the vault owes: backing, bonds, deposit money, credits, and deposits in open claims. */
async function owed(ctx: Ctx, addresses: string[], claims: bigint[]) {
  const { vault } = ctx;
  let total = await vault.reserve();
  // Fees held until a message earns them, or the lock is returned.
  for (let i = 1n; i <= (await vault.lockCount()); i++) {
    const l = await vault.getLock(i);
    if (!l.feePaid) total += l.fee * GWEI;
  }
  for (const a of addresses) {
    const c = await vault.getChain(a);
    total += c.bond + c.deposits + (await vault.credit(a));
  }
  for (const id of claims) {
    const cl = await vault.getClaim(id);
    if (!cl.decided) total += (cl.answers + cl.objections) * DEPOSIT;
    else for (const a of addresses) total += (cl.accepted ? await vault.answersOf(id, a) : await vault.objectionsOf(id, a)) * cl.payout;
  }
  return total;
}

describe("iPoWVault: review cases", function () {
  it("never pays a lock's fee again after the lock was returned", async function () {
    const ctx = await deploy();
    const { vault, user, operator } = ctx;
    await vault.connect(user).lock(RECIPIENT, 7n * GWEI, { value: ETH + 7n * GWEI });
    const r = await ready(ctx, ETH, async (p) => {
      await p.write(rec.bond(SOLANA, (5n * ETH) / GWEI));
      await p.write(rec.cancel(1n));
      await p.write(rec.lock(1n, ETH / GWEI, RECIPIENT, 7n));
    });
    await submit(ctx, r.pair.pending[0], r.real);
    await mineAt((await latestTime()) + WEEK);
    await vault.decide(1n);
    await submit(ctx, r.pair.pending[1], r.real);
    await mineAt((await latestTime()) + WEEK);
    await vault.decide(2n);
    await vault.returnLock(2n, 1n);
    expect(await vault.credit(user.address)).to.equal(ETH + 7n * GWEI);
    // A LOCK record about the returned lock is true, but earns nothing.
    await expect(submit(ctx, r.pair.pending[2], r.real)).to.not.emit(vault, "FeeEarned");
    expect((await vault.getChain(operator.address)).slashed).to.equal(false);
  });

  it("lets a batch state the whole bond and open a claim, and keeps working", async function () {
    const ctx = await deploy();
    const { vault, operator } = ctx;
    const r = await ready(ctx, ETH, async (p) => {
      await p.write(ethers.concat([rec.bond(ETHEREUM, ETH / GWEI), rec.bond(SOLANA, ETH / GWEI)]));
      await p.write(rec.exit());
    });
    await submit(ctx, r.pair.pending[0], r.real);
    const c = await vault.getChain(operator.address);
    expect(c.stated).to.equal(ETH);
    expect(c.bond).to.equal(ETH);
    expect(await vault.freeBond(operator.address)).to.equal(0n);
    await submit(ctx, r.pair.pending[1], r.real);
    await mineAt((await latestTime()) + WEEK);
    await vault.decide(1n);
    await vault.connect(operator).withdrawBond(ETH);
    await vault.connect(operator).collect(1n);
    expect(await owed(ctx, [operator.address], [1n])).to.be.lte(await ethers.provider.getBalance(await vault.getAddress()));
  });

  it("decides a claim after many rounds at the same cost, and each winner collects", async function () {
    const ctx = await deploy();
    const { vault, operator, guardian, user } = ctx;
    const r = await ready(ctx, ETH, async (p) => {
      await p.write(rec.bond(SOLANA, ETH / GWEI));
    });
    await submit(ctx, r.pair.pending[0], r.real);
    for (let i = 0; i < 10; i++) {
      await vault.connect(guardian).object(1n, { value: DEPOSIT });
      await vault.connect(user).answer(1n, { value: DEPOSIT });
    }
    await mineAt((await latestTime()) + WEEK);
    await vault.decide(1n);
    // 11 answers share 10 objections.
    const payout = DEPOSIT + (10n * DEPOSIT) / 11n;
    expect((await vault.getClaim(1n)).payout).to.equal(payout);
    await vault.connect(user).collect(1n);
    await vault.connect(operator).collect(1n);
    expect(await vault.credit(user.address)).to.equal(10n * payout);
    expect(await vault.credit(operator.address)).to.equal(payout);
    const all = [operator.address, guardian.address, user.address];
    expect(await owed(ctx, all, [1n])).to.be.lte(await ethers.provider.getBalance(await vault.getAddress()));
  });

  it("slashes a CANCEL of a lock that does not exist here, and a REQUEST to address zero", async function () {
    const ctx = await deploy();
    const r = await ready(ctx, ETH, async (p) => {
      await p.write(rec.cancel(500n));
    });
    await submit(ctx, r.pair.pending[0], r.real);
    expect((await ctx.vault.getChain(ctx.operator.address)).slashed).to.equal(true);

    const ctx2 = await deploy();
    const r2 = await ready(ctx2, ETH, async (p) => {
      await p.write(rec.request(1n, 1n, ethers.ZeroAddress, 0n));
    });
    await submit(ctx2, r2.pair.pending[0], r2.real);
    expect((await ctx2.vault.getChain(ctx2.operator.address)).slashed).to.equal(true);
  });

  it("slashes a message too large to be judged on every network (section 11.3)", async function () {
    // 205 BOND records of 10 bytes: a batch of 2,050 bytes, above 2,048.
    const ctx = await deploy();
    const r = await ready(ctx, ETH, async (p) => {
      await p.write(ethers.concat(Array.from({ length: 205 }, () => rec.bond(SOLANA, 1n))));
    });
    await submit(ctx, r.pair.pending[0], r.real);
    expect((await ctx.vault.getChain(ctx.operator.address)).slashed).to.equal(true);

    // 33 CANCEL records: more than 32 records about Solana.
    const ctx2 = await deploy();
    await ctx2.vault.connect(ctx2.user).lock(RECIPIENT, 0, { value: ETH });
    const r2 = await ready(ctx2, ETH, async (p) => {
      await p.write(ethers.concat(Array.from({ length: 33 }, () => rec.cancel(1n))));
    });
    await submit(ctx2, r2.pair.pending[0], r2.real);
    expect((await ctx2.vault.getChain(ctx2.operator.address)).slashed).to.equal(true);
  });

  it("acts on no second BOND for Solana, and leaves judging it to Solana", async function () {
    const ctx = await deploy();
    const { vault, operator } = ctx;
    const r = await ready(ctx, ETH, async (p) => {
      await p.write(rec.bond(SOLANA, 1n));
      await p.write(rec.bond(SOLANA, 2n));
    });
    await submit(ctx, r.pair.pending[0], r.real);
    const count = await vault.claimCount();
    await submit(ctx, r.pair.pending[1], r.real);
    expect(await vault.claimCount()).to.equal(count);
    expect((await vault.getChain(operator.address)).slashed).to.equal(false);
  });

  it("lets a BOND for Solana be carried again when its claim could not open", async function () {
    const ctx = await deploy();
    const { vault, operator, stranger } = ctx;
    const r = await ready(ctx, ETH, async (p) => {
      // The REQUEST passes the cover while no Solana bond counts: no claim.
      await p.write(ethers.concat([rec.bond(SOLANA, (5n * ETH) / GWEI), rec.request(7n, 1n, stranger.address, 0n)]));
      await p.write(rec.bond(SOLANA, (5n * ETH) / GWEI));
    });
    await submit(ctx, r.pair.pending[0], r.real);
    expect(await vault.claimCount()).to.equal(0n);
    await submit(ctx, r.pair.pending[1], r.real);
    expect(await vault.claimCount()).to.equal(1n);
    await mineAt((await latestTime()) + WEEK);
    await vault.decide(1n);
    const c = await vault.getChain(operator.address);
    expect(c.slashed).to.equal(false);
    expect(c.peerBond).to.equal(5n * ETH);
  });

  it("takes the same BOND for Ethereum again as true, and another amount as false", async function () {
    const ctx = await deploy();
    const { vault, operator } = ctx;
    const r = await ready(ctx, 2n * ETH, async (p) => {
      await p.write(rec.bond(ETHEREUM, ETH / GWEI));
      await p.write(rec.bond(ETHEREUM, ETH / GWEI));
      await p.write(rec.bond(ETHEREUM, 2n * ETH / GWEI));
    });
    await submit(ctx, r.pair.pending[0], r.real);
    await submit(ctx, r.pair.pending[1], r.real);
    expect((await vault.getChain(operator.address)).slashed).to.equal(false);
    expect((await vault.getChain(operator.address)).stated).to.equal(ETH);
    await submit(ctx, r.pair.pending[2], r.real);
    expect((await vault.getChain(operator.address)).slashed).to.equal(true);
  });

  it("slashes two BOND records for Ethereum in one batch", async function () {
    const ctx = await deploy();
    const r = await ready(ctx, 2n * ETH, async (p) => {
      await p.write(ethers.concat([rec.bond(ETHEREUM, ETH / GWEI), rec.bond(ETHEREUM, 1n)]));
    });
    await submit(ctx, r.pair.pending[0], r.real);
    expect((await ctx.vault.getChain(ctx.operator.address)).slashed).to.equal(true);
  });

  it("collects once, only for the winning side, and pays a request number once across claims", async function () {
    const ctx = await deploy();
    const { vault, operator, guardian, user, stranger } = ctx;
    await vault.connect(user).lock(RECIPIENT, 0, { value: 2n * ETH });
    const r = await ready(ctx, ETH, async (p) => {
      await p.write(rec.bond(SOLANA, (5n * ETH) / GWEI));
      await p.write(rec.request(7n, ETH / GWEI, stranger.address, 0n));
      await p.write(rec.request(7n, ETH / GWEI, user.address, 0n));
    });
    await submit(ctx, r.pair.pending[0], r.real);
    await mineAt((await latestTime()) + WEEK);
    await vault.decide(1n);
    await submit(ctx, r.pair.pending[1], r.real); // claim 2
    await submit(ctx, r.pair.pending[2], r.real); // claim 3
    await vault.connect(guardian).object(2n, { value: DEPOSIT });
    await vault.connect(user).answer(2n, { value: DEPOSIT });
    await mineAt((await latestTime()) + WEEK);
    await vault.decide(2n);
    await vault.decide(3n);
    await expect(vault.connect(guardian).collect(2n)).to.be.revertedWithCustomError(vault, "NothingToCollect");
    await vault.connect(user).collect(2n);
    await expect(vault.connect(user).collect(2n)).to.be.revertedWithCustomError(vault, "NothingToCollect");
    await vault.payRequest(2n, 7n);
    await expect(vault.payRequest(3n, 7n)).to.be.revertedWithCustomError(vault, "AlreadyDone");
    void operator;
  });

  it("sends the pot of a claim refused with no objection to the backing", async function () {
    const ctx = await deploy();
    const { vault } = ctx;
    const r = await ready(ctx, ETH, async (p) => {
      await p.write(rec.bond(SOLANA, 1n));
      await p.write(rec.lock(3n, 1n, RECIPIENT, 0n)); // false: slashes the chain here
    });
    await submit(ctx, r.pair.pending[0], r.real);
    await submit(ctx, r.pair.pending[1], r.real);
    const before = await vault.reserve();
    await mineAt((await latestTime()) + WEEK);
    await vault.decide(1n);
    expect((await vault.getClaim(1n)).accepted).to.equal(false);
    expect(await vault.reserve()).to.equal(before + DEPOSIT);
  });

  it("pays a request carried by an honest chain while a false carrier's claim is open", async function () {
    const ctx = await deploy();
    const { vault, chain, operator, stranger, guardian, user } = ctx;
    await vault.connect(user).lock(RECIPIENT, 0, { value: 2n * ETH });
    // Two operators: the honest one and Mallory (the stranger).
    const honest = await writeRegistration(ctx, operator);
    const mallory = await writeRegistration(ctx, stranger);
    await honest.write(rec.bond(SOLANA, (5n * ETH) / GWEI));
    await mallory.write(rec.bond(SOLANA, (5n * ETH) / GWEI));
    await mallory.write(rec.request(7n, ETH / GWEI, stranger.address, 0n));
    await honest.write(rec.request(7n, ETH / GWEI, user.address, 0n));
    const real = await checkpoint(ctx);
    for (const [who, pair] of [[operator, honest], [stranger, mallory]] as const) {
      await register(ctx, pair, real, who);
      await vault.connect(who).addBond({ value: ETH });
      await vault.connect(who).addDeposits({ value: 10n * DEPOSIT });
      await submit(ctx, pair.pending[0], real, guardian, who);
    }
    await mineAt((await latestTime()) + WEEK);
    await vault.decide(1n);
    await vault.decide(2n);
    await submit(ctx, mallory.pending[1], real, guardian, stranger); // claim 3
    await submit(ctx, honest.pending[1], real, guardian, operator); // claim 4
    await vault.connect(guardian).object(3n, { value: DEPOSIT });
    await mineAt((await latestTime()) + WEEK);
    await vault.decide(3n);
    await vault.decide(4n);
    await expect(vault.payRequest(3n, 7n)).to.be.revertedWithCustomError(vault, "NotAccepted");
    await vault.payRequest(4n, 7n);
    expect(await vault.credit(user.address)).to.equal(ETH);
    void chain;
  });

  it("refuses every other claim of a chain that lost one", async function () {
    const ctx = await deploy();
    const { vault, stranger, guardian } = ctx;
    await vault.connect(ctx.user).lock(RECIPIENT, 0, { value: 2n * ETH });
    const r = await ready(ctx, ETH, async (p) => {
      await p.write(rec.bond(SOLANA, (5n * ETH) / GWEI));
      await p.write(rec.request(7n, 1n, stranger.address, 0n));
      await p.write(rec.request(8n, 1n, stranger.address, 0n));
    });
    await submit(ctx, r.pair.pending[0], r.real);
    await mineAt((await latestTime()) + WEEK);
    await vault.decide(1n);
    await submit(ctx, r.pair.pending[1], r.real); // claim 2
    await submit(ctx, r.pair.pending[2], r.real); // claim 3, opened before claim 2 is refused
    await vault.connect(guardian).object(2n, { value: DEPOSIT });
    await mineAt((await latestTime()) + WEEK);
    await vault.decide(2n);
    await vault.decide(3n);
    expect((await vault.getClaim(3n)).accepted).to.equal(false);
  });

  it("pays no fee to a slashed operator; the fee waits for another", async function () {
    const ctx = await deploy();
    const { vault, user, operator } = ctx;
    await vault.connect(user).lock(RECIPIENT, 4n * GWEI, { value: ETH + 4n * GWEI });
    const r = await ready(ctx, ETH, async (p) => {
      await p.write(rec.lock(9n, 1n, RECIPIENT, 0n)); // false
      await p.write(rec.lock(1n, ETH / GWEI, RECIPIENT, 4n)); // true
    });
    await submit(ctx, r.pair.pending[0], r.real);
    await expect(submit(ctx, r.pair.pending[1], r.real)).to.not.emit(vault, "FeeEarned");
    expect((await vault.getLock(1n)).feePaid).to.equal(false);
    expect(await vault.credit(operator.address)).to.equal(0n);
  });

  it("refuses objections after the window and credits can be withdrawn", async function () {
    const ctx = await deploy();
    const { vault, guardian, operator } = ctx;
    const r = await ready(ctx, ETH, async (p) => {
      await p.write(rec.bond(SOLANA, 1n));
    });
    await submit(ctx, r.pair.pending[0], r.real);
    await mineAt((await latestTime()) + WEEK);
    await expect(vault.connect(guardian).object(1n, { value: DEPOSIT })).to.be.revertedWithCustomError(vault, "WindowOver");
    await vault.decide(1n);
    await expect(vault.connect(guardian).object(1n, { value: DEPOSIT })).to.be.revertedWithCustomError(vault, "NotOpen");
    await vault.connect(operator).collect(1n);
    await expect(vault.connect(operator).withdrawCredit()).to.changeEtherBalance(ethers, operator, DEPOSIT);
  });

  it("certifies no job whose escrow is below the minimum (D118)", async function () {
    const ctx = await deploy();
    const { vault, protocol, chain, operator, stranger } = ctx;
    // Mallory's own application opens a cheap job, and her operator proves it.
    await protocol.connect(stranger).registerApplication([]);
    // Far below the minimum, and above the protocol's lowest at any base fee here.
    const escrow = ETH / 100n;
    await protocol.connect(stranger).openJob(ethers.id("cheap"), escrow, 50, 6, 0, stranger.address, { value: FEES, gasLimit: GAS });
    const jobId = await protocol.jobCount();
    await protocol.connect(operator).bid(jobId, await protocol.minimumBidOf(jobId));
    await mineAt((await latestTime()) + 61);
    const anchor = await chain.add();
    await protocol.connect(operator).anchorJob(jobId, anchor);
    const head = await protocol.chainHeadOf(operator.address);
    const tagged = tx([{ txidLE: head.txid, vout: Number(head.vout) }], [
      { value: 546n, script: COIN_SCRIPT },
      { value: 0n, script: opReturn(tagPayload(ethers.id("cheap"))) },
    ]);
    const proofBlock = await chain.add([tagged]);
    for (let i = 0; i < 5; i++) await chain.add();
    const { siblings, txIndex } = chain.proofOf(proofBlock, tagged);
    await protocol.connect(operator).proveJob(jobId, { proofBlock, tip: chain.tip, prevEpochTime: 0, rawTx: tagged.raw, siblings, txIndex, headIndex: 0, tagIndex: 1 });
    await mineAt(Number((await protocol.getDuty(jobId)).lockEnd) + 1);
    await expect(vault.recordRealFromJob(jobId)).to.be.revertedWithCustomError(vault, "EscrowTooLow");
    // A checkpoint through the vault asks the minimum.
    await vault.connect(stranger).openCheckpoint(6, { value: FEES, gasLimit: GAS });
    expect((await protocol.getJob(await protocol.jobCount())).escrow).to.equal(MIN_CERTIFYING_ESCROW);
  });

  it("certifies no expired or slashed job", async function () {
    const ctx = await deploy();
    const { vault, protocol, operator, stranger, guardian } = ctx;
    await vault.connect(stranger).openCheckpoint(6, { value: FEES, gasLimit: GAS });
    await mineAt((await latestTime()) + HOUR);
    await expect(vault.recordRealFromJob(1n)).to.be.revertedWithCustomError(vault, "NotCertified");

    await vault.connect(stranger).openCheckpoint(6, { value: FEES, gasLimit: GAS });
    await protocol.connect(operator).bid(2n, await protocol.minimumBidOf(2n));
    await mineAt((await latestTime()) + 61);
    await mineAt(Number(await protocol.deadlineOf(2n)) + 1);
    const salt = ethers.id("salt");
    await protocol.connect(guardian).sealNote(await protocol.noteFor(guardian.address, 2n, ZERO_HASH, salt));
    await protocol.connect(guardian).reportMissedDuty(2n, salt);
    await expect(vault.recordRealFromJob(2n)).to.be.revertedWithCustomError(vault, "NotCertified");
    // The application's share of the slash is the vault's to collect.
    const before = await vault.reserve();
    await vault.collectProtocolCredit();
    expect(await vault.reserve()).to.be.gt(before);
  });
});
