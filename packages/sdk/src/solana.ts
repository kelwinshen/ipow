// The vault on Solana (spec section 11): one program serves every pair,
// each pair under its config account ["config", <peer's number>] (D132).
// Lock SOL or a token for its receipt on the EVM network of the pair, burn a
// receipt of that network's asset for the asset back, and see whether a lock
// made there had its receipt issued here. Amounts are in record units.

import anchor from "@coral-xyz/anchor";
import { Connection, Keypair, PublicKey, SystemProgram, Transaction, VersionedTransaction } from "@solana/web3.js";
import { getAddress } from "ethers";

import { IDLS, SOLANA } from "./generated/solana.ts";

const { AnchorProvider, BN, Program } = anchor;

// From the programs' IDLs.
const TOKEN_PROGRAM = new PublicKey("TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA");
const ASSOCIATED_TOKEN_PROGRAM = new PublicKey("ATokenGPvbdGVxr1b2hvZbsiqW5xWH25efTNsLJA8knL");

/** What a wallet must do to send: Anchor's wallet shape, which browser
 *  wallets' adapters also give. */
export type SolanaWallet = {
  publicKey: PublicKey;
  signTransaction<T extends Transaction | VersionedTransaction>(tx: T): Promise<T>;
  signAllTransactions<T extends Transaction | VersionedTransaction>(txs: T[]): Promise<T[]>;
};

export type SolanaAsset = { number: number; mint: string | null; decimals: number; recordDecimals: number; unit: bigint };

export type SolanaLockState = {
  lockId: bigint;
  stage: "Locked" | "Carried" | "Returned";
  owner: string;
  asset: number;
  amount: bigint;
  fee: bigint;
  fastFee: bigint;
  lockedAt: number;
  /** The EVM address it is for. */
  recipient: string;
};

/** Whether a lock made on the EVM network had its receipt issued here. */
export type LockMark = { issued: boolean; givenUp: boolean; attests: number; settled: number };

const u32 = (n: number) => {
  const b = Buffer.alloc(4);
  b.writeUInt32LE(n);
  return b;
};
const u64 = (n: bigint | number) => {
  const b = Buffer.alloc(8);
  b.writeBigUInt64LE(BigInt(n));
  return b;
};
const big = (x: { toString(): string }) => BigInt(x.toString());

/** A wallet that cannot sign, for reading. */
function readOnly(): SolanaWallet {
  const k = Keypair.generate();
  return {
    publicKey: k.publicKey,
    signTransaction: async () => {
      throw new Error("a read-only wallet");
    },
    signAllTransactions: async () => {
      throw new Error("a read-only wallet");
    },
  };
}

export class SolanaVault {
  readonly connection: Connection;
  readonly peer: number;
  readonly program: InstanceType<typeof Program>;
  readonly config: PublicKey;
  readonly protocol: InstanceType<typeof Program>;

  /** The vault's pair with EVM network number `peer` (D133). Programs can
   *  be given for a local test; by default, those of Solana devnet. */
  constructor(connection: Connection, peer: number, wallet: SolanaWallet = readOnly(), programs: { vault?: string; protocol?: string } = {}) {
    this.connection = connection;
    this.peer = peer;
    const provider = new AnchorProvider(connection, wallet as any, { commitment: "confirmed" });
    this.program = new Program({ ...(IDLS.vault as any), address: programs.vault ?? SOLANA.programs.vault }, provider);
    this.protocol = new Program({ ...(IDLS.protocol as any), address: programs.protocol ?? SOLANA.programs.protocol }, provider);
    this.config = this.pda([Buffer.from("config"), Buffer.from([peer])]);
  }

  private pda(seeds: Buffer[]): PublicKey {
    return PublicKey.findProgramAddressSync(seeds, this.program.programId)[0];
  }
  /** An account of this pair: its first seed, then the config, then the rest. */
  private at(first: string, ...rest: Buffer[]): PublicKey {
    return this.pda([Buffer.from(first), this.config.toBuffer(), ...rest]);
  }
  private get accounts(): any {
    return this.program.account as any;
  }

  async assets(): Promise<SolanaAsset[]> {
    const c = await this.accounts.config.fetch(this.config);
    const out: SolanaAsset[] = [];
    for (let i = 0; i < Number(c.assetCount); i++) {
      const a = await this.accounts.homeAsset.fetch(this.at("asset", u32(i)));
      const native = a.mint.equals(PublicKey.default);
      out.push({ number: i, mint: native ? null : a.mint.toBase58(), decimals: a.decimals, recordDecimals: a.recordDecimals, unit: big(a.unit) });
    }
    return out;
  }

  /** Locks `amount` of asset `asset` (in its own units: lamports for SOL)
   *  for its receipt on the EVM network, to `recipient` there; the fee and
   *  fast fee in the same units. SOL only for now: a token lock needs its
   *  token accounts, not yet built here. */
  async lock(options: { asset?: number; recipient: string; amount: bigint; fee?: bigint; fastFee?: bigint }): Promise<{ lockId: bigint; signature: string }> {
    const assetNumber = options.asset ?? 0;
    const asset = (await this.assets())[assetNumber];
    if (!asset) throw new Error(`no asset ${assetNumber}`);
    if (asset.mint) throw new Error("a token lock on Solana is not built in the SDK yet");
    const record = (x: bigint, what: string) => {
      if (x % asset.unit !== 0n) throw new Error(`${what} must be a whole number of record units (${asset.unit} each)`);
      return x / asset.unit;
    };
    const amount = record(options.amount, "the amount");
    if (amount === 0n) throw new Error("an amount above zero");
    const c = await this.accounts.config.fetch(this.config);
    const lockId = big(c.lockCount) + 1n;
    const recipient = Buffer.from(getAddress(options.recipient).slice(2).padStart(64, "0"), "hex");
    const signature = await this.program.methods
      .lock(assetNumber, Array.from(recipient), new BN(amount.toString()), new BN(record(options.fee ?? 0n, "the fee").toString()), new BN(record(options.fastFee ?? 0n, "the fast fee").toString()))
      .accountsStrict({
        config: this.config,
        homeAsset: this.at("asset", u32(assetNumber)),
        lock: this.at("home_lock", u64(lockId)),
        from: null,
        tokens: null,
        mint: null,
        tokenProgram: null,
        user: this.program.provider.publicKey!,
        systemProgram: SystemProgram.programId,
        // The token accounts are left out for SOL: Anchor takes null for an
        // optional account, though its types do not say so.
      } as any)
      .rpc();
    return { lockId, signature };
  }

  async getLock(lockId: bigint | number): Promise<SolanaLockState> {
    const l = await this.accounts.homeLock.fetch(this.at("home_lock", u64(lockId)));
    return {
      lockId: BigInt(lockId),
      stage: l.returned ? "Returned" : l.feePaid ? "Carried" : "Locked",
      owner: l.owner.toBase58(),
      asset: l.asset,
      amount: big(l.amount),
      fee: big(l.fee),
      fastFee: big(l.fastFee),
      lockedAt: Number(l.lockedAt),
      recipient: getAddress("0x" + Buffer.from(l.recipient).subarray(12).toString("hex")),
    };
  }

  /** The receipt here of the EVM network's asset `asset` (its number there). */
  receiptMint(asset: number): PublicKey {
    return this.at("receipt", u32(asset));
  }

  /** Receipts of that asset `owner` holds here, in record units; 0 with no
   *  account for them. */
  async receiptBalance(owner: string, asset: number): Promise<bigint> {
    const account = associatedTokenAccount(new PublicKey(owner), this.receiptMint(asset));
    const info = await this.connection.getAccountInfo(account);
    if (!info) return 0n;
    return big((await this.connection.getTokenAccountBalance(account)).value.amount);
  }

  /** Whether lock `lockId` made on the EVM network had its receipt issued
   *  here; null while no claim or attest has touched it. */
  async evmLockMark(lockId: bigint | number): Promise<LockMark | null> {
    const address = this.at("lock", u64(lockId));
    if (!(await this.connection.getAccountInfo(address))) return null;
    const m = await this.accounts.lockMark.fetch(address);
    return { issued: m.issued, givenUp: m.givenUp, attests: m.attests, settled: m.settled };
  }

  /** Burns receipts of the EVM network's asset `asset` for the asset, paid
   *  to `to` there; in record units. */
  async burn(options: { asset: number; to: string; amount: bigint; fee?: bigint; fastFee?: bigint }): Promise<{ requestId: bigint; signature: string }> {
    const c = await this.accounts.config.fetch(this.config);
    const requestId = big(c.requestCount) + 1n;
    const user: PublicKey = this.program.provider.publicKey!;
    const mint = this.receiptMint(options.asset);
    const to = Buffer.from(getAddress(options.to).slice(2).padStart(64, "0"), "hex");
    const signature = await this.program.methods
      // The amount before the recipient here, as the program takes them.
      .makeRequest(options.asset, new BN(options.amount.toString()), Array.from(to), new BN((options.fee ?? 0n).toString()), new BN((options.fastFee ?? 0n).toString()))
      .accountsStrict({
        config: this.config,
        request: this.at("request", u64(requestId)),
        mint,
        from: associatedTokenAccount(user, mint),
        holding: this.at("holding", u32(options.asset)),
        user,
        tokenProgram: TOKEN_PROGRAM,
        systemProgram: SystemProgram.programId,
      })
      .rpc();
    return { requestId, signature };
  }
}

export type SolanaBurnState = {
  requestId: bigint;
  /** `Burned` until an operator's message carrying it is judged true here,
   *  then `Carried`; the asset is paid on the EVM network. */
  stage: "Burned" | "Carried";
  owner: string;
  asset: number;
  amount: bigint;
  fee: bigint;
  fastFee: bigint;
  at: number;
  to: string;
};

export async function getSolanaBurn(vault: SolanaVault, requestId: bigint | number): Promise<SolanaBurnState> {
  const r = await (vault.program.account as any).request.fetch(
    PublicKey.findProgramAddressSync([Buffer.from("request"), vault.config.toBuffer(), u64(requestId)], vault.program.programId)[0]
  );
  return {
    requestId: BigInt(requestId),
    stage: r.feePaid ? "Carried" : "Burned",
    owner: r.owner.toBase58(),
    asset: r.asset,
    amount: big(r.amount),
    fee: big(r.fee),
    fastFee: big(r.fastFee),
    at: Number(r.requestedAt),
    to: getAddress("0x" + Buffer.from(r.to).subarray(12).toString("hex")),
  };
}

/** A wallet's account for a token, at its usual address. */
export function associatedTokenAccount(owner: PublicKey, mint: PublicKey): PublicKey {
  return PublicKey.findProgramAddressSync([owner.toBuffer(), TOKEN_PROGRAM.toBuffer(), mint.toBuffer()], ASSOCIATED_TOKEN_PROGRAM)[0];
}
