// The SDK's Solana vault against a local validator running the production
// builds of the protocol and vault programs (test/helpers/validator.ts).

import { test, before, after } from "node:test";
import assert from "node:assert/strict";
import anchor from "@coral-xyz/anchor";
import { Connection, Keypair, PublicKey, SystemProgram } from "@solana/web3.js";

import { IDLS, SOLANA, SolanaVault } from "../src/index.ts";
import { startValidator, type Local } from "./helpers/validator.ts";

const { AnchorProvider, BN, Program, Wallet } = anchor;
const EVM_VAULT = "0x" + "11".repeat(20);
const ETHEREUM = 1;
const BASE = 3;
let local: Local;
let connection: Connection;
let authority: Keypair;

before(async () => {
  local = await startValidator(18899, { [SOLANA.programs.protocol]: "ipow_protocol", [SOLANA.programs.vault]: "ipow_vault" });
  ({ connection, authority } = local);

  // The set-up an operator of the deployment does once (scripts/init-testnet.ts).
  const provider = new AnchorProvider(connection, new Wallet(authority), { commitment: "confirmed" });
  const pr = new Program(IDLS.protocol as any, provider);
  const vt = new Program(IDLS.vault as any, provider);
  const pda = (p: PublicKey, seeds: Buffer[]) => PublicKey.findProgramAddressSync(seeds, p)[0];
  const loader = new PublicKey("BPFLoaderUpgradeab1e11111111111111111111111");
  await pr.methods.initializeProtocol().accountsStrict({
    protocol: pda(pr.programId, [Buffer.from("protocol")]), vault: pda(pr.programId, [Buffer.from("vault")]), payer: authority.publicKey, systemProgram: SystemProgram.programId,
  }).rpc();
  // Two pairs, so a reader of one is shown not to see the other's.
  for (const peer of [ETHEREUM, BASE]) {
    const config = pda(vt.programId, [Buffer.from("config"), Buffer.from([peer])]);
    await vt.methods.initialize(peer, Array.from(Buffer.from(EVM_VAULT.slice(2), "hex")), new BN(1_000_000), new BN(10_000_000)).accountsStrict({
      config,
      sol: pda(vt.programId, [Buffer.from("asset"), config.toBuffer(), Buffer.from([0, 0, 0, 0])]),
      application: pda(pr.programId, [Buffer.from("application"), config.toBuffer()]),
      payer: authority.publicKey,
      programData: pda(loader, [vt.programId.toBuffer()]),
      protocolProgram: pr.programId,
      systemProgram: SystemProgram.programId,
    }).rpc();
  }
});

after(() => local?.stop());

test("locks SOL for its receipt on Ethereum, in record units, and follows it", async () => {
  const vault = new SolanaVault(connection, ETHEREUM, new Wallet(authority));
  const assets = await vault.assets();
  assert.deepEqual(assets.map((a) => [a.number, a.mint, a.decimals, a.unit]), [[0, null, 9, 1n]]);
  const user = "0x" + "22".repeat(20);
  const before = await connection.getBalance(vault.config);
  const { lockId } = await vault.lock({ recipient: user, amount: 10_000_000n, fee: 5_000n });
  assert.equal(lockId, 1n);
  assert.equal((await connection.getBalance(vault.config)) - before, 10_005_000);
  const l = await vault.getLock(lockId);
  assert.deepEqual([l.stage, l.owner, l.amount, l.fee, l.recipient.toLowerCase()], ["Locked", authority.publicKey.toBase58(), 10_000_000n, 5_000n, user]);
  // A lock made on Ethereum has no mark here until a claim or attest.
  assert.equal(await vault.evmLockMark(1), null);
  assert.equal(await vault.receiptBalance(authority.publicKey.toBase58(), 0), 0n);

  // The owner's locks in this pair, newest first; one in the Base pair, with
  // the same id, is not among them. Nobody else has any, and no burn was paid.
  const second = await vault.lock({ recipient: user, amount: 2_000_000n });
  const base = await new SolanaVault(connection, BASE, new Wallet(authority)).lock({ recipient: user, amount: 3_000_000n });
  assert.equal(base.lockId, 1n);
  const me = authority.publicKey.toBase58();
  assert.deepEqual((await vault.locksOf(me)).map((x) => [x.lockId, x.amount]), [[second.lockId, 2_000_000n], [lockId, 10_000_000n]]);
  assert.deepEqual((await vault.locksOf(me, 1)).map((x) => x.lockId), [second.lockId]);
  assert.deepEqual(await vault.locksOf(Keypair.generate().publicKey.toBase58()), []);
  assert.deepEqual(await vault.burnsOf(me), []);
  assert.equal(await vault.evmBurnPaid(1), false);
});

test("refuses to burn a receipt that does not exist yet", async () => {
  const vault = new SolanaVault(connection, ETHEREUM, new Wallet(authority));
  await assert.rejects(vault.burn({ asset: 0, to: "0x" + "22".repeat(20), amount: 1n }));
});
