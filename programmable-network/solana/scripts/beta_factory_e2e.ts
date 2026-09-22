// BETA v2 devnet driver for `beta_factory` (docs/DESIGN_V2.md §6).
// Wallet/cluster: ~/.config/solana/id.json on devnet (same as the scratch
// scripts). Every action is one instruction; statements are printed as
// hex for `core/operator`'s `anchor_statement` example, which turns them
// into real Bitcoin anchors.
//
//   ACTION=status
//   ACTION=init                       [SOL_PER_UNIT=0.01] [ETH_GWEI_PER_UNIT=1000000] ...
//   ACTION=register-operator          PARTY_ID=<32-byte hex> ANCHOR_TXID=<be hex> ANCHOR_VOUT=n BOND_SOL=0.2
//   ACTION=register-auditor           PARTY_ID=... ANCHOR_TXID=... ANCHOR_VOUT=n BOND_SOL=0.1
//   ACTION=lock                       NONCE=1 UNITS=1 DEADLINE_SECS=86400
//   ACTION=approve                    NONCE=1 ETH_LOCK_ID=1
//   ACTION=statement-mint             NONCE=1 ETH_LOCK_ID=1            (user = wallet)
//   ACTION=burn                       UNITS=1 TO_ETH=0x<20 bytes>
//   ACTION=statement-release          ETH_LOCK_ID=1 BURN_ID=0
//   ACTION=relay-header               HEIGHT=<bitcoin height>         (ipow relay; jump allowed when idle)
//   ACTION=process                    PARTY_ID=... TXID=<be hex> STATEMENT=<hex>
//   ACTION=exercise                   PARTY_ID=... TXID=<be hex>
//   ACTION=settle                     TXID=<be hex>                  (v3: after the challenge window)
//   ACTION=statement-target           KIND=5|6|7 TARGET=<be txid hex, or party id hex for 7>
//   ACTION=statement-veto             TARGET_PARTY=<hex> [TARGET_TXID=<be hex>]   (omit for dead-veto)
//
// Everything is run with:  npx ts-node scripts/beta_factory_e2e.ts

import * as anchor from "@coral-xyz/anchor";
import { createHash } from "node:crypto";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";

const WORKSPACE_ROOT = path.join(__dirname, "..");
const IPOW_ID = new anchor.web3.PublicKey("EsmGbkui9ZFC6Fch9J6xoyNRZSp6TwfvcjS88beP1Vem");
const FACTORY_ID = new anchor.web3.PublicKey("3GUPbVjjBRNEYafHprb2fnh6Wzyif9a4gWphSrhkPVEf");
const TOKEN_PROGRAM = new anchor.web3.PublicKey("TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA");
const ATA_PROGRAM = new anchor.web3.PublicKey("ATokenGPvbdGVxr1b2hvZbsiqW5xWH25efTNsLJA8knL");
const ESPLORA = "https://blockstream.info/api";

const env = (k: string, d?: string) => process.env[k] ?? d ?? (() => { throw new Error(`missing ${k}`); })();
const hex = (b: Uint8Array | Buffer) => Buffer.from(b).toString("hex");
const unhex = (s: string) => Buffer.from(s.replace(/^0x/, ""), "hex");
const u64le = (n: anchor.BN | number) => new anchor.BN(n).toArrayLike(Buffer, "le", 8);
const u64be = (n: bigint) => { const b = Buffer.alloc(8); b.writeBigUInt64BE(n); return b; };
const sha256 = (b: Buffer) => createHash("sha256").update(b).digest();

function loadProvider(): anchor.AnchorProvider {
  const walletPath = (process.env.ANCHOR_WALLET ?? "~/.config/solana/id.json").replace(/^~/, os.homedir());
  const kp = anchor.web3.Keypair.fromSecretKey(Uint8Array.from(JSON.parse(fs.readFileSync(walletPath, "utf8"))));
  const conn = new anchor.web3.Connection(process.env.ANCHOR_PROVIDER_URL ?? "https://api.devnet.solana.com", "confirmed");
  return new anchor.AnchorProvider(conn, new anchor.Wallet(kp), { commitment: "confirmed" });
}
const idl = (name: string) => JSON.parse(fs.readFileSync(path.join(WORKSPACE_ROOT, "target", "idl", `${name}.json`), "utf8"));
const pda = (seeds: (Buffer | Uint8Array)[], pid = FACTORY_ID) => anchor.web3.PublicKey.findProgramAddressSync(seeds, pid)[0];
const ata = (owner: anchor.web3.PublicKey, mint: anchor.web3.PublicKey) =>
  pda([owner.toBuffer(), TOKEN_PROGRAM.toBuffer(), mint.toBuffer()], ATA_PROGRAM);

// --- statement encodings (byte-identical to statement.rs / BetaVault.sol)
function stmtMint(ethLockId: bigint, solUser: anchor.web3.PublicKey, nonce: bigint, units: bigint, deadline: bigint) {
  return Buffer.concat([Buffer.from([1]), u64be(ethLockId), solUser.toBuffer(), u64be(nonce), u64be(units), u64be(deadline)]);
}
function stmtRelease(ethLockId: bigint, burnId: bigint, toEth: Buffer, units: bigint) {
  return Buffer.concat([Buffer.from([2]), u64be(ethLockId), u64be(burnId), toEth, u64be(units)]);
}

// --- Bitcoin helpers
function readVarInt(b: Buffer, o: number): [number, number] {
  const p = b[o];
  if (p < 0xfd) return [p, o + 1];
  if (p === 0xfd) return [b.readUInt16LE(o + 1), o + 3];
  if (p === 0xfe) return [b.readUInt32LE(o + 1), o + 5];
  return [Number(b.readBigUInt64LE(o + 1)), o + 9];
}
/** Witness-stripped serialization (what both chains hash and parse). */
function stripWitness(tx: Buffer): Buffer {
  if (!(tx[4] === 0x00 && tx[5] === 0x01)) return tx;
  const out: Buffer[] = [tx.subarray(0, 4)];
  let o = 6;
  const start = o;
  let [inCount, n] = readVarInt(tx, o); o = n;
  for (let i = 0; i < inCount; i++) { o += 36; const [slen, n2] = readVarInt(tx, o); o = n2 + slen + 4; }
  let [outCount, n3] = readVarInt(tx, o); o = n3;
  for (let j = 0; j < outCount; j++) { o += 8; const [slen, n4] = readVarInt(tx, o); o = n4 + slen; }
  out.push(tx.subarray(start, o));
  out.push(tx.subarray(tx.length - 4)); // locktime
  return Buffer.concat(out);
}
const dsha = (b: Buffer) => sha256(sha256(b));
async function getText(url: string) { const r = await fetch(url); if (!r.ok) throw new Error(`${url}: ${r.status}`); return (await r.text()).trim(); }
async function getJson(url: string): Promise<any> { const r = await fetch(url); if (!r.ok) throw new Error(`${url}: ${r.status}`); return r.json(); }

async function main() {
  const provider = loadProvider();
  anchor.setProvider(provider);
  const wallet = provider.wallet.publicKey;
  const factory: any = new anchor.Program(idl("beta_factory"), provider);
  const ipow: any = new anchor.Program(idl("ipow"), provider);
  const config = pda([Buffer.from("config")]);
  const vault = pda([Buffer.from("vault")]);
  const mintAuthority = pda([Buffer.from("mint_authority")]);
  const bondEscrow = pda([Buffer.from("bond_escrow")]);
  const insurance = pda([Buffer.from("insurance")]);
  const ipowGlobalState = pda([Buffer.from("global_state")], IPOW_ID);
  const action = env("ACTION", "status");
  const partyPda = (id: string) => pda([Buffer.from("party"), unhex(id)]);
  const anchorPda = (txidLe: Buffer) => pda([Buffer.from("anchor"), txidLe]);
  const pendingPda = (user: anchor.web3.PublicKey, nonce: bigint) => pda([Buffer.from("pending"), user.toBuffer(), u64le(new anchor.BN(nonce.toString()))]);
  const burnPda = (id: bigint) => pda([Buffer.from("burn"), u64le(new anchor.BN(id.toString()))]);
  const sol = (x: string) => new anchor.BN(Math.round(parseFloat(x) * 1e9));

  if (action === "status") {
    const c = await factory.account.factoryConfig.fetchNullable(config);
    console.log("config", c ? { betaMint: c.betaMint.toBase58(), paused: c.paused, ethClaimsUnits: c.ethClaimsUnits.toString(), reserve: c.reserveLamports.toString(), pending: c.pendingLamports.toString(), nextBurnId: c.nextBurnId.toString(), window: [c.windowStartHeight.toString(), c.windowUnits.toString()] } : null);
    const gs = await ipow.account.globalState.fetch(ipowGlobalState);
    console.log("ipow relay tip", gs.globalTipHeight.toString(), "activeOpen", gs.activeOpenConversions.toString());
    if (process.env.PARTY_ID) { const p = await factory.account.party.fetchNullable(partyPda(env("PARTY_ID"))); console.log("party", p ? { owner: p.owner.toBase58(), kind: p.kind, head: hex(p.anchorTxidLe) + ":" + p.anchorVout, seq: p.seq.toString(), bond: p.bond.toString(), dead: p.dead, pausedUntil: p.pausedUntil.toString() } : null); }
    if (process.env.NONCE) { const p = await factory.account.pending.fetchNullable(pendingPda(wallet, BigInt(env("NONCE")))); console.log("pending", p ? { units: p.units.toString(), deadline: p.deadline.toString(), ethLockId: p.ethLockId.toString(), approved: p.approved, queued: p.queued } : null); }
    if (process.env.TXID) { const a = await factory.account.processedAnchor.fetchNullable(anchorPda(unhex(env("TXID")).reverse())); console.log("anchor", a ? { kind: a.kind, status: a.status, held: a.held, attestedBy: hex(a.attestedBy).slice(0, 8), escrow: a.escrow.toString(), challengeUntil: a.challengeUntil.toString(), settled: a.settled, units: a.units.toString() } : null); }
    return;
  }

  if (action === "init") {
    const betaMint = anchor.web3.Keypair.generate();
    const params = {
      solPerUnit: sol(env("SOL_PER_UNIT", "0.01")),
      ethGweiPerUnit: new anchor.BN(env("ETH_GWEI_PER_UNIT", "1000000")),
      windowBlocks: new anchor.BN(env("WINDOW_BLOCKS", "6")), // v2 field, unused in v3
      mintCapUnitsPerWindow: new anchor.BN(env("MINT_CAP", "100")), // v2 field, unused in v3
      tSkipSecs: new anchor.BN(env("T_SKIP", "7200")),
      tChallengeSecs: new anchor.BN(env("T_CHALLENGE", String(7 * 86400))),
      unbondDelaySecs: new anchor.BN(env("UNBOND_DELAY", "60")),
      minOperatorBond: sol(env("MIN_OP_BOND", "0.2")),
      minAuditorBond: sol(env("MIN_AUD_BOND", "0.1")),
      compLamportsPerUnit: sol(env("COMP_PER_UNIT", "0.01")),
      vetoSlashLamports: sol(env("VETO_SLASH", "0.05")),
      vetoRewardLamports: sol(env("VETO_REWARD", "0.01")),
      bountyBps: 1000,
    };
    const sig = await factory.methods.initialize(wallet, params)
      .accounts({ config, vault, mintAuthority, bondEscrow, insurance, betaMint: betaMint.publicKey, admin: wallet, tokenProgram: TOKEN_PROGRAM, systemProgram: anchor.web3.SystemProgram.programId })
      .signers([betaMint]).rpc();
    console.log("initialized; beta_mint", betaMint.publicKey.toBase58(), "sig", sig);
    return;
  }

  if (action === "set-params") {
    const c = await factory.account.factoryConfig.fetch(config);
    const p = { ...c.params, tChallengeSecs: new anchor.BN(env("T_CHALLENGE", c.params.tChallengeSecs.toString())) };
    if (process.env.COMP_PER_UNIT) p.compLamportsPerUnit = sol(env("COMP_PER_UNIT"));
    if (process.env.T_SKIP) p.tSkipSecs = new anchor.BN(env("T_SKIP"));
    const sig = await factory.methods.setParams(p, process.env.PAUSED === "1").accounts({ config, governance: wallet }).rpc();
    console.log("params set; t_challenge", p.tChallengeSecs.toString(), sig);
    return;
  }

  if (action === "statement-target") {
    const kind = parseInt(env("KIND"));
    const t = env("TARGET");
    const target = kind === 7 ? unhex(t) : unhex(t).reverse();
    const stmt = Buffer.concat([Buffer.from([kind]), target]);
    console.log(`kind ${kind} statement hex:`, hex(stmt));
    return;
  }

  if (action === "statement-veto") {
    const target = unhex(env("TARGET_PARTY"));
    const txid = process.env.TARGET_TXID ? unhex(env("TARGET_TXID")).reverse() : Buffer.alloc(32);
    const stmt = Buffer.concat([Buffer.from([3]), target, txid]);
    console.log("VETO statement hex:", hex(stmt));
    return;
  }

  if (action === "settle") {
    const txidLe = unhex(env("TXID")).reverse();
    const a = await factory.account.processedAnchor.fetch(anchorPda(txidLe));
    const attester = a.escrow.toString() !== "0" ? pda([Buffer.from("party"), Buffer.from(a.attestedBy)]) : null;
    const pending = a.held && "queued" in a.status ? pendingPda(a.solUser, BigInt(a.nonce.toString())) : null;
    const sig = await factory.methods.settleMint([...txidLe])
      .accounts({ config, processed: anchorPda(txidLe), bondEscrow, insurance, systemProgram: anchor.web3.SystemProgram.programId, attester, pending })
      .rpc();
    const after = await factory.account.processedAnchor.fetch(anchorPda(txidLe));
    console.log("settled", sig, "status", after.status, "escrow", after.escrow.toString());
    return;
  }

  if (action === "register-operator" || action === "register-auditor") {
    const kind = action === "register-operator" ? { operator: {} } : { auditor: {} };
    const sig = await factory.methods.registerParty([...unhex(env("PARTY_ID"))], kind, [...unhex(env("ANCHOR_TXID")).reverse()], parseInt(env("ANCHOR_VOUT", "0")), sol(env("BOND_SOL")))
      .accounts({ config, party: partyPda(env("PARTY_ID")), bondEscrow, owner: wallet, governance: action === "register-operator" ? wallet : null, systemProgram: anchor.web3.SystemProgram.programId })
      .rpc();
    console.log("registered", action, partyPda(env("PARTY_ID")).toBase58(), sig);
    return;
  }

  if (action === "lock") {
    const c = await factory.account.factoryConfig.fetch(config);
    const nonce = BigInt(env("NONCE"));
    const deadline = process.env.DEADLINE ? parseInt(env("DEADLINE")) : Math.floor(Date.now() / 1000) + parseInt(env("DEADLINE_SECS", "86400"));
    // Optional acceleration fee: what you're willing to pay whoever ATTESTs
    // this mint instead of the free 7-day path (ATTEST_FEE_SOL=0.01, say).
    // Refunded automatically if nobody ever does.
    const attestFee = sol(env("ATTEST_FEE_SOL", "0"));
    const sig = await factory.methods.lockSol(new anchor.BN(nonce.toString()), new anchor.BN(env("UNITS", "1")), new anchor.BN(deadline), attestFee)
      .accounts({ config, pending: pendingPda(wallet, nonce), vault, fees: pda([Buffer.from("fees")]), betaMint: c.betaMint, userBeta: ata(wallet, c.betaMint), user: wallet, tokenProgram: TOKEN_PROGRAM, associatedTokenProgram: ATA_PROGRAM, systemProgram: anchor.web3.SystemProgram.programId })
      .rpc();
    console.log("locked; deadline", deadline, "attest_fee", attestFee.toString(), "pending", pendingPda(wallet, nonce).toBase58(), sig);
    return;
  }

  if (action === "approve") {
    const nonce = BigInt(env("NONCE"));
    const sig = await factory.methods.approvePending(new anchor.BN(nonce.toString()), new anchor.BN(env("ETH_LOCK_ID")))
      .accounts({ pending: pendingPda(wallet, nonce), user: wallet }).rpc();
    console.log("approved", sig);
    return;
  }

  if (action === "statement-mint") {
    const nonce = BigInt(env("NONCE"));
    const p = await factory.account.pending.fetch(pendingPda(wallet, nonce));
    const s = stmtMint(BigInt(env("ETH_LOCK_ID")), wallet, nonce, BigInt(p.units.toString()), BigInt(p.deadline.toString()));
    console.log("MINT statement hex:", hex(s));
    console.log("sha256:", hex(sha256(s)));
    return;
  }

  if (action === "burn") {
    const c = await factory.account.factoryConfig.fetch(config);
    const burnId = BigInt(c.nextBurnId.toString());
    const sig = await factory.methods.burnRedeem(new anchor.BN(env("UNITS", "1")), [...unhex(env("TO_ETH"))])
      .accounts({ config, burn: burnPda(burnId), vault, betaMint: c.betaMint, burnerBeta: ata(wallet, c.betaMint), burner: wallet, tokenProgram: TOKEN_PROGRAM, systemProgram: anchor.web3.SystemProgram.programId })
      .rpc();
    console.log("burned; burn_id", burnId.toString(), sig);
    return;
  }

  if (action === "statement-release") {
    const b = await factory.account.burn.fetch(burnPda(BigInt(env("BURN_ID"))));
    const s = stmtRelease(BigInt(env("ETH_LOCK_ID")), BigInt(env("BURN_ID")), Buffer.from(b.toEth), BigInt(b.units.toString()));
    console.log("RELEASE statement hex:", hex(s));
    return;
  }

  if (action === "relay-header") {
    const height = parseInt(env("HEIGHT"));
    const gs = await ipow.account.globalState.fetch(ipowGlobalState);
    const tip = Number(gs.globalTipHeight.toString());
    const hash = await getText(`${ESPLORA}/block-height/${height}`);
    const header80 = unhex(await getText(`${ESPLORA}/block/${hash}/header`));
    const hb = (h: number) => pda([Buffer.from("header"), u64le(h)], IPOW_ID);
    const extending = height === tip + 1 && !!(await provider.connection.getAccountInfo(hb(tip)));
    if (process.env.RELAY_LEGACY) {
      // Deployed ipow predates the §6.9 change: old account layout (no prev_header, `operator` signer).
      const legacyIdl = JSON.parse(fs.readFileSync(env("RELAY_LEGACY"), "utf8"));
      const legacy: any = new anchor.Program(legacyIdl, provider);
      const sigL = await legacy.methods.commitGlobalHeader([...header80], new anchor.BN(height))
        .accounts({ globalState: ipowGlobalState, header: hb(height), prevHeightTracker: pda([Buffer.from("tracker"), u64le(height - 1)], IPOW_ID), prevEpochStartHeader: null, prevEpochEndHeader: null, operator: wallet, systemProgram: anchor.web3.SystemProgram.programId })
        .rpc();
      console.log("relayed (legacy layout)", height, hash, sigL);
      return;
    }
    const sig = await ipow.methods.commitGlobalHeader([...header80], new anchor.BN(height))
      .accounts({
        globalState: ipowGlobalState, header: hb(height),
        prevHeightTracker: pda([Buffer.from("tracker"), u64le(height - 1)], IPOW_ID),
        prevEpochStartHeader: null, prevEpochEndHeader: null,
        prevHeader: extending ? hb(tip) : null,
        submitter: wallet, systemProgram: anchor.web3.SystemProgram.programId,
      }).rpc();
    console.log("relayed", height, hash, extending ? "(extend)" : "(jump/anchor)", sig);
    return;
  }

  if (action === "process" || action === "exercise") {
    const txidBe = env("TXID");
    const txidLe = unhex(txidBe).reverse();
    const party = partyPda(env("PARTY_ID"));
    if (action === "exercise") {
      const a = await factory.account.processedAnchor.fetch(anchorPda(txidLe));
      const c = await factory.account.factoryConfig.fetch(config);
      const sig = await factory.methods.exerciseMint([...txidLe])
        .preInstructions([anchor.web3.ComputeBudgetProgram.setComputeUnitLimit({ units: 600_000 })])
        .accounts({ config, processed: anchorPda(txidLe), party, pending: pendingPda(a.solUser, BigInt(a.nonce.toString())), user: a.solUser, userBeta: ata(a.solUser, c.betaMint), betaMint: c.betaMint, mintAuthority, fees: pda([Buffer.from("fees")]), tokenProgram: TOKEN_PROGRAM, systemProgram: anchor.web3.SystemProgram.programId })
        .rpc();
      console.log("exercised", sig);
      return;
    }
    const statement = unhex(env("STATEMENT"));
    const raw = stripWitness(unhex(await getText(`${ESPLORA}/tx/${txidBe}/hex`)));
    if (!dsha(raw).equals(txidLe)) throw new Error("stripped tx does not hash to txid");
    const proof = await getJson(`${ESPLORA}/tx/${txidBe}/merkle-proof`);
    const branchLe = (proof.merkle as string[]).map((h) => [...unhex(h).reverse()]);
    const header = pda([Buffer.from("header"), u64le(proof.block_height)], IPOW_ID);
    if (!(await provider.connection.getAccountInfo(header))) throw new Error(`relay header ${proof.block_height} missing; run ACTION=relay-header HEIGHT=${proof.block_height}`);
    const kind = statement[0];
    let extra: any = { pending: null, pendingUser: null, burn: null, targetParty: null, targetAnchor: null, priorParty: process.env.PRIOR_PARTY ? pda([Buffer.from("party"), unhex(env("PRIOR_PARTY"))]) : null };
    if (kind === 1) {
      const solUser = new anchor.web3.PublicKey(statement.subarray(9, 41));
      const nonce = statement.readBigUInt64BE(41);
      extra.pending = pendingPda(solUser, nonce);
      extra.pendingUser = solUser;
      if (!(await provider.connection.getAccountInfo(extra.pending))) extra.pending = null;
    } else if (kind === 2) {
      const burnId = statement.readBigUInt64BE(9);
      extra.burn = burnPda(burnId);
      if (!(await provider.connection.getAccountInfo(extra.burn))) extra.burn = null;
    } else if (kind === 3) {
      const targetId = statement.subarray(1, 33);
      const targetTxidLe = statement.subarray(33, 65);
      extra.targetParty = pda([Buffer.from("party"), targetId]);
      if (!targetTxidLe.equals(Buffer.alloc(32))) extra.targetAnchor = anchorPda(Buffer.from(targetTxidLe));
    } else if (kind === 5 || kind === 6) {
      extra.targetAnchor = anchorPda(Buffer.from(statement.subarray(1, 33)));
      // ATTEST targeting a MINT also needs that mint's pending slot, so
      // process_anchor can pay out (kind 5) whatever acceleration fee the
      // user posted at lock_sol.
      if (kind === 5) {
        const target = await factory.account.processedAnchor.fetchNullable(extra.targetAnchor);
        if (target && target.kind === 1) extra.pending = pendingPda(target.solUser, BigInt(target.nonce.toString()));
      }
    } else if (kind === 7) {
      extra.targetParty = pda([Buffer.from("party"), statement.subarray(1, 33)]);
    }
    const p = await factory.account.party.fetch(party);
    const sig = await factory.methods.processAnchor([...txidLe], statement, raw, new anchor.BN(proof.block_height), branchLe, new anchor.BN(proof.pos))
      .preInstructions([anchor.web3.ComputeBudgetProgram.setComputeUnitLimit({ units: 1_400_000 })])
      .accounts({ config, party, partyOwner: p.owner, processed: anchorPda(txidLe), header, bondEscrow, insurance, rewardPool: pda([Buffer.from("rewards")]), fees: pda([Buffer.from("fees")]), submitter: wallet, systemProgram: anchor.web3.SystemProgram.programId, ...extra })
      .rpc();
    const a = await factory.account.processedAnchor.fetch(anchorPda(txidLe));
    console.log("processed", sig, "status", a.status);
    return;
  }
  throw new Error(`unknown ACTION ${action}`);
}

main().catch((e) => { console.error(e); process.exit(1); });
