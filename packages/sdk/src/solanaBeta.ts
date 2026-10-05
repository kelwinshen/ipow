// BETA on Solana (programs/beta-basket): the same baskets as on EVM
// networks. A part is an SPL token, such as a vault receipt (vETH) or
// wrapped SOL; one whole BETA is 10^9 of its smallest units. Solana has no
// view of a mint's cost, so the SDK prices it as the program does.

// Imported, not global: in a browser this is the `buffer` package.
import { Buffer } from "buffer";
import { AnchorProvider, BN, Program } from "@coral-xyz/anchor";
import { Connection, PublicKey, SystemProgram, Transaction, TransactionInstruction, type AccountMeta } from "@solana/web3.js";

import { IDLS, SOLANA } from "./generated/solana.ts";
import { associatedTokenAccount, type SolanaWallet } from "./solana.ts";


const TOKEN_PROGRAM = new PublicKey("TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA");
const ASSOCIATED_TOKEN_PROGRAM = new PublicKey("ATokenGPvbdGVxr1b2hvZbsiqW5xWH25efTNsLJA8knL");
const ONE = 1_000_000_000n;
/** Wrapped SOL: a part SOL itself stands in for, wrapped as it is minted. */
export const WRAPPED_SOL = "So11111111111111111111111111111111111111112";
const BPS = 10_000n;

export type SolanaBasketPart = {
  mint: string;
  amount: bigint;
  tokenProgram: string;
  decimals: number;
  /** False while the part is a receipt the vault has not made yet (E5):
   *  nothing can be minted until the asset is bridged here. */
  made: boolean;
  held: bigint;
  owed: bigint;
};
export type SolanaBasket = {
  address: string;
  creator: string;
  id: bigint;
  feeTo: string;
  mintFeeBps: number;
  burnFeeBps: number;
  beta: string;
  /** The token's name, symbol and metadata URI, the creator's (E4), from
   *  its Token Metadata account; "BETA", "BETA" and "" for a basket of the
   *  program's first build, which had none. */
  name: string;
  symbol: string;
  uri: string;
  supply: bigint;
  parts: SolanaBasketPart[];
};
/** The limits of a basket's naming on Solana (Token Metadata's), in bytes. */
export const SOLANA_BASKET_NAMING = { name: 32, symbol: 10, uri: 200 } as const;
/** Metaplex Token Metadata, where a token's name, symbol and image are read from. */
export const METADATA_PROGRAM = new PublicKey("metaqbxxUerdq28cj1RbAWkYQm3ybzjb6a8bt518x1s");
const TOKEN_2022_PROGRAM = new PublicKey("TokenzQdBNbLqP5VEhdkAS6EPFLC1PHnBqCXEpPxuEb");
export const metadataAddress = (mint: PublicKey) => PublicKey.findProgramAddressSync([Buffer.from("metadata"), METADATA_PROGRAM.toBuffer(), mint.toBuffer()], METADATA_PROGRAM)[0];
/** The name, symbol and URI a Token Metadata account holds (fixed-width, NUL-padded). */
export function parseMetadata(data: Buffer): { name: string; symbol: string; uri: string } {
  let at = 1 + 32 + 32;
  const str = (width: number) => {
    const len = data.readUInt32LE(at);
    const v = data.subarray(at + 4, at + 4 + len).toString("utf8").replace(/\0+$/, "");
    at += 4 + width;
    return v;
  };
  return { name: str(32), symbol: str(10), uri: str(200) };
}
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
  private owedRecord(basket: PublicKey, owner: PublicKey, index: number): PublicKey {
    return this.pda(Buffer.from("owed"), basket.toBuffer(), owner.toBuffer(), Buffer.from([index]));
  }
  private feesRecord(basket: PublicKey, feeTo: PublicKey): PublicKey {
    return this.pda(Buffer.from("fees"), basket.toBuffer(), feeTo.toBuffer());
  }

  /** Creates basket `id` of the wallet, its parts SPL tokens, with what one
   *  whole BETA holds of each, and fees of at most 1% each. */
  /** Creates basket `id` of the wallet: its token's name, symbol and
   *  metadata URI (in a Token Metadata account, fixed for good), its parts
   *  and fees. */
  async createBasket(options: {
    id: bigint | number;
    name: string;
    symbol: string;
    uri?: string;
    /** A part is a mint here; `pending` names a receipt the vault has not
     *  made yet (E5), whose mint account may not exist. Any other mint
     *  must exist: a typed address that does not would otherwise become a
     *  basket that can never mint. */
    parts: { mint: string; amount: bigint; pending?: boolean }[];
    mintFeeBps?: number;
    burnFeeBps?: number;
  }): Promise<{ basket: string; beta: string; signature: string }> {
    const creator = this.wallet.publicKey;
    const basket = this.basketAddress(creator, options.id);
    const beta = this.betaMint(basket);
    for (const p of options.parts) {
      if (p.pending) continue;
      if ((await this.connection.getAccountInfo(new PublicKey(p.mint))) === null) throw new Error(`no mint at ${p.mint}; a receipt not made yet is named with pending: true`);
    }
    const parts = options.parts.map((p) => ({ mint: new PublicKey(p.mint), amount: new BN(p.amount.toString()), tokenProgram: PublicKey.default, decimals: 0, powers: 0, owed: new BN(0) }));
    const signature = await this.program.methods
      .createBasket(new BN(BigInt(options.id).toString()), options.name, options.symbol, options.uri ?? "", parts, options.mintFeeBps ?? 0, options.burnFeeBps ?? 0)
      .accountsStrict({ basket, beta, fees: this.feesRecord(basket, creator), metadata: metadataAddress(beta), creator, tokenProgram: TOKEN_PROGRAM, systemProgram: SystemProgram.programId, metadataProgram: METADATA_PROGRAM })
      .remainingAccounts(parts.map((p) => ({ pubkey: p.mint, isSigner: false, isWritable: false })))
      .rpc();
    return { basket: basket.toBase58(), beta: this.betaMint(basket).toBase58(), signature };
  }

  async getBasket(address: string): Promise<SolanaBasket> {
    const key = new PublicKey(address);
    const b: any = await (this.program.account as any).basket.fetch(key);
    const supply = big((await this.connection.getTokenSupply(b.beta)).value.amount);
    const meta = await this.connection.getAccountInfo(metadataAddress(b.beta));
    const { name, symbol, uri } = meta ? parseMetadata(meta.data) : { name: "BETA", symbol: "BETA", uri: "" };
    const parts: SolanaBasketPart[] = [];
    for (const p of b.parts) {
      // A pending part (E5) has no token program recorded yet: once its mint
      // exists, the chain says which program and decimals, and the first
      // mint records them.
      let tokenProgram: PublicKey = p.tokenProgram;
      let decimals: number = p.decimals;
      let made = !tokenProgram.equals(PublicKey.default);
      if (!made) {
        const mintInfo = await this.connection.getAccountInfo(p.mint);
        if (mintInfo && (mintInfo.owner.equals(TOKEN_PROGRAM) || mintInfo.owner.equals(TOKEN_2022_PROGRAM))) {
          tokenProgram = mintInfo.owner;
          decimals = (await this.connection.getTokenSupply(p.mint)).value.decimals;
          made = true;
        }
      }
      let balance = 0n;
      if (made) {
        const held = associatedTokenAddress(key, p.mint, tokenProgram);
        const info = await this.connection.getAccountInfo(held);
        balance = info ? big((await this.connection.getTokenAccountBalance(held)).value.amount) : 0n;
      }
      parts.push({ mint: p.mint.toBase58(), amount: big(p.amount), tokenProgram: tokenProgram.toBase58(), decimals, made, held: balance - big(p.owed), owed: big(p.owed) });
    }
    return { address, creator: b.creator.toBase58(), id: big(b.id), feeTo: b.feeTo.toBase58(), mintFeeBps: b.mintFeeBps, burnFeeBps: b.burnFeeBps, beta: b.beta.toBase58(), name, symbol, uri, supply, parts };
  }

  /** What minting `amount` BETA takes, by part, as the program prices it:
   *  rounded up, the creator's fee on top, rounded up too. */
  async quoteMint(address: string, amount: bigint): Promise<SolanaMintQuote> {
    const b = await this.getBasket(address);
    const unmade = b.parts.find((p) => !p.made);
    if (unmade) throw new Error(`part ${unmade.mint} is a receipt the vault has not made yet: bridge the asset here first`);
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
   *  if it has none. The parts come from its accounts for them; a wrapped
   *  SOL part short of what the mint takes is wrapped from the wallet's SOL
   *  first, in the same transaction. */
  async mint(address: string, amount: bigint): Promise<{ signature: string }> {
    const b = await this.getBasket(address);
    const user = this.wallet.publicKey;
    const beta = new PublicKey(b.beta);
    const wrap: TransactionInstruction[] = [];
    const sol = b.parts.findIndex((p) => p.mint === WRAPPED_SOL);
    if (sol >= 0) {
      const q = await this.quoteMint(address, amount);
      const take = q.need[sol] + q.fee[sol];
      const short = take - (await this.tokenBalance(user, new PublicKey(WRAPPED_SOL)));
      if (short > 0n) wrap.push(...wrapSolInstructions(user, short));
    }
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
      .preInstructions([...wrap, createAccountIdempotent(user, user, beta)])
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
      // After every part's accounts, the wallet's owed record of each
      // deferred part, in order: the program opens one that is missing.
      .remainingAccounts([
        ...(await this.partAccounts(b, user)),
        ...b.parts.flatMap((_, i) => (bits & (1 << i) ? [{ pubkey: this.owedRecord(new PublicKey(address), user, i), isSigner: false, isWritable: true }] : [])),
      ])
      .rpc();
    return { signature };
  }

  /** What `owner` is owed of part `index`, deferred in a burn: paid, less
   *  the burn fee, by `collectOwed`. */
  async owed(address: string, index: number, owner: PublicKey = this.wallet.publicKey): Promise<bigint> {
    const at = this.owedRecord(new PublicKey(address), owner, index);
    if (!(await this.connection.getAccountInfo(at))) return 0n;
    return big(((await (this.program.account as any).owed.fetch(at)) as any).amount);
  }

  /** The fees `receiver` can collect, by part. */
  async feesOf(address: string, receiver: PublicKey = this.wallet.publicKey): Promise<bigint[]> {
    const at = this.feesRecord(new PublicKey(address), receiver);
    if (!(await this.connection.getAccountInfo(at))) return [];
    return ((await (this.program.account as any).fees.fetch(at)) as any).amounts.map(big);
  }

  /** The accounts of part `index` alone, as collecting takes them. */
  private async partAccountsOf(b: SolanaBasket, index: number, user: PublicKey): Promise<AccountMeta[]> {
    return (await this.partAccounts(b, user)).slice(4 * index, 4 * index + 4);
  }

  /** Collects what the wallet is owed of part `index`, less the burn fee;
   *  its account for the part is made first if missing. */
  async collectOwed(address: string, index: number): Promise<{ signature: string }> {
    const b = await this.getBasket(address);
    const user = this.wallet.publicKey;
    const part = b.parts[index];
    if (!part) throw new Error(`no part ${index}`);
    const basket = new PublicKey(address);
    const signature = await this.program.methods
      .collectOwed(index)
      .accountsStrict({
        basket,
        owed: this.owedRecord(basket, user, index),
        fees: this.feesFor(b),
        user,
      } as any)
      .preInstructions([createAccountIdempotent(user, user, new PublicKey(part.mint), new PublicKey(part.tokenProgram))])
      .remainingAccounts(await this.partAccountsOf(b, index, user))
      .rpc();
    return { signature };
  }

  /** Collects the wallet's fees of part `index`, as a receiver of the
   *  basket's fees, to its own account for the part. */
  async collectFees(address: string, index: number): Promise<{ signature: string }> {
    const b = await this.getBasket(address);
    const user = this.wallet.publicKey;
    const part = b.parts[index];
    if (!part) throw new Error(`no part ${index}`);
    const basket = new PublicKey(address);
    const signature = await this.program.methods
      .collectFees(index)
      .accountsStrict({ basket, fees: this.feesRecord(basket, user), receiver: user })
      .preInstructions([createAccountIdempotent(user, user, new PublicKey(part.mint), new PublicKey(part.tokenProgram))])
      .remainingAccounts(await this.partAccountsOf(b, index, user))
      .rpc();
    return { signature };
  }

  /** What `owner` holds of a token, 0 without an account for it. */
  async tokenBalance(owner: PublicKey, mint: PublicKey): Promise<bigint> {
    const account = associatedTokenAccount(owner, mint);
    if (!(await this.connection.getAccountInfo(account))) return 0n;
    return big((await this.connection.getTokenAccountBalance(account)).value.amount);
  }

  /** Turns all the wallet's wrapped SOL back into SOL, closing its account
   *  for it. */
  async unwrapSol(): Promise<{ signature: string }> {
    const user = this.wallet.publicKey;
    const account = associatedTokenAccount(user, new PublicKey(WRAPPED_SOL));
    const tx = new Transaction().add(
      new TransactionInstruction({
        programId: TOKEN_PROGRAM,
        keys: [
          { pubkey: account, isSigner: false, isWritable: true },
          { pubkey: user, isSigner: false, isWritable: true },
          { pubkey: user, isSigner: true, isWritable: false },
        ],
        // The token program's CloseAccount.
        data: Buffer.from([9]),
      })
    );
    const signature = await (this.program.provider as AnchorProvider).sendAndConfirm(tx);
    return { signature };
  }

  /** Every basket of this program: their addresses, newest id first. */
  async allBaskets(): Promise<string[]> {
    const all = await (this.program.account as any).basket.all();
    return all
      .sort((a: any, b: any) => (big(b.account.id) > big(a.account.id) ? 1 : -1))
      .map((a: any) => (a.publicKey as PublicKey).toBase58());
  }

  /** BETA the wallet holds of a basket. */
  async betaBalance(address: string, owner: PublicKey = this.wallet.publicKey): Promise<bigint> {
    const b: any = await (this.program.account as any).basket.fetch(new PublicKey(address));
    const account = associatedTokenAccount(owner, b.beta);
    if (!(await this.connection.getAccountInfo(account))) return 0n;
    return big((await this.connection.getTokenAccountBalance(account)).value.amount);
  }
}

/** Instructions that wrap `lamports` of the wallet's SOL into its wrapped
 *  SOL account, made first if missing. */
export function wrapSolInstructions(owner: PublicKey, lamports: bigint): TransactionInstruction[] {
  const mint = new PublicKey(WRAPPED_SOL);
  const account = associatedTokenAccount(owner, mint);
  return [
    createAccountIdempotent(owner, owner, mint),
    SystemProgram.transfer({ fromPubkey: owner, toPubkey: account, lamports }),
    // The token program's SyncNative: the account's lamports become its balance.
    new TransactionInstruction({ programId: TOKEN_PROGRAM, keys: [{ pubkey: account, isSigner: false, isWritable: true }], data: Buffer.from([17]) }),
  ];
}
