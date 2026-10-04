// Makes Greatwall.finance's test stand-ins for tokenized assets on Solana
// devnet (AAPL, TSLA, NVDA, GOLD: SPL tokens of 9 decimals, no backing, no
// value), mints the operator's inventory for swaps, and registers each with
// the vault's pair with Ethereum so it can be locked. Run from
// programmable-network/solana:
//
//   node scripts/deploy-mock-rwa.ts [--inventory <whole tokens>] [--dry]
//
// The payer and the operator are the Solana CLI's key (~/.config/solana/id.json),
// never printed. The tokens' mint authority is a key of its own, made here
// in .keys/mock-rwa-mint-authority.json (gitignored): Greatwall's faucet
// holds it, so the CLI key, the programs' upgrade authority, never goes to
// a web server. Writes testnet-rwa.json; each step is skipped when done.

import { Connection, Keypair, PublicKey, SystemProgram, Transaction, TransactionInstruction, sendAndConfirmTransaction } from "@solana/web3.js";
import { existsSync, mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { homedir } from "node:os";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const TOKENS = [
  { symbol: "AAPL", name: "Apple (test)", faucet: 5 },
  { symbol: "TSLA", name: "Tesla (test)", faucet: 5 },
  { symbol: "NVDA", name: "NVIDIA (test)", faucet: 5 },
  { symbol: "GOLD", name: "Gold, 1 troy ounce (test)", faucet: 1 },
];
const DECIMALS = 9;
/** The vault's pair the tokens are registered with: Ethereum (D133). */
const PEER = 1;

const TOKEN_PROGRAM = new PublicKey("TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA");
const ATA_PROGRAM = new PublicKey("ATokenGPvbdGVxr1b2hvZbsiqW5xWH25efTNsLJA8knL");

const here = dirname(fileURLToPath(import.meta.url));
const root = join(here, "..");
const args = process.argv.slice(2);
const DRY = args.includes("--dry");
const inventory = BigInt(args[args.indexOf("--inventory") + 1] && args.includes("--inventory") ? args[args.indexOf("--inventory") + 1] : "1000");

const payer = Keypair.fromSecretKey(Uint8Array.from(JSON.parse(readFileSync(join(homedir(), ".config/solana/id.json"), "utf8"))));
const connection = new Connection("https://api.devnet.solana.com", "confirmed");
const vaultIdl = JSON.parse(readFileSync(join(root, "programs", "ipow-vault", "idls", "ipow_vault.json"), "utf8"));
const VAULT = new PublicKey(vaultIdl.address);

// The mint authority: made once, kept out of git.
const authorityPath = join(root, ".keys", "mock-rwa-mint-authority.json");
let authority: Keypair;
if (existsSync(authorityPath)) authority = Keypair.fromSecretKey(Uint8Array.from(JSON.parse(readFileSync(authorityPath, "utf8"))));
else {
  authority = Keypair.generate();
  if (!DRY) {
    mkdirSync(dirname(authorityPath), { recursive: true });
    writeFileSync(authorityPath, JSON.stringify(Array.from(authority.secretKey)), { mode: 0o600 });
  }
}

const ata = (owner: PublicKey, mint: PublicKey) => PublicKey.findProgramAddressSync([owner.toBuffer(), TOKEN_PROGRAM.toBuffer(), mint.toBuffer()], ATA_PROGRAM)[0];
const createAta = (owner: PublicKey, mint: PublicKey) =>
  new TransactionInstruction({
    programId: ATA_PROGRAM,
    keys: [
      { pubkey: payer.publicKey, isSigner: true, isWritable: true },
      { pubkey: ata(owner, mint), isSigner: false, isWritable: true },
      { pubkey: owner, isSigner: false, isWritable: false },
      { pubkey: mint, isSigner: false, isWritable: false },
      { pubkey: SystemProgram.programId, isSigner: false, isWritable: false },
      { pubkey: TOKEN_PROGRAM, isSigner: false, isWritable: false },
    ],
    data: Buffer.from([1]), // CreateIdempotent
  });
const mintTo = (mint: PublicKey, to: PublicKey, amount: bigint) => {
  const data = Buffer.alloc(9);
  data[0] = 7; // MintTo
  data.writeBigUInt64LE(amount, 1);
  return new TransactionInstruction({
    programId: TOKEN_PROGRAM,
    keys: [
      { pubkey: mint, isSigner: false, isWritable: true },
      { pubkey: to, isSigner: false, isWritable: true },
      { pubkey: authority.publicKey, isSigner: true, isWritable: false },
    ],
    data,
  });
};

/** Where Config keeps asset_count: after the discriminator, peer (1),
 *  peer_vault (20) and six u64 counters (the vault's IDL). */
const ASSET_COUNT_AT = 8 + 1 + 20 + 8 * 6;
const vaultPda = (...seeds: Buffer[]) => PublicKey.findProgramAddressSync(seeds, VAULT)[0];
const config = vaultPda(Buffer.from("config"), Buffer.from([PEER]));
const disc = (name: string) => Buffer.from(vaultIdl.instructions.find((i: any) => i.name === name).discriminator);

/** Registers a mint with the vault's pair, unless it already is. Returns its asset number. */
async function register(mint: PublicKey): Promise<number> {
  const tokens = vaultPda(Buffer.from("home_tokens"), config.toBuffer(), mint.toBuffer());
  if (await connection.getAccountInfo(tokens)) {
    // Registered: find its number among the pair's assets.
    const info = await connection.getAccountInfo(config);
    const count = info!.data.readUInt32LE(ASSET_COUNT_AT);
    for (let n = 0; n < count; n++) {
      const a = await connection.getAccountInfo(vaultPda(Buffer.from("asset"), config.toBuffer(), u32(n)));
      if (a && new PublicKey(a.data.subarray(8 + 4, 8 + 4 + 32)).equals(mint)) return n;
    }
    throw new Error(`${mint.toBase58()} is registered but its number was not found`);
  }
  const info = await connection.getAccountInfo(config);
  if (!info) throw new Error("the vault's pair with Ethereum is not set up on devnet");
  const n = info.data.readUInt32LE(ASSET_COUNT_AT);
  const ix = new TransactionInstruction({
    programId: VAULT,
    keys: [
      { pubkey: config, isSigner: false, isWritable: true },
      { pubkey: vaultPda(Buffer.from("asset"), config.toBuffer(), u32(n)), isSigner: false, isWritable: true },
      { pubkey: mint, isSigner: false, isWritable: false },
      { pubkey: tokens, isSigner: false, isWritable: true },
      { pubkey: payer.publicKey, isSigner: true, isWritable: true },
      { pubkey: TOKEN_PROGRAM, isSigner: false, isWritable: false },
      { pubkey: SystemProgram.programId, isSigner: false, isWritable: false },
    ],
    data: disc("register_asset"),
  });
  await sendAndConfirmTransaction(connection, new Transaction().add(ix), [payer]);
  return n;
}
function u32(n: number) {
  const b = Buffer.alloc(4);
  b.writeUInt32LE(n);
  return b;
}

const out = join(root, "testnet-rwa.json");
const record = existsSync(out) ? JSON.parse(readFileSync(out, "utf8")) : { tokens: {} };
console.log(`Solana devnet: operator ${payer.publicKey.toBase58()}, mint authority ${authority.publicKey.toBase58()}${DRY ? " (dry run)" : ""}`);

for (const t of TOKENS) {
  let mint = record.tokens[t.symbol]?.mint ? new PublicKey(record.tokens[t.symbol].mint) : null;
  if (mint && (await connection.getAccountInfo(mint))) {
    console.log(`${t.symbol}: made already, ${mint.toBase58()}`);
  } else if (DRY) {
    console.log(`${t.symbol}: would make the mint (${DECIMALS} decimals), mint ${inventory} to the operator, register with the vault's pair with Ethereum`);
    continue;
  } else {
    const k = Keypair.generate();
    mint = k.publicKey;
    const rent = await connection.getMinimumBalanceForRentExemption(82);
    const init = Buffer.alloc(35);
    init[0] = 20; // InitializeMint2
    init[1] = DECIMALS;
    authority.publicKey.toBuffer().copy(init, 2);
    const tx = new Transaction().add(
      SystemProgram.createAccount({ fromPubkey: payer.publicKey, newAccountPubkey: mint, lamports: rent, space: 82, programId: TOKEN_PROGRAM }),
      new TransactionInstruction({ programId: TOKEN_PROGRAM, keys: [{ pubkey: mint, isSigner: false, isWritable: true }], data: init }),
      createAta(payer.publicKey, mint),
      mintTo(mint, ata(payer.publicKey, mint), inventory * 10n ** BigInt(DECIMALS))
    );
    const sig = await sendAndConfirmTransaction(connection, tx, [payer, k, authority]);
    console.log(`${t.symbol}: made ${mint.toBase58()}, minted ${inventory} to the operator (${sig})`);
  }
  const asset = await register(mint!);
  record.tokens[t.symbol] = { mint: mint!.toBase58(), name: t.name, decimals: DECIMALS, faucet: t.faucet, vaultAsset: asset };
  console.log(`${t.symbol}: vault asset ${asset} in the pair with Ethereum`);
}

if (!DRY) {
  // Read back: each mint's authority and decimals, and the operator's balance.
  for (const [symbol, t] of Object.entries<any>(record.tokens)) {
    const info = await connection.getAccountInfo(new PublicKey(t.mint));
    if (!info) throw new Error(`${symbol}: no mint at ${t.mint}`);
    const hasAuthority = info.data.readUInt32LE(0) === 1 && new PublicKey(info.data.subarray(4, 36)).equals(authority.publicKey);
    if (!hasAuthority || info.data[44] !== DECIMALS) throw new Error(`${symbol} does not read back as made`);
    const bal = await connection.getTokenAccountBalance(ata(payer.publicKey, new PublicKey(t.mint)));
    console.log(`checked ${symbol}: ${t.mint}, operator holds ${bal.value.uiAmountString}`);
  }
  writeFileSync(out, JSON.stringify({ network: "solana-devnet", operator: payer.publicKey.toBase58(), mintAuthority: authority.publicKey.toBase58(), vaultPeer: PEER, tokens: record.tokens }, null, 2) + "\n");
  console.log(`wrote ${out}`);
}
