// BETA v2 daemon (docs/DESIGN_V2.md §6.13): the automation that keeps the
// protocol running without anyone typing commands.
//
//   ROLE=watchtower   follow every registered party's statement chain on
//                     Bitcoin; relay headers; submit each anchor to both
//                     chains (finding the statement preimage from the local
//                     statements dir or by reconstructing it from chain state);
//                     exercise queued mints; execute due releases; skip stalled
//                     anchors. With AUDITOR_ID set, also anchor VETOs and
//                     dead-vetoes where the two chains' verdicts disagree.
//   ROLE=operator     watch Ethereum deposits and Solana burns; build and
//                     anchor MINT / RELEASE statements for OPERATOR_ID.
//   ROLE=both         both loops.
//
// Anchors are only broadcast when BROADCAST=yes (otherwise dry-run output).
// It orchestrates the two CLI drivers (beta_factory_e2e.ts / beta_vault_e2e.mjs)
// and core/operator's anchor_statement example rather than re-implementing them.
//
//   ROLE=watchtower PARTIES=0303…,0404… npx ts-node scripts/beta_daemon.ts
//   ROLE=operator OPERATOR_ID=0303… BROADCAST=yes npx ts-node scripts/beta_daemon.ts

import * as anchor from "@coral-xyz/anchor";
import { spawnSync } from "node:child_process";
import { createHash } from "node:crypto";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";

const SOL_DIR = path.join(__dirname, "..");
const ETH_DIR = path.join(SOL_DIR, "..", "ethereum");
const OPERATOR_DIR = path.join(SOL_DIR, "..", "..", "core", "operator");
// ethers lives in the Ethereum workspace (pnpm keeps node_modules per package).
// eslint-disable-next-line @typescript-eslint/no-var-requires
const { ethers } = require(path.join(SOL_DIR, "..", "ethereum", "node_modules", "ethers")) as any;
const STATEMENTS_DIR = process.env.STATEMENTS_DIR ?? path.join(SOL_DIR, "scripts", "statements");
const STATE_FILE = path.join(SOL_DIR, "scripts", "daemon_state.json");
const FACTORY_ID = new anchor.web3.PublicKey("3GUPbVjjBRNEYafHprb2fnh6Wzyif9a4gWphSrhkPVEf");
const IPOW_ID = new anchor.web3.PublicKey("EsmGbkui9ZFC6Fch9J6xoyNRZSp6TwfvcjS88beP1Vem");
const ESPLORA = "https://blockstream.info/api";
const ROLE = process.env.ROLE ?? "watchtower";
const INTERVAL = parseInt(process.env.INTERVAL_SEC ?? "60") * 1000;
const PARTIES = (process.env.PARTIES ?? "03".repeat(32) + "," + "04".repeat(32)).split(",").map((s) => s.trim().replace(/^0x/, ""));
const OPERATOR_ID = (process.env.OPERATOR_ID ?? "").replace(/^0x/, "");
const AUDITOR_ID = (process.env.AUDITOR_ID ?? "").replace(/^0x/, "");
const BROADCAST = process.env.BROADCAST === "yes";
const HEAD_SATS = 294;
/// Hard spend limits when broadcasting: sats per anchor fee, anchors per
/// hour, and sats per hour (fees). The daemon refuses beyond them and logs.
const MAX_FEE_SATS = parseInt(process.env.MAX_FEE_SATS ?? "600");
const MAX_ANCHORS_PER_HOUR = parseInt(process.env.MAX_ANCHORS_PER_HOUR ?? "4");
const MAX_SATS_PER_HOUR = parseInt(process.env.MAX_SATS_PER_HOUR ?? "1500");

const log = (...a: any[]) => console.log(new Date().toISOString().slice(11, 19), ...a);
const sha256 = (b: Buffer) => createHash("sha256").update(b).digest();
const hex = (b: Uint8Array | Buffer) => Buffer.from(b).toString("hex");
const unhex = (s: string) => Buffer.from(s.replace(/^0x/, ""), "hex");
const u64be = (n: bigint) => { const b = Buffer.alloc(8); b.writeBigUInt64BE(n); return b; };
const rev = (h: string) => hex(unhex(h).reverse());
/** Esplora GET with retry/backoff and a mempool.space fallback (blockstream resets bursts). */
async function fetchOk(url: string): Promise<Response> {
  const alts = url.startsWith(ESPLORA) ? [url, url.replace(ESPLORA, "https://mempool.space/api")] : [url];
  let last = "";
  for (let attempt = 0; attempt < 4; attempt++) {
    for (const u of alts) {
      try {
        const r = await fetch(u);
        if (r.ok) return r;
        if (r.status === 404) throw new Error(`${u}: 404`);
        last = `${u}: ${r.status}`;
      } catch (e: any) {
        if (String(e.message).endsWith(": 404")) throw e;
        last = `${u}: ${e.cause?.code ?? e.cause?.message ?? e.message}`;
      }
    }
    await new Promise((res) => setTimeout(res, 500 * 2 ** attempt));
  }
  throw new Error(last);
}
async function getJson(url: string): Promise<any> { return (await fetchOk(url)).json(); }
async function getText(url: string) { return (await (await fetchOk(url)).text()).trim(); }

// ---------- persistent daemon state
type DState = { anchoredBurns: Record<string, string>; anchoredLocks: Record<string, string>; skipped: string[]; anchoredStatements: Record<string, string>; spends: { t: number; fee: number; txid: string }[] };
const state: DState = Object.assign({ anchoredBurns: {}, anchoredLocks: {}, skipped: [], anchoredStatements: {}, spends: [] }, fs.existsSync(STATE_FILE) ? JSON.parse(fs.readFileSync(STATE_FILE, "utf8")) : {});
function spendAllowed(fee: number): string | null {
  const hourAgo = Date.now() - 3600_000;
  const recent = state.spends.filter((x) => x.t > hourAgo);
  if (fee > MAX_FEE_SATS) return `fee ${fee} > MAX_FEE_SATS ${MAX_FEE_SATS}`;
  if (recent.length >= MAX_ANCHORS_PER_HOUR) return `${recent.length} anchors in the last hour ≥ MAX_ANCHORS_PER_HOUR ${MAX_ANCHORS_PER_HOUR}`;
  const spent = recent.reduce((a, x) => a + x.fee, 0);
  if (spent + fee > MAX_SATS_PER_HOUR) return `${spent}+${fee} sats in the last hour > MAX_SATS_PER_HOUR ${MAX_SATS_PER_HOUR}`;
  return null;
}
const saveState = () => fs.writeFileSync(STATE_FILE, JSON.stringify(state, null, 2));
fs.mkdirSync(STATEMENTS_DIR, { recursive: true });
const saveStatement = (stmt: Buffer) => fs.writeFileSync(path.join(STATEMENTS_DIR, hex(sha256(stmt)) + ".hex"), hex(stmt));
const loadStatement = (hash: string): Buffer | null => { const f = path.join(STATEMENTS_DIR, hash + ".hex"); return fs.existsSync(f) ? unhex(fs.readFileSync(f, "utf8").trim()) : null; };

// ---------- chains
function solProvider() {
  const kp = anchor.web3.Keypair.fromSecretKey(Uint8Array.from(JSON.parse(fs.readFileSync((process.env.ANCHOR_WALLET ?? "~/.config/solana/id.json").replace(/^~/, os.homedir()), "utf8"))));
  return new anchor.AnchorProvider(new anchor.web3.Connection(process.env.ANCHOR_PROVIDER_URL ?? "https://api.devnet.solana.com", "confirmed"), new anchor.Wallet(kp), { commitment: "confirmed" });
}
const provider = solProvider();
const factoryIdl = JSON.parse(fs.readFileSync(path.join(SOL_DIR, "target/idl/beta_factory.json"), "utf8"));
const factory: any = new anchor.Program(factoryIdl, provider);
const ipow: any = new anchor.Program(JSON.parse(fs.readFileSync(path.join(SOL_DIR, "target/idl/ipow.json"), "utf8")), provider);
const pda = (seeds: Buffer[], pid = FACTORY_ID) => anchor.web3.PublicKey.findProgramAddressSync(seeds, pid)[0];
const u64le = (n: number | bigint) => new anchor.BN(n.toString()).toArrayLike(Buffer, "le", 8);

const ethEnv = Object.fromEntries(fs.readFileSync(path.join(ETH_DIR, ".env"), "utf8").split("\n").filter((l) => l.includes("=") && !l.startsWith("#")).map((l) => { const i = l.indexOf("="); return [l.slice(0, i).trim(), l.slice(i + 1).trim().replace(/^"|"$/g, "")]; }));
const ethProvider = new ethers.JsonRpcProvider(ethEnv.SEPOLIA_RPC_URL);
const deployed = JSON.parse(fs.readFileSync(path.join(ETH_DIR, "ignition/deployments/chain-11155111/deployed_addresses.json"), "utf8"));
const VAULT = process.env.BETA_VAULT ?? deployed["BetaVaultV4Module#BetaVault"] ?? deployed["BetaVaultV3Module#BetaVault"] ?? deployed["BetaVaultV2Module#BetaVault"];
const IPOW_V1 = process.env.IPOW_V1 ?? deployed["BetaVaultModule#iPoWV1"];
const vaultAbi = JSON.parse(fs.readFileSync(path.join(ETH_DIR, "artifacts/contracts/BetaVault.sol/BetaVault.json"), "utf8")).abi;
const vault = new ethers.Contract(VAULT, vaultAbi, ethProvider);
const ipowV1 = new ethers.Contract(IPOW_V1, ["function globalTipHeight() view returns (uint256)", "function globalHeightToHashLE(uint256) view returns (bytes32)"], ethProvider);

// ---------- drivers
function runSol(env: Record<string, string>) {
  const r = spawnSync("npx", ["ts-node", "scripts/beta_factory_e2e.ts"], { cwd: SOL_DIR, env: { ...process.env, ...env }, encoding: "utf8" });
  return { ok: r.status === 0, out: (r.stdout + r.stderr).trim() };
}
function runEth(env: Record<string, string>) {
  const r = spawnSync("node", ["scripts/beta_vault_e2e.mjs"], { cwd: ETH_DIR, env: { ...process.env, ...env }, encoding: "utf8" });
  return { ok: r.status === 0, out: (r.stdout + r.stderr).trim() };
}
function runAnchor(args: string[], env: Record<string, string>) {
  const r = spawnSync("cargo", ["run", "-q", "--example", "anchor_statement", "-p", "ipow-core", "--", ...args], { cwd: OPERATOR_DIR, env: { ...process.env, ...env, ...(BROADCAST ? { ANCHOR_STATEMENT_CONFIRM: "yes" } : {}) }, encoding: "utf8" });
  return { ok: r.status === 0, out: (r.stdout + r.stderr).trim() };
}

// ---------- header relays
// Extend header-by-header when the gap is small or a Conversion is open
// (contiguity needed for proof windows); otherwise jump straight to the
// target height — BETA only ever reads a header by height. The EVM relay
// additionally needs the target epoch's first header on record before a
// jump (retarget validation), so pre-relay it when missing.
const JUMP_GAP = parseInt(process.env.JUMP_GAP ?? "6");
const DIFF_PERIOD = 2016;
async function relayTo(height: number) {
  const gs = await ipow.account.globalState.fetch(pda([Buffer.from("global_state")], IPOW_ID));
  const solTip = Number(gs.globalTipHeight.toString());
  if (height > solTip) {
    const extend = height - solTip <= JUMP_GAP || Number(gs.activeOpenConversions.toString()) > 0;
    const heights = extend ? Array.from({ length: height - solTip }, (_, i) => solTip + 1 + i) : [height];
    for (const h of heights) { const r = runSol({ ACTION: "relay-header", HEIGHT: String(h) }); log("sol relay", h, extend ? "(extend)" : "(jump)", r.ok ? "ok" : r.out.slice(-120)); if (!r.ok) throw new Error("sol relay failed"); }
  }
  const tip = Number(await ipowV1.globalTipHeight());
  if (height > tip) {
    const extend = height - tip <= JUMP_GAP;
    let heights: number[];
    if (extend) heights = Array.from({ length: height - tip }, (_, i) => tip + 1 + i);
    else {
      const epochStart = height - (height % DIFF_PERIOD);
      heights = [];
      if (epochStart > 0 && epochStart < height && (await ipowV1.globalHeightToHashLE(epochStart)) === ethers.ZeroHash) heights.push(epochStart);
      heights.push(height);
    }
    for (const h of heights) { const r = runEth({ ACTION: "relay-header", HEIGHT: String(h) }); log("eth relay", h, extend ? "(extend)" : "(jump)", r.ok ? "ok" : r.out.slice(-120)); if (!r.ok) throw new Error("eth relay failed"); }
  }
}

// ---------- statements
const stmtMint = (lockId: bigint, solUser: Buffer, nonce: bigint, units: bigint, deadline: bigint) => Buffer.concat([Buffer.from([1]), u64be(lockId), solUser, u64be(nonce), u64be(units), u64be(deadline)]);
const stmtRelease = (lockId: bigint, burnId: bigint, to: Buffer, units: bigint) => Buffer.concat([Buffer.from([2]), u64be(lockId), u64be(burnId), to, u64be(units)]);
const stmtVeto = (target: Buffer, txidLe: Buffer) => Buffer.concat([Buffer.from([3]), target, txidLe]);
const stmtTarget = (kind: number, target: Buffer) => Buffer.concat([Buffer.from([kind]), target]);

/** Try to find the preimage of an anchor's statement hash: local store first, then reconstruction from chain state. */
async function findStatement(kind: number, hash: string): Promise<Buffer | null> {
  const local = loadStatement(hash); if (local) return local;
  const match = (cands: Buffer[]) => cands.find((c) => hex(sha256(c)) === hash) ?? null;
  if (kind === 1) {
    const evs = await vault.queryFilter(vault.filters.Deposited(), 0);
    return match(evs.map((e: any) => stmtMint(BigInt(e.args.lockId), unhex(e.args.solUser), BigInt(e.args.nonce), BigInt(e.args.units), BigInt(e.args.deadline))));
  }
  if (kind === 2) {
    const burns = await factory.account.burn.all();
    const next = Number(await vault.nextLockId());
    const c: Buffer[] = [];
    for (const b of burns) for (let l = 0; l < next; l++) c.push(stmtRelease(BigInt(l), BigInt(b.account.burnId.toString()), Buffer.from(b.account.toEth), BigInt(b.account.units.toString())));
    return match(c);
  }
  if (kind === 3) {
    const anchors = await factory.account.processedAnchor.all();
    const c: Buffer[] = [];
    for (const p of PARTIES) { c.push(stmtVeto(unhex(p), Buffer.alloc(32))); for (const a of anchors) c.push(stmtVeto(unhex(p), Buffer.from(a.account.txidLe))); }
    return match(c);
  }
  if (kind === 4) { const next = Number(await vault.nextLockId()); return match(Array.from({ length: next }, (_, l) => Buffer.concat([Buffer.from([4]), u64be(BigInt(l))]))); }
  if (kind === 5 || kind === 6) { const anchors = await solAnchors(); return match(anchors.map((a: any) => stmtTarget(kind, Buffer.from(a.account.txidLe)))); }
  if (kind === 7) return match(PARTIES.map((p) => stmtTarget(7, unhex(p))));
  return null;
}

/** ProcessedAnchor accounts of the current (v3) layout only: fetch by
 *  discriminator and decode each, skipping accounts of older layouts. */
async function solAnchors(): Promise<any[]> {
  const acc = factoryIdl.accounts.find((a: any) => a.name === "ProcessedAnchor");
  const disc = Buffer.from(acc.discriminator);
  const raw = await provider.connection.getProgramAccounts(FACTORY_ID, { filters: [{ memcmp: { offset: 0, bytes: anchor.utils.bytes.bs58.encode(disc) } }] });
  const V3_SIZE = 236; // 8 + ProcessedAnchor::INIT_SPACE (v3 layout); older anchors are shorter
  const out: any[] = [];
  for (const r of raw) {
    if (r.account.data.length < V3_SIZE) continue;
    try { out.push({ publicKey: r.pubkey, account: factory.coder.accounts.decode("processedAnchor", r.account.data) }); } catch { /* skip undecodable */ }
  }
  return out;
}

// ---------- Bitcoin statement-chain walking
function readVarInt(b: Buffer, o: number): [number, number] { const p = b[o]; if (p < 0xfd) return [p, o + 1]; if (p === 0xfd) return [b.readUInt16LE(o + 1), o + 3]; if (p === 0xfe) return [b.readUInt32LE(o + 1), o + 5]; return [Number(b.readBigUInt64LE(o + 1)), o + 9]; }
function parsePayload(txHex: string): { kind: number; hash: string } | null {
  const tx = unhex(txHex); let o = 4; if (tx[4] === 0x00 && tx[5] === 0x01) o = 6;
  let [inCount, n] = readVarInt(tx, o); o = n;
  for (let i = 0; i < inCount; i++) { o += 36; const [sl, n2] = readVarInt(tx, o); o = n2 + sl + 4; }
  let [outCount, n3] = readVarInt(tx, o); o = n3;
  for (let j = 0; j < outCount; j++) { o += 8; const [sl, n4] = readVarInt(tx, o); o = n4; if (j === 1 && sl === 36 && tx[o] === 0x6a && tx[o + 1] === 34 && tx[o + 2] === 1) return { kind: tx[o + 3], hash: hex(tx.subarray(o + 4, o + 36)) }; o += sl; }
  return null;
}
async function nextAnchor(txid: string, vout: number): Promise<{ txid: string; height: number | null } | null> {
  const s = await getJson(`${ESPLORA}/tx/${txid}/outspend/${vout}`);
  if (!s.spent) return null;
  return { txid: s.txid, height: s.status?.confirmed ? s.status.block_height : null };
}

// ---------- watchtower
async function processOneChain(chain: "sol" | "eth", partyId: string, txid: string, kind: number, hash: string, height: number) {
  const stmt = await findStatement(kind, hash);
  if (!stmt) {
    const hdr = await getJson(`${ESPLORA}/block-height/${height}`).catch(() => null);
    log(chain, "no preimage for", txid.slice(0, 10), "kind", kind, "— will skip once stale");
    return false;
  }
  await relayTo(height);
  const r = chain === "sol"
    ? runSol({ ACTION: "process", PARTY_ID: partyId, TXID: txid, STATEMENT: hex(stmt) })
    : runEth({ ACTION: "process", PARTY_ID: "0x" + partyId, TXID: txid, STATEMENT: hex(stmt) });
  log(chain, "process", txid.slice(0, 10), r.ok ? "ok" : "FAILED " + r.out.slice(-160));
  return r.ok;
}
async function watchtowerCycle() {
  for (const partyId of PARTIES) {
    // Solana side of this party's chain
    const sp = await factory.account.party.fetchNullable(pda([Buffer.from("party"), unhex(partyId)]));
    const ep = await vault.parties("0x" + partyId);
    if (!sp || !ep.exists) continue;
    for (const chain of ["sol", "eth"] as const) {
      let head = chain === "sol" ? { txid: rev(hex(sp.anchorTxidLe)), vout: sp.anchorVout } : { txid: rev(ep.anchorTxidLE), vout: Number(ep.anchorVout) };
      for (let guard = 0; guard < 25; guard++) {
        const nx = await nextAnchor(head.txid, head.vout);
        if (!nx) break;
        if (nx.height === null) { log(chain, partyId.slice(0, 4), "anchor", nx.txid.slice(0, 10), "unconfirmed; waiting"); break; }
        const payload = parsePayload(await getText(`${ESPLORA}/tx/${nx.txid}/hex`));
        if (!payload) { log(chain, "anchor without payload", nx.txid.slice(0, 10)); break; }
        const ok = await processOneChain(chain, partyId, nx.txid, payload.kind, payload.hash, nx.height);
        if (!ok) break;
        head = { txid: nx.txid, vout: 0 };
      }
    }
  }
  // v3: exercise queued mints (attested or past the window), settle closed windows.
  const anchors = await solAnchors();
  const nowS = BigInt(Math.floor(Date.now() / 1000));
  for (const a of anchors) {
    const txid = rev(hex(a.account.txidLe));
    if (a.account.kind === 1 && "queued" in a.account.status && !a.account.held) {
      const attested = hex(a.account.attestedBy) !== "00".repeat(32);
      if (attested || nowS >= BigInt(a.account.challengeUntil.toString())) {
        const r = runSol({ ACTION: "exercise", PARTY_ID: hex(a.account.partyId), TXID: txid });
        log("sol exercise", txid.slice(0, 10), r.ok ? "ok" : r.out.match(/Error Code: (\w+)/)?.[1] ?? "not yet");
      }
    }
    if (a.account.kind === 1 && !a.account.settled && nowS >= BigInt(a.account.challengeUntil.toString())) {
      const r = runSol({ ACTION: "settle", TXID: txid });
      log("sol settle", txid.slice(0, 10), r.ok ? "ok" : r.out.match(/Error Code: (\w+)/)?.[1] ?? "failed");
    }
  }
  const evs = await vault.queryFilter(vault.filters.ReleaseQueued(), 0);
  for (const e of evs as any[]) {
    const a = await vault.anchors(e.args.txidLE);
    if (Number(a.status) !== 4) continue;
    const txid = rev(e.args.txidLE);
    if (nowS >= a.challengeUntil) {
      if (!a.paid && !a.held) { const r = runEth({ ACTION: "execute-release", TXID: txid }); log("eth execute-release", txid.slice(0, 10), r.ok ? "ok" : "refused"); }
      if (!a.settled) { const r = runEth({ ACTION: "settle-release", TXID: txid }); log("eth settle-release", txid.slice(0, 10), r.ok ? "ok" : "refused"); }
    }
  }
  if (AUDITOR_ID) await auditorCycle(anchors);
}

// ---------- auditor: veto where the two chains disagree
async function auditorCycle(anchors: any[]) {
  const nowS = BigInt(Math.floor(Date.now() / 1000));
  for (const a of anchors) {
    const txidBe = rev(hex(a.account.txidLe));
    const ea = await vault.anchors("0x" + hex(a.account.txidLe));
    // False release (Solana says no burn) still queued on Ethereum → VETO (stands until cleared).
    if (a.account.kind === 2 && "slashed" in a.account.status && Number(ea.status) === 4 && !ea.held) {
      log("AUDITOR: false release", txidBe.slice(0, 10), "queued on Ethereum — anchoring VETO");
      await anchorStatement(AUDITOR_ID, stmtVeto(unhex(hex(a.account.partyId)), Buffer.from(a.account.txidLe)));
    }
    // True release (Solana verified the burn) unpaid on Ethereum → ATTEST: pay the user now from our escrow.
    if (a.account.kind === 2 && "exercised" in a.account.status && Number(ea.status) === 4 && !ea.paid && !ea.held && nowS < ea.challengeUntil) {
      log("AUDITOR: verified release", txidBe.slice(0, 10), "— anchoring ATTEST (fast path)");
      await anchorStatement(AUDITOR_ID, stmtTarget(5, Buffer.from(a.account.txidLe)));
    }
    // Queued mint that Ethereum has finalized (lock real) and nobody attested → ATTEST for instant exercise.
    if (a.account.kind === 1 && "queued" in a.account.status && !a.account.held && hex(a.account.attestedBy) === "00".repeat(32) && Number(ea.status) === 1 && nowS < BigInt(a.account.challengeUntil.toString())) {
      log("AUDITOR: mint", txidBe.slice(0, 10), "verified on Ethereum — anchoring ATTEST");
      await anchorStatement(AUDITOR_ID, stmtTarget(5, Buffer.from(a.account.txidLe)));
    }
    // Queued/exercised mint Ethereum found false → VETO on Solana (holds; escrow forfeits at settle).
    if (a.account.kind === 1 && !a.account.held && !a.account.settled && Number(ea.status) === 2) {
      log("AUDITOR: mint", txidBe.slice(0, 10), "slashed on Ethereum — anchoring VETO");
      await anchorStatement(AUDITOR_ID, stmtVeto(unhex(hex(a.account.partyId)), Buffer.from(a.account.txidLe)));
    }
  }
  for (const p of PARTIES) {
    const ep = await vault.parties("0x" + p); const sp = await factory.account.party.fetchNullable(pda([Buffer.from("party"), unhex(p)]));
    if (ep.exists && ep.dead && sp && !sp.dead && sp.pausedUntil.toString() === "0" && "operator" in sp.kind) {
      log("AUDITOR: operator", p.slice(0, 4), "dead on Ethereum, live on Solana — anchoring dead-veto");
      await anchorStatement(AUDITOR_ID, stmtVeto(unhex(p), Buffer.alloc(32)));
    }
  }
}

// ---------- anchoring (operator + auditor share this)
async function bitcoinHead(partyId: string): Promise<{ txid: string; vout: number }> {
  // Walk from the on-chain head to the true tip (anchors may be broadcast but not yet processed).
  const sp = await factory.account.party.fetch(pda([Buffer.from("party"), unhex(partyId)]));
  let head = { txid: rev(hex(sp.anchorTxidLe)), vout: sp.anchorVout as number };
  for (let g = 0; g < 50; g++) { const nx = await nextAnchor(head.txid, head.vout); if (!nx) break; head = { txid: nx.txid, vout: 0 }; }
  return head;
}
/** Anchors `stmt` on `partyId`'s chain. Returns the txid, or null if it could not be funded/broadcast. */
async function anchorStatement(partyId: string, stmt: Buffer): Promise<string | null> {
  // Never anchor the same statement twice: on-chain state only changes once
  // the first anchor is confirmed and processed, so without this every
  // cycle would re-fire it (cost us 311 sats on 2026-09-22).
  const key = `${partyId}:${hex(sha256(stmt))}`;
  if (state.anchoredStatements[key]) { log("already anchored", key.slice(0, 12), "→", state.anchoredStatements[key].slice(0, 10)); return state.anchoredStatements[key]; }
  saveStatement(stmt);
  const head = await bitcoinHead(partyId);
  const addr = fs.readFileSync(path.join(OPERATOR_DIR, ".env"), "utf8").match(/^OPERATOR_BTC_WALLET_ADDRESS=(.*)$/m)![1].trim();
  const utxos: any[] = await getJson(`${ESPLORA}/address/${addr}/utxo`);
  const rate = Math.max(parseFloat(process.env.MIN_FEE_RATE ?? "1"), (await getJson(`${ESPLORA}/fee-estimates`))["6"] ?? 1);
  const fee = Math.ceil(rate * 260);
  // Any non-head UTXO that covers the fee (heads of other parties are dust; never spend those).
  const heads = new Set<string>();
  for (const p of PARTIES) { const h = await bitcoinHead(p).catch(() => null); if (h) heads.add(`${h.txid}:${h.vout}`); }
  const fund = utxos.filter((u) => !heads.has(`${u.txid}:${u.vout}`) && u.value >= fee).sort((a, b) => b.value - a.value)[0];
  if (!fund) {
    const have = utxos.filter((u) => !heads.has(`${u.txid}:${u.vout}`)).reduce((a, u) => a + u.value, 0);
    log(`cannot anchor: need a funding UTXO ≥ ${fee} sats (${rate} sat/vB); wallet ${addr} has ${have} spendable sats — top up ~${Math.max(0, 2 * fee + 300 - have)} sats`);
    return null;
  }
  if (BROADCAST) { const why = spendAllowed(fee); if (why) { log(`SPEND CAP: not anchoring (${why})`); return null; } }
  const r = runAnchor([head.txid, String(head.vout), String(stmt[0]), hex(stmt), String(HEAD_SATS), String(fee)], { FUND_MAIN: "1", FUND_TXID: fund.txid, FUND_VOUT: String(fund.vout) });
  const txid = r.out.match(/^txid: (\w+)/m)?.[1] ?? null;
  const broadcastOk = !BROADCAST || /^broadcast: 200/m.test(r.out);
  if (txid && broadcastOk && BROADCAST) { state.anchoredStatements[key] = txid; state.spends.push({ t: Date.now(), fee, txid }); saveState(); }
  log(BROADCAST ? "anchored" : "DRY-RUN anchor", "kind", stmt[0], txid ?? r.out.slice(-200), broadcastOk ? "" : "BROADCAST FAILED " + (r.out.match(/^broadcast: .*/m)?.[0] ?? ""));
  return txid && broadcastOk ? txid : null;
}

// ---------- operator
async function operatorCycle() {
  // MINTs: Ethereum locks that are PENDING and whose Solana pending slot is approved for them.
  const evs = await vault.queryFilter(vault.filters.Deposited(), 0);
  for (const e of evs as any[]) {
    const lockId = e.args.lockId.toString();
    if (state.anchoredLocks[lockId]) continue;
    const l = await vault.locks(e.args.lockId);
    if (Number(l.state) !== 1) continue;
    const solUser = new anchor.web3.PublicKey(unhex(e.args.solUser));
    const pendingPda = pda([Buffer.from("pending"), solUser.toBuffer(), u64le(BigInt(e.args.nonce))]);
    const p = await factory.account.pending.fetchNullable(pendingPda);
    if (!p || !p.approved || p.ethLockId.toString() !== lockId || p.queued) continue;
    if (p.units.toString() !== e.args.units.toString() || p.deadline.toString() !== e.args.deadline.toString()) { log("lock", lockId, "does not match pending; not anchoring"); continue; }
    log("OPERATOR: lock", lockId, "matches Solana pending — anchoring MINT");
    const mintTxid = await anchorStatement(OPERATOR_ID, stmtMint(BigInt(lockId), unhex(e.args.solUser), BigInt(e.args.nonce), BigInt(e.args.units), BigInt(e.args.deadline)));
    if (!mintTxid) continue; // retry next cycle
    state.anchoredLocks[lockId] = mintTxid; saveState();
    // v3 fast path: the operator attests its own MINT (escrow from its bond) so the user needn't wait the window.
    if (process.env.SELF_ATTEST !== "no") await anchorStatement(OPERATOR_ID, stmtTarget(5, unhex(mintTxid).reverse()));
  }
  // RELEASEs: unclaimed Solana burns → a FINAL unreleased lock of the same size, else insurance.
  const burns = await factory.account.burn.all();
  const next = Number(await vault.nextLockId());
  for (const b of burns) {
    const id = b.account.burnId.toString();
    if (b.account.claimed || state.anchoredBurns[id]) continue;
    let lockId = BigInt(0);
    for (let l = 1; l < next; l++) { const lk = await vault.locks(l); if (Number(lk.state) === 2 && lk.units.toString() === b.account.units.toString()) { lockId = BigInt(l); break; } }
    if (lockId === BigInt(0)) {
      const cover = (await vault.insurance()) - (await vault.insuranceReserved());
      if (cover < BigInt(b.account.units.toString()) * (await vault.params()).ethWeiPerUnit) { log("burn", id, "has no FINAL lock and insurance can't cover it; waiting"); continue; }
    }
    log("OPERATOR: burn", id, "→ RELEASE via", lockId === BigInt(0) ? "insurance" : `lock ${lockId}`);
    const relTxid = await anchorStatement(OPERATOR_ID, stmtRelease(lockId, BigInt(id), Buffer.from(b.account.toEth), BigInt(b.account.units.toString())));
    if (!relTxid) continue;
    state.anchoredBurns[id] = relTxid; saveState();
    if (process.env.SELF_ATTEST !== "no") await anchorStatement(OPERATOR_ID, stmtTarget(5, unhex(relTxid).reverse()));
  }
}

async function main() {
  log(`beta daemon role=${ROLE} vault=${VAULT} parties=${PARTIES.map((p) => p.slice(0, 4)).join(",")} broadcast=${BROADCAST} caps: fee≤${MAX_FEE_SATS} sats, ≤${MAX_ANCHORS_PER_HOUR} anchors/h, ≤${MAX_SATS_PER_HOUR} sats/h`);
  for (;;) {
    try {
      if (ROLE === "watchtower" || ROLE === "both") await watchtowerCycle();
      if ((ROLE === "operator" || ROLE === "both") && OPERATOR_ID) await operatorCycle();
    } catch (e: any) { log("cycle error:", e.message ?? e, process.env.DEBUG ? e.stack : ""); }
    if (process.env.ONCE) break;
    await new Promise((r) => setTimeout(r, INTERVAL));
  }
}
main();
