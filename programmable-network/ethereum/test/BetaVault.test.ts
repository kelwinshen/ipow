import { expect } from "chai";
import { network } from "hardhat";
import { sha256, getBytes, hexlify, concat, toBeHex, zeroPadValue, AbiCoder, keccak256 } from "ethers";

import { IPoWV1__factory, BetaVault__factory, MockERC20__factory } from "../types/ethers-contracts/index.ts";
import { COMMIT_FEE_BPS, NATIVE_DECIMALS, SELF_NETWORK_ID } from "./helpers/deploy.ts";

const { ethers } = await network.create();

const KIND_MINT = 1;
const KIND_RELEASE = 2;
const KIND_VETO = 3;
const KIND_CANCEL = 4;
const KIND_ATTEST = 5;
const KIND_CLEAR = 6;
const KIND_ALIVE = 7;
const T_CHALLENGE = 7 * 86400;
const GENESIS = "0x" + "07".repeat(32);
const OP_ID = "0x" + "01".repeat(32);
const AUD_ID = "0x" + "02".repeat(32);
const ZERO32 = "0x" + "00".repeat(32);
const ONE = ethers.parseEther("1");

function dsha256(data: Uint8Array): string {
  return sha256(getBytes(sha256(data)));
}
function u64be(v: bigint | number): Uint8Array {
  return getBytes(zeroPadValue(toBeHex(BigInt(v)), 8));
}
function u32le(v: number): Uint8Array {
  return getBytes(zeroPadValue(toBeHex(v), 4)).reverse();
}
function u64le(v: bigint): Uint8Array {
  return getBytes(zeroPadValue(toBeHex(v), 8)).reverse();
}

/// Anchor on a statement chain: spends (prev, vout); out0 = new head; out1 = OP_RETURN ver|kind|hash.
function anchorTx(prevTxidLE: string, prevVout: number, payload: { kind: number; hash: string } | null, salt: number): string {
  const p: Uint8Array[] = [];
  p.push(u32le(1));
  p.push(Uint8Array.of(0x01));
  p.push(getBytes(prevTxidLE));
  p.push(u32le(prevVout));
  p.push(Uint8Array.of(0x01, salt & 0xff));
  p.push(new Uint8Array(4).fill(0xff));
  p.push(Uint8Array.of(0x02));
  p.push(u64le(1000n));
  p.push(Uint8Array.of(22, 0x00, 0x14));
  p.push(new Uint8Array(20).fill(0x11));
  p.push(u64le(0n));
  if (payload) {
    p.push(Uint8Array.of(36, 0x6a, 34, 1, payload.kind));
    p.push(getBytes(payload.hash));
  } else {
    p.push(Uint8Array.of(3, 0x6a, 0x01, 0x00));
  }
  return hexlify(concat(p));
}

// compositionId/componentIndex are Solana-hub routing this contract never
// judges (DESIGN_V2 §8.4) — fixed at 0 here since these tests only exercise
// this contract's own predicate, which parses past them unconditionally.
function stmtMint(lockId: bigint, solUser: string, nonce: bigint, units: bigint, deadline: bigint): string {
  return hexlify(
    concat([Uint8Array.of(KIND_MINT), u64be(0n), Uint8Array.of(0), u64be(lockId), getBytes(solUser), u64be(nonce), u64be(units), u64be(deadline)]),
  );
}
function stmtRelease(lockId: bigint, burnId: bigint, to: string, units: bigint): string {
  return hexlify(concat([Uint8Array.of(KIND_RELEASE), u64be(lockId), u64be(burnId), getBytes(to), u64be(units)]));
}
function stmtVeto(targetPartyId: string, targetTxid: string): string {
  return hexlify(concat([Uint8Array.of(KIND_VETO), getBytes(targetPartyId), getBytes(targetTxid)]));
}
function stmtTarget(kind: number, target: string): string {
  return hexlify(concat([Uint8Array.of(kind), getBytes(target)]));
}
function stmtCancel(lockId: bigint): string {
  return hexlify(concat([Uint8Array.of(KIND_CANCEL), u64be(lockId)]));
}

/// Seeds `iPoWV1`'s header maps via storage cheat-code (same slot layout
/// `iPoWV1Conversion.test.ts::seedHeader` derives), plus a timestamp.
async function seedHeader(ipowAddress: string, height: bigint, headerHashLE: string, merkleRootLE: string, timestamp: bigint) {
  const abi = AbiCoder.defaultAbiCoder();
  const heightSlot = keccak256(abi.encode(["uint256", "uint256"], [height, 9n]));
  await ethers.provider.send("hardhat_setStorageAt", [ipowAddress, heightSlot, headerHashLE]);
  const baseSlot = BigInt(keccak256(abi.encode(["bytes32", "uint256"], [headerHashLE, 10n])));
  await ethers.provider.send("hardhat_setStorageAt", [ipowAddress, zeroPadValue(toBeHex(baseSlot + 1n), 32), merkleRootLE]);
  const packed = (1n << 64n) | (timestamp << 32n);
  await ethers.provider.send("hardhat_setStorageAt", [ipowAddress, zeroPadValue(toBeHex(baseSlot + 2n), 32), zeroPadValue(toBeHex(packed), 32)]);
}

async function setTip(ipowAddress: string, tip: bigint) {
  // globalTipHeight is slot 11 (after globalHeaders at 10) — verified by reading it back below.
  await ethers.provider.send("hardhat_setStorageAt", [ipowAddress, zeroPadValue(toBeHex(11n), 32), zeroPadValue(toBeHex(tip), 32)]);
}

async function now(): Promise<bigint> {
  const b = await ethers.provider.getBlock("latest");
  return BigInt(b!.timestamp);
}
async function warp(seconds: number) {
  await ethers.provider.send("evm_increaseTime", [seconds]);
  await ethers.provider.send("evm_mine", []);
}

const PARAMS = {
  ethWeiPerUnit: ONE,
  tFinSecs: 1800n,
  tChallengeSecs: BigInt(T_CHALLENGE),
  tSkipSecs: 3600n,
  refundMarginSecs: 600n,
  unbondDelaySecs: 60n,
  minOperatorBond: ethers.parseEther("5"),
  minAuditorBond: ONE,
  vetoSlashWei: ethers.parseEther("0.5"),
  vetoRewardWei: ethers.parseEther("0.1"),
  bountyBps: 1000n,
};

async function deploy() {
  const [gov, op, aud, user, other] = await ethers.getSigners();
  const ipow = await new IPoWV1__factory(gov).deploy(NATIVE_DECIMALS, SELF_NETWORK_ID, await gov.getAddress(), COMMIT_FEE_BPS);
  await ipow.waitForDeployment();
  const vault = await new BetaVault__factory(gov).deploy(await gov.getAddress(), await ipow.getAddress(), PARAMS);
  await vault.waitForDeployment();
  await vault.connect(gov).approveOperator(OP_ID, await op.getAddress());
  await vault.connect(op).registerParty(OP_ID, 0, GENESIS, 0, { value: ethers.parseEther("10") });
  await vault.connect(aud).registerParty(AUD_ID, 1, GENESIS, 0, { value: ethers.parseEther("2") });
  const ipowAddr = await ipow.getAddress();
  await setTip(ipowAddr, 1000n);
  expect(await ipow.globalTipHeight()).to.equal(1000n);
  let height = 2000n;
  /// Anchors `statement` on `partyId`'s chain from `prev`, seeds its header, processes it. Returns txid.
  /// `noWait`: return the unawaited promise (for revert assertions); otherwise the tx is mined before returning.
  async function process(partyId: string, prev: [string, number], statement: string, submitter: any, salt: number, opts: { late?: number; noWait?: boolean } = {}) {
    const txRaw = anchorTx(prev[0], prev[1], { kind: parseInt(statement.slice(2, 4), 16), hash: sha256(statement) }, salt);
    const txid = dsha256(getBytes(txRaw));
    const h = height++;
    const ts = (await now()) - BigInt(opts.late ?? 0);
    await seedHeader(ipowAddr, h, keccak256(ethers.toUtf8Bytes(`hdr-${h}`)), txid, ts);
    const tx = vault.connect(submitter).processAnchor(partyId, statement, txRaw, h, [], 0);
    if (!opts.noWait) await (await tx).wait();
    return { txid, tx };
  }
  async function skip(partyId: string, prev: [string, number], payload: { kind: number; hash: string } | null, submitter: any, salt: number) {
    const txRaw = anchorTx(prev[0], prev[1], payload, salt);
    const txid = dsha256(getBytes(txRaw));
    const h = height++;
    await seedHeader(ipowAddr, h, keccak256(ethers.toUtf8Bytes(`hdr-${h}`)), txid, await now());
    const tx = vault.connect(submitter).skipAnchor(partyId, txRaw, h, [], 0);
    return { txid, tx, h };
  }
  return { gov, op, aud, user, other, ipow, vault, ipowAddr, process, skip };
}

const SOL_USER = "0x" + "aa".repeat(32);

describe("BetaVault: locks and MINT anchors", function () {
  it("a matching MINT anchor finalizes the lock; a late reveal does not; refunds follow the state", async function () {
    const { op, user, vault, process } = await deploy();
    const deadline = (await now()) + 86_400n;
    await vault.connect(user).deposit(ethers.ZeroAddress, SOL_USER, 1n, 2n, deadline, { value: 2n * ONE });
    expect((await vault.locks(1n)).state).to.equal(1); // Pending
    expect(await vault.totalLocked(ethers.ZeroAddress)).to.equal(2n * ONE);

    // Pending → refund not allowed before deadline+margin.
    await expect(vault.connect(user).refund(1n)).to.be.revertedWithCustomError(vault, "RefundNotReady");

    const { txid, tx } = await process(OP_ID, [GENESIS, 0], stmtMint(1n, SOL_USER, 1n, 2n, deadline), op, 1);
    await expect(tx).to.emit(vault, "Finalized").withArgs(1n, txid);
    expect((await vault.locks(1n)).state).to.equal(2); // Final
    const p = await vault.parties(OP_ID);
    expect(p.anchorTxidLE).to.equal(txid);
    expect(p.seq).to.equal(1n);

    // Final locks never refund.
    await warp(90_000);
    await expect(vault.connect(user).refund(1n)).to.be.revertedWithCustomError(vault, "BadLockState");

    // A second lock whose MINT anchor is revealed too late stays Pending and refunds.
    const d2 = (await now()) + 86_400n;
    await vault.connect(user).deposit(ethers.ZeroAddress, SOL_USER, 2n, 1n, d2, { value: ONE });
    const r2 = await process(OP_ID, [txid, 0], stmtMint(2n, SOL_USER, 2n, 1n, d2), op, 2, { late: 1801 });
    await expect(r2.tx).to.not.emit(vault, "Finalized");
    expect((await vault.locks(2n)).state).to.equal(1);
    expect((await vault.anchors(r2.txid)).status).to.equal(1); // Exercised, no slash — the statement was true
    await warp(86_400 + 601);
    const before = await ethers.provider.getBalance(await user.getAddress());
    const rtx = await vault.connect(user).refund(2n);
    const rc = await rtx.wait();
    const after = await ethers.provider.getBalance(await user.getAddress());
    expect(after - before + rc!.gasUsed * rc!.gasPrice).to.equal(ONE);
  });

  it("a MINT for a lock that does not exist is a lie: ETH bond slashed to insurance, bounty to submitter", async function () {
    const { op, other, vault, process } = await deploy();
    const stmt = stmtMint(99n, SOL_USER, 1n, 3n, (await now()) + 1000n);
    const { txid, tx } = await process(OP_ID, [GENESIS, 0], stmt, other, 1);
    await expect(tx).to.emit(vault, "Slashed").withArgs(OP_ID, 3n * ONE, await other.getAddress());
    expect((await vault.anchors(txid)).status).to.equal(2); // Slashed
    const p = await vault.parties(OP_ID);
    expect(p.dead).to.equal(true);
    expect(p.bond).to.equal(ethers.parseEther("7"));
    expect(await vault.insurance()).to.equal((3n * ONE * 9n) / 10n);
  });

  it("a MINT with the wrong amount for an existing lock is also a lie", async function () {
    const { op, user, vault, process } = await deploy();
    const deadline = (await now()) + 86_400n;
    await vault.connect(user).deposit(ethers.ZeroAddress, SOL_USER, 1n, 2n, deadline, { value: 2n * ONE });
    const { txid } = await process(OP_ID, [GENESIS, 0], stmtMint(1n, SOL_USER, 1n, 3n, deadline), op, 1);
    expect((await vault.anchors(txid)).status).to.equal(2);
    expect((await vault.locks(1n)).state).to.equal(1); // untouched
  });

  it("CANCEL refunds a pending lock", async function () {
    const { op, user, vault, process } = await deploy();
    const deadline = (await now()) + 86_400n;
    await vault.connect(user).deposit(ethers.ZeroAddress, SOL_USER, 1n, 1n, deadline, { value: ONE });
    const { tx } = await process(OP_ID, [GENESIS, 0], stmtCancel(1n), op, 1);
    await expect(tx).to.emit(vault, "Refunded").withArgs(1n);
    expect((await vault.locks(1n)).state).to.equal(4);
    expect(await vault.totalLocked(ethers.ZeroAddress)).to.equal(0n);
  });
});

describe("BetaVault: RELEASE — v3 challenge window, attest, veto, settle", function () {
  async function finalizedLock(d: Awaited<ReturnType<typeof deploy>>, units: bigint) {
    const deadline = (await now()) + 86_400n;
    await d.vault.connect(d.user).deposit(ethers.ZeroAddress, SOL_USER, 1n, units, deadline, { value: units * ONE });
    const { txid } = await d.process(OP_ID, [GENESIS, 0], stmtMint(1n, SOL_USER, 1n, units, deadline), d.op, 1);
    expect((await d.vault.locks(1n)).state).to.equal(2);
    return txid;
  }

  it("slow path: an unattested release pays from the vault only after the window", async function () {
    const d = await deploy();
    const mtx = await finalizedLock(d, 2n);
    const to = await d.other.getAddress();
    const { txid, tx } = await d.process(OP_ID, [mtx, 0], stmtRelease(1n, 0n, to, 2n), d.op, 2);
    await expect(tx).to.emit(d.vault, "ReleaseQueued");
    expect((await d.vault.anchors(txid)).status).to.equal(4);
    await expect(d.vault.executeRelease(txid)).to.be.revertedWithCustomError(d.vault, "NotReady");
    await warp(T_CHALLENGE + 1);
    const before = await ethers.provider.getBalance(to);
    await expect(d.vault.executeRelease(txid)).to.emit(d.vault, "Released").withArgs(txid, 1n, to, 2n * ONE);
    expect((await ethers.provider.getBalance(to)) - before).to.equal(2n * ONE);
    expect(await d.vault.totalLocked(ethers.ZeroAddress)).to.equal(0n);
    await expect(d.vault.executeRelease(txid)).to.be.revertedWithCustomError(d.vault, "BadAnchorState");
  });

  it("fast path: an ATTEST pays the user now from the attester's escrow; settle reimburses it after the window", async function () {
    const d = await deploy();
    const mtx = await finalizedLock(d, 1n);
    const to = await d.other.getAddress();
    const { txid } = await d.process(OP_ID, [mtx, 0], stmtRelease(1n, 0n, to, 1n), d.op, 2);
    const before = await ethers.provider.getBalance(to);
    const { tx } = await d.process(AUD_ID, [GENESIS, 0], stmtTarget(KIND_ATTEST, txid), d.aud, 3);
    await expect(tx).to.emit(d.vault, "ReleasePaidFromEscrow");
    expect((await ethers.provider.getBalance(to)) - before).to.equal(ONE);
    expect((await d.vault.parties(AUD_ID)).bond).to.equal(ethers.parseEther("2") - ONE);
    expect(await d.vault.totalLocked(ethers.ZeroAddress)).to.equal(ONE); // vault untouched so far
    await expect(d.vault.settleRelease(txid)).to.be.revertedWithCustomError(d.vault, "NotReady");
    await expect(d.vault.executeRelease(txid)).to.be.revertedWithCustomError(d.vault, "BadAnchorState"); // already paid
    await warp(T_CHALLENGE + 1);
    await expect(d.vault.settleRelease(txid)).to.emit(d.vault, "ReleaseSettled").withArgs(txid, true);
    expect((await d.vault.parties(AUD_ID)).bond).to.equal(ethers.parseEther("2"));
    expect(await d.vault.totalLocked(ethers.ZeroAddress)).to.equal(0n);
    expect((await d.vault.anchors(txid)).status).to.equal(1);
  });

  it("a held attested release is never reimbursed: the lock returns to FINAL and the vault never paid", async function () {
    const d = await deploy();
    const mtx = await finalizedLock(d, 1n);
    const to = await d.other.getAddress();
    const { txid } = await d.process(OP_ID, [mtx, 0], stmtRelease(1n, 0n, to, 1n), d.op, 2);
    const a = await d.process(AUD_ID, [GENESIS, 0], stmtTarget(KIND_ATTEST, txid), d.aud, 3);
    // Another auditor vetoes (judged on Solana); no fee, no expiry.
    const AUD2 = "0x" + "03".repeat(32);
    await d.vault.connect(d.other).registerParty(AUD2, 1, GENESIS, 0, { value: ethers.parseEther("2") });
    const v = await d.process(AUD2, [GENESIS, 0], stmtVeto(OP_ID, txid), d.other, 4);
    await expect(v.tx).to.emit(d.vault, "ReleaseHeld").withArgs(txid, true);
    await warp(30 * 86400);
    expect((await d.vault.anchors(txid)).held).to.equal(true); // no clock lifts it
    await expect(d.vault.settleRelease(txid)).to.emit(d.vault, "ReleaseSettled").withArgs(txid, false);
    expect((await d.vault.anchors(txid)).status).to.equal(5); // Cancelled
    expect((await d.vault.locks(1n)).state).to.equal(2); // back to FINAL
    expect(await d.vault.totalLocked(ethers.ZeroAddress)).to.equal(ONE);
    expect((await d.vault.parties(AUD_ID)).bond).to.equal(ethers.parseEther("2") - ONE); // attester ate it
    void a;
  });

  it("a CLEAR lifts a hold; an unattested held release cannot execute", async function () {
    const d = await deploy();
    const mtx = await finalizedLock(d, 1n);
    const to = await d.other.getAddress();
    const { txid } = await d.process(OP_ID, [mtx, 0], stmtRelease(1n, 0n, to, 1n), d.op, 2);
    const v = await d.process(AUD_ID, [GENESIS, 0], stmtVeto(OP_ID, txid), d.aud, 3);
    await warp(T_CHALLENGE + 1);
    await expect(d.vault.executeRelease(txid)).to.be.revertedWithCustomError(d.vault, "Held");
    await d.process(AUD_ID, [v.txid, 0], stmtTarget(KIND_CLEAR, txid), d.aud, 4);
    await expect(d.vault.executeRelease(txid)).to.emit(d.vault, "Released");
  });

  it("a RELEASE on a lock that is not FINAL is a lie about Ethereum", async function () {
    const d = await deploy();
    const deadline = (await now()) + 86_400n;
    await d.vault.connect(d.user).deposit(ethers.ZeroAddress, SOL_USER, 1n, 1n, deadline, { value: ONE });
    const { txid } = await d.process(OP_ID, [GENESIS, 0], stmtRelease(1n, 0n, await d.other.getAddress(), 1n), d.op, 1);
    expect((await d.vault.anchors(txid)).status).to.equal(2);
    expect((await d.vault.parties(OP_ID)).dead).to.equal(true);
    expect((await d.vault.locks(1n)).state).to.equal(1);
  });

  it("a RELEASE with lockId 0 redeems from insurance when it can cover it, else it is a lie", async function () {
    const d = await deploy();
    const to = await d.other.getAddress();
    const bad = await d.process(OP_ID, [GENESIS, 0], stmtRelease(0n, 0n, to, 2n), d.op, 1); // 2 units: slash 2 ETH → 1.8 ETH insurance
    expect((await d.vault.anchors(bad.txid)).status).to.equal(2);
    expect((await d.vault.parties(OP_ID)).dead).to.equal(true);
    const OP2 = "0x" + "05".repeat(32);
    await d.vault.connect(d.gov).approveOperator(OP2, await d.other.getAddress());
    await d.vault.connect(d.other).registerParty(OP2, 0, GENESIS, 0, { value: ethers.parseEther("10") });
    const insBefore = await d.vault.insurance();
    const ok = await d.process(OP2, [GENESIS, 0], stmtRelease(0n, 1n, to, 1n), d.other, 2);
    expect((await d.vault.anchors(ok.txid)).status).to.equal(4);
    expect(await d.vault.insuranceReserved()).to.equal(ONE);
    await warp(T_CHALLENGE + 1);
    const before = await ethers.provider.getBalance(to);
    await expect(d.vault.executeRelease(ok.txid)).to.emit(d.vault, "Released").withArgs(ok.txid, 0n, to, ONE);
    expect((await ethers.provider.getBalance(to)) - before).to.equal(ONE);
    expect(insBefore - (await d.vault.insurance())).to.equal(ONE);
    expect(await d.vault.insuranceReserved()).to.equal(0n);
    const dup = await d.process(OP2, [ok.txid, 0], stmtRelease(0n, 2n, to, 1n), d.other, 3);
    expect((await d.vault.anchors(dup.txid)).status).to.equal(2);
    expect((await d.vault.parties(OP2)).dead).to.equal(true);
  });

  it("a false MINT slashes the party that ATTESTed it, whether the attest came first or later", async function () {
    const d = await deploy();
    // Attest first (Ethereum hasn't seen the MINT yet), then the MINT is judged false.
    const stmt = stmtMint(99n, SOL_USER, 1n, 1n, (await now()) + 1000n);
    const txRaw = anchorTx(GENESIS, 0, { kind: KIND_MINT, hash: sha256(stmt) }, 1);
    const mintTxid = dsha256(getBytes(txRaw));
    const a1 = await d.process(AUD_ID, [GENESIS, 0], stmtTarget(KIND_ATTEST, mintTxid), d.aud, 2);
    expect((await d.vault.anchors(a1.txid)).status).to.equal(1);
    expect(await d.vault.mintAttester(mintTxid)).to.equal(AUD_ID);
    const m = await d.process(OP_ID, [GENESIS, 0], stmt, d.other, 1);
    expect((await d.vault.anchors(m.txid)).status).to.equal(2);
    expect((await d.vault.parties(OP_ID)).dead).to.equal(true);
    expect((await d.vault.parties(AUD_ID)).dead).to.equal(true);
    expect((await d.vault.parties(AUD_ID)).bond).to.equal(ONE); // 2 − 1
    // A later attest of the (already slashed) MINT by a third party is slashed on arrival.
    const AUD3 = "0x" + "06".repeat(32);
    await d.vault.connect(d.other).registerParty(AUD3, 1, GENESIS, 0, { value: ethers.parseEther("2") });
    const a2 = await d.process(AUD3, [GENESIS, 0], stmtTarget(KIND_ATTEST, mintTxid), d.other, 3);
    expect((await d.vault.anchors(a2.txid)).status).to.equal(2);
  });

  it("only the first attester of an unresolved MINT is recorded — a second attest neither credits nor blames the latecomer", async function () {
    const d = await deploy();
    const stmt = stmtMint(99n, SOL_USER, 1n, 1n, (await now()) + 1000n);
    const txRaw = anchorTx(GENESIS, 0, { kind: KIND_MINT, hash: sha256(stmt) }, 1);
    const mintTxid = dsha256(getBytes(txRaw));
    const a1 = await d.process(AUD_ID, [GENESIS, 0], stmtTarget(KIND_ATTEST, mintTxid), d.aud, 2);
    expect((await d.vault.anchors(a1.txid)).status).to.equal(1);
    expect(await d.vault.mintAttester(mintTxid)).to.equal(AUD_ID);

    const AUD2 = "0x" + "07".repeat(32);
    await d.vault.connect(d.other).registerParty(AUD2, 1, GENESIS, 0, { value: ethers.parseEther("2") });
    const a2 = await d.process(AUD2, [GENESIS, 0], stmtTarget(KIND_ATTEST, mintTxid), d.other, 3);
    expect((await d.vault.anchors(a2.txid)).status).to.equal(1);
    // Still the first attester — the second one's call was accepted (its own chain
    // advanced) but did not overwrite who is actually on the hook.
    expect(await d.vault.mintAttester(mintTxid)).to.equal(AUD_ID);

    const m = await d.process(OP_ID, [GENESIS, 0], stmt, d.other, 1);
    expect((await d.vault.parties(OP_ID)).dead).to.equal(true);
    expect((await d.vault.parties(AUD_ID)).dead).to.equal(true); // the real attester pays
    expect((await d.vault.parties(AUD2)).dead).to.equal(false); // the latecomer never took the risk
    void m;
  });

  it("vetoes on MINTs are judged here: false → slashed, true → rewarded; ALIVE is judged too", async function () {
    const d = await deploy();
    await d.vault.connect(d.gov).fundRewards({ value: ONE });
    const mtx = await finalizedLock(d, 1n);
    // Veto a TRUE mint → slashed.
    const v1 = await d.process(AUD_ID, [GENESIS, 0], stmtVeto(OP_ID, mtx), d.aud, 2);
    expect((await d.vault.anchors(v1.txid)).status).to.equal(2);
    // ALIVE on a live operator: fine. ALIVE on a dead one: slashed.
    const AUD2 = "0x" + "03".repeat(32);
    await d.vault.connect(d.other).registerParty(AUD2, 1, GENESIS, 0, { value: ethers.parseEther("2") });
    const al = await d.process(AUD2, [GENESIS, 0], stmtTarget(KIND_ALIVE, OP_ID), d.other, 3);
    expect((await d.vault.anchors(al.txid)).status).to.equal(1);
    const bad = await d.process(OP_ID, [mtx, 0], stmtMint(99n, SOL_USER, 9n, 1n, (await now()) + 1000n), d.other, 4);
    expect((await d.vault.parties(OP_ID)).dead).to.equal(true);
    const v2 = await d.process(AUD2, [al.txid, 0], stmtVeto(OP_ID, bad.txid), d.other, 5);
    expect((await d.vault.anchors(v2.txid)).status).to.equal(1);
    expect(await d.vault.rewardPool()).to.equal(ONE - PARAMS.vetoRewardWei);
    const al2 = await d.process(AUD2, [v2.txid, 0], stmtTarget(KIND_ALIVE, OP_ID), d.other, 6);
    expect((await d.vault.anchors(al2.txid)).status).to.equal(2);
  });
});

describe("BetaVault: vetoes judged here, chain rules, skip, registration", function () {
  it("a dead-veto on a live operator slashes the auditor; on a dead operator it is rewarded", async function () {
    const d = await deploy();
    const { txid: v1 } = await d.process(AUD_ID, [GENESIS, 0], stmtVeto(OP_ID, ZERO32), d.aud, 1);
    expect((await d.vault.anchors(v1)).status).to.equal(2);
    expect((await d.vault.parties(AUD_ID)).dead).to.equal(true);
    expect((await d.vault.parties(AUD_ID)).bond).to.equal(ethers.parseEther("1.5"));

    // Kill the operator with a lie, then a fresh auditor's dead-veto is rewarded from insurance.
    await d.process(OP_ID, [GENESIS, 0], stmtMint(99n, SOL_USER, 1n, 1n, (await now()) + 1000n), d.other, 2);
    expect((await d.vault.parties(OP_ID)).dead).to.equal(true);
    const AUD2 = "0x" + "03".repeat(32);
    await d.vault.connect(d.other).registerParty(AUD2, 1, GENESIS, 0, { value: ethers.parseEther("2") });
    await d.vault.connect(d.gov).fundRewards({ value: ONE });
    const insBefore = await d.vault.insurance();
    const { txid: v2 } = await d.process(AUD2, [GENESIS, 0], stmtVeto(OP_ID, ZERO32), d.user, 3);
    expect((await d.vault.anchors(v2)).status).to.equal(1);
    expect(await d.vault.rewardPool()).to.equal(ONE - PARAMS.vetoRewardWei);
    expect(await d.vault.insurance()).to.equal(insBefore); // insurance untouched by rewards
  });

  it("anchors must be on the party's chain, in order, and never twice", async function () {
    const d = await deploy();
    const bad = await d.process(OP_ID, ["0x" + "09".repeat(32), 0], stmtCancel(1n), d.op, 1, { noWait: true });
    await expect(bad.tx).to.be.revertedWithCustomError(d.vault, "NotOnStatementChain");
    const a = await d.process(OP_ID, [GENESIS, 0], stmtCancel(1n), d.op, 1);
    await a.tx;
    // Same anchor again.
    const txRaw = anchorTx(GENESIS, 0, { kind: KIND_CANCEL, hash: sha256(stmtCancel(1n)) }, 1);
    await expect(d.vault.processAnchor(OP_ID, stmtCancel(1n), txRaw, 2000n, [], 0)).to.be.revertedWithCustomError(d.vault, "AlreadyProcessed");
    // Genesis is spent; next must spend a.txid.
    const b = await d.process(OP_ID, [GENESIS, 0], stmtCancel(2n), d.op, 2, { noWait: true });
    await expect(b.tx).to.be.revertedWithCustomError(d.vault, "NotOnStatementChain");
    const c = await d.process(OP_ID, [a.txid, 0], stmtCancel(2n), d.op, 3);
    await c.tx;
    expect((await d.vault.parties(OP_ID)).seq).to.equal(2n);
  });

  it("statement hash and kind must match the anchor; proofs must match the header", async function () {
    const d = await deploy();
    const good = stmtCancel(1n);
    const other = stmtCancel(2n);
    const txRaw = anchorTx(GENESIS, 0, { kind: KIND_CANCEL, hash: sha256(other) }, 1);
    const txid = dsha256(getBytes(txRaw));
    await seedHeader(d.ipowAddr, 9000n, keccak256(ethers.toUtf8Bytes("h9000")), txid, await now());
    await expect(d.vault.processAnchor(OP_ID, good, txRaw, 9000n, [], 0)).to.be.revertedWithCustomError(d.vault, "StatementHashMismatch");
    const txRaw2 = anchorTx(GENESIS, 0, { kind: KIND_RELEASE, hash: sha256(good) }, 2);
    const txid2 = dsha256(getBytes(txRaw2));
    await seedHeader(d.ipowAddr, 9001n, keccak256(ethers.toUtf8Bytes("h9001")), txid2, await now());
    await expect(d.vault.processAnchor(OP_ID, good, txRaw2, 9001n, [], 0)).to.be.revertedWithCustomError(d.vault, "KindMismatch");
    // Header whose root is not this tx.
    await seedHeader(d.ipowAddr, 9002n, keccak256(ethers.toUtf8Bytes("h9002")), "0x" + "ab".repeat(32), await now());
    await expect(d.vault.processAnchor(OP_ID, good, txRaw, 9002n, [], 0)).to.be.revertedWithCustomError(d.vault, "InvalidMerkleBranch");
    await expect(d.vault.processAnchor(OP_ID, good, txRaw, 9999n, [], 0)).to.be.revertedWithCustomError(d.vault, "InvalidHeader");
  });

  it("a stalled anchor can be skipped once its header is tSkip old", async function () {
    const d = await deploy();
    const s = await d.skip(OP_ID, [GENESIS, 0], { kind: KIND_RELEASE, hash: sha256(stmtRelease(1n, 7n, await d.other.getAddress(), 1n)) }, d.op, 1);
    await expect(s.tx).to.be.revertedWithCustomError(d.vault, "SkipNotReady");
    await warp(3601);
    const s2 = await d.skip(OP_ID, [GENESIS, 0], { kind: KIND_RELEASE, hash: sha256(stmtRelease(1n, 7n, await d.other.getAddress(), 1n)) }, d.op, 1);
    // Same tx bytes: the header re-seeded at a fresh height carries a fresh timestamp, so
    // wait again for that header to age.
    await expect(s2.tx).to.be.revertedWithCustomError(d.vault, "SkipNotReady");
    await warp(3601);
    const s3 = await d.skip(OP_ID, [GENESIS, 0], null, d.op, 2);
    await expect(s3.tx).to.be.revertedWithCustomError(d.vault, "SkipNotReady");
    await warp(3601);
    const txRaw = anchorTx(GENESIS, 0, null, 2);
    // Re-use s3's header (its timestamp is now old enough).
    await d.vault.skipAnchor(OP_ID, txRaw, s3.h, [], 0);
    const txid = dsha256(getBytes(txRaw));
    expect((await d.vault.anchors(txid)).status).to.equal(3); // Skipped
    expect((await d.vault.parties(OP_ID)).anchorTxidLE).to.equal(txid);
  });

  it("operator registration needs governance approval and the minimum bond; unbond retires the party", async function () {
    const d = await deploy();
    const X = "0x" + "0f".repeat(32);
    await expect(d.vault.connect(d.other).registerParty(X, 0, GENESIS, 0, { value: ethers.parseEther("10") })).to.be.revertedWithCustomError(d.vault, "NotApproved");
    await d.vault.connect(d.gov).approveOperator(X, await d.other.getAddress());
    await expect(d.vault.connect(d.other).registerParty(X, 0, GENESIS, 0, { value: ONE })).to.be.revertedWithCustomError(d.vault, "BondTooSmall");
    await d.vault.connect(d.other).registerParty(X, 0, GENESIS, 0, { value: ethers.parseEther("5") });
    await expect(d.vault.connect(d.other).registerParty(X, 0, GENESIS, 0, { value: ethers.parseEther("5") })).to.be.revertedWithCustomError(d.vault, "PartyExists");

    await d.vault.connect(d.aud).requestUnbond(AUD_ID);
    await expect(d.vault.connect(d.aud).withdrawBond(AUD_ID)).to.be.revertedWithCustomError(d.vault, "UnbondNotReady");
    await warp(61);
    const before = await ethers.provider.getBalance(await d.aud.getAddress());
    const rc = await (await d.vault.connect(d.aud).withdrawBond(AUD_ID)).wait();
    const after = await ethers.provider.getBalance(await d.aud.getAddress());
    expect(after - before + rc!.gasUsed * rc!.gasPrice).to.equal(ethers.parseEther("2"));
    expect((await d.vault.parties(AUD_ID)).dead).to.equal(true);
    const v = await d.process(AUD_ID, [GENESIS, 0], stmtVeto(OP_ID, ZERO32), d.aud, 1, { noWait: true });
    await expect(v.tx).to.be.revertedWithCustomError(d.vault, "PartyDead");
  });
});

describe("BetaVault: ERC20 local legs (DESIGN_V2 §8.12)", function () {
  async function deployWithToken() {
    const d = await deploy();
    const token = await new MockERC20__factory(d.gov).deploy("Mock", "MOCK");
    await token.waitForDeployment();
    const tokenAddr = await token.getAddress();
    const AMOUNT_PER_UNIT = ethers.parseUnits("2", 18);
    const SLASH_WEI_PER_UNIT = ethers.parseEther("0.3");
    await d.vault.connect(d.gov).setTokenParams(tokenAddr, { amountPerUnit: AMOUNT_PER_UNIT, slashWeiPerUnit: SLASH_WEI_PER_UNIT });
    await token.mint(await d.user.getAddress(), ethers.parseUnits("1000", 18));
    await token.connect(d.user).approve(await d.vault.getAddress(), ethers.parseUnits("1000", 18));
    return { ...d, token, tokenAddr, AMOUNT_PER_UNIT, SLASH_WEI_PER_UNIT };
  }

  it("depositing an unregistered token is refused", async function () {
    const d = await deploy();
    const token = await new MockERC20__factory(d.gov).deploy("Mock", "MOCK");
    await token.waitForDeployment();
    await expect(
      d.vault.connect(d.user).deposit(await token.getAddress(), SOL_USER, 1n, 1n, (await now()) + 86_400n),
    ).to.be.revertedWithCustomError(d.vault, "TokenNotRegistered");
  });

  it("an ERC20 lock finalizes on a true MINT and pulls exactly amountPerUnit * units", async function () {
    const d = await deployWithToken();
    const deadline = (await now()) + 86_400n;
    const before = await d.token.balanceOf(await d.user.getAddress());
    await d.vault.connect(d.user).deposit(d.tokenAddr, SOL_USER, 1n, 3n, deadline);
    expect(before - (await d.token.balanceOf(await d.user.getAddress()))).to.equal(3n * d.AMOUNT_PER_UNIT);
    expect(await d.token.balanceOf(await d.vault.getAddress())).to.equal(3n * d.AMOUNT_PER_UNIT);
    expect(await d.vault.totalLocked(d.tokenAddr)).to.equal(3n * d.AMOUNT_PER_UNIT);
    expect((await d.vault.locks(1n)).token).to.equal(d.tokenAddr);

    const { txid, tx } = await d.process(OP_ID, [GENESIS, 0], stmtMint(1n, SOL_USER, 1n, 3n, deadline), d.op, 1);
    await expect(tx).to.emit(d.vault, "Finalized").withArgs(1n, txid);
    expect((await d.vault.locks(1n)).state).to.equal(2); // Final
  });

  it("an ERC20 lock that never finalizes refunds back in the same token, not ETH", async function () {
    const d = await deployWithToken();
    const deadline = (await now()) + 86_400n;
    await d.vault.connect(d.user).deposit(d.tokenAddr, SOL_USER, 1n, 2n, deadline);
    await warp(86_400 + 601);
    const before = await d.token.balanceOf(await d.user.getAddress());
    const ethBefore = await ethers.provider.getBalance(await d.user.getAddress());
    const rc = await (await d.vault.connect(d.user).refund(1n)).wait();
    expect((await d.token.balanceOf(await d.user.getAddress())) - before).to.equal(2n * d.AMOUNT_PER_UNIT);
    // No native ETH moved for this refund beyond the caller's own gas.
    const ethAfter = await ethers.provider.getBalance(await d.user.getAddress());
    expect(ethBefore - ethAfter).to.equal(rc!.gasUsed * rc!.gasPrice);
    expect(await d.vault.totalLocked(d.tokenAddr)).to.equal(0n);
    expect((await d.vault.locks(1n)).state).to.equal(4); // Refunded
  });

  it("a false MINT for an ERC20 lock slashes the operator in wei at the registered slash rate, not token units", async function () {
    const d = await deployWithToken();
    const deadline = (await now()) + 86_400n;
    await d.vault.connect(d.user).deposit(d.tokenAddr, SOL_USER, 1n, 2n, deadline);
    // Wrong units for lock 1 — a lie about this network.
    const { tx } = await d.process(OP_ID, [GENESIS, 0], stmtMint(1n, SOL_USER, 1n, 5n, deadline), d.other, 1);
    await expect(tx).to.emit(d.vault, "Slashed").withArgs(OP_ID, 5n * d.SLASH_WEI_PER_UNIT, await d.other.getAddress());
    const p = await d.vault.parties(OP_ID);
    expect(p.dead).to.equal(true);
    expect(p.bond).to.equal(ethers.parseEther("10") - 5n * d.SLASH_WEI_PER_UNIT);
    // The lock's own ERC20 balance is completely untouched by the slash.
    expect(await d.token.balanceOf(await d.vault.getAddress())).to.equal(2n * d.AMOUNT_PER_UNIT);
  });

  it("RELEASE on an ERC20-backed lock is refused; native locks are unaffected by the change", async function () {
    const d = await deployWithToken();
    const deadline = (await now()) + 86_400n;
    await d.vault.connect(d.user).deposit(d.tokenAddr, SOL_USER, 1n, 1n, deadline);
    const { txid: mtx } = await d.process(OP_ID, [GENESIS, 0], stmtMint(1n, SOL_USER, 1n, 1n, deadline), d.op, 1);
    expect((await d.vault.locks(1n)).state).to.equal(2);

    const to = await d.other.getAddress();
    const r = await d.process(OP_ID, [mtx, 0], stmtRelease(1n, 0n, to, 1n), d.op, 2, { noWait: true });
    await expect(r.tx).to.be.revertedWithCustomError(d.vault, "ReleaseTokenUnsupported");

    // A native lock's release path still works exactly as before.
    await d.vault.connect(d.user).deposit(ethers.ZeroAddress, SOL_USER, 2n, 1n, deadline, { value: ONE });
    const { txid: mtx2 } = await d.process(OP_ID, [mtx, 0], stmtMint(2n, SOL_USER, 2n, 1n, deadline), d.op, 3);
    const { txid: rtx } = await d.process(OP_ID, [mtx2, 0], stmtRelease(2n, 0n, to, 1n), d.op, 4);
    expect((await d.vault.anchors(rtx)).status).to.equal(4); // QueuedRelease
  });
});
