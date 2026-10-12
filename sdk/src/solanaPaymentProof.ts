// The user's own proof of a buy's payment (D10) on Solana: the same proof as
// paymentProof.ts gives an EVM network, for Conversion's `prove_my_payment`.
// It is accepted in the same two cases:
//
// - the operator proved a close mined after the payment blocks: the
//   payment's block lies between the job's anchor and the close's block, on
//   the chain the operator proved;
// - the operator failed (slashed, its deadline passed, or its close came
//   within the payment blocks): the payment's block lies on top of the
//   anchor's parent, with the job's confirmations on top of it.
//
// What Solana makes different: a block is its own account in the light
// client, added at most 7 at a time (`extend`); and the link between two
// blocks is read by a walk, an account made first (`begin_walk`) and read in
// steps of up to 20 blocks (`walk_step`), which the proof then names. The
// walks are closed afterwards, their rent back to the signer. The payment is
// sent inside the proving instruction: when it and its Merkle path do not fit
// a Solana transaction (1,232 bytes), the proof names its accounts through an
// address lookup table made for it, which makes room for a payment of a few
// inputs. One with many may still not fit: the app asks the user to pay from
// few coins.
// Design: docs/specs/ipow-conversion-app.md ("Bitcoin to token", "Limits of
// the Solana program") and docs/specs/ipow-conversion-tunnel.md (T3).

import { Buffer } from "buffer";
import { AnchorProvider, Program } from "@coral-xyz/anchor";
import BN from "bn.js";
import {
  AddressLookupTableProgram,
  ComputeBudgetProgram,
  Connection,
  PublicKey,
  SystemProgram,
  Transaction,
  TransactionMessage,
  VersionedTransaction,
  type AccountMeta,
  type TransactionInstruction,
} from "@solana/web3.js";

import { Bitcoin } from "./bitcoin.ts";
import { addressToScript } from "./btcAddress.ts";
import { IDLS, SOLANA } from "./generated/solana.ts";
import { merklePath, type BitcoinSource, type PaymentProofPlan } from "./paymentProof.ts";
import { associatedTokenAccount, type SolanaWallet } from "./solana.ts";
import { createAccountIdempotent } from "./solanaBeta.ts";
import { SolanaConversion } from "./solanaConversion.ts";

const EPOCH = 2016;
/** Conversion's PAY_BLOCKS: the payment is in one of these after the anchor. */
const PAY_BLOCKS = 12;
/** Headers per `extend`: the light client's MAX_EXTEND. */
const MAX_EXTEND = 7;
/** Blocks read per `walk_step`: what fits a transaction's accounts. */
const WALK_STEP = 20;
/** Compute for the light client's work, above Solana's default. */
const COMPUTE = 1_400_000;
/** The most a Solana transaction holds, in bytes. */
const MAX_TX = 1232;

type Ref = { hash: Buffer; height: number; epochTime: number };

const fromDisplay = (hex: string) => Buffer.from(hex.replace(/^0x/, ""), "hex").reverse();
const bytes = (hex: string) => Buffer.from(hex.replace(/^0x/, ""), "hex");
const u32 = (n: number) => {
  const b = Buffer.alloc(4);
  b.writeUInt32LE(n);
  return b;
};
const u64 = (n: bigint) => {
  const b = Buffer.alloc(8);
  b.writeBigUInt64LE(n);
  return b;
};
const headerTime = (header: Buffer) => header.readUInt32LE(68);
const arg = (r: Ref) => ({ hash: [...r.hash], height: r.height, epochTime: r.epochTime });

/**
 * Proves a funded buy's payment on Solana on the user's behalf and gives the
 * user the coin (SOL or a token). The payment is found among the
 * transactions paying the operator's address. Throws, saying why, when the
 * program would refuse: no payment in the payment blocks, too few
 * confirmations yet, or the operator has not failed and has proven no close.
 * Anyone may send it; the signer pays for the blocks added and the fees.
 */
export async function provePaymentOnSolana(
  connection: Connection,
  wallet: SolanaWallet,
  swapId: bigint | number,
  bitcoin: BitcoinSource = new Bitcoin(),
  programs: { conversion?: string; protocol?: string; lightClient?: string } = {}
): Promise<PaymentProofPlan & { signature: string }> {
  const conv = new SolanaConversion(connection, wallet, programs);
  const provider = new AnchorProvider(connection, wallet as any, { commitment: "confirmed" });
  const payer = wallet.publicKey;
  const lc = new Program({ ...(IDLS.lightClient as any), address: programs.lightClient ?? SOLANA.programs.lightClient }, provider);
  const pr = new Program({ ...(IDLS.protocol as any), address: programs.protocol ?? SOLANA.programs.protocol }, provider);
  const lcPda = (...seeds: Buffer[]) => PublicKey.findProgramAddressSync(seeds, lc.programId)[0];
  const nodeOf = (r: Ref) => lcPda(Buffer.from("node"), r.hash, u32(r.height), u32(r.epochTime));
  const config = lcPda(Buffer.from("config"));
  const budget = () => [ComputeBudgetProgram.setComputeUnitLimit({ units: COMPUTE })];

  const s = await conv.getSwap(swapId);
  if (s.side !== "Buy" || s.state !== "Funded" || !s.address) throw new Error(`swap ${swapId} is not a funded buy`);
  const jobKey = PublicKey.findProgramAddressSync([Buffer.from("job"), u64(s.jobId)], pr.programId)[0];
  const job = await (pr.account as any).job.fetch(jobKey);
  const anchor: Ref = { hash: Buffer.from(job.anchor.hash), height: Number(job.anchor.height), epochTime: Number(job.anchor.epochTime) };

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

  // Each block of Bitcoin's best chain, by height.
  const headers = new Map<number, { hash: Buffer; header: Buffer }>();
  const blockAt = async (h: number) => {
    let b = headers.get(h);
    if (!b) {
      const display = await bitcoin.blockHashAt(h);
      b = { hash: fromDisplay(display), header: bytes(await bitcoin.header(display)) };
      headers.set(h, b);
    }
    return b;
  };
  const epochTimeAt = async (h: number) => headerTime((await blockAt(h - (h % EPOCH))).header);
  const refAt = async (h: number): Promise<Ref> => ({ hash: (await blockAt(h)).hash, height: h, epochTime: await epochTimeAt(h) });

  // Which case: below the operator's close, or on top of the anchor's parent.
  const closedAfter = (s.stage === "Proven" || s.stage === "Settled") && !job.slashed && Number(job.proofBlock.height) > anchor.height + PAY_BLOCKS;
  let low: Ref;
  let highHeight: number;
  if (closedAfter) {
    low = anchor;
    highHeight = Number(job.proofBlock.height);
    if (!(await blockAt(highHeight)).hash.equals(Buffer.from(job.proofBlock.hash))) throw new Error("the operator's close is on a chain other than Bitcoin's best: prove against it by hand");
  } else {
    // The deadline as the program judges it: by the network's clock.
    const now = (await connection.getBlockTime(await connection.getSlot("confirmed"))) ?? Math.floor(Date.now() / 1000);
    const failed = s.stage === "Slashed" || s.stage === "Proven" || s.stage === "Settled" || (s.stage === "Assigned" && s.times.deadline !== null && now > s.times.deadline);
    if (!failed) throw new Error(`swap ${swapId}: its operator can still deliver its receipt; the user's proof is accepted once it has failed`);
    const confirmations = Number(job.confirmations);
    highHeight = payment.height + confirmations - 1;
    if ((await bitcoin.tip()) < highHeight) throw new Error(`the payment needs ${confirmations} confirmations; Bitcoin is not there yet`);
    // The anchor's parent, as the anchor's own block names it.
    const anchorNode = await (lc.account as any).node.fetch(nodeOf(anchor));
    // Its epoch time as the program takes it: the anchor's, unless the
    // anchor opens an epoch.
    low = { hash: Buffer.from(anchorNode.prevHash), height: anchor.height - 1, epochTime: anchor.height % EPOCH === 0 ? await epochTimeAt(anchor.height - 1) : anchor.epochTime };
    if (!(await blockAt(low.height)).hash.equals(low.hash)) throw new Error("the job's anchor is no longer on Bitcoin's best chain: prove against it by hand");
  }

  /**
   * A send whose confirmation timed out is not a failure (web3.js gives up
   * after 30 seconds; devnet is often slower): its status is asked for, up to
   * two minutes, before giving up.
   */
  const landed = async (e: any): Promise<string> => {
    const signature: string | undefined = e?.signature;
    if (!signature) throw e;
    for (let i = 0; i < 40; i++) {
      await new Promise((r) => setTimeout(r, 3000));
      const st = (await connection.getSignatureStatuses([signature])).value[0];
      if (st?.err) throw new Error(`transaction ${signature} failed: ${JSON.stringify(st.err)}`);
      if (st?.confirmationStatus === "confirmed" || st?.confirmationStatus === "finalized") return signature;
    }
    throw e;
  };

  // A block is stored when its account is the light client's own, with its
  // record: an address someone only sent lamports to is not.
  const stored = async (r: Ref) => {
    const info = await connection.getAccountInfo(nodeOf(r));
    return info !== null && info.owner.equals(lc.programId) && info.data.length > 0;
  };
  const added: number[] = [];

  // The anchor's parent: an anchor reached by a jump has no block below it
  // stored, and the walk down must end on one. It is added from its child.
  if (!closedAfter && !(await stored(low))) {
    const prevEpochTime = anchor.height % EPOCH === 0 ? low.epochTime : 0;
    await lc.methods
      .extendBack([...(await blockAt(low.height)).header], prevEpochTime)
      .accountsStrict({ config, child: nodeOf(anchor), parent: nodeOf(low), payer, systemProgram: SystemProgram.programId } as any)
      .preInstructions(budget())
      .rpc()
      .catch(landed);
    added.push(low.height);
  }

  // Add the blocks the light client lacks above it: only the batches that
  // hold one not stored yet (the program skips those already there).
  let parent: Ref = low;
  for (let from = low.height + 1; from <= highHeight; from += MAX_EXTEND) {
    const to = Math.min(highHeight, from + MAX_EXTEND - 1);
    const batch: Buffer[] = [];
    const nodes: AccountMeta[] = [];
    const missing: number[] = [];
    let last = parent;
    for (let h = from; h <= to; h++) {
      const r = await refAt(h);
      batch.push((await blockAt(h)).header);
      nodes.push({ pubkey: nodeOf(r), isSigner: false, isWritable: true });
      if (!(await stored(r))) missing.push(h);
      last = r;
    }
    if (missing.length) {
      if (!(await stored(parent))) throw new Error(`block ${parent.height} is not in the light client: it cannot be extended from there`);
      await lc.methods
        .extend(Buffer.concat(batch))
        .accountsStrict({ config, parent: nodeOf(parent), payer, systemProgram: SystemProgram.programId } as any)
        .remainingAccounts(nodes)
        .preInstructions(budget())
        .rpc()
        .catch(landed);
      added.push(...missing);
    }
    parent = last;
  }

  // The payment in its block.
  const payRef = await refAt(payment.height);
  const payBlock = await blockAt(payment.height);
  const txids = await bitcoin.blockTxids(Buffer.from(payBlock.hash).reverse().toString("hex"));
  const index = txids.indexOf(payment.txid);
  if (index < 0) throw new Error(`the payment ${payment.txid} is not in block ${payment.height}`);
  const { siblings, root } = merklePath(txids, index);
  if (!bytes(root).equals(payBlock.header.subarray(36, 68))) throw new Error("the block's transactions do not make its Merkle root");

  // A walk from `descendant` down to `ancestor`: made, then read in steps.
  // Its address is kept before it is made, so it is closed whatever happens.
  const walks: PublicKey[] = [];
  const walk = async (descendant: Ref, ancestor: Ref): Promise<PublicKey> => {
    const prevEpochTime = descendant.epochTime !== ancestor.epochTime ? ancestor.epochTime : 0;
    const path: Ref[] = [];
    for (let h = descendant.height; h > ancestor.height; h--) path.push(h === descendant.height ? descendant : await refAt(h));
    path.push(ancestor);
    const id = BigInt("0x" + Buffer.from(crypto.getRandomValues(new Uint8Array(8))).toString("hex"));
    const key = lcPda(Buffer.from("walk"), payer.toBuffer(), u64(id));
    walks.push(key);
    await lc.methods
      .beginWalk(new BN(id.toString()), arg(descendant), arg(ancestor), prevEpochTime)
      .accountsStrict({ walk: key, payer, systemProgram: SystemProgram.programId } as any)
      .rpc()
      .catch(landed);
    for (let i = 0; i < path.length; i += WALK_STEP) {
      await lc.methods
        .walkStep()
        .accountsStrict({ config, walk: key } as any)
        .remainingAccounts(path.slice(i, i + WALK_STEP).map((r) => ({ pubkey: nodeOf(r), isSigner: false, isWritable: false })))
        .preInstructions(budget())
        .rpc()
        .catch(landed);
    }
    return key;
  };
  // A walk never made has no account: closing it fails, and nothing is lost.
  const closeWalks = async () => {
    for (const w of walks) await lc.methods.closeWalk().accountsStrict({ walk: w, owner: payer } as any).rpc().catch(landed).catch(() => {});
  };

  /** Sends `ixs` as one transaction; through an address lookup table made
   *  for it when they do not fit a plain one. */
  const sendFitting = async (ixs: TransactionInstruction[]): Promise<string> => {
    const { blockhash } = await connection.getLatestBlockhash("confirmed");
    const plain = new Transaction({ feePayer: payer, recentBlockhash: blockhash }).add(...ixs);
    let fits = true;
    try {
      // 64 bytes of signature are added when it is signed.
      fits = plain.serializeMessage().length + 65 <= MAX_TX;
    } catch {
      fits = false;
    }
    if (fits) return provider.sendAndConfirm(plain).catch(landed);

    // Every account the instructions name, but the signer: each then takes
    // one byte in the transaction, not 32.
    const keys = new Map<string, PublicKey>();
    for (const ix of ixs) for (const k of [ix.programId, ...ix.keys.map((m) => m.pubkey)]) if (!k.equals(payer)) keys.set(k.toBase58(), k);
    const [create, table] = AddressLookupTableProgram.createLookupTable({ authority: payer, payer, recentSlot: await connection.getSlot("finalized") });
    const extend = AddressLookupTableProgram.extendLookupTable({ payer, authority: payer, lookupTable: table, addresses: [...keys.values()] });
    await provider.sendAndConfirm(new Transaction().add(create, extend)).catch(landed);
    try {
      // A table's addresses can be used from the slot after they were added.
      let account = null;
      for (let i = 0; i < 30 && !account; i++) {
        await new Promise((r) => setTimeout(r, 1000));
        const got = (await connection.getAddressLookupTable(table, { commitment: "confirmed" })).value;
        if (got && got.state.addresses.length === keys.size && (await connection.getSlot("confirmed")) > Number(got.state.lastExtendedSlot)) account = got;
      }
      if (!account) throw new Error("the proof's address lookup table was not ready in time: try again");
      const fresh = (await connection.getLatestBlockhash("confirmed")).blockhash;
      const message = new TransactionMessage({ payerKey: payer, recentBlockhash: fresh, instructions: ixs }).compileToV0Message([account]);
      const tx = new VersionedTransaction(message);
      if (tx.serialize().length > MAX_TX) throw new Error("the payment's transaction is too large to prove on Solana: it was paid from too many coins");
      return await provider.sendAndConfirm(tx).catch(landed);
    } finally {
      // Deactivated now; its rent can be taken back by closing it later.
      await provider.sendAndConfirm(new Transaction().add(AddressLookupTableProgram.deactivateLookupTable({ lookupTable: table, authority: payer }))).catch(() => {});
    }
  };

  try {
    const top = closedAfter ? payRef : await refAt(highHeight);
    const high: Ref = closedAfter ? { hash: Buffer.from(job.proofBlock.hash), height: highHeight, epochTime: Number(job.proofBlock.epochTime) } : top;
    const walkLow = await walk(payRef, low);
    const walkHigh = await walk(high, payRef);
    // The anchor's parent's epoch time, needed only when the anchor opens an epoch.
    const parentEpochTime = !closedAfter && anchor.height % EPOCH === 0 ? low.epochTime : 0;

    const user = new PublicKey(s.user);
    const mint = s.token ? new PublicKey(s.token) : null;
    const tokenProgram = mint ? (await connection.getAccountInfo(mint))!.owner : null;
    const swap = conv.swapAddress(swapId);
    // A token goes to the user's own account: made first, in a transaction
    // of its own, when missing, so the proof keeps its room for the payment.
    if (mint && !(await connection.getAccountInfo(associatedTokenAccount(user, mint, tokenProgram!)))) {
      await provider.sendAndConfirm(new Transaction().add(createAccountIdempotent(payer, user, mint, tokenProgram!))).catch(landed);
    }
    const raw = bytes(await bitcoin.rawTx(payment.txid));
    const prove: TransactionInstruction = await (conv.program.methods as any)
      .proveMyPayment(raw, payment.vout, arg(payRef), siblings.map((x) => [...bytes(x)]), new BN(index), arg(top), parentEpochTime)
      .accountsStrict({
        swap,
        job: jobKey,
        anchorNode: nodeOf(anchor),
        payNode: nodeOf(payRef),
        walkLow,
        walkHigh,
        user,
        mint,
        escrow: mint ? associatedTokenAccount(swap, mint, tokenProgram!) : null,
        to: mint ? associatedTokenAccount(user, mint, tokenProgram!) : null,
        tokenProgram,
      } as any)
      .instruction();
    // Its walks are closed only once its outcome is known: `landed` has
    // waited for it, or it failed.
    const signature = await sendFitting([...budget(), prove]);
    return { payment, added, signature };
  } finally {
    await closeWalks();
  }
}
