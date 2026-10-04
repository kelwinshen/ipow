import { expect } from "chai";
import { network } from "hardhat";

import { COIN_SCRIPT, buildTx, concat, headerHashLE, merkle, mine, reverseHex, targetFromBits, txidLE } from "./helpers/bitcoin.ts";
import { provePayment, type BitcoinSource } from "../../../packages/sdk/src/index.ts";

const { ethers } = await network.create();

// The SDK's proof of a buy's payment by the user (D10), against a Bitcoin
// chain the test mines: blocks the light client has not stored are added by
// the SDK from the Bitcoin source before it proves.

const HOUR = 3600;
const DAY = 24 * HOUR;
const ETH = 10n ** 18n;
const EASY = 0x207fffff;
const ZERO_HASH = "0x" + "00".repeat(32);
const FEES = ETH / 10n;
const GAS = 2_000_000n;
const OPERATOR_SCRIPT = "0x0014" + "33".repeat(20);

async function latestTime() {
  return (await ethers.provider.getBlock("latest"))!.timestamp;
}
async function mineAt(t: number) {
  await ethers.provider.send("evm_setNextBlockTimestamp", [t]);
  await ethers.provider.send("evm_mine", []);
}
const display = (le: string) => reverseHex(le);

/** A Bitcoin chain known to the test (and served as the SDK's source); a
 *  block reaches the light client only when `store` is true. */
class Chain implements BitcoinSource {
  private byHeight = new Map<number, { le: string; header: string }>();
  private txidsOf = new Map<string, string[]>();
  private txs = new Map<string, { raw: string; outputs: { script: string; sats: bigint }[]; at?: { blockHash: string; height: number } }>();
  head!: { hash: string; height: number; epochTime: number };
  private salt = 0;
  constructor(private lightClient: any) {}

  async start(now: number) {
    const epochTime = now - HOUR;
    const headers: string[] = [];
    let prevLE = ZERO_HASH;
    for (let i = 0; i < 6; i++) {
      const h = mine({ prevLE, time: epochTime + i * 600, bits: EASY });
      headers.push(h);
      prevLE = headerHashLE(h);
      this.byHeight.set(i, { le: prevLE, header: h });
      this.txidsOf.set(display(prevLE).slice(2), []);
    }
    await this.lightClient.addEpochStart(concat(headers), 0);
    this.head = { hash: prevLE, height: 5, epochTime };
  }

  async add(txs: { raw: string; outputs: { script: string; sats: bigint }[] }[] = [], store = true) {
    const on = this.head;
    const coinbase = buildTx({ inputs: [{ txidLE: ZERO_HASH, vout: 0xffffffff }], outputs: [{ value: BigInt(++this.salt), script: COIN_SCRIPT }] });
    const ids = [txidLE(coinbase), ...txs.map((t) => txidLE(t.raw))];
    const header = mine({ prevLE: on.hash, time: (await latestTime()) + 1, bits: EASY, merkleRootLE: merkle(ids, 0).rootLE });
    if (store) await this.lightClient.extend(header, on.height, on.epochTime);
    this.head = { hash: headerHashLE(header), height: on.height + 1, epochTime: on.epochTime };
    const hashDisplay = display(this.head.hash).slice(2);
    this.byHeight.set(this.head.height, { le: this.head.hash, header });
    this.txidsOf.set(hashDisplay, ids.map((i) => display(i).slice(2)));
    for (const t of txs) this.txs.set(display(txidLE(t.raw)).slice(2), { ...t, at: { blockHash: hashDisplay, height: this.head.height } });
    return this.head;
  }

  /** Stores in the light client the blocks from height `h` to the head. */
  async storeFrom(h: number) {
    const headers: string[] = [];
    for (let i = h; i <= this.head.height; i++) headers.push(this.byHeight.get(i)!.header);
    await this.lightClient.extend(concat(headers), h - 1, this.head.epochTime);
  }

  // The SDK's Bitcoin source.
  async blockHashAt(h: number) { return display(this.byHeight.get(h)!.le).slice(2); }
  async header(hash: string) { return [...this.byHeight.values()].find((b) => display(b.le).slice(2) === hash)!.header; }
  async blockTxids(hash: string) { return this.txidsOf.get(hash)!; }
  async minedIn(txid: string) { return this.txs.get(txid)?.at ?? null; }
  async rawTx(txid: string) { return this.txs.get(txid)!.raw; }
  async outputs(txid: string) { return this.txs.get(txid)!.outputs.map((o) => ({ script: o.script.slice(2), sats: o.sats })); }
  async paymentsTo(_address: string) { return [...this.txs.keys()].map((txid) => ({ txid, confirmed: true })); }
  async tip() { return this.head.height; }
}

async function setup() {
  const lightClient = await ethers.deployContract("iPoWLightClientHarness", [0]);
  await lightClient.setLimits(targetFromBits(EASY), targetFromBits(EASY));
  const protocol = await ethers.deployContract("iPoWProtocolHarness", [await lightClient.getAddress()]);
  const conversion = await ethers.deployContract("Conversion", [await protocol.getAddress(), 10_000_000n, ethers.ZeroAddress]);
  const [, user, operator] = await ethers.getSigners();
  const now = Math.ceil(((await latestTime()) + DAY) / DAY) * DAY;
  await mineAt(now);
  const chain = new Chain(lightClient);
  await chain.start(now);
  // The operator's bond and first chain head.
  await protocol.connect(operator).lockBond(3n * ETH, { value: 3n * ETH });
  const first = buildTx({
    inputs: [{ txidLE: ethers.id("funding"), vout: 0 }],
    outputs: [
      { value: 546n, script: COIN_SCRIPT },
      { value: 0n, script: "0x6a20" + (await protocol.chainHeadCommitment(operator.address)).slice(2) },
    ],
  });
  const block = await chain.add([{ raw: first, outputs: [] }]);
  const ids = (await chain.blockTxids(display(block.hash).slice(2))).map((i) => display(i));
  const index = ids.indexOf(txidLE(first));
  await protocol.connect(operator).registerChainHead(block, first, merkle(ids, index).siblings, index, 0, 1);

  // A funded buy: the user asks 1 ETH for 0.05 BTC, the operator wins, anchors and locks it.
  await conversion.connect(user).buy(ethers.ZeroAddress, ETH, 5_000_000n, "0x", 6, FEES, { value: FEES, gasLimit: GAS });
  const jobId = (await conversion.getSwap(1n)).jobId;
  await protocol.connect(operator).bid(jobId, await protocol.minimumBidOf(jobId));
  await mineAt((await latestTime()) + 61);
  const anchor = await chain.add();
  await protocol.connect(operator).anchorJob(jobId, anchor);
  await conversion.connect(operator).fund(1n, OPERATOR_SCRIPT, { value: ETH });

  const d = {
    name: "local", network: "ethereum", number: 1, chainId: 31337,
    coin: { kind: "native", decimals: 18, price: "baseFee" }, workScale: null, dataFee: "none",
    lightClient: await lightClient.getAddress(), protocol: await protocol.getAddress(), conversion: await conversion.getAddress(), vaults: [],
  } as any;
  // The user's payment in the first payment block, then 5 blocks: none stored yet.
  const payment = buildTx({ inputs: [{ txidLE: ethers.id("user coin"), vout: 0 }], outputs: [{ value: 5_000_000n, script: OPERATOR_SCRIPT }] });
  await chain.add([{ raw: payment, outputs: [{ script: OPERATOR_SCRIPT, sats: 5_000_000n }] }], false);
  for (let i = 0; i < 5; i++) await chain.add([], false);
  return { protocol, conversion, user, operator, jobId, anchor, d, chain, source: chain as BitcoinSource };
}

describe("SDK: the user's own payment proof (D10)", function () {
  it("waits while the operator can still deliver its receipt", async function () {
    const { user, d, source } = await setup();
    let error = "";
    await provePayment(user, d, 1n, source).catch((e) => (error = e.message));
    expect(error).to.match(/can still deliver/);
  });

  it("adds the blocks the light client lacks and gives the user the coin once the operator failed", async function () {
    const { protocol, user, jobId, anchor, d, source } = await setup();
    await mineAt(Number(await protocol.deadlineOf(jobId)) + 1);
    const before = await ethers.provider.getBalance(user.address);
    const r = await provePayment(user, d, 1n, source);
    expect(r.payment.height).to.equal(anchor.height + 1);
    expect(r.added).to.deep.equal([1, 2, 3, 4, 5, 6].map((i) => anchor.height + i));
    const after = await ethers.provider.getBalance(user.address);
    // 1 ETH, less the user's gas for the blocks and the proof.
    expect(after - before).to.be.greaterThan((ETH * 99n) / 100n);
  });

  it("proves below the operator's close when it closed after the payment blocks", async function () {
    const { protocol, conversion, user, operator, jobId, anchor, d, chain, source } = await setup();
    // The operator stores the blocks and, past the payment blocks, closes:
    // a tagged transaction that spends no payment.
    await chain.storeFrom(anchor.height + 1);
    while (chain.head.height < anchor.height + 12) await chain.add();
    const head = await protocol.chainHeadOf(operator.address);
    const tag = ethers.sha256(ethers.concat([ethers.toUtf8Bytes("iPoW job"), await conversion.tagOf(1n)]));
    const close = buildTx({
      inputs: [{ txidLE: head.txid, vout: Number(head.vout) }],
      outputs: [
        { value: 546n, script: COIN_SCRIPT },
        { value: 0n, script: "0x6a20" + tag.slice(2) },
      ],
    });
    const proofBlock = await chain.add([{ raw: close, outputs: [] }]);
    for (let i = 0; i < 5; i++) await chain.add();
    const ids = (await chain.blockTxids(display(proofBlock.hash).slice(2))).map((i) => display(i));
    const index = ids.indexOf(txidLE(close));
    await protocol.connect(operator).proveJob(jobId, {
      proofBlock,
      tip: chain.head,
      prevEpochTime: 0,
      rawTx: close,
      siblings: merkle(ids, index).siblings,
      txIndex: index,
      headIndex: 0,
      tagIndex: 1,
    });
    expect(proofBlock.height).to.equal(anchor.height + 13);

    // The operator has not failed, but its close came after the payment
    // blocks: the user proves the payment between the anchor and the close.
    const before = await ethers.provider.getBalance(user.address);
    const r = await provePayment(user, d, 1n, source);
    expect(r.payment.height).to.equal(anchor.height + 1);
    expect(r.added).to.deep.equal([]);
    const after = await ethers.provider.getBalance(user.address);
    expect(after - before).to.be.greaterThan((ETH * 99n) / 100n);
  });
});
