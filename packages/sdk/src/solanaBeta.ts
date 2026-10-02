// BETA on Solana (programs/beta-basket): the same baskets as on EVM
// networks. A part is an SPL token, such as a vault receipt (vETH) or
// wrapped SOL; one whole BETA is 10^9 of its smallest units. Solana has no
// view of a mint's cost, so the SDK prices it as the program does.

import anchor from "@coral-xyz/anchor";
import { Connection, PublicKey, SystemProgram, TransactionInstruction, type AccountMeta } from "@solana/web3.js";

import { IDLS, SOLANA } from "./generated/solana.ts";
import { associatedTokenAccount, type SolanaWallet } from "./solana.ts";

const { AnchorProvider, BN, Program } = anchor;

const TOKEN_PROGRAM = new PublicKey("TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA");
const ASSOCIATED_TOKEN_PROGRAM = new PublicKey("ATokenGPvbdGVxr1b2hvZbsiqW5xWH25efTNsLJA8knL");
const ONE = 1_000_000_000n;
const BPS = 10_000n;

export type SolanaBasketPart = { mint: string; amount: bigint; tokenProgram: string; decimals: number; held: bigint; owed: bigint };
export type SolanaBasket = { address: string; creator: string; id: bigint; feeTo: string; mintFeeBps: number; burnFeeBps: number; beta: string; supply: bigint; parts: SolanaBasketPart[] };
export type SolanaMintQuote = { basket: string; amount: bigint; need: bigint[]; fee: bigint[] };

const divUp = (a: bigint, b: bigint) => (a + b - 1n) / b;
const big = (x: { toString(): string }) => BigInt(x.toString());

/** An instruction that makes `owner`'s account for `mint` unless it exists. */
export function createAccountIdempotent(payer: PublicKey, owner: PublicKey, mint: PublicKey, tokenProgram = TOKEN_PROGRAM): TransactionInstruction {
  const ata = associatedTokenAddress(owner, mint, tokenProgram);
  return new TransactionInstruction({
    programId: ASSOCIATED_TOKEN_PROGRAM,
    keys: [
      { pubkey: payer, isSigner: true, isWritable: true },
      { pubkey: ata, isSigner: false, isWritable: true },
      { pubkey: owner, isSigner: false, isWritable: false },
      { pubkey: mint, isSigner: false, isWritable: false },
      { pubkey: SystemProgram.programId, isSigner: false, isWritable: false },
      { pubkey: tokenProgram, isSigner: false, isWritable: false },
    ],
    data: Buffer.from([1]),
  });
}

function associatedTokenAddress(owner: PublicKey, mint: PublicKey, tokenProgram: PublicKey): PublicKey {
  if (tokenProgram.equals(TOKEN_PROGRAM)) return associatedTokenAccount(owner, mint);
  return PublicKey.findProgramAddressSync([owner.toBuffer(), tokenProgram.toBuffer(), mint.toBuffer()], ASSOCIATED_TOKEN_PROGRAM)[0];
}

export class SolanaBeta {
  readonly connection: Connection;
  readonly program: InstanceType<typeof Program>;
  readonly wallet: SolanaWallet;

  constructor(connection: Connection, wallet: SolanaWallet, programId: string = SOLANA.programs.betaBasket) {
    this.connection = connection;
    this.wallet = wallet;
    const provider = new AnchorProvider(connection, wallet as any, { commitment: "confirmed" });
    this.program = new Program({ ...(IDLS.betaBasket as any), address: programId }, provider);
  }

  private pda(...seeds: Buffer[]): PublicKey {
    return PublicKey.findProgramAddressSync(seeds, this.program.programId)[0];
  }
  basketAddress(creator: PublicKey, id: bigint | number): PublicKey {
    const b = Buffer.alloc(8);
    b.writeBigUInt64LE(BigInt(id));
    return this.pda(Buffer.from("basket"), creator.toBuffer(), b);
  }
  betaMint(basket: PublicKey): PublicKey {
    return this.pda(Buffer.from("beta"), basket.toBuffer());
  }
  private feesRecord(basket: PublicKey, feeTo: PublicKey): PublicKey {
    return this.pda(Buffer.from("fees"), basket.toBuffer(), feeTo.toBuffer());
  }

  /** Creates basket `id` of the wallet, its parts SPL tokens, with what one
   *  whole BETA holds of each, and fees of at most 1% each. */
  async createBasket(options: { id: bigint | number; parts: { mint: string; amount: bigint }[]; mintFeeBps?: number; burnFeeBps?: number }): Promise<{ basket: string; beta: string; signature: string }> {
    const creator = this.wallet.publicKey;
    const basket = this.basketAddress(creator, options.id);
    const parts = options.parts.map((p) => ({ mint: new PublicKey(p.mint), amount: new BN(p.amount.toString()), tokenProgram: PublicKey.default, decimals: 0, powers: 0, owed: new BN(0) }));
    const signature = await this.program.methods
      .createBasket(new BN(BigInt(options.id).toString()), parts, options.mintFeeBps ?? 0, options.burnFeeBps ?? 0)
      .accountsStrict({ basket, beta: this.betaMint(basket), fees: this.feesRecord(basket, creator), creator, tokenProgram: TOKEN_PROGRAM, systemProgram: SystemProgram.programId })
      .remainingAccounts(parts.map((p) => ({ pubkey: p.mint, isSigner: false, isWritable: false })))
      .rpc();
    return { basket: basket.toBase58(), beta: this.betaMint(basket).toBase58(), signature };
  }

  async getBasket(address: string): Promise<SolanaBasket> {
    const key = new PublicKey(address);
    const b: any = await (this.program.account as any).basket.fetch(key);
    const supply = big((await this.connection.getTokenSupply(b.beta)).value.amount);
    const parts: SolanaBasketPart[] = [];
    for (const p of b.parts) {
      const held = associatedTokenAddress(key, p.mint, p.tokenProgram);
      const info = await this.connection.getAccountInfo(held);
      const balance = info ? big((await this.connection.getTokenAccountBalance(held)).value.amount) : 0n;
      parts.push({ mint: p.mint.toBase58(), amount: big(p.amount), tokenProgram: p.tokenProgram.toBase58(), decimals: p.decimals, held: balance - big(p.owed), owed: big(p.owed) });
    }
    return { address, creator: b.creator.toBase58(), id: big(b.id), feeTo: b.feeTo.toBase58(), mintFeeBps: b.mintFeeBps, burnFeeBps: b.burnFeeBps, beta: b.beta.toBase58(), supply, parts };
  }

  /** What minting `amount` BETA takes, by part, as the program prices it:
   *  rounded up, the creator's fee on top, rounded up too. */
  async quoteMint(address: string, amount: bigint): Promise<SolanaMintQuote> {
    const b = await this.getBasket(address);
    if (b.supply === 0n && amount % ONE !== 0n) throw new Error("the first mint of a basket is a whole number of BETA");
    const need = b.parts.map((p) => (b.supply === 0n ? divUp(amount * p.amount, ONE) : divUp(amount * p.held, b.supply)));
    const fee = need.map((n) => divUp(n * BigInt(b.mintFeeBps), BPS));
    return { basket: address, amount, need, fee };
  }

  private async partAccounts(b: SolanaBasket, user: PublicKey): Promise<AccountMeta[]> {
    const basket = new PublicKey(b.address);
    return b.parts.flatMap((p) => {
      const mint = new PublicKey(p.mint);
      const program = new PublicKey(p.tokenProgram);
      return [
        { pubkey: mint, isSigner: false, isWritable: false },
        { pubkey: associatedTokenAddress(basket, mint, program), isSigner: false, isWritable: true },
        { pubkey: associatedTokenAddress(user, mint, program), isSigner: false, isWritable: true },
        { pubkey: program, isSigner: false, isWritable: false },
      ];
    });
  }

  private feesFor(b: SolanaBasket): PublicKey | null {
    return b.feeTo === b.address ? null : this.feesRecord(new PublicKey(b.address), new PublicKey(b.feeTo));
  }

  /** Mints `amount` BETA to the wallet; its account for BETA is made first
   *  if it has none. The parts come from its accounts for them. */
  async mint(address: string, amount: bigint): Promise<{ signature: string }> {
    const b = await this.getBasket(address);
    const user = this.wallet.publicKey;
    const beta = new PublicKey(b.beta);
    const signature = await this.program.methods
      .mint(new BN(amount.toString()))
      .accountsStrict({
        basket: new PublicKey(address),
        fees: this.feesFor(b),
        beta,
        userBeta: associatedTokenAccount(user, beta),
        user,
        tokenProgram: TOKEN_PROGRAM,
        associatedTokenProgram: ASSOCIATED_TOKEN_PROGRAM,
        systemProgram: SystemProgram.programId,
      } as any)
      .preInstructions([createAccountIdempotent(user, user, beta)])
      .remainingAccounts(await this.partAccounts(b, user))
      .rpc();
    return { signature };
  }

  /** Burns `amount` BETA for each part's share; a part whose index is in
   *  `defer` is owed instead, collected later. The wallet's accounts for
   *  the parts are made first if missing. */
  async burn(address: string, amount: bigint, defer: number[] = []): Promise<{ signature: string }> {
    const b = await this.getBasket(address);
    const user = this.wallet.publicKey;
    const beta = new PublicKey(b.beta);
    const bits = defer.reduce((x, i) => x | (1 << i), 0);
    const signature = await this.program.methods
      .burn(new BN(amount.toString()), bits)
      .accountsStrict({
        basket: new PublicKey(address),
        fees: this.feesFor(b),
        beta,
        userBeta: associatedTokenAccount(user, beta),
        user,
        tokenProgram: TOKEN_PROGRAM,
        systemProgram: SystemProgram.programId,
      } as any)
      .preInstructions(b.parts.map((p) => createAccountIdempotent(user, user, new PublicKey(p.mint), new PublicKey(p.tokenProgram))))
      .remainingAccounts(await this.partAccounts(b, user))
      .rpc();
    return { signature };
  }

  /** BETA the wallet holds of a basket. */
  async betaBalance(address: string, owner: PublicKey = this.wallet.publicKey): Promise<bigint> {
    const b: any = await (this.program.account as any).basket.fetch(new PublicKey(address));
    const account = associatedTokenAccount(owner, b.beta);
    if (!(await this.connection.getAccountInfo(account))) return 0n;
    return big((await this.connection.getTokenAccountBalance(account)).value.amount);
  }
}

