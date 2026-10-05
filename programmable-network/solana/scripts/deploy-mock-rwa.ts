// Makes Greatwall.finance's test stand-ins for tokenized assets on Solana
// devnet (SPL tokens of 9 decimals, named "<asset> (GW Devnet)" after the
// real tokens they stand in for, see TOKENS — Token Metadata allows a name
// of 32 bytes, so the suffix is short; no backing, no value), with
// each one's name, symbol and logo in a Token Metadata account, mints the
// operator's inventory for swaps, and registers each with the vault's pair
// with Ethereum so it can be locked. Run from programmable-network/solana:
//
//   node scripts/deploy-mock-rwa.ts [--inventory <whole tokens>] [--dry]
//
// The payer and the operator are the Solana CLI's key (~/.config/solana/id.json),
// never printed. The tokens' mint authority is a key of its own, made here
// in .keys/mock-rwa-mint-authority.json (gitignored): Greatwall's faucet
// holds it, so the CLI key, the programs' upgrade authority, never goes to
// a web server. Writes testnet-rwa.json; each step is skipped when done, so
// the list can grow between runs.

import { Connection, Keypair, PublicKey, SystemProgram, Transaction, TransactionInstruction, sendAndConfirmTransaction } from "@solana/web3.js";
import { existsSync, mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { homedir } from "node:os";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

/** The tokens: the real tokenized asset each stands in for (rwa.xyz), and
 *  what the faucet gives at a time. The first four (AAPL, TSLA, NVDA, GOLD)
 *  were the 2026-10-04 set, still there and recorded, without metadata; the
 *  rest are the 2026-10-05 set Greatwall lists. */
const TOKENS = [
  { symbol: "ONyc", name: "OnRe Reinsurance (GW Devnet)", faucet: 5 },
  { symbol: "MSTRx", name: "MicroStrategy xStock (GW Devnet)", faucet: 5 },
  { symbol: "CRCLx", name: "Circle xStock (GW Devnet)", faucet: 5 },
  { symbol: "SPYx", name: "SP500 xStock (GW Devnet)", faucet: 5 },
  { symbol: "TSLAx", name: "Tesla xStock (GW Devnet)", faucet: 5 },
  { symbol: "GLDx", name: "Gold xStock (GW Devnet)", faucet: 5 },
  { symbol: "GOOGLx", name: "Alphabet xStock (GW Devnet)", faucet: 5 },
  { symbol: "SPCXx", name: "SpaceX xStock (GW Devnet)", faucet: 5 },
  { symbol: "QQQx", name: "Nasdaq xStock (GW Devnet)", faucet: 5 },
  { symbol: "HOODx", name: "Robinhood xStock (GW Devnet)", faucet: 5 },
];
/** Where each token's metadata JSON (name, symbol, image) is served from:
 *  Greatwall's public/assets/rwa/solana/<symbol>.json. */
/** Token Metadata's limits: a name of 32 bytes, a symbol of 10, a URI of 200. */
const LIMITS = { name: 32, symbol: 10, uri: 200 };
const METADATA_URI = (symbol: string) => `https://greatwall.finance/assets/rwa/solana/${symbol}.json`;
const DECIMALS = 9;
/** The vault's pair the tokens are registered with: Ethereum (D133). */
const PEER = 1;

const TOKEN_PROGRAM = new PublicKey("TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA");
const ATA_PROGRAM = new PublicKey("ATokenGPvbdGVxr1b2hvZbsiqW5xWH25efTNsLJA8knL");
/** Metaplex Token Metadata: the account wallets and explorers read a token's name, symbol and image from. */
const METADATA_PROGRAM = new PublicKey("metaqbxxUerdq28cj1RbAWkYQm3ybzjb6a8bt518x1s");

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

const metadataPda = (mint: PublicKey) => PublicKey.findProgramAddressSync([Buffer.from("metadata"), METADATA_PROGRAM.toBuffer(), mint.toBuffer()], METADATA_PROGRAM)[0];
/** CreateMetadataAccountV3 (instruction 33): DataV2 with no creators,
 *  collection or uses, mutable, no collection details. The operator (the
 *  payer) is the update authority; the mint authority signs. */
function createMetadata(mint: PublicKey, name: string, symbol: string, uri: string) {
  const str = (v: string) => {
    const b = Buffer.from(v, "utf8");
    const len = Buffer.alloc(4);
    len.writeUInt32LE(b.length);
    return Buffer.concat([len, b]);
  };
  const data = Buffer.concat([
    Buffer.from([33]),
    str(name),
    str(symbol),
    str(uri),
    Buffer.from([0, 0]), // seller_fee_basis_points
    Buffer.from([0]), // creators: None
    Buffer.from([0]), // collection: None
    Buffer.from([0]), // uses: None
    Buffer.from([1]), // is_mutable
    Buffer.from([0]), // collection_details: None
  ]);
  return new TransactionInstruction({
    programId: METADATA_PROGRAM,
    keys: [
      { pubkey: metadataPda(mint), isSigner: false, isWritable: true },
      { pubkey: mint, isSigner: false, isWritable: false },
      { pubkey: authority.publicKey, isSigner: true, isWritable: false },
      { pubkey: payer.publicKey, isSigner: true, isWritable: true },
      { pubkey: payer.publicKey, isSigner: true, isWritable: false },
      { pubkey: SystemProgram.programId, isSigner: false, isWritable: false },
    ],
    data,
  });
}
/** The name and symbol a metadata account holds (fixed-width, NUL-padded). */
function readMetadata(data: Buffer): { name: string; symbol: string; uri: string } {
  let at = 1 + 32 + 32; // key, update authority, mint
  const str = (width: number) => {
    const len = data.readUInt32LE(at);
    const v = data.subarray(at + 4, at + 4 + len).toString("utf8").replace(/\0+$/, "");
    at += 4 + width;
    return v;
  };
  return { name: str(32), symbol: str(10), uri: str(200) };
}

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
for (const t of TOKENS) {
  const uri = METADATA_URI(t.symbol);
  for (const [field, v] of [["name", t.name], ["symbol", t.symbol], ["uri", uri]] as const)
    if (Buffer.byteLength(v) > LIMITS[field]) throw new Error(`${t.symbol}: ${field} "${v}" is over ${LIMITS[field]} bytes`);
}
console.log(`Solana devnet: operator ${payer.publicKey.toBase58()}, mint authority ${authority.publicKey.toBase58()}${DRY ? " (dry run)" : ""}`);

for (const t of TOKENS) {
  let mint = record.tokens[t.symbol]?.mint ? new PublicKey(record.tokens[t.symbol].mint) : null;
  if (mint && (await connection.getAccountInfo(mint))) {
    console.log(`${t.symbol}: made already, ${mint.toBase58()}`);
  } else if (DRY) {
    console.log(`${t.symbol}: would make the mint (${DECIMALS} decimals) with metadata "${t.name}" at ${METADATA_URI(t.symbol)}, mint ${inventory} to the operator, register with the vault's pair with Ethereum`);
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
      createMetadata(mint, t.name, t.symbol, METADATA_URI(t.symbol)),
      createAta(payer.publicKey, mint),
      mintTo(mint, ata(payer.publicKey, mint), inventory * 10n ** BigInt(DECIMALS))
    );
    const sig = await sendAndConfirmTransaction(connection, tx, [payer, k, authority]);
    console.log(`${t.symbol}: made ${mint.toBase58()}, minted ${inventory} to the operator (${sig})`);
  }
  const asset = await register(mint!);
  record.tokens[t.symbol] = { mint: mint!.toBase58(), name: t.name, decimals: DECIMALS, faucet: t.faucet, vaultAsset: asset, metadata: METADATA_URI(t.symbol) };
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
    let named = "";
    if (t.metadata) {
      const m = await connection.getAccountInfo(metadataPda(new PublicKey(t.mint)));
      if (!m) throw new Error(`${symbol}: no metadata account`);
      const read = readMetadata(m.data);
      if (read.name !== t.name || read.symbol !== symbol || read.uri !== t.metadata) throw new Error(`${symbol}: metadata reads back as ${JSON.stringify(read)}`);
      named = `, named "${read.name}" (${read.symbol})`;
    }
    console.log(`checked ${symbol}: ${t.mint}, operator holds ${bal.value.uiAmountString}${named}`);
  }
  writeFileSync(out, JSON.stringify({ network: "solana-devnet", operator: payer.publicKey.toBase58(), mintAuthority: authority.publicKey.toBase58(), vaultPeer: PEER, tokens: record.tokens }, null, 2) + "\n");
  console.log(`wrote ${out}`);
}
