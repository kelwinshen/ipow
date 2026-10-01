import { expect } from "chai";
import { network } from "hardhat";

import { COIN_SCRIPT, buildTx, concat, headerHashLE, merkle, mine, targetFromBits, txidLE } from "./helpers/bitcoin.ts";

const { ethers } = await network.create();

// The protocol's vault on Ethereum: its core, `home` for Ethereum's assets
// and `receipts` for Solana's. Spec: docs/design/ipow-protocol.md, section
// 11. The Bitcoin blocks here are mined by the test at a low difficulty,
// which only the test light client allows.

const MINUTE = 60;
const HOUR = 3600;
const DAY = 24 * HOUR;
const WEEK = 7 * DAY;
const ETH = 10n ** 18n;
const GWEI = 10n ** 9n;
/** One ETH in record units (gwei). */
const E = ETH / GWEI;
const EASY = 0x207fffff;
const ZERO_HASH = "0x" + "00".repeat(32);
const FEES = ETH / 10n;
const DEPOSIT = ETH / 100n;
const PEER_VAULT = ethers.id("solana vault");
const PEER_OPERATOR = ethers.id("carl on solana");
const RECIPIENT = ethers.id("alice on solana");
const SOL_TOKEN = ethers.id("native SOL");
const GAS = 3_000_000n;
const MIN_CERTIFYING_ESCROW = ETH;
const ETHEREUM = 1;
const SOLANA = 2;

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

// Records, as section 11.9 writes them. Amounts in record units, big-endian.
const u8 = (n: number) => ethers.toBeHex(n, 1);
const u32 = (n: number | bigint) => ethers.toBeHex(n, 4);
const u64 = (n: number | bigint) => ethers.toBeHex(n, 8);
/** An Ethereum address in 32 bytes. */
const a32 = (a: string) => ethers.zeroPadValue(a, 32);
const rec = {
  lock: (home: number, asset: number | bigint, id: bigint, amount: bigint, recipient: string, fee = 0n, fast = 0n, at = 0n) =>
    ethers.concat(["0x01", u8(home), u32(asset), u64(id), u64(amount), recipient, u64(fee), u64(fast), u64(at)]),
  request: (net: number, asset: number | bigint, id: bigint, amount: bigint, to: string, fee = 0n, fast = 0n, at = 0n) =>
    ethers.concat(["0x02", u8(net), u32(asset), u64(id), u64(amount), to, u64(fee), u64(fast), u64(at)]),
  cancel: (net: number, id: bigint) => ethers.concat(["0x03", u8(net), u64(id)]),
  bond: (net: number, home: number, asset: number | bigint, amount: bigint) =>
    ethers.concat(["0x04", u8(net), u8(home), u32(asset), u64(amount)]),
  asset: (home: number, asset: number | bigint, token: string, decimals: number) =>
    ethers.concat(["0x06", u8(home), u32(asset), token, u8(decimals)]),
  exit: () => "0x05",
};
/** An asset, by its home network and number. */
const key = (home: number, asset: number | bigint) => (BigInt(home) << 32n) | BigInt(asset);
const ETH_KEY = key(ETHEREUM, 0);
const SOL_KEY = key(SOLANA, 0);

/** The factories that make a vault's parts, deployed once. */
let factories: [string, string] | undefined;

/** A vault for the pair (`here`, `peer`). */
async function deployVault(protocol: string, here: number, peer: number) {
  if (!factories) {
    const h = await ethers.deployContract("VaultHomeFactory");
    const r = await ethers.deployContract("VaultReceiptsFactory");
    factories = [await h.getAddress(), await r.getAddress()];
  }
  return ethers.deployContract("iPoWVaultNative", [protocol, here, peer, PEER_VAULT, DEPOSIT, MIN_CERTIFYING_ESCROW, ...factories]);
}

async function deploy() {
  const lightClient = await ethers.deployContract("iPoWLightClientHarness", [0]);
  await lightClient.setLimits(targetFromBits(EASY), targetFromBits(EASY));
  const protocol = await ethers.deployContract("iPoWProtocolHarness", [await lightClient.getAddress()]);
  const vault = await deployVault(await protocol.getAddress(), ETHEREUM, SOLANA);
  const home = await ethers.getContractAt("VaultHome", await vault.home());
  const receipts = await ethers.getContractAt("VaultReceipts", await vault.receipts());
  const [, user, operator, guardian, stranger] = await ethers.getSigners();

  const now = Math.ceil(((await latestTime()) + DAY) / DAY) * DAY;
  await mineAt(now);
  const chain = new TestChain(lightClient);
  await chain.start(now);

  // The operator's protocol bond and chain head, for checkpoint jobs.
  await protocol.connect(operator).lockBond(3n * ETH, { value: 3n * ETH });
  const first = tx([{ txidLE: ethers.id("funding"), vout: 0 }], [
    { value: 546n, script: COIN_SCRIPT },
    { value: 0n, script: opReturn(await protocol.chainHeadCommitment(operator.address)) },
  ]);
  const block = await chain.add([first]);
  const { siblings, txIndex } = chain.proofOf(block, first);
  await protocol.connect(operator).registerChainHead(block, first.raw, siblings, txIndex, 0, 1);

  return { lightClient, protocol, vault, home, receipts, chain, user, operator, guardian, stranger };
}

type Ctx = Awaited<ReturnType<typeof deploy>>;

/**
 * D108, D116: a checkpoint job opened through the vault, proven by the
 * operator on top of the chain, its lock ended. Its proof block is real.
 */
async function checkpoint(ctx: Ctx): Promise<Ref> {
  const { protocol, vault, chain, operator, stranger } = ctx;
  await vault.connect(stranger).openCheckpoint(6, FEES, { value: FEES, gasLimit: GAS });
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
  return ctx.vault.connect(from).submitMessage(operator.address, btcOf(ctx, m, real), 0, 1, m.batch, { gasLimit: 10_000_000n });
}

/** Locks `amount` ETH (record units) for RECIPIENT on Solana. */
async function lockEth(ctx: Ctx, amount: bigint, fee = 0n, fast = 0n, who = ctx.user) {
  await ctx.home.connect(who).lock(0, RECIPIENT, amount, fee, fast, { value: (amount + fee + fast) * GWEI });
  return await ctx.home.lockCount();
}

/** The LOCK record that states lock `id` of `home`. */
async function lockRecord(ctx: Ctx, id: bigint) {
  const l = await ctx.home.getLock(id);
  return rec.lock(ETHEREUM, l.asset, id, l.amount, l.recipient, l.fee, l.fastFee, l.lockedAt);
}

/** A registered operator with `bond` gwei of ETH bonded; the messages written in `write` before the checkpoint. */
async function ready(ctx: Ctx, bond: bigint, write: (pair: PairChain) => Promise<void>, who = ctx.operator) {
  const pair = await writeRegistration(ctx, who);
  await write(pair);
  const real = await checkpoint(ctx);
  await register(ctx, pair, real, who);
  await ctx.vault.connect(who).addBond(ETH_KEY, bond, { value: bond * GWEI });
  await ctx.vault.connect(who).addDeposits(10n * DEPOSIT, { value: 10n * DEPOSIT });
  return { pair, real };
}

/** Lets 7 days pass and decides claim `id`. */
async function accept(ctx: Ctx, id: bigint) {
  await mineAt((await latestTime()) + WEEK);
  await ctx.vault.decide(id);
}

/**
 * The operator's first message, accepted: vSOL's ASSET record and its bonds
 * on Solana, 100 SOL and 100 vETH, counted here. Then `more` messages.
 */
async function withSolana(ctx: Ctx, bond: bigint, more: (p: PairChain) => Promise<void>) {
  const first = ethers.concat([rec.asset(SOLANA, 0, SOL_TOKEN, 9), rec.bond(SOLANA, SOLANA, 0, 100n * E), rec.bond(SOLANA, ETHEREUM, 0, 100n * E)]);
  const r = await ready(ctx, bond, async (p) => {
    await p.write(first);
    await more(p);
  });
  await submit(ctx, r.pair.pending[0], r.real);
  await accept(ctx, 1n);
  await ctx.receipts.makeReceipt(1n, rec.asset(SOLANA, 0, SOL_TOKEN, 9));
  const vSol = await ethers.getContractAt("VaultReceipt", await ctx.receipts.receiptOf(0));
  return { ...r, vSol };
}

describe("iPoWVault: assets and locks (section 11.9)", function () {
  it("locks ETH in record units, and keeps the amount and fast fee as backing", async function () {
    const ctx = await deploy();
    const { home, user } = ctx;
    await home.connect(user).lock(0, RECIPIENT, E, 10n, 3n, { value: (E + 13n) * GWEI });
    const l = await home.getLock(1n);
    expect(l.amount).to.equal(E);
    expect(l.fee).to.equal(10n);
    expect(l.fastFee).to.equal(3n);
    expect(l.lockedAt).to.equal(BigInt(await latestTime()));
    expect(await home.reserve(0)).to.equal(E + 3n);
    await expect(home.connect(user).lock(0, RECIPIENT, E, 0, 0, { value: ETH + 1n })).to.be.revertedWithCustomError(home, "WrongValue");
    await expect(home.connect(user).lock(0, RECIPIENT, 0, 0, 0, { value: 0 })).to.be.revertedWithCustomError(home, "ZeroAmount");
    await expect(home.connect(user).lock(0, ZERO_HASH, E, 0, 0, { value: ETH })).to.be.revertedWithCustomError(home, "ZeroAddress");
    await expect(home.connect(user).lock(1, RECIPIENT, E, 0, 0, { value: ETH })).to.be.revertedWithCustomError(home, "UnknownAsset");
  });

  it("registers any token once, counting at most 9 decimals, and locks what arrived", async function () {
    const ctx = await deploy();
    const { home, user, stranger } = ctx;
    const big = await ethers.deployContract("MockToken", ["Big", "BIG", 18]);
    const usd = await ethers.deployContract("MockToken", ["Dollar", "USD", 6]);
    const fee = await ethers.deployContract("FeeOnTransferToken");
    for (const t of [big, usd, fee]) await home.connect(stranger).registerAsset(await t.getAddress());
    await expect(home.registerAsset(await usd.getAddress())).to.be.revertedWithCustomError(home, "AssetExists");
    expect((await home.getAsset(1)).unit).to.equal(GWEI);
    expect((await home.getAsset(2)).unit).to.equal(1n);
    expect((await home.getAsset(2)).recordDecimals).to.equal(6n);

    // 2 BIG locks 2 * 10^9 record units.
    await big.mint(user.address, 2n * ETH);
    await big.connect(user).approve(await home.getAddress(), 2n * ETH);
    await home.connect(user).lock(1, RECIPIENT, 2n * E, 0, 0);
    expect((await home.getLock(1n)).amount).to.equal(2n * E);
    expect(await home.reserve(1)).to.equal(2n * E);

    // A token that takes 1% on transfer locks what arrived: 990 of 1,000.
    await fee.mint(user.address, 1000n);
    await fee.connect(user).approve(await home.getAddress(), 1000n);
    await home.connect(user).lock(3, RECIPIENT, 990n, 5n, 5n);
    expect((await home.getLock(2n)).amount).to.equal(980n);
    expect(await home.reserve(3)).to.equal(985n);

    // Its ASSET record states its token and the receipt's decimals.
    const expected = ethers.keccak256(rec.asset(ETHEREUM, 2, a32(await usd.getAddress()), 6));
    expect(await home.assetRecordHash(2)).to.equal(expected);
    expect(await home.assetRecordHash(9)).to.equal(ZERO_HASH);
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

  it("names both networks, their vaults and operators in a registration, the lower number first (section 11.3, D133)", async function () {
    const { vault, protocol, operator } = await deploy();
    const pad = (a: string) => ethers.zeroPadValue(a, 32);
    const side = (net: number, v: string, op: string) => ethers.concat([ethers.toBeHex(net, 1), v, op]);
    const ours = side(ETHEREUM, pad(await vault.getAddress()), pad(operator.address));
    const theirs = side(SOLANA, PEER_VAULT, PEER_OPERATOR);
    expect(await vault.pairCommitment(operator.address, PEER_OPERATOR)).to.equal(ethers.sha256(ethers.concat([ethers.toUtf8Bytes("iPoW pair"), ours, theirs])));
    expect(await vault.messagePayload("0x05")).to.equal(ethers.sha256(ethers.concat([ethers.toUtf8Bytes("iPoW vault"), "0x05"])));
    expect([await vault.here(), await vault.peer()]).to.deep.equal([BigInt(ETHEREUM), BigInt(SOLANA)]);

    // A vault on Base (3) paired with Ethereum (1): Ethereum's side first.
    const base = await deployVault(await protocol.getAddress(), 3, ETHEREUM);
    const baseSide = side(3, pad(await base.getAddress()), pad(operator.address));
    const ethSide = side(ETHEREUM, PEER_VAULT, PEER_OPERATOR);
    expect(await base.pairCommitment(operator.address, PEER_OPERATOR)).to.equal(ethers.sha256(ethers.concat([ethers.toUtf8Bytes("iPoW pair"), ethSide, baseSide])));
    // Its parts know the pair too.
    const home = await ethers.getContractAt("VaultHome", await base.home());
    expect([await home.here(), await home.peer()]).to.deep.equal([3n, BigInt(ETHEREUM)]);
    // Its peer is an EVM network: a lock's recipient is an address there,
    // not a 32-byte key, whose receipt could never be issued.
    const { user } = await deploy();
    await expect(home.connect(user).lock(0, RECIPIENT, E, 0, 0, { value: ETH })).to.be.revertedWithCustomError(home, "ZeroAddress");
    await home.connect(user).lock(0, pad(user.address), E, 0, 0, { value: ETH });

    // A pair needs two different networks.
    const factory = await ethers.getContractFactory("iPoWVaultNative");
    for (const [a, b] of [[1, 1], [0, 2], [1, 0]]) {
      await expect(deployVault(await protocol.getAddress(), a, b)).to.be.revertedWithCustomError(factory, "BadNetworks");
    }
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

describe("iPoWVault: judging records about Ethereum (D109)", function () {
  it("pays the lock's fee for a true LOCK, once", async function () {
    const ctx = await deploy();
    const { home, operator } = ctx;
    await lockEth(ctx, E, 5n);
    const record = await lockRecord(ctx, 1n);
    const { pair, real } = await ready(ctx, E, async (p) => {
      await p.write(record);
      await p.write(record);
    });
    await expect(submit(ctx, pair.pending[0], real)).to.emit(home, "FeeEarned").withArgs(1n, operator.address, 5n);
    await expect(submit(ctx, pair.pending[1], real)).to.not.emit(home, "FeeEarned");
    expect(await home.credit(operator.address, 0)).to.equal(5n * GWEI);
    expect((await ctx.vault.getChain(operator.address)).messages).to.equal(2n);
  });

  it("judges a LOCK by every field: amount, recipient, fee, fast fee and time", async function () {
    const ctx = await deploy();
    await lockEth(ctx, E, 1n, 4n);
    const l = await ctx.home.getLock(1n);
    const wrong = [
      rec.lock(ETHEREUM, 0, 1n, E + 1n, RECIPIENT, 1n, 4n, l.lockedAt),
      rec.lock(ETHEREUM, 0, 1n, E, PEER_OPERATOR, 1n, 4n, l.lockedAt),
      rec.lock(ETHEREUM, 0, 1n, E, RECIPIENT, 2n, 4n, l.lockedAt),
      rec.lock(ETHEREUM, 0, 1n, E, RECIPIENT, 1n, 5n, l.lockedAt),
      rec.lock(ETHEREUM, 0, 1n, E, RECIPIENT, 1n, 4n, l.lockedAt + 1n),
      rec.lock(ETHEREUM, 1, 1n, E, RECIPIENT, 1n, 4n, l.lockedAt),
    ];
    for (const record of wrong) {
      const c = await deploy();
      await lockEth(c, E, 1n, 4n);
      const at = (await c.home.getLock(1n)).lockedAt;
      // The same lock in a fresh vault, at its own time.
      const r = await ready(c, E, async (p) => {
        await p.write(ethers.concat([record.slice(0, 2 + 70 * 2), u64(at - l.lockedAt + BigInt("0x" + record.slice(2 + 70 * 2)))]));
      });
      await submit(c, r.pair.pending[0], r.real);
      expect((await c.vault.getChain(c.operator.address)).slashed).to.equal(true);
    }
  });

  it("slashes every bond for a false record: 80% backs each asset, 20% to the submitter", async function () {
    const ctx = await deploy();
    const { vault, home, user, operator, guardian } = ctx;
    const usd = await ethers.deployContract("MockToken", ["Dollar", "USD", 6]);
    await home.registerAsset(await usd.getAddress());
    await lockEth(ctx, E);
    const { pair, real } = await ready(ctx, 5n * E, async (p) => {
      await p.write(rec.lock(ETHEREUM, 0, 1n, 2n * E, RECIPIENT)); // wrong amount
    });
    await usd.mint(operator.address, 1000n);
    await usd.connect(operator).approve(await vault.getAddress(), 1000n);
    await vault.connect(operator).addBond(key(ETHEREUM, 1), 1000n);
    expect(await vault.assetsOf(operator.address)).to.deep.equal([ETH_KEY, key(ETHEREUM, 1)]);
    await expect(submit(ctx, pair.pending[0], real, guardian)).to.emit(vault, "Slashed").withArgs(operator.address, guardian.address);
    // The backing is settled per asset, by anyone.
    expect(await vault.slashBacking(operator.address, ETH_KEY)).to.equal(4n * E);
    await vault.connect(user).settleSlash(operator.address, ETH_KEY);
    await vault.settleSlash(operator.address, key(ETHEREUM, 1));
    await expect(vault.settleSlash(operator.address, ETH_KEY)).to.be.revertedWithCustomError(vault, "NothingToCollect");
    expect(await home.reserve(0)).to.equal(E + 4n * E);
    expect(await home.reserve(1)).to.equal(800n);
    expect(await usd.balanceOf(await home.getAddress())).to.equal(800n);
    expect(await vault.credit(guardian.address, ETH_KEY)).to.equal(ETH);
    expect(await vault.credit(guardian.address, key(ETHEREUM, 1))).to.equal(200n);
    const c = await vault.getChain(operator.address);
    expect(c.slashed).to.equal(true);
    expect((await vault.getPosition(operator.address, ETH_KEY)).bond).to.equal(0n);
    // The submitter withdraws its share in each asset.
    await vault.connect(guardian).withdrawCredit(key(ETHEREUM, 1));
    expect(await usd.balanceOf(guardian.address)).to.equal(200n);
    await expect(vault.connect(operator).addBond(ETH_KEY, 1n, { value: GWEI })).to.be.revertedWithCustomError(vault, "ChainEnded");
    void user;
  });

  it("gives the whole slash to the backing when the operator submits its own lie", async function () {
    const ctx = await deploy();
    const { vault, home, operator } = ctx;
    const { pair, real } = await ready(ctx, 5n * E, async (p) => {
      await p.write(rec.lock(ETHEREUM, 0, 7n, 1n, RECIPIENT)); // no lock #7
    });
    await submit(ctx, pair.pending[0], real, operator);
    await vault.settleSlash(operator.address, ETH_KEY);
    expect(await home.reserve(0)).to.equal(5n * E);
    expect(await vault.credit(operator.address, ETH_KEY)).to.equal(0n);
  });

  // Network 3 (Base) exists, but is not in this vault's pair: false too.
  it("treats a batch that does not parse, or names a network outside the pair, as false", async function () {
    for (const batch of ["0x0901", "0x03" + "03" + "00".repeat(8), rec.lock(ETHEREUM, 0, 1n, 1n, RECIPIENT).slice(0, 40)]) {
      const ctx = await deploy();
      const { pair, real } = await ready(ctx, E, async (p) => {
        await p.write(batch);
      });
      await submit(ctx, pair.pending[0], real);
      expect((await ctx.vault.getChain(ctx.operator.address)).slashed).to.equal(true);
    }
  });

  it("processes a chain in order and never twice, publishing each batch", async function () {
    const ctx = await deploy();
    const { vault } = ctx;
    const { pair, real } = await ready(ctx, E, async (p) => {
      await p.write(rec.bond(ETHEREUM, ETHEREUM, 0, 1n));
      await p.write(rec.bond(ETHEREUM, ETHEREUM, 0, 1n));
    });
    await expect(submit(ctx, pair.pending[1], real)).to.be.revertedWithCustomError(vault, "WrongCoin");
    await expect(submit(ctx, pair.pending[0], real)).to.emit(vault, "MessageBatch").withArgs(ctx.operator.address, 0n, 0n, pair.pending[0].batch);
    const first = BigInt((await ethers.provider.getBlock("latest"))!.number);
    expect((await vault.getChain(ctx.operator.address)).lastMessageBlock).to.equal(first);
    await expect(submit(ctx, pair.pending[0], real)).to.be.revertedWithCustomError(vault, "WrongCoin");
    await expect(submit(ctx, pair.pending[1], real)).to.emit(vault, "MessageBatch").withArgs(ctx.operator.address, 1n, first, pair.pending[1].batch);
  });

  it("keeps a stated BOND locked, takes the same again as true, and another amount or a second in a batch as false", async function () {
    const ctx = await deploy();
    const { vault, operator } = ctx;
    const { pair, real } = await ready(ctx, 2n * E, async (p) => {
      await p.write(rec.bond(ETHEREUM, ETHEREUM, 0, E));
      await p.write(rec.bond(ETHEREUM, ETHEREUM, 0, E));
      await p.write(rec.bond(ETHEREUM, ETHEREUM, 0, 2n * E));
    });
    await submit(ctx, pair.pending[0], real);
    expect((await vault.getPosition(operator.address, ETH_KEY)).stated).to.equal(E);
    expect(await vault.freeBond(operator.address, ETH_KEY)).to.equal(E);
    await expect(vault.connect(operator).withdrawBond(ETH_KEY, E + 1n)).to.be.revertedWithCustomError(vault, "BondNotFree");
    await submit(ctx, pair.pending[1], real);
    expect((await vault.getChain(operator.address)).slashed).to.equal(false);
    await submit(ctx, pair.pending[2], real);
    expect((await vault.getChain(operator.address)).slashed).to.equal(true);

    for (const batch of [
      rec.bond(ETHEREUM, ETHEREUM, 0, 3n * E), // above the bond
      ethers.concat([rec.bond(ETHEREUM, ETHEREUM, 0, E), rec.bond(ETHEREUM, ETHEREUM, 0, 1n)]), // twice in a batch
      rec.bond(ETHEREUM, 3, 0, 1n), // unknown home
      rec.bond(ETHEREUM, ETHEREUM, 0, 0n), // zero
    ]) {
      const c = await deploy();
      const r = await ready(c, 2n * E, async (p) => {
        await p.write(batch);
      });
      await submit(c, r.pair.pending[0], r.real);
      expect((await c.vault.getChain(c.operator.address)).slashed).to.equal(true);
    }
  });

  it("judges an ASSET record of Ethereum by its token and the receipt's decimals", async function () {
    const ctx = await deploy();
    const usd = await ethers.deployContract("MockToken", ["Dollar", "USD", 6]);
    await ctx.home.registerAsset(await usd.getAddress());
    const { pair, real } = await ready(ctx, E, async (p) => {
      await p.write(rec.asset(ETHEREUM, 1, a32(await usd.getAddress()), 6));
      await p.write(rec.asset(ETHEREUM, 0, ZERO_HASH, 9));
      await p.write(rec.asset(ETHEREUM, 1, a32(await usd.getAddress()), 9));
    });
    await submit(ctx, pair.pending[0], real);
    await submit(ctx, pair.pending[1], real);
    expect((await ctx.vault.getChain(ctx.operator.address)).slashed).to.equal(false);
    await submit(ctx, pair.pending[2], real);
    expect((await ctx.vault.getChain(ctx.operator.address)).slashed).to.equal(true);
  });
});

describe("iPoWVault: claims from Solana (D110, D111, D129)", function () {
  it("pays a REQUEST for ETH after 7 days with no objection", async function () {
    const ctx = await deploy();
    const { vault, home, stranger, operator } = ctx;
    await lockEth(ctx, 2n * E);
    const request = rec.request(SOLANA, 0, 7n, E, a32(stranger.address));
    const { pair, real } = await withSolana(ctx, E, async (p) => {
      await p.write(request);
    });
    await expect(submit(ctx, pair.pending[1], real)).to.emit(vault, "ClaimBatch").withArgs(2n, pair.pending[1].batch);
    expect(await vault.claimAsset(2n, ETH_KEY)).to.deep.equal([E, 0n]);
    expect((await vault.getPosition(operator.address, ETH_KEY)).openValue).to.equal(E);
    await expect(home.payRequest(2n, request)).to.be.revertedWithCustomError(home, "NotAccepted");
    await expect(vault.decide(2n)).to.be.revertedWithCustomError(vault, "WindowNotOver");
    await accept(ctx, 2n);
    await home.payRequest(2n, request);
    expect(await home.credit(stranger.address, 0)).to.equal(ETH);
    expect(await home.reserve(0)).to.equal(E);
    await expect(home.payRequest(2n, request)).to.be.revertedWithCustomError(home, "AlreadyDone");
    await home.connect(stranger).withdrawCredit(0);
    // The operator collects its deposit back.
    await vault.connect(operator).collect(2n);
    expect(await vault.credit(operator.address, ETH_KEY)).to.equal(DEPOSIT);
    await expect(vault.connect(operator).collect(2n)).to.be.revertedWithCustomError(vault, "NothingToCollect");
  });

  it("pays a REQUEST for a token in its own units", async function () {
    const ctx = await deploy();
    const { home, user, stranger } = ctx;
    const usd = await ethers.deployContract("MockToken", ["Dollar", "USD", 6]);
    await home.registerAsset(await usd.getAddress());
    await usd.mint(user.address, 500n);
    await usd.connect(user).approve(await home.getAddress(), 500n);
    await home.connect(user).lock(1, RECIPIENT, 500n, 0, 0);
    const request = rec.request(SOLANA, 1, 1n, 300n, a32(stranger.address));
    const { pair, real } = await withSolana(ctx, E, async (p) => {
      await p.write(rec.bond(SOLANA, ETHEREUM, 1, 1000n));
      await p.write(request);
    });
    await submit(ctx, pair.pending[1], real);
    await accept(ctx, 2n);
    await submit(ctx, pair.pending[2], real);
    await accept(ctx, 3n);
    await home.payRequest(3n, request);
    await home.connect(stranger).withdrawCredit(1);
    expect(await usd.balanceOf(stranger.address)).to.equal(300n);
    expect(await home.reserve(1)).to.equal(200n);
  });

  it("opens no claim past 80% of the bond in that asset on Solana", async function () {
    const ctx = await deploy();
    const { vault, stranger } = ctx;
    const usd = await ethers.deployContract("MockToken", ["Dollar", "USD", 6]);
    await ctx.home.registerAsset(await usd.getAddress());
    const { pair, real } = await withSolana(ctx, E, async (p) => {
      // 100 vETH counted: 80 vETH may be open, and nothing in the token.
      await p.write(rec.request(SOLANA, 0, 7n, 80n * E + 1n, a32(stranger.address)));
      await p.write(rec.request(SOLANA, 1, 8n, 1n, a32(stranger.address)));
      await p.write(rec.request(SOLANA, 0, 9n, 80n * E, a32(stranger.address)));
    });
    for (const i of [1, 2]) {
      const before = await vault.claimCount();
      await submit(ctx, pair.pending[i], real);
      expect(await vault.claimCount()).to.equal(before);
    }
    await submit(ctx, pair.pending[3], real);
    expect(await vault.claimCount()).to.equal(2n);
  });

  it("repays an attester that paid a burn at once, with its share of the fast fee (D122, D124, D126)", async function () {
    const ctx = await deploy();
    const { home, stranger, guardian } = ctx;
    await lockEth(ctx, 2n * E);
    // The burn on Solana was made a few days before the payment. (Setting
    // up takes a week first.)
    const burnedAt = BigInt(await latestTime()) + BigInt(WEEK - DAY);
    const fastFee = 8_000_000n;
    const request = rec.request(SOLANA, 0, 7n, E, a32(stranger.address), 0n, fastFee, burnedAt);
    const { pair, real } = await withSolana(ctx, E, async (p) => {
      await p.write(request);
    });
    const before = await ethers.provider.getBalance(stranger.address);
    await expect(home.connect(guardian).fastPay(request, { value: ETH }))
      .to.emit(home, "FastPaid")
      .withArgs(7n, guardian.address, stranger.address, 0, E);
    const paidAt = BigInt(await latestTime());
    expect(await ethers.provider.getBalance(stranger.address)).to.equal(before + ETH);
    // The same record once only; another record may still be paid, and
    // blocks nothing.
    await expect(home.connect(guardian).fastPay(request, { value: ETH })).to.be.revertedWithCustomError(home, "AlreadyPaid");
    await home.connect(guardian).fastPay(rec.request(SOLANA, 0, 7n, 1n, a32(guardian.address)), { value: GWEI });
    await expect(home.connect(guardian).fastPay(rec.request(SOLANA, 0, 8n, 1n, a32(guardian.address)), { value: 1n })).to.be.revertedWithCustomError(home, "WrongValue");
    expect((await home.getFastPay(7n, request)).attester).to.equal(guardian.address);

    await submit(ctx, pair.pending[1], real);
    expect(await ctx.vault.claimAsset(2n, ETH_KEY)).to.deep.equal([E + fastFee, 0n]);
    await accept(ctx, 2n);
    await home.payRequest(2n, request);
    const share = (fastFee * (BigInt(8 * DAY) - (paidAt - burnedAt))) / BigInt(8 * DAY);
    expect(share > 0n && share < fastFee).to.equal(true);
    expect(await home.credit(guardian.address, 0)).to.equal((E + share) * GWEI);
    expect(await home.credit(stranger.address, 0)).to.equal((fastFee - share) * GWEI);
    await expect(home.connect(guardian).fastPay(rec.request(SOLANA, 0, 7n, 2n, a32(guardian.address)), { value: 2n * GWEI })).to.be.revertedWithCustomError(home, "AlreadyPaid");
  });

  it("pays the user when the attester stated the burn wrongly, or nobody attested", async function () {
    const ctx = await deploy();
    const { home, stranger, guardian } = ctx;
    await lockEth(ctx, 2n * E);
    const r7 = rec.request(SOLANA, 0, 7n, E, a32(stranger.address), 0n, 3n, 100n);
    const r8 = rec.request(SOLANA, 0, 8n, E / 2n, a32(stranger.address), 0n, 5n, 100n);
    const { pair, real } = await withSolana(ctx, E, async (p) => {
      await p.write(r7);
      await p.write(r8);
    });
    // Wrong fast fee: the attester's ETH is lost.
    await home.connect(guardian).fastPay(rec.request(SOLANA, 0, 7n, E, a32(stranger.address), 0n, 4n, 100n), { value: ETH });
    for (const [i, id, record, total] of [[1, 2n, r7, ETH + 3n * GWEI], [2, 3n, r8, ETH / 2n + 5n * GWEI]] as const) {
      await submit(ctx, pair.pending[i], real);
      await accept(ctx, id);
      const credit = await home.credit(stranger.address, 0);
      await home.payRequest(id, record);
      expect(await home.credit(stranger.address, 0)).to.equal(credit + total);
    }
    expect(await home.credit(guardian.address, 0)).to.equal(0n);
  });

  it("returns a lock whose CANCEL was accepted, with its unearned fee and its fast fee", async function () {
    const ctx = await deploy();
    const { home, user } = ctx;
    await lockEth(ctx, E, 3n, 4n);
    const cancel = rec.cancel(SOLANA, 1n);
    const { pair, real } = await withSolana(ctx, E, async (p) => {
      await p.write(cancel);
    });
    await submit(ctx, pair.pending[1], real);
    expect(await ctx.vault.claimAsset(2n, ETH_KEY)).to.deep.equal([E + 4n, 0n]);
    await accept(ctx, 2n);
    await home.returnLock(2n, cancel);
    expect(await home.credit(user.address, 0)).to.equal((E + 7n) * GWEI);
    expect((await home.getLock(1n)).returned).to.equal(true);
    expect(await home.reserve(0)).to.equal(0n);
    await expect(home.returnLock(2n, cancel)).to.be.revertedWithCustomError(home, "AlreadyDone");
    await expect(home.returnLock(2n, rec.cancel(SOLANA, 2n))).to.be.revertedWithCustomError(home, "NotAccepted");
  });

  it("slashes a CANCEL of a lock that does not exist here, and a REQUEST to no Ethereum address", async function () {
    for (const batch of [
      rec.cancel(SOLANA, 500n),
      rec.request(SOLANA, 0, 1n, 1n, ZERO_HASH),
      rec.request(SOLANA, 0, 1n, 1n, RECIPIENT),
      rec.request(SOLANA, 5, 1n, 1n, a32(ethers.Wallet.createRandom().address)),
    ]) {
      const ctx = await deploy();
      const r = await ready(ctx, E, async (p) => {
        await p.write(batch);
      });
      await submit(ctx, r.pair.pending[0], r.real);
      expect((await ctx.vault.getChain(ctx.operator.address)).slashed).to.equal(true);
    }
  });

  it("refuses a claim whose objection stood for 7 days, pays the objector, and refuses the chain's other claims", async function () {
    const ctx = await deploy();
    const { vault, home, stranger, guardian } = ctx;
    await lockEth(ctx, 2n * E);
    const r7 = rec.request(SOLANA, 0, 7n, E, a32(stranger.address));
    const { pair, real } = await withSolana(ctx, E, async (p) => {
      await p.write(r7);
      await p.write(rec.request(SOLANA, 0, 8n, 1n, a32(stranger.address)));
      await p.write(rec.request(SOLANA, 0, 9n, 1n, a32(stranger.address)));
    });
    await submit(ctx, pair.pending[1], real); // claim 2
    await submit(ctx, pair.pending[2], real); // claim 3, opened before 2 is refused
    await expect(vault.connect(guardian).object(2n, { value: 1n })).to.be.revertedWithCustomError(vault, "WrongValue");
    await vault.connect(guardian).object(2n, { value: DEPOSIT });
    await expect(vault.connect(guardian).object(2n, { value: DEPOSIT })).to.be.revertedWithCustomError(vault, "AlreadyHeld");
    await accept(ctx, 2n);
    expect((await vault.getClaim(2n)).accepted).to.equal(false);
    await expect(home.payRequest(2n, r7)).to.be.revertedWithCustomError(home, "NotAccepted");
    await vault.connect(guardian).collect(2n);
    expect(await vault.credit(guardian.address, ETH_KEY)).to.equal(2n * DEPOSIT);
    await vault.decide(3n);
    expect((await vault.getClaim(3n)).accepted).to.equal(false);
    // No later claim of the chain opens.
    await submit(ctx, pair.pending[3], real);
    expect(await vault.claimCount()).to.equal(3n);
    expect((await vault.getPosition(ctx.operator.address, ETH_KEY)).openValue).to.equal(0n);
  });

  it("accepts a claim whose last answer stood for 7 days, and pays the answerers", async function () {
    const ctx = await deploy();
    const { vault, stranger, guardian, user } = ctx;
    const { pair, real } = await withSolana(ctx, E, async (p) => {
      await p.write(rec.request(SOLANA, 0, 7n, 1n, a32(stranger.address)));
    });
    await submit(ctx, pair.pending[1], real);
    await mineAt((await latestTime()) + 6 * DAY);
    await vault.connect(guardian).object(2n, { value: DEPOSIT });
    await mineAt((await latestTime()) + 6 * DAY);
    await vault.connect(user).answer(2n, { value: DEPOSIT });
    // The 7 days restart from the answer.
    await mineAt((await latestTime()) + 6 * DAY);
    await expect(vault.decide(2n)).to.be.revertedWithCustomError(vault, "WindowNotOver");
    await accept(ctx, 2n);
    expect((await vault.getClaim(2n)).accepted).to.equal(true);
    await vault.connect(user).collect(2n);
    expect(await vault.credit(user.address, ETH_KEY)).to.equal(DEPOSIT + DEPOSIT / 2n);
    await expect(vault.connect(guardian).collect(2n)).to.be.revertedWithCustomError(vault, "NothingToCollect");
  });

  it("refuses the open claims of a chain proven false here", async function () {
    const ctx = await deploy();
    const { vault, stranger } = ctx;
    const { pair, real } = await withSolana(ctx, E, async (p) => {
      await p.write(rec.request(SOLANA, 0, 7n, 1n, a32(stranger.address)));
      await p.write(rec.lock(ETHEREUM, 0, 99n, 1n, RECIPIENT)); // no lock #99
    });
    await submit(ctx, pair.pending[1], real);
    await submit(ctx, pair.pending[2], real);
    await accept(ctx, 2n);
    expect((await vault.getClaim(2n)).accepted).to.equal(false);
  });
});

describe("iPoWVault: receipts of Solana's assets (section 11.9)", function () {
  it("makes a receipt from an accepted ASSET record, issues it for a LOCK, once, and never for a given-up lock", async function () {
    const ctx = await deploy();
    const { receipts, user, stranger } = ctx;
    const toUser = rec.lock(SOLANA, 0, 1n, 3n * E, a32(user.address), 0n, 2n, 100n);
    const toStranger = rec.lock(SOLANA, 0, 2n, E, a32(stranger.address));
    const { pair, real, vSol } = await withSolana(ctx, E, async (p) => {
      await p.write(ethers.concat([toUser, toStranger]));
    });
    expect(await vSol.symbol()).to.equal("vSOL");
    expect(await vSol.decimals()).to.equal(9n);
    await expect(receipts.makeReceipt(1n, rec.asset(SOLANA, 0, SOL_TOKEN, 9))).to.be.revertedWithCustomError(receipts, "AssetExists");
    await submit(ctx, pair.pending[1], real);
    expect(await ctx.vault.claimAsset(2n, SOL_KEY)).to.deep.equal([4n * E + 2n, 0n]);
    await expect(receipts.issue(2n, toUser)).to.be.revertedWithCustomError(receipts, "NotAccepted");
    // Not before the claim is accepted: it may carry a false LOCK record.
    await expect(receipts.connect(stranger).giveUp(2n, toStranger)).to.be.revertedWithCustomError(receipts, "NotAccepted");
    await accept(ctx, 2n);
    await receipts.connect(stranger).giveUp(2n, toStranger);
    await expect(receipts.connect(user).giveUp(2n, toUser.replace(a32(user.address).slice(2), a32(stranger.address).slice(2)))).to.be.revertedWithCustomError(receipts, "NotAccepted");
    await receipts.issue(2n, toUser);
    // The fast fee nobody earned goes to the recipient.
    expect(await vSol.balanceOf(user.address)).to.equal(3n * E + 2n);
    await expect(receipts.issue(2n, toUser)).to.be.revertedWithCustomError(receipts, "AlreadyDone");
    await expect(receipts.issue(2n, toStranger)).to.be.revertedWithCustomError(receipts, "AlreadyDone");
    expect(await receipts.givenUp(2n)).to.equal(true);
  });

  it("burns receipts for the asset on Solana; a true REQUEST earns its fee, a false one slashes", async function () {
    const ctx = await deploy();
    const { receipts, user, operator, guardian } = ctx;
    const toUser = rec.lock(SOLANA, 0, 1n, 3n * E, a32(user.address));
    let burnRecord = "";
    const { pair, real, vSol } = await withSolana(ctx, E, async (p) => {
      await p.write(toUser);
    });
    await submit(ctx, pair.pending[1], real);
    await accept(ctx, 2n);
    await receipts.issue(2n, toUser);
    await receipts.connect(user).burn(0, RECIPIENT, E, 5n, 2n);
    expect(await vSol.balanceOf(user.address)).to.equal(2n * E - 7n);
    // (A struct's field named `at` is hidden by the array method.)
    const at = (await receipts.getBurn(1n))[6];
    burnRecord = rec.request(ETHEREUM, 0, 1n, E, RECIPIENT, 5n, 2n, at);
    expect(await receipts.burnRecordHash(1n)).to.equal(ethers.keccak256(burnRecord));
    const m1 = await pair.write(burnRecord);
    const m2 = await pair.write(rec.request(ETHEREUM, 0, 1n, E + 1n, RECIPIENT, 5n, 2n, at));
    const real2 = await checkpoint(ctx);
    await expect(submit(ctx, m1, real2)).to.emit(receipts, "FeeEarned").withArgs(1n, operator.address, 5n);
    expect(await receipts.credit(operator.address, 0)).to.equal(5n);
    await receipts.connect(operator).withdrawCredit(0);
    expect(await vSol.balanceOf(operator.address)).to.equal(5n);
    await submit(ctx, m2, real2, guardian);
    expect((await ctx.vault.getChain(operator.address)).slashed).to.equal(true);
  });

  it("judges a CANCEL from Ethereum by a give-up here, and bonds and slashes in receipts", async function () {
    const ctx = await deploy();
    const { vault, receipts, user, operator, stranger, guardian } = ctx;
    const toStranger = rec.lock(SOLANA, 0, 2n, E, a32(stranger.address));
    const toOperator = rec.lock(SOLANA, 0, 3n, 10n * E, a32(operator.address));
    const { pair, real, vSol } = await withSolana(ctx, E, async (p) => {
      await p.write(ethers.concat([toStranger, toOperator]));
    });
    await submit(ctx, pair.pending[1], real);
    await accept(ctx, 2n);
    await receipts.issue(2n, toOperator);
    // A vSOL bond here backs the operator's records about burns of vSOL.
    await vault.connect(operator).addBond(SOL_KEY, 5n * E);
    expect(await vSol.balanceOf(await vault.getAddress())).to.equal(5n * E);
    const supply = await vSol.totalSupply();
    const m1 = await pair.write(rec.cancel(ETHEREUM, 2n)); // not given up yet: false
    const real2 = await checkpoint(ctx);
    await submit(ctx, m1, real2, guardian);
    expect((await vault.getChain(operator.address)).slashed).to.equal(true);
    // 80% of the vSOL bond burned, 20% to the submitter.
    await vault.settleSlash(operator.address, SOL_KEY);
    expect(await vSol.totalSupply()).to.equal(supply - 4n * E);
    expect(await vault.credit(guardian.address, SOL_KEY)).to.equal(E);
    await vault.connect(guardian).withdrawCredit(SOL_KEY);
    expect(await vSol.balanceOf(guardian.address)).to.equal(E);
    void user;

    // Given up, the same CANCEL is true.
    const ctx2 = await deploy();
    const r2 = await withSolana(ctx2, E, async (p) => {
      await p.write(toStranger);
    });
    await submit(ctx2, r2.pair.pending[1], r2.real);
    await accept(ctx2, 2n);
    await ctx2.receipts.connect(ctx2.stranger).giveUp(2n, toStranger);
    const m = await r2.pair.write(rec.cancel(ETHEREUM, 2n));
    await submit(ctx2, m, await checkpoint(ctx2));
    expect((await ctx2.vault.getChain(ctx2.operator.address)).slashed).to.equal(false);
  });
});

describe("iPoWVault: the fast path of locks on Solana (section 11.7)", function () {
  /** The operator holds 10 vSOL; `locks` are LOCK records of Solana, all in message 2. */
  async function withVsol(ctx: Ctx, locks: string[]) {
    const toOperator = rec.lock(SOLANA, 0, 1n, 10n * E, a32(ctx.operator.address));
    const r = await withSolana(ctx, E, async (p) => {
      await p.write(toOperator);
      await p.write(ethers.concat(locks));
    });
    await submit(ctx, r.pair.pending[1], r.real);
    await accept(ctx, 2n);
    await ctx.receipts.issue(2n, toOperator);
    await r.vSol.connect(ctx.operator).approve(await ctx.receipts.getAddress(), ethers.MaxUint256);
    return r;
  }

  it("issues a receipt at once, and repays the attester with its share of the fast fee", async function () {
    const ctx = await deploy();
    const { receipts, operator, user, guardian } = ctx;
    const at = BigInt(await latestTime()) + BigInt(3 * WEEK);
    const lock = rec.lock(SOLANA, 0, 5n, E, a32(user.address), 0n, 800n, at);
    const { pair, real, vSol } = await withVsol(ctx, [lock]);
    const supply = await vSol.totalSupply();
    // Only an operator attests.
    await expect(receipts.connect(guardian).attestLock(lock)).to.be.revertedWithCustomError(receipts, "NotOperator");
    await mineAt(Number(at) + 3 * DAY);
    await receipts.connect(operator).attestLock(lock);
    expect(await vSol.balanceOf(user.address)).to.equal(E);
    expect(await vSol.totalSupply()).to.equal(supply + E - (E * 5n) / 4n + (E * 5n) / 4n);
    expect(await vSol.balanceOf(await receipts.getAddress())).to.equal((E * 5n) / 4n);
    await submit(ctx, pair.pending[2], real);
    // Only the attester links, and only its own chain's claim.
    await expect(receipts.connect(guardian).linkFast(1n, 3n)).to.be.revertedWithCustomError(receipts, "NotAttester");
    await receipts.connect(operator).linkFast(1n, 3n);
    await mineAt((await latestTime()) + WEEK);
    await expect(receipts.burnFast(1n)).to.be.revertedWithCustomError(receipts, "NotRefused");
    await ctx.vault.decide(3n);
    // Not issued a second time, nor given up.
    await expect(receipts.issue(3n, lock)).to.be.revertedWithCustomError(receipts, "NotInOrder");
    await expect(receipts.connect(user).giveUp(3n, lock)).to.be.revertedWithCustomError(receipts, "AlreadyDone");
    await receipts.connect(guardian).settleFast(1n, 3n, lock);
    // Attested 3 days after the lock: about 5 of the 8 days left (D124).
    const left = at + BigInt(8 * DAY) - (await receipts.getAttest(1n)).attestedAt;
    const share = (800n * left) / BigInt(8 * DAY);
    expect(share).to.equal(499n);
    expect(await receipts.credit(operator.address, 0)).to.equal((E * 5n) / 4n + share);
    expect(await vSol.balanceOf(user.address)).to.equal(E + 800n - share);
    expect(await vSol.totalSupply()).to.equal(supply + E + 800n);
    await expect(receipts.settleFast(1n, 3n, lock)).to.be.revertedWithCustomError(receipts, "AlreadyDone");
  });

  it("burns an unlinked attest after 7 days; a late true record pays the attester", async function () {
    const ctx = await deploy();
    const { receipts, operator, user, guardian } = ctx;
    const lock = rec.lock(SOLANA, 0, 5n, E, a32(user.address), 0n, 0n, 0n);
    const r = await withSolana(ctx, E, async (p) => {
      await p.write(rec.lock(SOLANA, 0, 1n, 10n * E, a32(operator.address)));
    });
    await submit(ctx, r.pair.pending[1], r.real);
    await accept(ctx, 2n);
    await receipts.issue(2n, rec.lock(SOLANA, 0, 1n, 10n * E, a32(operator.address)));
    const supply = await r.vSol.totalSupply();
    await receipts.connect(operator).attestLock(lock);
    await expect(receipts.burnFast(1n)).to.be.revertedWithCustomError(receipts, "WindowNotOver");
    await mineAt((await latestTime()) + WEEK);
    await receipts.connect(guardian).burnFast(1n);
    expect(await r.vSol.totalSupply()).to.equal(supply);
    expect(await receipts.credit(guardian.address, 0)).to.equal(E / 4n);
    await expect(receipts.burnFast(1n)).to.be.revertedWithCustomError(receipts, "AlreadyDone");
    // The true record arrives late.
    const m = await r.pair.write(lock);
    await submit(ctx, m, await checkpoint(ctx));
    const id = await ctx.vault.claimCount();
    await expect(receipts.connect(operator).linkFast(1n, id)).to.be.revertedWithCustomError(receipts, "AlreadyDone");
    await accept(ctx, id);
    await receipts.settleFast(1n, id, lock);
    expect(await receipts.credit(operator.address, 0)).to.equal(E);
    expect(await r.vSol.totalSupply()).to.equal(supply + E);
  });

  it("settles a lock's attests in the order made: a wrong one first blocks nothing, a later copy is extra", async function () {
    const ctx = await deploy();
    const { receipts, operator, user, guardian } = ctx;
    const lock = rec.lock(SOLANA, 0, 5n, E, a32(user.address));
    const { pair, real, vSol } = await withVsol(ctx, [lock]);
    const supply = await vSol.totalSupply();
    // A wrong attest first, 1 unit to the guardian; then the true one; then
    // a copy of it.
    await receipts.connect(operator).attestLock(rec.lock(SOLANA, 0, 5n, 1n, a32(guardian.address)));
    await receipts.connect(operator).attestLock(lock);
    await receipts.connect(operator).attestLock(lock);
    expect((await receipts.getAttest(3n)).prev).to.equal(2n);
    await submit(ctx, pair.pending[2], real);
    await receipts.connect(operator).linkFast(2n, 3n);
    await accept(ctx, 3n);
    await expect(receipts.settleFast(2n, 3n, lock)).to.be.revertedWithCustomError(receipts, "NotInOrder");
    await receipts.connect(guardian).settleFast(1n, 3n, lock);
    await receipts.connect(guardian).settleFast(2n, 3n, lock);
    await receipts.connect(guardian).settleFast(3n, 3n, lock);
    // The true attest repaid; the wrong one's unit and the copy's receipt
    // burned from their collateral: supply grew by the lock only.
    expect(await vSol.totalSupply()).to.equal(supply + E);
    expect(await receipts.credit(operator.address, 0)).to.equal((E * 5n) / 4n);
    expect(await receipts.credit(guardian.address, 0)).to.equal(1n + E / 4n);
    // No attest once the receipt counted.
    await expect(receipts.connect(operator).attestLock(lock)).to.be.revertedWithCustomError(receipts, "AlreadyDone");
  });

  it("takes attests for 7 days; with none true, the recipient issues the receipt after them", async function () {
    const ctx = await deploy();
    const { receipts, operator, user, guardian } = ctx;
    const lock = rec.lock(SOLANA, 0, 5n, E, a32(user.address), 0n, 4n);
    const { pair, real, vSol } = await withVsol(ctx, [lock]);
    await submit(ctx, pair.pending[2], real);
    await accept(ctx, 3n);
    const wrong = rec.lock(SOLANA, 0, 5n, 1n, a32(guardian.address));
    await receipts.connect(operator).attestLock(wrong);
    await receipts.connect(guardian).settleFast(1n, 3n, lock);
    expect(await vSol.balanceOf(user.address)).to.equal(0n);
    await expect(receipts.issue(3n, lock)).to.be.revertedWithCustomError(receipts, "NotInOrder");
    await mineAt((await latestTime()) + WEEK);
    await expect(receipts.connect(operator).attestLock(wrong)).to.be.revertedWithCustomError(receipts, "WindowOver");
    await receipts.issue(3n, lock);
    expect(await vSol.balanceOf(user.address)).to.equal(E + 4n);
    await expect(receipts.issue(3n, lock)).to.be.revertedWithCustomError(receipts, "AlreadyDone");
  });
});

describe("iPoWVault: leaving (D112)", function () {
  it("ends the chain at EXIT, and frees the bond once its claims have ended", async function () {
    const ctx = await deploy();
    const { vault, operator } = ctx;
    const { pair, real } = await ready(ctx, 2n * E, async (p) => {
      await p.write(rec.bond(ETHEREUM, ETHEREUM, 0, E));
      await p.write(ethers.concat([rec.bond(SOLANA, ETHEREUM, 0, E), rec.exit()]));
      await p.write(rec.bond(ETHEREUM, ETHEREUM, 0, 1n));
    });
    await submit(ctx, pair.pending[0], real);
    await submit(ctx, pair.pending[1], real);
    await expect(submit(ctx, pair.pending[2], real)).to.be.revertedWithCustomError(vault, "ChainEnded");
    // The BOND claim for Solana is still open.
    expect(await vault.freeBond(operator.address, ETH_KEY)).to.equal(E);
    await accept(ctx, 1n);
    expect(await vault.freeBond(operator.address, ETH_KEY)).to.equal(2n * E);
    await expect(vault.connect(operator).withdrawBond(ETH_KEY, 2n * E)).to.changeEtherBalance(ethers, operator, 2n * ETH);
  });

  it("treats a record after EXIT as false", async function () {
    const ctx = await deploy();
    const { pair, real } = await ready(ctx, E, async (p) => {
      await p.write(ethers.concat([rec.exit(), rec.bond(ETHEREUM, ETHEREUM, 0, 1n)]));
    });
    await submit(ctx, pair.pending[0], real);
    expect((await ctx.vault.getChain(ctx.operator.address)).slashed).to.equal(true);
  });
});

describe("iPoWVault: review cases", function () {
  it("never pays a lock's fee again after the lock was returned", async function () {
    const ctx = await deploy();
    const { home, user, operator } = ctx;
    await lockEth(ctx, E, 7n);
    const record = await lockRecord(ctx, 1n);
    const r = await withSolana(ctx, E, async (p) => {
      await p.write(rec.cancel(SOLANA, 1n));
      await p.write(record);
    });
    await submit(ctx, r.pair.pending[1], r.real);
    await accept(ctx, 2n);
    await home.returnLock(2n, rec.cancel(SOLANA, 1n));
    expect(await home.credit(user.address, 0)).to.equal((E + 7n) * GWEI);
    // A LOCK record about the returned lock is true, but earns nothing.
    await expect(submit(ctx, r.pair.pending[2], r.real)).to.not.emit(home, "FeeEarned");
    expect((await ctx.vault.getChain(operator.address)).slashed).to.equal(false);
  });

  it("decides a claim after many rounds at the same cost, and each winner collects", async function () {
    const ctx = await deploy();
    const { vault, operator, guardian, user } = ctx;
    const r = await ready(ctx, E, async (p) => {
      await p.write(rec.bond(SOLANA, ETHEREUM, 0, E));
    });
    await submit(ctx, r.pair.pending[0], r.real);
    for (let i = 0; i < 10; i++) {
      await vault.connect(guardian).object(1n, { value: DEPOSIT });
      await vault.connect(user).answer(1n, { value: DEPOSIT });
    }
    await accept(ctx, 1n);
    // 11 answers share 10 objections; what does not divide backs vETH.
    const payout = DEPOSIT + (10n * DEPOSIT) / 11n;
    expect((await vault.getClaim(1n)).payout).to.equal(payout);
    await vault.connect(user).collect(1n);
    await vault.connect(operator).collect(1n);
    expect(await vault.credit(user.address, ETH_KEY)).to.equal(10n * payout);
    expect(await vault.credit(operator.address, ETH_KEY)).to.equal(payout);
    await expect(vault.connect(operator).withdrawCredit(ETH_KEY)).to.changeEtherBalance(ethers, operator, payout);
    expect((await vault.getPosition(operator.address, ETH_KEY)).peerBond).to.equal(E);
  });

  it("slashes a message too large to be judged on every network (D119)", async function () {
    // 137 BOND records of 15 bytes: a batch of 2,055 bytes, above 2,048.
    const ctx = await deploy();
    const r = await ready(ctx, E, async (p) => {
      await p.write(ethers.concat(Array.from({ length: 137 }, () => rec.bond(SOLANA, ETHEREUM, 0, 1n))));
    });
    await submit(ctx, r.pair.pending[0], r.real);
    expect((await ctx.vault.getChain(ctx.operator.address)).slashed).to.equal(true);

    // 33 records that name a lock, burn, cancel or asset, on both
    // networks together.
    const ctx2 = await deploy();
    await lockEth(ctx2, E);
    const r2 = await ready(ctx2, E, async (p) => {
      await p.write(ethers.concat([...Array.from({ length: 17 }, () => rec.cancel(SOLANA, 1n)), ...Array.from({ length: 16 }, () => rec.cancel(ETHEREUM, 1n))]));
    });
    await submit(ctx2, r2.pair.pending[0], r2.real);
    expect((await ctx2.vault.getChain(ctx2.operator.address)).slashed).to.equal(true);
  });

  it("acts on no second BOND from Solana in an asset, and lets one be carried again when its claim could not open", async function () {
    const ctx = await deploy();
    const { vault, operator, stranger } = ctx;
    const r = await ready(ctx, E, async (p) => {
      // The REQUEST passes the cover while no Solana bond counts: no claim.
      await p.write(ethers.concat([rec.bond(SOLANA, ETHEREUM, 0, 5n * E), rec.request(SOLANA, 0, 7n, 1n, a32(stranger.address))]));
      await p.write(rec.bond(SOLANA, ETHEREUM, 0, 5n * E));
      await p.write(rec.bond(SOLANA, ETHEREUM, 0, 2n));
    });
    await submit(ctx, r.pair.pending[0], r.real);
    expect(await vault.claimCount()).to.equal(0n);
    await submit(ctx, r.pair.pending[1], r.real);
    expect(await vault.claimCount()).to.equal(1n);
    await submit(ctx, r.pair.pending[2], r.real);
    expect(await vault.claimCount()).to.equal(1n);
    await accept(ctx, 1n);
    expect((await vault.getPosition(operator.address, ETH_KEY)).peerBond).to.equal(5n * E);
    expect((await vault.getChain(operator.address)).slashed).to.equal(false);
  });

  it("pays a burn number once across claims, and only from an accepted one", async function () {
    const ctx = await deploy();
    const { vault, home, guardian, user, stranger } = ctx;
    await lockEth(ctx, 2n * E);
    const a = rec.request(SOLANA, 0, 7n, E, a32(stranger.address));
    const b = rec.request(SOLANA, 0, 7n, E, a32(user.address));
    const r = await withSolana(ctx, E, async (p) => {
      await p.write(a);
      await p.write(b);
    });
    await submit(ctx, r.pair.pending[1], r.real); // claim 2
    await submit(ctx, r.pair.pending[2], r.real); // claim 3
    await vault.connect(guardian).object(2n, { value: DEPOSIT });
    await vault.connect(user).answer(2n, { value: DEPOSIT });
    await accept(ctx, 2n);
    await vault.decide(3n);
    await home.payRequest(2n, a);
    await expect(home.payRequest(3n, b)).to.be.revertedWithCustomError(home, "AlreadyDone");
    await expect(home.payRequest(3n, a)).to.be.revertedWithCustomError(home, "NotAccepted");
  });

  it("sends the pot of a claim refused with no objection to the backing", async function () {
    const ctx = await deploy();
    const { vault, home } = ctx;
    const r = await ready(ctx, E, async (p) => {
      await p.write(rec.bond(SOLANA, ETHEREUM, 0, 1n));
      await p.write(rec.lock(ETHEREUM, 0, 3n, 1n, RECIPIENT)); // false: slashes the chain here
    });
    await submit(ctx, r.pair.pending[0], r.real);
    await submit(ctx, r.pair.pending[1], r.real);
    const before = await home.reserve(0);
    await accept(ctx, 1n);
    expect((await vault.getClaim(1n)).accepted).to.equal(false);
    expect(await home.reserve(0)).to.equal(before + DEPOSIT / GWEI);
  });

  it("pays no fee to a slashed operator; the fee waits for another", async function () {
    const ctx = await deploy();
    const { home, operator } = ctx;
    await lockEth(ctx, E, 4n);
    const record = await lockRecord(ctx, 1n);
    const r = await ready(ctx, E, async (p) => {
      await p.write(rec.lock(ETHEREUM, 0, 9n, 1n, RECIPIENT)); // false
      await p.write(record); // true
    });
    await submit(ctx, r.pair.pending[0], r.real);
    await expect(submit(ctx, r.pair.pending[1], r.real)).to.not.emit(home, "FeeEarned");
    expect((await home.getLock(1n)).feePaid).to.equal(false);
    expect(await home.credit(operator.address, 0)).to.equal(0n);
  });

  it("refuses objections after the window", async function () {
    const ctx = await deploy();
    const { vault, guardian } = ctx;
    const r = await ready(ctx, E, async (p) => {
      await p.write(rec.bond(SOLANA, ETHEREUM, 0, 1n));
    });
    await submit(ctx, r.pair.pending[0], r.real);
    await mineAt((await latestTime()) + WEEK);
    await expect(vault.connect(guardian).object(1n, { value: DEPOSIT })).to.be.revertedWithCustomError(vault, "WindowOver");
    await vault.decide(1n);
    await expect(vault.connect(guardian).object(1n, { value: DEPOSIT })).to.be.revertedWithCustomError(vault, "NotOpen");
  });

  it("keeps at most 8 assets per chain, and receipts only once made", async function () {
    const ctx = await deploy();
    const { vault, home, operator } = ctx;
    await ready(ctx, E, async () => {});
    await expect(vault.connect(operator).addBond(SOL_KEY, 1n)).to.be.revertedWithCustomError(vault, "UnknownAsset");
    for (let i = 1; i <= 7; i++) {
      const t = await ethers.deployContract("MockToken", ["T", "T", 6]);
      await home.registerAsset(await t.getAddress());
      await t.mint(operator.address, 1n);
      await t.connect(operator).approve(await vault.getAddress(), 1n);
      await vault.connect(operator).addBond(key(ETHEREUM, i), 1n);
    }
    const t = await ethers.deployContract("MockToken", ["T", "T", 6]);
    await home.registerAsset(await t.getAddress());
    await expect(vault.connect(operator).addBond(key(ETHEREUM, 8), 1n)).to.be.revertedWithCustomError(vault, "TooManyAssets");
  });

  it("certifies no job whose escrow is below the minimum (D118)", async function () {
    const ctx = await deploy();
    const { vault, protocol, chain, operator, stranger } = ctx;
    // Mallory's own application opens a cheap job, and her operator proves it.
    await protocol.connect(stranger).registerApplication([]);
    const escrow = ETH / 100n;
    await protocol.connect(stranger).openJob(ethers.id("cheap"), escrow, 50, 6, 0, stranger.address, FEES, { value: FEES, gasLimit: GAS });
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
    await vault.connect(stranger).openCheckpoint(6, FEES, { value: FEES, gasLimit: GAS });
    expect((await protocol.getJob(await protocol.jobCount())).escrow).to.equal(MIN_CERTIFYING_ESCROW);
  });

  it("certifies no expired or slashed job, and takes the application's share into the backing", async function () {
    const ctx = await deploy();
    const { vault, home, protocol, operator, stranger, guardian } = ctx;
    await vault.connect(stranger).openCheckpoint(6, FEES, { value: FEES, gasLimit: GAS });
    await mineAt((await latestTime()) + HOUR);
    await expect(vault.recordRealFromJob(1n)).to.be.revertedWithCustomError(vault, "NotCertified");

    await vault.connect(stranger).openCheckpoint(6, FEES, { value: FEES, gasLimit: GAS });
    await protocol.connect(operator).bid(2n, await protocol.minimumBidOf(2n));
    await mineAt((await latestTime()) + 61);
    await mineAt(Number(await protocol.deadlineOf(2n)) + 1);
    const salt = ethers.id("salt");
    await protocol.connect(guardian).sealNote(await protocol.noteFor(guardian.address, 2n, ZERO_HASH, salt));
    await protocol.connect(guardian).reportMissedDuty(2n, salt);
    await expect(vault.recordRealFromJob(2n)).to.be.revertedWithCustomError(vault, "NotCertified");
    const before = await home.reserve(0);
    await vault.collectProtocolCredit();
    expect(await home.reserve(0)).to.be.gt(before);
  });
});

describe("iPoWVault: review cases of section 11.9", function () {
  it("slashes a chain even when a bonded token will not move, and settles the other assets", async function () {
    const ctx = await deploy();
    const { vault, home, operator, guardian } = ctx;
    const bad = await ethers.deployContract("MockToken", ["Bad", "BAD", 6]);
    await home.registerAsset(await bad.getAddress());
    const { pair, real } = await ready(ctx, E, async (p) => {
      await p.write(rec.lock(ETHEREUM, 0, 7n, 1n, RECIPIENT)); // no lock #7
    });
    await bad.mint(operator.address, 10n);
    await bad.connect(operator).approve(await vault.getAddress(), 10n);
    await vault.connect(operator).addBond(key(ETHEREUM, 1), 10n);
    await submit(ctx, pair.pending[0], real, guardian);
    expect((await vault.getChain(operator.address)).slashed).to.equal(true);
    await vault.settleSlash(operator.address, ETH_KEY);
    expect(await home.reserve(0)).to.equal((E * 4n) / 5n);
  });

  it("settles a wrong attest in its own asset, and leaves a lock of another asset to its slow issue", async function () {
    const ctx = await deploy();
    const { vault, receipts, operator, user, guardian } = ctx;
    // A second asset of Solana, and the operator's vSOL.
    const toOperator = rec.lock(SOLANA, 0, 1n, 10n * E, a32(operator.address));
    const lock = rec.lock(SOLANA, 1, 5n, 7n * E, a32(user.address));
    const r = await withSolana(ctx, E, async (p) => {
      await p.write(ethers.concat([rec.asset(SOLANA, 1, ethers.id("token"), 6), rec.bond(SOLANA, SOLANA, 1, 100n * E), toOperator]));
      await p.write(lock);
    });
    await submit(ctx, r.pair.pending[1], r.real);
    await accept(ctx, 2n);
    await receipts.makeReceipt(2n, rec.asset(SOLANA, 1, ethers.id("token"), 6));
    await receipts.issue(2n, toOperator);
    // The attester names lock #5 as 1 vSOL, though it is 7 of asset 1.
    const wrong = rec.lock(SOLANA, 0, 5n, E, a32(user.address));
    await receipts.connect(operator).attestLock(wrong);
    const vSol = r.vSol;
    const other = await ethers.getContractAt("VaultReceipt", await receipts.receiptOf(1));
    const supply = await vSol.totalSupply();
    await submit(ctx, r.pair.pending[2], r.real);
    await accept(ctx, 3n);
    await mineAt((await latestTime()) + WEEK);
    await receipts.connect(guardian).settleFast(1n, 3n, lock);
    // vSOL: the wrong receipt burned from its collateral, the rest to the
    // caller; nothing minted in asset 1 by it.
    expect(await vSol.totalSupply()).to.equal(supply - E);
    expect(await receipts.credit(guardian.address, 0)).to.equal(E / 4n);
    expect(await other.totalSupply()).to.equal(0n);
    // The true receipt, by the slow issue.
    await receipts.issue(3n, lock);
    expect(await other.balanceOf(user.address)).to.equal(7n * E);
    void vault;
  });
});
