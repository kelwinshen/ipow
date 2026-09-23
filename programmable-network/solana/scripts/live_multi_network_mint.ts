// Live, real-money multi-network BETA mint (2026-09-23): registers a new
// composition (id=2) on the already-live, composition-capable beta_factory
// (BkNb9JfN...) — local leg = a real devnet SPL token
// (9GLcTUA6FY2waGqVQJkmnYJNC4FESQcS7JpDubiUbEpc), remote legs = Ethereum
// Sepolia (2), Base Sepolia (5), Robinhood Chain testnet (6), Hyperliquid
// HyperEVM testnet (8) — using each network's REAL, already-live BetaVault
// (or BetaHub router, for Hyperliquid). Composition id=1 on this factory is
// an earlier scratch run with placeholder network ids/fake lock ids — left
// untouched; a composition is immutable once registered, so this uses a
// fresh id. Tempo is excluded: it has no BetaVault deployed (hub-only),
// so it cannot judge a remote leg's lock.
//
// Registers the SAME operator+auditor party ids used on the EVM side (see
// scripts/live_multi_network_evm.mjs), pointed at the SAME shared Bitcoin
// statement-chain head — the whole point being that ONE real Bitcoin
// transaction per remote leg gets independently re-verified by Solana's own
// header relay (`ipow`) and by that leg's own EVM chain's iPoWV1.
//
//   npx ts-node scripts/live_multi_network_mint.ts <action>
// actions: register-composition | register-parties | lock-and-approve |
//          status | process-mint | exercise

import * as anchor from "@coral-xyz/anchor";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";

const WORKSPACE_ROOT = path.join(__dirname, "..");
const TOKEN_PROGRAM = new anchor.web3.PublicKey("TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA");
const ATA_PROGRAM = new anchor.web3.PublicKey("ATokenGPvbdGVxr1b2hvZbsiqW5xWH25efTNsLJA8knL");
const IPOW_PROGRAM_ID = new anchor.web3.PublicKey("EsmGbkui9ZFC6Fch9J6xoyNRZSp6TwfvcjS88beP1Vem");

export const LOCAL_TOKEN_MINT = new anchor.web3.PublicKey("9GLcTUA6FY2waGqVQJkmnYJNC4FESQcS7JpDubiUbEpc");
export const COMPOSITION_ID = BigInt(2);
export const NETWORK_ETHEREUM = BigInt(2);
export const NETWORK_BASE = BigInt(5);
export const NETWORK_ROBINHOOD = BigInt(6);
export const NETWORK_HYPERLIQUID = BigInt(8);
export const REMOTE_NETWORKS = [NETWORK_ETHEREUM, NETWORK_BASE, NETWORK_ROBINHOOD, NETWORK_HYPERLIQUID];
// component_index for process_anchor's Statement::Mint is the position
// within the REMOTE-only array (process_anchor.rs / exercise_mint.rs), i.e.
// 0=Ethereum, 1=Base, 2=Robinhood, 3=Hyperliquid, in this composition's
// registration order below.

export const OPERATOR_PARTY_ID = Buffer.alloc(32, 0x05); // fresh party id, distinct from the old factory's 0x01-0x04
// NOTE: nonce=1 for this wallet on this factory is already occupied by the
// earlier scratch_deploy_v8_composition.ts run's inert pending (fake lock
// ids, compositionId=1) — `lock_sol`'s `pending` account uses `init`, so
// reusing nonce=1 would collide with that PDA. Use 2.
export const NONCE = BigInt(2);
export const UNITS = BigInt(1);

function loadProvider(): anchor.AnchorProvider {
  const walletPath = (process.env.ANCHOR_WALLET ?? "~/.config/solana/id.json").replace(/^~/, os.homedir());
  const kp = anchor.web3.Keypair.fromSecretKey(Uint8Array.from(JSON.parse(fs.readFileSync(walletPath, "utf8"))));
  const conn = new anchor.web3.Connection(process.env.ANCHOR_PROVIDER_URL ?? "https://api.devnet.solana.com", "confirmed");
  return new anchor.AnchorProvider(conn, new anchor.Wallet(kp), { commitment: "confirmed" });
}
const idl = (name: string) => JSON.parse(fs.readFileSync(path.join(WORKSPACE_ROOT, "target", "idl", `${name}.json`), "utf8"));
const pda = (seeds: (Buffer | Uint8Array)[], pid: anchor.web3.PublicKey) => anchor.web3.PublicKey.findProgramAddressSync(seeds, pid)[0];
const sol = (x: string) => new anchor.BN(Math.round(parseFloat(x) * 1e9));
const u64le = (n: bigint | number) => new anchor.BN(n.toString()).toArrayLike(Buffer, "le", 8);

async function main() {
  const action = process.argv[2];
  const provider = loadProvider();
  anchor.setProvider(provider);
  const wallet = provider.wallet.publicKey;
  const factory: any = new anchor.Program(idl("beta_factory"), provider);
  const FACTORY_ID = factory.programId as anchor.web3.PublicKey;

  const config = pda([Buffer.from("config")], FACTORY_ID);
  const vault = pda([Buffer.from("vault")], FACTORY_ID);
  const mintAuthority = pda([Buffer.from("mint_authority")], FACTORY_ID);
  const bondEscrow = pda([Buffer.from("bond_escrow")], FACTORY_ID);
  const insurance = pda([Buffer.from("insurance")], FACTORY_ID);
  const rewardPool = pda([Buffer.from("rewards")], FACTORY_ID);
  const feesPda = pda([Buffer.from("fees")], FACTORY_ID);
  const tokenVaultAuthority = pda([Buffer.from("token_vault_authority")], FACTORY_ID);
  const compositionPda = pda([Buffer.from("composition"), u64le(COMPOSITION_ID)], FACTORY_ID);
  const pendingPda = pda([Buffer.from("pending"), wallet.toBuffer(), u64le(NONCE)], FACTORY_ID);
  const ata = (owner: anchor.web3.PublicKey, mint: anchor.web3.PublicKey) => pda([owner.toBuffer(), TOKEN_PROGRAM.toBuffer(), mint.toBuffer()], ATA_PROGRAM);
  const partyPda = (id: Buffer) => pda([Buffer.from("party"), id], FACTORY_ID);

  const cfg: any = await factory.account.factoryConfig.fetch(config);

  if (action === "register-composition") {
    const ZERO32 = Buffer.alloc(32);
    const localMintBytes = LOCAL_TOKEN_MINT.toBuffer();
    const components = [
      { networkId: new anchor.BN(0), tokenId: [...localMintBytes], amountPerUnit: new anchor.BN("1000000000") }, // 1.0 token (9 decimals)
      ...REMOTE_NETWORKS.map((n) => ({
        networkId: new anchor.BN(n.toString()),
        tokenId: [...ZERO32], // native on each remote leg
        amountPerUnit: sol("0.001"), // descriptive only — not enforced by beta-factory for remote legs; matches each BetaVault's own real configured rate
      })),
    ];
    const sig = await factory.methods
      .registerComposition(new anchor.BN(COMPOSITION_ID.toString()), components)
      .accounts({ config, composition: compositionPda, governance: wallet, systemProgram: anchor.web3.SystemProgram.programId })
      .rpc();
    console.log("composition", COMPOSITION_ID.toString(), "registered; sig", sig);
    return;
  }

  if (action === "register-parties") {
    const bond = sol("0.03"); // above minOperatorBond (0.02 SOL)
    // anchor_txid_le/anchor_vout: the operator's current real Bitcoin
    // statement-chain head (shared across every system this party is
    // registered on) — passed via env so it's set once, correctly, from
    // the live UTXO check rather than hardcoded here.
    const headTxidLE = process.env.CHAIN_HEAD_TXID_LE;
    const headVout = Number(process.env.CHAIN_HEAD_VOUT ?? "0");
    if (!headTxidLE) throw new Error("set CHAIN_HEAD_TXID_LE (32-byte hex, little-endian) and CHAIN_HEAD_VOUT");
    const anchorTxidLE = [...Buffer.from(headTxidLE, "hex")];
    const sig = await factory.methods
      .registerParty(
        [...OPERATOR_PARTY_ID],
        { operator: {} },
        anchorTxidLE,
        headVout,
        bond
      )
      .accounts({
        config,
        party: partyPda(OPERATOR_PARTY_ID),
        bondEscrow,
        owner: wallet,
        governance: wallet, // operator registration needs governance's co-signature — same wallet here (I'm both)
        systemProgram: anchor.web3.SystemProgram.programId,
      })
      .rpc();
    console.log("operator party registered on beta_factory; sig", sig);
    return;
  }

  if (action === "lock-and-approve") {
    const deadline = Math.floor(Date.now() / 1000) + 7 * 86_400;
    const userLocalTokenAta = ata(wallet, LOCAL_TOKEN_MINT);
    const tokenVaultAta = ata(tokenVaultAuthority, LOCAL_TOKEN_MINT);
    const lockSig = await factory.methods
      .lockSol(new anchor.BN(NONCE.toString()), new anchor.BN(COMPOSITION_ID.toString()), new anchor.BN(UNITS.toString()), new anchor.BN(deadline), new anchor.BN(0))
      .accounts({
        config,
        pending: pendingPda,
        composition: compositionPda,
        vault,
        fees: feesPda,
        betaMint: cfg.betaMint,
        userBeta: ata(wallet, cfg.betaMint),
        user: wallet,
        tokenProgram: TOKEN_PROGRAM,
        associatedTokenProgram: ATA_PROGRAM,
        systemProgram: anchor.web3.SystemProgram.programId,
        tokenVaultAuthority,
        localSpl0Mint: LOCAL_TOKEN_MINT,
        localSpl0User: userLocalTokenAta,
        localSpl0Vault: tokenVaultAta,
        localSpl1Mint: null,
        localSpl1User: null,
        localSpl1Vault: null,
        localSpl2Mint: null,
        localSpl2User: null,
        localSpl2Vault: null,
      })
      .rpc();
    console.log("lock_sol (SPL local leg) sig", lockSig);

    const remoteLockIdsJson = process.env.REMOTE_LOCK_IDS; // JSON array of 4 numbers, Ethereum/Base/Robinhood/Hyperliquid order
    if (!remoteLockIdsJson) {
      console.log("Set REMOTE_LOCK_IDS='[e,b,r,h]' (real lockIds from each BetaVault.deposit()) and re-run with action approve-only.");
      return;
    }
    const remoteLockIds = JSON.parse(remoteLockIdsJson).map((n: number) => new anchor.BN(n));
    const approveSig = await factory.methods
      .approvePending(new anchor.BN(NONCE.toString()), remoteLockIds)
      .accounts({ pending: pendingPda, user: wallet })
      .rpc();
    console.log("approve_pending sig", approveSig);
    return;
  }

  if (action === "approve-only") {
    const remoteLockIdsJson = process.env.REMOTE_LOCK_IDS;
    if (!remoteLockIdsJson) throw new Error("set REMOTE_LOCK_IDS='[e,b,r,h]'");
    const remoteLockIds = JSON.parse(remoteLockIdsJson).map((n: number) => new anchor.BN(n));
    const approveSig = await factory.methods
      .approvePending(new anchor.BN(NONCE.toString()), remoteLockIds)
      .accounts({ pending: pendingPda, user: wallet })
      .rpc();
    console.log("approve_pending sig", approveSig);
    return;
  }

  if (action === "status") {
    const compExists = await provider.connection.getAccountInfo(compositionPda);
    console.log("composition", COMPOSITION_ID.toString(), "exists:", !!compExists);
    if (compExists) {
      const comp: any = await factory.account.composition.fetch(compositionPda);
      console.log("components:", comp.components.map((c: any) => ({ networkId: c.networkId.toString(), amountPerUnit: c.amountPerUnit.toString() })));
    }
    const partyExists = await provider.connection.getAccountInfo(partyPda(OPERATOR_PARTY_ID));
    console.log("operator party exists:", !!partyExists);
    if (partyExists) {
      const party: any = await factory.account.party.fetch(partyPda(OPERATOR_PARTY_ID));
      console.log("party:", { owner: party.owner.toBase58(), bond: party.bond.toString(), dead: party.dead, anchorVout: party.anchorVout, seq: party.seq.toString() });
    }
    const pendingExists = await provider.connection.getAccountInfo(pendingPda);
    console.log("pending exists:", !!pendingExists);
    if (pendingExists) {
      const p: any = await factory.account.pending.fetch(pendingPda);
      console.log("pending:", {
        compositionId: p.compositionId.toString(),
        units: p.units.toString(),
        approved: p.approved,
        remoteLockId: p.remoteLockId.map((b: anchor.BN) => b.toString()),
        remoteAnchorTxid: p.remoteAnchorTxid.map((b: number[]) => Buffer.from(b).toString("hex")),
        queuedBy: Buffer.from(p.queuedBy).toString("hex"),
      });
    }
    console.log("\nPDAs for reference:");
    console.log("  composition:", compositionPda.toBase58());
    console.log("  pending:", pendingPda.toBase58());
    console.log("  operator party:", partyPda(OPERATOR_PARTY_ID).toBase58());
    console.log("  betaMint:", cfg.betaMint.toBase58());
    console.log("  local token vault ATA:", ata(tokenVaultAuthority, LOCAL_TOKEN_MINT).toBase58());
    return;
  }

  console.log("Usage: ts-node live_multi_network_mint.ts <register-composition|register-parties|lock-and-approve|approve-only|status>");
}

if (require.main === module) {
  main().catch((e) => {
    console.error(e);
    process.exit(1);
  });
}
