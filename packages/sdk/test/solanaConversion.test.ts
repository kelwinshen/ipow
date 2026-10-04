// The SDK's Conversion on Solana against a local validator running the
// production builds of the protocol and Conversion: the swaps a user opens,
// and the tunnel's legs (a sell held to a window, a buy for someone else).

import { test, before, after } from "node:test";
import assert from "node:assert/strict";
import anchor from "@coral-xyz/anchor";
import { Keypair, PublicKey, SystemProgram } from "@solana/web3.js";

import { IDLS, SOLANA, SolanaConversion, quoteSolanaSwap } from "../src/index.ts";
import { startValidator, type Local } from "./helpers/validator.ts";

const { AnchorProvider, Program, Wallet } = anchor;
const SOL = 1_000_000_000n;
// A Bitcoin mainnet address (BIP173's example) and its script.
const BTC = "bc1qw508d6qejxtdg4y5r3zarvary0c5xw7kv8f3t4";
let local: Local;

before(async () => {
  local = await startValidator(18799, {
    [SOLANA.programs.lightClient]: "ipow_light_client",
    [SOLANA.programs.protocol]: "ipow_protocol",
    [SOLANA.programs.conversion]: "conversion",
  });
  const { connection, authority } = local;
  const provider = new AnchorProvider(connection, new Wallet(authority), { commitment: "confirmed" });
  const pr = new Program(IDLS.protocol as any, provider);
  const cv = new Program(IDLS.conversion as any, provider);
  const pda = (p: PublicKey, seeds: Buffer[]) => PublicKey.findProgramAddressSync(seeds, p)[0];
  await pr.methods.initializeProtocol().accountsStrict({
    protocol: pda(pr.programId, [Buffer.from("protocol")]), vault: pda(pr.programId, [Buffer.from("vault")]), payer: authority.publicKey, systemProgram: SystemProgram.programId,
  }).rpc();
  const config = pda(cv.programId, [Buffer.from("config")]);
  await cv.methods.initialize().accountsStrict({
    config,
    application: pda(pr.programId, [Buffer.from("application"), config.toBuffer()]),
    payer: authority.publicKey,
    protocolProgram: pr.programId,
    systemProgram: SystemProgram.programId,
  }).rpc();
});

after(() => local?.stop());

test("prices a job as the protocol does", () => {
  const q = quoteSolanaSwap(6);
  // ((25 + 6 - 1) + 20) x 5,000 x 1.5; escrow 5 x that, at 0.5%.
  assert.equal(q.commitmentFee, 375_000n);
  assert.equal(q.escrowFee, 9_375n);
  assert.equal(q.paid, 375_000n + 9_375n + 375_000n);
});

test("opens sells and buys, a tunnel's legs too, and reads them back", async () => {
  const { connection, authority } = local;
  const conv = new SolanaConversion(connection, new Wallet(authority));
  const me = authority.publicKey.toBase58();

  const sold = await conv.sell({ lamports: SOL / 10n, sats: 50_000n, to: BTC });
  let s = await conv.getSwap(sold.swapId);
  assert.deepEqual([s.side, s.state, s.stage, s.user, s.amount, s.sats, s.address, s.payWindow], ["Sell", "Open", "Auction", me, SOL / 10n, 50_000n, BTC, null]);

  const leg = await conv.sellInWindow({ lamports: SOL / 10n, sats: 50_000n, to: BTC, window: { first: 900_001, last: 900_012 } });
  s = await conv.getSwap(leg.swapId);
  assert.deepEqual(s.payWindow, { first: 900_001, last: 900_012 });

  const bought = await conv.buy({ lamports: SOL / 10n, sats: 50_000n });
  s = await conv.getSwap(bought.swapId);
  assert.deepEqual([s.side, s.state, s.user, s.address, s.payBlocks], ["Buy", "Open", me, null, null]);

  // A buy opened for someone else: theirs, though the signer paid.
  const other = Keypair.generate().publicKey.toBase58();
  const forOther = await conv.buy({ lamports: SOL / 10n, sats: 50_000n, recipient: other });
  assert.equal((await conv.getSwap(forOther.swapId)).user, other);

  // The signer's swaps, newest first; the other's buy is theirs.
  assert.deepEqual((await conv.swapsOf(me)).map((x) => x.id), [bought.swapId, leg.swapId, sold.swapId]);
  assert.deepEqual((await conv.swapsOf(other)).map((x) => x.id), [forOther.swapId]);
});
