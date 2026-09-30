// BETA v2 Sepolia driver for BetaVault + its iPoW relay (docs/design/ipow-implementation.md §6).
// Reads SEPOLIA_RPC_URL / SEPOLIA_PRIVATE_KEY from ./.env and the deployed
// addresses from ./ignition/deployments/chain-11155111/deployed_addresses.json
// (override with IPOW_V1 / BETA_VAULT env vars).
//
//   ACTION=status            [LOCK_ID=n] [PARTY_ID=hex] [TXID=hex]
//   ACTION=approve-operator   PARTY_ID=<32-byte hex>            (governance)
//   ACTION=register           PARTY_ID=... KIND=0|1 ANCHOR_TXID=<be hex> ANCHOR_VOUT=n BOND_ETH=0.01
//   ACTION=deposit            SOL_USER=<32-byte hex> NONCE=1 UNITS=1 DEADLINE_SECS=86400
//   ACTION=relay-header       HEIGHT=<bitcoin height>
//   ACTION=process            PARTY_ID=... TXID=<be hex> STATEMENT=<hex>
//   ACTION=execute-release    TXID=<be hex>            (v3 slow path, after the challenge window)
//   ACTION=settle-release     TXID=<be hex>            (v3: reimburse attester / cancel held)
//   ACTION=fund-rewards       AMOUNT_ETH=0.002
//   ACTION=refund             LOCK_ID=n
//
//   node scripts/beta_vault_e2e.mjs
import fs from "node:fs";
import { createHash } from "node:crypto";
import { ethers } from "ethers";

const env = Object.fromEntries(fs.readFileSync(".env", "utf8").split("\n").filter((l) => l.includes("=") && !l.startsWith("#")).map((l) => { const i = l.indexOf("="); return [l.slice(0, i).trim(), l.slice(i + 1).trim().replace(/^"|"$/g, "")]; }));
const E = (k, d) => process.env[k] ?? d ?? (() => { throw new Error(`missing ${k}`); })();
const ESPLORA = "https://blockstream.info/api";
const unhex = (s) => Buffer.from(s.replace(/^0x/, ""), "hex");
const sha256 = (b) => createHash("sha256").update(b).digest();
const dsha = (b) => sha256(sha256(b));

const provider = new ethers.JsonRpcProvider(env.SEPOLIA_RPC_URL);
const wallet = new ethers.Wallet(env.SEPOLIA_PRIVATE_KEY, provider);
const deployed = JSON.parse(fs.readFileSync("ignition/deployments/chain-11155111/deployed_addresses.json", "utf8"));
const IPOW = process.env.IPOW_V1 ?? deployed["BetaVaultModule#iPoW"];
const VAULT = process.env.BETA_VAULT ?? deployed["BetaVaultV4Module#BetaVault"] ?? deployed["BetaVaultV3Module#BetaVault"] ?? deployed["BetaVaultV2Module#BetaVault"] ?? deployed["BetaVaultModule#BetaVault"];
const artifact = (n) => JSON.parse(fs.readFileSync(`artifacts/contracts/${n}.sol/${n}.json`, "utf8")).abi;
const ipow = new ethers.Contract(IPOW, artifact("iPoW"), wallet);
const vault = new ethers.Contract(VAULT, artifact("BetaVault"), wallet);

function readVarInt(b, o) { const p = b[o]; if (p < 0xfd) return [p, o + 1]; if (p === 0xfd) return [b.readUInt16LE(o + 1), o + 3]; if (p === 0xfe) return [b.readUInt32LE(o + 1), o + 5]; return [Number(b.readBigUInt64LE(o + 1)), o + 9]; }
function stripWitness(tx) {
  if (!(tx[4] === 0x00 && tx[5] === 0x01)) return tx;
  let o = 6; const start = o;
  let [inCount, n] = readVarInt(tx, o); o = n;
  for (let i = 0; i < inCount; i++) { o += 36; const [slen, n2] = readVarInt(tx, o); o = n2 + slen + 4; }
  let [outCount, n3] = readVarInt(tx, o); o = n3;
  for (let j = 0; j < outCount; j++) { o += 8; const [slen, n4] = readVarInt(tx, o); o = n4 + slen; }
  return Buffer.concat([tx.subarray(0, 4), tx.subarray(start, o), tx.subarray(tx.length - 4)]);
}
const getText = async (u) => { const r = await fetch(u); if (!r.ok) throw new Error(`${u}: ${r.status}`); return (await r.text()).trim(); };
const getJson = async (u) => { const r = await fetch(u); if (!r.ok) throw new Error(`${u}: ${r.status}`); return r.json(); };
const wait = async (p) => { const tx = await p; const rc = await tx.wait(); console.log("tx", tx.hash, "gas", rc.gasUsed.toString()); return rc; };

const action = E("ACTION", "status");
console.log("wallet", wallet.address, "iPoW", IPOW, "BetaVault", VAULT);

if (action === "status") {
  console.log("relay tip", (await ipow.globalTipHeight()).toString(), "operator", await ipow.operator());
  console.log("vault: nextLockId", (await vault.nextLockId()).toString(), "totalLocked", ethers.formatEther(await vault.totalLocked()), "insurance", ethers.formatEther(await vault.insurance()), "totalBonds", ethers.formatEther(await vault.totalBonds()), "paused", await vault.paused());
  if (process.env.LOCK_ID) console.log("lock", await vault.locks(E("LOCK_ID")));
  if (process.env.PARTY_ID) console.log("party", await vault.parties(E("PARTY_ID")));
  if (process.env.TXID) console.log("anchor", await vault.anchors("0x" + unhex(E("TXID")).reverse().toString("hex")));
} else if (action === "approve-operator") {
  await wait(vault.approveOperator(E("PARTY_ID"), wallet.address));
} else if (action === "register") {
  await wait(vault.registerParty(E("PARTY_ID"), parseInt(E("KIND", "0")), "0x" + unhex(E("ANCHOR_TXID")).reverse().toString("hex"), parseInt(E("ANCHOR_VOUT", "0")), { value: ethers.parseEther(E("BOND_ETH")) }));
} else if (action === "deposit") {
  const p = await vault.params();
  const units = BigInt(E("UNITS", "1"));
  const deadline = process.env.DEADLINE ? BigInt(E("DEADLINE")) : BigInt(Math.floor(Date.now() / 1000) + parseInt(E("DEADLINE_SECS", "86400")));
  const rc = await wait(vault.deposit(E("SOL_USER"), BigInt(E("NONCE")), units, deadline, { value: units * p.ethWeiPerUnit }));
  const ev = rc.logs.map((l) => { try { return vault.interface.parseLog(l); } catch { return null; } }).find((e) => e && e.name === "Deposited");
  console.log("lockId", ev.args.lockId.toString(), "deadline", deadline.toString());
} else if (action === "relay-header") {
  const height = parseInt(E("HEIGHT"));
  const hash = await getText(`${ESPLORA}/block-height/${height}`);
  const header = "0x" + (await getText(`${ESPLORA}/block/${hash}/header`));
  await wait(ipow.commitGlobalBitcoinHeader80(header, height));
  console.log("relayed", height, hash, "tip", (await ipow.globalTipHeight()).toString());
} else if (action === "process") {
  const txidBe = E("TXID");
  const raw = stripWitness(unhex(await getText(`${ESPLORA}/tx/${txidBe}/hex`)));
  if (!dsha(raw).equals(unhex(txidBe).reverse())) throw new Error("stripped tx does not hash to txid");
  const proof = await getJson(`${ESPLORA}/tx/${txidBe}/merkle-proof`);
  const branchLe = proof.merkle.map((h) => "0x" + unhex(h).reverse().toString("hex"));
  if ((await ipow.globalHeightToHashLE(proof.block_height)) === ethers.ZeroHash) throw new Error(`relay header ${proof.block_height} missing; ACTION=relay-header HEIGHT=${proof.block_height}`);
  await wait(vault.processAnchor(E("PARTY_ID"), "0x" + unhex(E("STATEMENT")).toString("hex"), "0x" + raw.toString("hex"), proof.block_height, branchLe, proof.pos));
  console.log("anchor", await vault.anchors("0x" + unhex(txidBe).reverse().toString("hex")));
} else if (action === "execute-release") {
  await wait(vault.executeRelease("0x" + unhex(E("TXID")).reverse().toString("hex")));
} else if (action === "settle-release") {
  await wait(vault.settleRelease("0x" + unhex(E("TXID")).reverse().toString("hex")));
  console.log("anchor", await vault.anchors("0x" + unhex(E("TXID")).reverse().toString("hex")));
} else if (action === "fund-rewards") {
  await wait(vault.fundRewards({ value: ethers.parseEther(E("AMOUNT_ETH")) }));
} else if (action === "withdraw-bond") {
  await wait(vault.withdrawBond(E("PARTY_ID")));
} else if (action === "refund") {
  await wait(vault.refund(BigInt(E("LOCK_ID"))));
} else {
  throw new Error(`unknown ACTION ${action}`);
}
