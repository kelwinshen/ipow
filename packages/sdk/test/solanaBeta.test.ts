// The SDK's BETA on Solana against a local validator running the
// production build of the basket program, with two SPL tokens as parts.

import { test, before, after } from "node:test";
import assert from "node:assert/strict";
import anchor from "@coral-xyz/anchor";
import { Keypair, PublicKey, SystemProgram, Transaction, TransactionInstruction, sendAndConfirmTransaction } from "@solana/web3.js";

import { SOLANA, SolanaBeta, WRAPPED_SOL, associatedTokenAccount, createAccountIdempotent } from "../src/index.ts";
import { startValidator, type Local } from "./helpers/validator.ts";

const { Wallet } = anchor;
const TOKEN_PROGRAM = new PublicKey("TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA");
const ONE = 1_000_000_000n;
let local: Local;

/** A new SPL token of `decimals`, `amount` of it minted to `owner`. */
async function token(owner: Keypair, decimals: number, amount: bigint): Promise<PublicKey> {
  const { connection } = local;
  const mint = Keypair.generate();
  const rent = await connection.getMinimumBalanceForRentExemption(82);
  const init = Buffer.alloc(35);
  init[0] = 20; // InitializeMint2
  init[1] = decimals;
  owner.publicKey.toBuffer().copy(init, 2);
  const mintTo = Buffer.alloc(9);
  mintTo[0] = 7; // MintTo
  mintTo.writeBigUInt64LE(amount, 1);
  const ata = associatedTokenAccount(owner.publicKey, mint.publicKey);
  const tx = new Transaction().add(
    SystemProgram.createAccount({ fromPubkey: owner.publicKey, newAccountPubkey: mint.publicKey, lamports: rent, space: 82, programId: TOKEN_PROGRAM }),
    new TransactionInstruction({ programId: TOKEN_PROGRAM, keys: [{ pubkey: mint.publicKey, isSigner: false, isWritable: true }], data: init }),
    createAccountIdempotent(owner.publicKey, owner.publicKey, mint.publicKey),
    new TransactionInstruction({
      programId: TOKEN_PROGRAM,
      keys: [
        { pubkey: mint.publicKey, isSigner: false, isWritable: true },
        { pubkey: ata, isSigner: false, isWritable: true },
        { pubkey: owner.publicKey, isSigner: true, isWritable: false },
      ],
      data: mintTo,
    })
  );
  await sendAndConfirmTransaction(connection, tx, [owner, mint]);
  return mint.publicKey;
}

const balance = async (owner: PublicKey, mint: PublicKey) =>
  BigInt((await local.connection.getTokenAccountBalance(associatedTokenAccount(owner, mint))).value.amount);

before(async () => {
  local = await startValidator(18999, { [SOLANA.programs.betaBasket]: "beta_basket" });
});
after(() => local?.stop());

test("makes a BETA basket of two tokens, prices a mint as the program does, mints and burns", async () => {
  const user = local.authority;
  const veth = await token(user, 9, 10n ** 12n); // standing in for vETH, the vault's receipt
  const usd = await token(user, 6, 10n ** 12n);
  const beta = new SolanaBeta(local.connection, new Wallet(user));
  // One BETA holds 0.001 vETH and 2 USD; 0.5% to mint and to burn.
  const { basket } = await beta.createBasket({ id: 1, parts: [{ mint: veth.toBase58(), amount: 1_000_000n }, { mint: usd.toBase58(), amount: 2_000_000n }], mintFeeBps: 50, burnFeeBps: 50 });

  // A first mint is a whole number of BETA.
  await assert.rejects(beta.quoteMint(basket, ONE + 1n), /whole number/);
  const q = await beta.quoteMint(basket, 3n * ONE);
  assert.deepEqual([q.need, q.fee], [[3_000_000n, 6_000_000n], [15_000n, 30_000n]]);
  const before = [await balance(user.publicKey, veth), await balance(user.publicKey, usd)];
  await beta.mint(basket, 3n * ONE);
  // What it took is exactly what the quote said.
  assert.deepEqual([before[0] - (await balance(user.publicKey, veth)), before[1] - (await balance(user.publicKey, usd))], [q.need[0] + q.fee[0], q.need[1] + q.fee[1]]);
  let b = await beta.getBasket(basket);
  assert.deepEqual([b.supply, b.parts.map((p) => p.held), await beta.betaBalance(basket)], [3n * ONE, [3_000_000n, 6_000_000n], 3n * ONE]);

  // A second mint is priced on what the basket holds per BETA.
  const q2 = await beta.quoteMint(basket, ONE / 2n);
  assert.deepEqual(q2.need, [1_500_000n / 3n, 1_000_000n]);

  // Burn 1 BETA: each part's share, less the 0.5% fee.
  const usdBefore = await balance(user.publicKey, usd);
  await beta.burn(basket, ONE);
  b = await beta.getBasket(basket);
  assert.equal(b.supply, 2n * ONE);
  assert.equal((await balance(user.publicKey, usd)) - usdBefore, 2_000_000n - 10_000n);
});

test("mints a basket with a wrapped SOL part from plain SOL, defers a part in a burn, collects it and the fees, and unwraps", async () => {
  const user = local.authority;
  const { connection } = local;
  const veth = await token(user, 9, 10n ** 12n);
  const beta = new SolanaBeta(connection, new Wallet(user));
  // One BETA holds 0.01 SOL and 0.001 vETH; 1% to mint and to burn.
  const { basket } = await beta.createBasket({ id: 2, parts: [{ mint: WRAPPED_SOL, amount: 10_000_000n }, { mint: veth.toBase58(), amount: 1_000_000n }], mintFeeBps: 100, burnFeeBps: 100 });
  const wsol = new PublicKey(WRAPPED_SOL);
  assert.equal(await beta.tokenBalance(user.publicKey, wsol), 0n);

  // No wrapped SOL held: the mint wraps what it takes.
  const q = await beta.quoteMint(basket, 2n * ONE);
  assert.deepEqual([q.need[0], q.fee[0]], [20_000_000n, 200_000n]);
  await beta.mint(basket, 2n * ONE);
  assert.equal(await beta.tokenBalance(user.publicKey, wsol), 0n);
  assert.equal(await beta.betaBalance(basket), 2n * ONE);
  // The creator's fees, by part.
  assert.deepEqual(await beta.feesOf(basket), [200_000n, 20_000n, 0n, 0n, 0n, 0n, 0n, 0n]);

  // Burn 1 BETA with vETH deferred: SOL paid now (less 1%), vETH owed.
  await beta.burn(basket, ONE, [1]);
  assert.equal(await beta.tokenBalance(user.publicKey, wsol), 10_000_000n - 100_000n);
  assert.equal(await beta.owed(basket, 1), 1_000_000n);
  assert.equal(await beta.owed(basket, 0), 0n);
  const vethBefore = await balance(user.publicKey, veth);
  await beta.collectOwed(basket, 1);
  assert.equal((await balance(user.publicKey, veth)) - vethBefore, 1_000_000n - 10_000n);
  assert.equal(await beta.owed(basket, 1), 0n);

  // The creator collects its SOL fees: the mint's and the burn's.
  const fees = (await beta.feesOf(basket))[0];
  assert.equal(fees, 200_000n + 100_000n);
  await beta.collectFees(basket, 0);
  assert.equal(await beta.tokenBalance(user.publicKey, wsol), 10_000_000n - 100_000n + fees);
  assert.equal((await beta.feesOf(basket))[0], 0n);

  // Unwrapping closes the account and returns its SOL.
  const solBefore = await connection.getBalance(user.publicKey);
  await beta.unwrapSol();
  assert.equal(await beta.tokenBalance(user.publicKey, wsol), 0n);
  assert.ok((await connection.getBalance(user.publicKey)) > solBefore + Number(10_000_000n - 100_000n + fees) - 10_000);
});

