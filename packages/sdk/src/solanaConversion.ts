// Conversion on Solana (programs/conversion): the same swaps as on EVM
// networks, SOL against real BTC, and the tunnel's legs (a sell held to a
// window of payment blocks, a buy opened for someone else). SOL only for
// now; an SPL token's swap needs its token accounts, not built here.
// Design: docs/drafts/ipow-conversion-app.md, docs/drafts/ipow-conversion-tunnel.md.

// Imported, not global: in a browser this is the `buffer` package.
import { Buffer } from "buffer";
import { AnchorProvider, BN, Program } from "@coral-xyz/anchor";
import { ComputeBudgetProgram, Connection, PublicKey, SystemProgram } from "@solana/web3.js";

import { addressToScript, scriptToAddress } from "./btcAddress.ts";
import { IDLS, SOLANA } from "./generated/solana.ts";
import type { SolanaWallet } from "./solana.ts";

/** The protocol's rules the fees and stages follow (ipow-protocol constants). */
const PROOF_RANGE = 25;
const FIXED_SIGNATURES = 20n;
const SIGNATURE_FEE = 5_000n;
const MIN_ESCROW_MULTIPLE = 5n;
const ESCROW_FEE_BPS = 50n;
const AUCTION_DURATION = 15 * 60;
const AUCTION_QUIET = 60;
const DEADLINE_PER_BLOCK = 48 * 60;
/** Conversion's: a buy's payment blocks after the anchor. */
const PAY_BLOCKS = 12;

export type SolanaSwapStage = "Auction" | "Expired" | "Assigned" | "Proven" | "Slashed" | "Settled";

export type SolanaSwapView = {
  id: bigint;
  side: "Sell" | "Buy";
  state: "Open" | "Funded" | "Done" | "Refunded" | "Cancelled" | "Reclaimed";
  user: string;
  /** Lamports. */
  amount: bigint;
  sats: bigint;
  /** Sell: the user's Bitcoin address. Buy: the operator's, once funded. */
  address: string | null;
  jobId: bigint;
  stage: SolanaSwapStage;
  operator: string | null;
  times: { opened: number; auctionEnd: number; deadline: number | null; proven: number | null; lockEnd: number | null };
  /** Buy, once funded: the Bitcoin blocks the payment must be in. */
  payBlocks: { first: number; last: number } | null;
  /** Sell in a tunnel: the Bitcoin blocks its payment must be mined in. */
  payWindow: { first: number; last: number } | null;
  /** The proven transaction, as explorers show it, and the height of the
   *  block the proof put it in; null before the proof. */
  provenTxid: string | null;
  provenHeight: number | null;
};

/** A tunnel's sell whose proven payment was mined outside its window (T1):
 *  its user takes the SOL back (`refundSell`, no transaction needed). */
export function solanaPaidOutsideWindow(s: SolanaSwapView): boolean {
  const h = s.provenHeight;
  if (s.side !== "Sell" || !s.payWindow || h === null) return false;
  if (s.stage !== "Proven" && s.stage !== "Settled") return false;
  return h < s.payWindow.first || h > s.payWindow.last;
}

export type SolanaSwapQuote = { confirmations: number; commitmentFee: bigint; escrowFee: bigint; margin: bigint; paid: bigint };

const u64 = (n: bigint | number) => {
  const b = Buffer.alloc(8);
  b.writeBigUInt64LE(BigInt(n));
  return b;
};
const big = (x: { toString(): string }) => BigInt(x.toString());
const variant = (e: any) => Object.keys(e)[0].replace(/^./, (c) => c.toUpperCase());

/** A job's fees on Solana: fixed by its confirmations, the signature fee
 *  being the network's (D24, D58). `margin` (the commitment fee by default)
 *  goes to the operator (D79). */
export function quoteSolanaSwap(confirmations = 6, marginBps = 10_000n): SolanaSwapQuote {
  const window = BigInt(PROOF_RANGE + confirmations - 1);
  const commitmentFee = ((window + FIXED_SIGNATURES) * SIGNATURE_FEE * 3n) / 2n;
  const escrowFee = (commitmentFee * MIN_ESCROW_MULTIPLE * ESCROW_FEE_BPS) / 10_000n;
  const margin = (commitmentFee * marginBps) / 10_000n;
  return { confirmations, commitmentFee, escrowFee, margin, paid: commitmentFee + escrowFee + margin };
}

export class SolanaConversion {
  readonly connection: Connection;
  readonly program: InstanceType<typeof Program>;
  readonly protocolId: PublicKey;
  readonly config: PublicKey;

  constructor(connection: Connection, wallet: SolanaWallet, programs: { conversion?: string; protocol?: string } = {}) {
    this.connection = connection;
    const provider = new AnchorProvider(connection, wallet as any, { commitment: "confirmed" });
    this.program = new Program({ ...(IDLS.conversion as any), address: programs.conversion ?? SOLANA.programs.conversion }, provider);
    this.protocolId = new PublicKey(programs.protocol ?? SOLANA.programs.protocol);
    this.config = PublicKey.findProgramAddressSync([Buffer.from("config")], this.program.programId)[0];
  }

  private pr(...seeds: Buffer[]) {
    return PublicKey.findProgramAddressSync(seeds, this.protocolId)[0];
  }
  swapAddress(id: bigint | number) {
    return PublicKey.findProgramAddressSync([Buffer.from("swap"), u64(id)], this.program.programId)[0];
  }
  private get accounts(): any {
    return this.program.account as any;
  }

  /** The tag of a swap's job (Conversion's `tag_of`). */
  private async tagOf(id: bigint): Promise<Buffer> {
    const data = Buffer.concat([Buffer.from("iPoW conversion"), this.program.programId.toBuffer(), u64(id)]);
    return Buffer.from(await crypto.subtle.digest("SHA-256", data));
  }

  /** The accounts a new swap's job is opened with. */
  private async openAccounts(user: PublicKey) {
    const config = await this.accounts.config.fetch(this.config);
    const id = big(config.swapCount) + 1n;
    const protocol = await (new Program({ ...(IDLS.protocol as any), address: this.protocolId.toBase58() }, this.program.provider).account as any).protocol.fetch(this.pr(Buffer.from("protocol")));
    const jobId = big(protocol.jobCount) + 1n;
    return {
      id,
      accounts: {
        config: this.config,
        swap: this.swapAddress(id),
        user,
        protocol: this.pr(Buffer.from("protocol")),
        application: this.pr(Buffer.from("application"), this.config.toBuffer()),
        job: this.pr(Buffer.from("job"), u64(jobId)),
        tagRecord: this.pr(Buffer.from("tag"), this.config.toBuffer(), await this.tagOf(id)),
        protocolVault: this.pr(Buffer.from("vault")),
        protocolProgram: this.protocolId,
        systemProgram: SystemProgram.programId,
      },
    };
  }

  /**
   * A send whose confirmation timed out (web3.js gives up after 30 seconds;
   * devnet is often slower) is not a failure: the transaction may well have
   * landed. Its status is asked for, up to two minutes, before giving up.
   */
  private async recover(e: any): Promise<string> {
    const signature: string | undefined = e?.signature;
    if (!signature) throw e;
    for (let i = 0; i < 40; i++) {
      await new Promise((r) => setTimeout(r, 3000));
      const st = (await this.connection.getSignatureStatuses([signature])).value[0];
      if (st?.err) throw new Error(`transaction ${signature} failed: ${JSON.stringify(st.err)}`);
      if (st?.confirmationStatus === "confirmed" || st?.confirmationStatus === "finalized") return signature;
    }
    throw e;
  }

  private budget() {
    return [ComputeBudgetProgram.setComputeUnitLimit({ units: 400_000 })];
  }

  /** Sells `lamports` of SOL for at least `sats`, paid to the Bitcoin address `to`. */
  async sell(options: { lamports: bigint; sats: bigint; to: string; quote?: SolanaSwapQuote }): Promise<{ swapId: bigint; signature: string }> {
    return this.openSell(options, null);
  }

  /** A sell as one leg of a tunnel (T1): paid to `to`, the funded buy's
   *  address on the other network, counting only when mined in `window`. */
  async sellInWindow(options: { lamports: bigint; sats: bigint; to: string; window: { first: number; last: number }; quote?: SolanaSwapQuote }): Promise<{ swapId: bigint; signature: string }> {
    return this.openSell(options, options.window);
  }

  private async openSell(o: { lamports: bigint; sats: bigint; to: string; quote?: SolanaSwapQuote }, window: { first: number; last: number } | null) {
    const user = this.program.provider.publicKey!;
    const q = o.quote ?? quoteSolanaSwap();
    const { id, accounts } = await this.openAccounts(user);
    const script = Buffer.from(addressToScript(o.to).slice(2), "hex");
    const none = { mint: null, escrow: null, from: null, tokenProgram: null, associatedTokenProgram: null };
    const call = window
      ? this.program.methods.sellInWindow(new BN(o.lamports.toString()), new BN(o.sats.toString()), script, window.first, window.last, q.confirmations, new BN(q.paid.toString()))
      : this.program.methods.sell(new BN(o.lamports.toString()), new BN(o.sats.toString()), script, q.confirmations, new BN(q.paid.toString()));
    const signature = await call.accountsStrict({ ...accounts, ...none } as any).preInstructions(this.budget()).rpc().catch((e: any) => this.recover(e));
    return { swapId: id, signature };
  }

  /** Buys `lamports` of SOL for `sats`, for `recipient` (the signer by
   *  default; another when an operator opens a tunnel's buy, T2). */
  async buy(options: { lamports: bigint; sats: bigint; recipient?: string; quote?: SolanaSwapQuote }): Promise<{ swapId: bigint; signature: string }> {
    const user = this.program.provider.publicKey!;
    const q = options.quote ?? quoteSolanaSwap();
    const { id, accounts } = await this.openAccounts(user);
    const call = options.recipient
      ? this.program.methods.buyFor(new PublicKey(options.recipient), new BN(options.lamports.toString()), new BN(options.sats.toString()), q.confirmations, new BN(q.paid.toString()))
      : this.program.methods.buy(new BN(options.lamports.toString()), new BN(options.sats.toString()), q.confirmations, new BN(q.paid.toString()));
    const signature = await call.accountsStrict({ ...accounts, mint: null } as any).preInstructions(this.budget()).rpc().catch((e: any) => this.recover(e));
    return { swapId: id, signature };
  }

  async getSwap(id: bigint | number): Promise<SolanaSwapView> {
    const s = await this.accounts.swap.fetch(this.swapAddress(id));
    const protocol = new Program({ ...(IDLS.protocol as any), address: this.protocolId.toBase58() }, this.program.provider);
    const job = await (protocol.account as any).job.fetch(this.pr(Buffer.from("job"), u64(big(s.jobId))));
    const now = Math.floor(Date.now() / 1000);
    const opened = Number(job.openedAt);
    const lastBid = Number(job.lastBidAt);
    const auctionEnd = job.hasOperator && lastBid + AUCTION_QUIET < opened + AUCTION_DURATION ? lastBid + AUCTION_QUIET : opened + AUCTION_DURATION;
    const stage: SolanaSwapStage =
      now < auctionEnd ? "Auction" : !job.hasOperator ? "Expired" : job.slashed ? "Slashed" : job.settled ? "Settled" : Number(job.provenAt) !== 0 ? "Proven" : "Assigned";
    const side = variant(s.side) as SolanaSwapView["side"];
    const state = variant(s.state) as SolanaSwapView["state"];
    const script = Buffer.from(s.script);
    const anchor = Number(job.anchor.height);
    const proven = Number(job.provenAt) !== 0;
    // Stored as hashed; explorers show it reversed.
    const txid = Buffer.from(job.txid).reverse().toString("hex");
    return {
      id: BigInt(id),
      side,
      state,
      user: s.user.toBase58(),
      amount: big(s.amount),
      sats: big(s.sats),
      address: script.length ? scriptToAddress("0x" + script.toString("hex")) : null,
      jobId: big(s.jobId),
      stage,
      operator: job.hasOperator ? job.operator.toBase58() : null,
      times: {
        opened,
        auctionEnd,
        deadline: job.hasOperator ? auctionEnd + (PROOF_RANGE + Number(job.confirmations) - 1) * DEADLINE_PER_BLOCK : null,
        proven: Number(job.provenAt) || null,
        lockEnd: Number(job.lockEnd) || null,
      },
      payBlocks: side === "Buy" && state === "Funded" && anchor ? { first: anchor + 1, last: anchor + PAY_BLOCKS } : null,
      payWindow: Number(s.payFrom) ? { first: Number(s.payFrom), last: Number(s.payTo) } : null,
      provenTxid: proven ? txid : null,
      provenHeight: proven ? Number(job.proofBlock.height) : null,
    };
  }

  /** The swaps whose user is `owner`, newest first. */
  async swapsOf(owner: string, limit = 50): Promise<SolanaSwapView[]> {
    // The user sits after the discriminator (8), id (8), side (1) and state (1).
    const all = await this.accounts.swap.all([{ memcmp: { offset: 18, bytes: new PublicKey(owner).toBase58() } }]);
    const ids = all.map((a: any) => big(a.account.id)).sort((a: bigint, b: bigint) => (b > a ? 1 : -1)).slice(0, limit);
    return Promise.all(ids.map((i: bigint) => this.getSwap(i)));
  }

  /** The sale of `owner`'s that pays `address` (a funded buy's, on the
   *  other network) and is still live: a tunnel's other leg. Null when
   *  there is none. */
  async sellPaying(owner: string, address: string): Promise<SolanaSwapView | null> {
    const mine = await this.swapsOf(owner);
    return mine.find((s) => s.side === "Sell" && s.address === address && s.state !== "Refunded" && s.state !== "Cancelled") ?? null;
  }

  /** Gives a sell's SOL back when it is refundable (no operator, a missed
   *  deadline, a slash, a payment outside its window). Anyone may call. */
  async refundSell(id: bigint | number, rawTx: Uint8Array = new Uint8Array()): Promise<{ signature: string }> {
    const s = await this.accounts.swap.fetch(this.swapAddress(id));
    const signature = await this.program.methods
      .refundSell(Buffer.from(rawTx))
      .accountsStrict({
        swap: this.swapAddress(id),
        job: this.pr(Buffer.from("job"), u64(big(s.jobId))),
        user: s.user,
        config: this.config,
        credit: this.pr(Buffer.from("credit"), this.config.toBuffer()),
        protocol: this.pr(Buffer.from("protocol")),
        protocolVault: this.pr(Buffer.from("vault")),
        protocolProgram: this.protocolId,
        mint: null,
        escrow: null,
        to: null,
        tokenProgram: null,
      } as any)
      .preInstructions(this.budget())
      .rpc().catch((e: any) => this.recover(e));
    return { signature };
  }

  /**
   * The job fees of a swap nobody took, back to the signer: its job expires
   * (D61, the fees credited to their payer, anyone may call), then the
   * signer's credit is paid out to them. Does each step only when due.
   */
  async reclaimFees(id: bigint | number): Promise<{ signatures: string[] }> {
    const owner = this.program.provider.publicKey!;
    const protocol = new Program({ ...(IDLS.protocol as any), address: this.protocolId.toBase58() }, this.program.provider);
    const s = await this.accounts.swap.fetch(this.swapAddress(id));
    const jobKey = this.pr(Buffer.from("job"), u64(big(s.jobId)));
    const job = await (protocol.account as any).job.fetch(jobKey);
    const credit = this.pr(Buffer.from("credit"), owner.toBuffer());
    const signatures: string[] = [];
    const v = await this.getSwap(id);
    if (v.stage === "Expired" && !job.feesReturned) {
      signatures.push(
        await protocol.methods
          .expire()
          .accountsStrict({ job: jobKey, payerCredit: this.pr(Buffer.from("credit"), job.payer.toBuffer()), funder: owner, systemProgram: SystemProgram.programId } as any)
          .rpc().catch((e: any) => this.recover(e))
      );
    }
    const record = await (protocol.account as any).credit.fetchNullable(credit);
    if (record && big(record.amount) > 0n) {
      signatures.push(
        await protocol.methods
          .withdrawCredit()
          .accountsStrict({ protocol: this.pr(Buffer.from("protocol")), credit, vault: this.pr(Buffer.from("vault")), owner } as any)
          .rpc().catch((e: any) => this.recover(e))
      );
    }
    return { signatures };
  }

  /** Ends a buy no operator took or funded in time. Anyone may call. */
  async cancel(id: bigint | number): Promise<{ signature: string }> {
    const s = await this.accounts.swap.fetch(this.swapAddress(id));
    const signature = await this.program.methods
      .cancel()
      .accountsStrict({ swap: this.swapAddress(id), job: this.pr(Buffer.from("job"), u64(big(s.jobId))) })
      .rpc().catch((e: any) => this.recover(e));
    return { signature };
  }
}
