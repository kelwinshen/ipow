// The user's own proof of a buy's payment (D10): when the buy's operator
// does not deliver its receipt, the user proves that they paid and receives
// the coin. Conversion accepts it (`proveMyPayment`) in two cases:
//
// - the operator proved a close mined after the payment blocks: the
//   payment's block must lie between the job's anchor and the close's block,
//   on the chain the operator proved;
// - the operator failed (slashed, its deadline passed, or its close came
//   within the payment blocks): the payment's block must lie on top of the
//   anchor's parent, with the job's confirmations on top of it.
//
// Every block on the way must be in the network's light client. Anyone may
// add blocks (`extend`, D64), so this adds the ones missing, read from a
// Bitcoin source, before proving. Design: docs/drafts/ipow-conversion-app.md
// ("Bitcoin to token") and docs/drafts/ipow-conversion-tunnel.md (T3).

import { Contract, concat, getBytes, hexlify, sha256, type Signer } from "ethers";

import { Bitcoin } from "./bitcoin.ts";
import { addressToScript } from "./btcAddress.ts";
import { getSwap } from "./conversion.ts";
import { ABIS } from "./generated/abis.ts";
import type { Deployment } from "./networks.ts";

const EPOCH = 2016;
/** Conversion.PAY_BLOCKS: the payment is in one of these after the anchor. */
const PAY_BLOCKS = 12;
/** Headers sent to the light client in one call (it takes up to MAX_WALK). */
const BATCH = 20;

/** What the proof reads from Bitcoin; the explorers' `Bitcoin` is one. All
 *  hashes and txids as explorers show them (display order). */
export type BitcoinSource = {
  blockHashAt(height: number): Promise<string>;
  header(blockHash: string): Promise<string>;
  blockTxids(blockHash: string): Promise<string[]>;
  minedIn(txid: string): Promise<{ blockHash: string; height: number } | null>;
  rawTx(txid: string): Promise<string>;
  outputs(txid: string): Promise<{ script: string; sats: bigint }[]>;
  paymentsTo(address: string): Promise<{ txid: string; confirmed: boolean }[]>;
  tip(): Promise<number>;
};

type Ref = { hash: string; height: number; epochTime: number };

const reverse = (hex: string) => "0x" + (hex.replace(/^0x/, "").match(/../g) ?? []).reverse().join("");
const sha256d = (bytes: Uint8Array | string) => sha256(sha256(bytes));
const headerTime = (header: string) => new DataView(getBytes(header).buffer).getUint32(68, true);
const headerRoot = (header: string) => hexlify(getBytes(header).subarray(36, 68));

/** A transaction's Merkle path in its block, in the order headers use. */
export function merklePath(txidsDisplay: string[], index: number): { siblings: string[]; root: string } {
  let level = txidsDisplay.map((t) => reverse(t));
  const siblings: string[] = [];
  let i = index;
  while (level.length > 1) {
    if (level.length % 2 === 1) level.push(level[level.length - 1]);
    siblings.push(level[i ^ 1]);
    const next: string[] = [];
    for (let k = 0; k < level.length; k += 2) next.push(sha256d(concat([level[k], level[k + 1]])));
    level = next;
    i >>= 1;
  }
  return { siblings, root: level[0] };
}

export type PaymentProofPlan = {
  /** The payment: its transaction and the output paying the operator. */
  payment: { txid: string; vout: number; height: number };
  /** Blocks added to the light client first, by height. */
  added: number[];
};

/**
 * Proves a funded buy's payment on the user's behalf and gives the user the
 * coin. The payment is found among the transactions paying the operator's
 * address. Throws, saying why, when the contract would refuse: no payment in
 * the payment blocks, too few confirmations yet, or the operator has not
 * failed and has proven no close.
 */
export async function provePayment(
  signer: Signer,
  d: Deployment,
  swapId: bigint | number,
  bitcoin: BitcoinSource = new Bitcoin()
): Promise<PaymentProofPlan & { txHash: string }> {
  const provider = signer.provider!;
  const s = await getSwap(provider, d, swapId);
  if (s.side !== "Buy" || s.state !== "Funded" || !s.address) throw new Error(`swap ${swapId} is not a funded buy`);
  const protocol = new Contract(d.protocol, ABIS.protocol, provider);
  const lightClient = new Contract(await protocol.lightClient(), ABIS.lightClient, signer);
  const duty = await protocol.getDuty(s.job.id);
  const anchor: Ref = { hash: duty.anchor.hash, height: Number(duty.anchor.height), epochTime: Number(duty.anchor.epochTime) };

  // The payment: an output paying the operator's script at least the sats.
  const script = addressToScript(s.address).slice(2).toLowerCase();
  let payment: PaymentProofPlan["payment"] | null = null;
  for (const p of await bitcoin.paymentsTo(s.address)) {
    const outs = await bitcoin.outputs(p.txid);
    const vout = outs.findIndex((o) => o.script.toLowerCase() === script && o.sats >= s.sats);
    const mined = vout >= 0 ? await bitcoin.minedIn(p.txid) : null;
    if (mined && mined.height > anchor.height && mined.height <= anchor.height + PAY_BLOCKS) {
      payment = { txid: p.txid, vout, height: mined.height };
      break;
    }
  }
  if (!payment) throw new Error(`no payment of ${s.sats} sats to ${s.address} mined in the payment blocks ${anchor.height + 1} to ${anchor.height + PAY_BLOCKS}`);

  // Which case: below the operator's close, or on top of the anchor's parent.
  const stage = s.job.stage;
  const closedAfter = (stage === "Proven" || stage === "Settled") && !duty.slashed && Number(duty.proofBlock.height) > anchor.height + PAY_BLOCKS;
  let low: Ref;
  let highHeight: number;
  if (closedAfter) {
    low = anchor;
    highHeight = Number(duty.proofBlock.height);
  } else {
    // The deadline as the contract judges it: by the network's time.
    const now = (await provider.getBlock("latest"))!.timestamp;
    const failed = stage === "Slashed" || stage === "Proven" || stage === "Settled" || (stage === "Assigned" && s.job.times.deadline !== null && now > s.job.times.deadline);
    if (!failed) throw new Error(`swap ${swapId}: its operator can still deliver its receipt; the user's proof is accepted once it has failed`);
    const confirmations = Number((await protocol.getJob(s.job.id)).confirmations);
    highHeight = payment.height + confirmations - 1;
    if ((await bitcoin.tip()) < highHeight) throw new Error(`the payment needs ${confirmations} confirmations; Bitcoin is not there yet`);
    const node = await lightClient.getNode(await lightClient.nodeId(anchor.hash, anchor.height, anchor.epochTime));
    low = { hash: node.prevHash, height: anchor.height - 1, epochTime: 0 };
  }

  // Each block's hash, header and epoch time, from `low` to the top.
  const headers = new Map<number, { hash: string; header: string }>();
  const blockAt = async (h: number) => {
    let b = headers.get(h);
    if (!b) {
      const display = await bitcoin.blockHashAt(h);
      b = { hash: reverse(display), header: await bitcoin.header(display) };
      headers.set(h, b);
    }
    return b;
  };
  const epochTimeAt = async (h: number) => headerTime((await blockAt(h - (h % EPOCH))).header);
  if (low.epochTime === 0) low.epochTime = await epochTimeAt(low.height);
  if (closedAfter && (await blockAt(highHeight)).hash.toLowerCase() !== String(duty.proofBlock.hash).toLowerCase())
    throw new Error("the operator's close is on a chain other than Bitcoin's best: prove against it by hand");
  const prevEpochTime = Math.floor(low.height / EPOCH) === Math.floor(highHeight / EPOCH) ? 0 : low.epochTime;

  // Add the blocks the light client lacks, from the first missing one up.
  const added: number[] = [];
  let firstMissing = -1;
  for (let h = low.height + 1; h <= highHeight; h++) {
    const b = await blockAt(h);
    if (!(await lightClient.isStored(await lightClient.nodeId(b.hash, h, await epochTimeAt(h))))) {
      firstMissing = h;
      break;
    }
  }
  if (firstMissing > 0) {
    for (let from = firstMissing; from <= highHeight; from += BATCH) {
      const to = Math.min(highHeight, from + BATCH - 1);
      const batch: string[] = [];
      for (let h = from; h <= to; h++) batch.push((await blockAt(h)).header);
      await (await lightClient.extend(concat(batch), from - 1, await epochTimeAt(from - 1))).wait();
      for (let h = from; h <= to; h++) added.push(h);
    }
  }

  // The payment in its block.
  const payBlock = await blockAt(payment.height);
  const txids = await bitcoin.blockTxids(reverse(payBlock.hash).slice(2));
  const index = txids.indexOf(payment.txid);
  if (index < 0) throw new Error(`the payment ${payment.txid} is not in block ${payment.height}`);
  const { siblings, root } = merklePath(txids, index);
  if (root.toLowerCase() !== headerRoot(payBlock.header).toLowerCase()) throw new Error("the block's transactions do not make its Merkle root");

  const top = closedAfter ? payBlock : await blockAt(highHeight);
  const topRef = { hash: top.hash, height: closedAfter ? payment.height : highHeight, epochTime: await epochTimeAt(closedAfter ? payment.height : highHeight) };
  const payRef = { hash: payBlock.hash, height: payment.height, epochTime: await epochTimeAt(payment.height) };
  const conversion = new Contract(d.conversion, ABIS.conversion, signer);
  const tx = await conversion.proveMyPayment(swapId, await bitcoin.rawTx(payment.txid), payment.vout, payRef, siblings, index, topRef, prevEpochTime);
  await tx.wait();
  return { payment, added, txHash: tx.hash };
}
