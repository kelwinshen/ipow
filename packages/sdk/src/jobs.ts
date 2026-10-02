// A job by stage, with when each stage began and what it waits for, so an
// app can show its progress in plain words. Read from the protocol's
// storage; the job's Bitcoin transaction from explorers.

import { Contract, ZeroAddress, ZeroHash, type Provider } from "ethers";

import { Bitcoin, displayTxid, type BitcoinTx } from "./bitcoin.ts";
import { ABIS } from "./generated/abis.ts";
import type { Deployment } from "./networks.ts";

/** The protocol's job statuses, in its own order. */
export const STAGES = ["None", "Auction", "Expired", "Assigned", "Proven", "Slashed", "Settled"] as const;
export type Stage = (typeof STAGES)[number];

export type JobView = {
  network: string;
  id: bigint;
  stage: Stage;
  /** What the job waits for now, in words an app can show; null when done. */
  waitingFor: string | null;
  application: string;
  payer: string;
  operator: string | null;
  escrow: bigint;
  commitmentFee: bigint;
  escrowFee: bigint;
  confirmations: number;
  tag: string;
  /** Seconds since 1970; null for a stage not reached. */
  times: {
    opened: number;
    auctionEnd: number;
    lastBid: number | null;
    deadline: number | null;
    anchored: number | null;
    proven: number | null;
    lockEnd: number | null;
  };
  /** The proven transaction, as explorers show it; null before the proof. */
  provenTxid: string | null;
};

const when = (t: bigint | number) => (Number(t) === 0 ? null : Number(t));
const at = (t: number) => new Date(t * 1000).toISOString();

export async function getJob(provider: Provider, d: Deployment, id: bigint | number): Promise<JobView> {
  const protocol = new Contract(d.protocol, ABIS.protocol, provider);
  const [status, job, duty, auctionEnd] = await Promise.all([
    protocol.statusOf(id),
    protocol.getJob(id),
    protocol.getDuty(id),
    protocol.auctionEndOf(id),
  ]);
  const stage = STAGES[Number(status)];
  if (stage === "None") throw new Error(`${d.name}: no job ${id}`);
  const operator = job.operator === ZeroAddress ? null : (job.operator as string);
  const deadline = operator ? Number(await protocol.deadlineOf(id)) : null;
  const times = {
    opened: Number(job.openedAt),
    auctionEnd: Number(auctionEnd),
    lastBid: when(job.lastBidAt),
    deadline,
    anchored: when(duty.anchoredAt),
    proven: when(duty.provenAt),
    lockEnd: when(duty.lockEnd),
  };
  const confirmations = Number(job.confirmations);
  const waitingFor: Record<Stage, string | null> = {
    None: null,
    // A bid does not end the auction: it ends a minute after the last bid
    // (D37), and the job is assigned then.
    Auction: operator
      ? `the auction to end at ${at(times.auctionEnd)}; an operator has bid`
      : `an operator's bid, until ${at(times.auctionEnd)}; if none, the fees come back`,
    Assigned: times.anchored
      ? `the operator's Bitcoin transaction to reach ${confirmations} confirmations and be proven, by ${at(deadline!)}`
      : `the operator to anchor the job and write it on Bitcoin, by ${at(deadline!)}`,
    Proven: times.lockEnd ? `the lock to end at ${at(times.lockEnd)}, unless a guardian challenges the proof` : "the lock to end",
    Expired: null,
    Slashed: null,
    Settled: null,
  };
  return {
    network: d.name,
    id: BigInt(id),
    stage,
    waitingFor: waitingFor[stage],
    application: job.application,
    payer: job.payer,
    operator,
    escrow: job.escrow,
    commitmentFee: job.commitmentFee,
    escrowFee: job.escrowFee,
    confirmations,
    tag: job.tag,
    times,
    provenTxid: duty.txid === ZeroHash ? null : displayTxid(duty.txid),
  };
}

/**
 * The job's Bitcoin transaction, before or after its proof. Before it, the
 * operator's tagged transaction spends its chain head, which the protocol
 * records, and carries the job's tag (D6), which is checked here; null
 * while it has not been sent.
 */
export async function findJobTx(provider: Provider, d: Deployment, job: JobView, bitcoin = new Bitcoin()): Promise<BitcoinTx | null> {
  if (job.provenTxid) return bitcoin.tx(job.provenTxid);
  if (!job.operator || !job.times.anchored) return null;
  const protocol = new Contract(d.protocol, ABIS.protocol, provider);
  const head = await protocol.chainHeadOf(job.operator);
  if (!head.set) return null;
  const spender = await bitcoin.spender(displayTxid(head.txid), Number(head.vout));
  if (!spender) return null;
  const payload: string = await protocol.tagPayload(job.tag);
  const tagged = (await bitcoin.outputScripts(spender)).includes("6a20" + payload.slice(2));
  return tagged ? bitcoin.tx(spender) : null;
}

/** Yields the job each time its stage or a time changes, until it ends. */
export async function* watchJob(provider: Provider, d: Deployment, id: bigint | number, intervalMs = 30_000): AsyncGenerator<JobView> {
  let last = "";
  for (;;) {
    const job = await getJob(provider, d, id);
    const key = JSON.stringify([job.stage, job.times, job.provenTxid]);
    if (key !== last) {
      last = key;
      yield job;
    }
    if (job.stage === "Settled" || job.stage === "Expired" || job.stage === "Slashed") return;
    await new Promise((r) => setTimeout(r, intervalMs));
  }
}
