import { expect } from "chai";
import { network } from "hardhat";
import { sha256, getBytes, hexlify, concat, toBeHex, zeroPadValue, AbiCoder, keccak256 } from "ethers";

import { IPoW__factory, BetaVault__factory, BetaHub__factory, HubToken__factory, MockERC20__factory } from "../types/ethers-contracts/index.ts";
import { COMMIT_FEE_BPS, NATIVE_DECIMALS, SELF_NETWORK_ID } from "./helpers/deploy.ts";

const { ethers } = await network.create();

// Same infrastructure as test/BetaVault.test.ts (chain-agnostic: raw
// Bitcoin statement-chain tx builders and iPoW storage cheat-codes),
// reused near-verbatim — see that file's own comments for why each piece
// works the way it does.
const KIND_MINT = 1;
const KIND_VETO = 3;
const KIND_ATTEST = 5;
const KIND_CLEAR = 6;
const KIND_ALIVE = 7;
const GENESIS = "0x" + "07".repeat(32);
const OP_ID = "0x" + "01".repeat(32);
const OP2_ID = "0x" + "03".repeat(32);
const AUD_ID = "0x" + "02".repeat(32);
const ZERO32 = "0x" + "00".repeat(32);
const ONE = ethers.parseEther("1");
const SOLANA_NETWORK_ID = 3n;

function dsha256(data: Uint8Array): string {
  return sha256(getBytes(sha256(data)));
}
function u8(v: number): Uint8Array {
  return Uint8Array.of(v);
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
function addrToBytes32(addr: string): string {
  return zeroPadValue(addr, 32);
}

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

// Unlike BetaVault.test.ts's own stmtMint (which hardcodes compositionId=0/
// componentIndex=0 since BetaVault ignores both), BetaHub actually parses
// and uses these fields — this is the first test suite to vary them.
function stmtMint(compositionId: bigint, componentIndex: number, lockId: bigint, hubUser: string, nonce: bigint, units: bigint, deadline: bigint): string {
  return hexlify(
    concat([u8(KIND_MINT), u64be(compositionId), u8(componentIndex), u64be(lockId), getBytes(addrToBytes32(hubUser)), u64be(nonce), u64be(units), u64be(deadline)]),
  );
}
function stmtVeto(targetPartyId: string, targetTxid: string): string {
  return hexlify(concat([u8(KIND_VETO), getBytes(targetPartyId), getBytes(targetTxid)]));
}
function stmtTarget(kind: number, target: string): string {
  return hexlify(concat([u8(kind), getBytes(target)]));
}

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
  tChallengeSecs: 7n * 86400n,
  tSkipSecs: 3600n,
  refundMarginSecs: 600n,
  unbondDelaySecs: 60n,
  minOperatorBond: ethers.parseEther("5"),
  minAuditorBond: ONE,
  slashWeiPerUnit: ONE,
  vetoSlashWei: ethers.parseEther("0.5"),
  vetoRewardWei: ethers.parseEther("0.1"),
  bountyBps: 1000n,
};

const SOL_USER = "0x" + "aa".repeat(32); // used as a remote leg's own opaque solUser field — irrelevant to the hub itself.

async function deploy() {
  const [gov, op, op2, aud, user, other] = await ethers.getSigners();
  const ipow = await new IPoW__factory(gov).deploy(NATIVE_DECIMALS, SELF_NETWORK_ID, await gov.getAddress(), COMMIT_FEE_BPS);
  await ipow.waitForDeployment();
  const hub = await new BetaHub__factory(gov).deploy(await gov.getAddress(), await ipow.getAddress(), SELF_NETWORK_ID, PARAMS, "iBETA", "iBETA");
  await hub.waitForDeployment();
  await hub.connect(gov).approveOperator(OP_ID, await op.getAddress());
  await hub.connect(op).registerParty(OP_ID, 0, GENESIS, 0, { value: ethers.parseEther("10") });
  await hub.connect(gov).approveOperator(OP2_ID, await op2.getAddress());
  await hub.connect(op2).registerParty(OP2_ID, 0, GENESIS, 0, { value: ethers.parseEther("10") });
  await hub.connect(aud).registerParty(AUD_ID, 1, GENESIS, 0, { value: ethers.parseEther("2") });
  const ipowAddr = await ipow.getAddress();
  await setTip(ipowAddr, 1000n);
  let height = 2000n;

  async function process(partyId: string, prev: [string, number], statement: string, submitter: any, salt: number, opts: { late?: number; noWait?: boolean } = {}) {
    const txRaw = anchorTx(prev[0], prev[1], { kind: parseInt(statement.slice(2, 4), 16), hash: sha256(statement) }, salt);
    const txid = dsha256(getBytes(txRaw));
    const h = height++;
    const ts = (await now()) - BigInt(opts.late ?? 0);
    await seedHeader(ipowAddr, h, keccak256(ethers.toUtf8Bytes(`hdr-${h}`)), txid, ts);
    const tx = hub.connect(submitter).processAnchor(partyId, statement, txRaw, h, [], 0);
    if (!opts.noWait) await (await tx).wait();
    return { txid, tx };
  }
  return { gov, op, op2, aud, user, other, ipow, hub, ipowAddr, process };
}

const NATIVE_COMPONENT = { networkId: SELF_NETWORK_ID, tokenId: ethers.ZeroAddress, amountPerUnit: ONE };
const REMOTE_COMPONENT_1 = { networkId: 5n, tokenId: ethers.ZeroAddress, amountPerUnit: ONE }; // Base
const REMOTE_COMPONENT_2 = { networkId: 6n, tokenId: ethers.ZeroAddress, amountPerUnit: ONE }; // Robinhood Chain

describe("BetaHub: composition registry", function () {
  it("governance-only, immutable-per-id, and rejects malformed compositions", async function () {
    const d = await deploy();
    await expect(d.hub.connect(d.user).registerComposition(1n, [NATIVE_COMPONENT])).to.be.revertedWithCustomError(d.hub, "Unauthorized");

    await d.hub.connect(d.gov).registerComposition(1n, [NATIVE_COMPONENT]);
    await expect(d.hub.connect(d.gov).registerComposition(1n, [NATIVE_COMPONENT])).to.be.revertedWithCustomError(d.hub, "CompositionExists");

    await expect(d.hub.connect(d.gov).registerComposition(2n, [])).to.be.revertedWithCustomError(d.hub, "InvalidComponents");
    await expect(d.hub.connect(d.gov).registerComposition(2n, [REMOTE_COMPONENT_1])).to.be.revertedWithCustomError(d.hub, "NoLocalComponent");
    await expect(
      d.hub.connect(d.gov).registerComposition(2n, [NATIVE_COMPONENT, { ...NATIVE_COMPONENT }]),
    ).to.be.revertedWithCustomError(d.hub, "DuplicateComponent");
    await expect(
      d.hub.connect(d.gov).registerComposition(2n, [NATIVE_COMPONENT, { networkId: SOLANA_NETWORK_ID, tokenId: ethers.ZeroAddress, amountPerUnit: ONE }]),
    ).to.be.revertedWithCustomError(d.hub, "UnsupportedNetwork");

    await d.hub.connect(d.gov).registerComposition(3n, [NATIVE_COMPONENT, REMOTE_COMPONENT_1, REMOTE_COMPONENT_2]);
    const stored = await d.hub.getComposition(3n);
    expect(stored.length).to.equal(3);
  });
});

describe("BetaHub: lockLocal / approvePending / expirePending", function () {
  it("a purely-local composition mints with no operator involved at all", async function () {
    const d = await deploy();
    await d.hub.connect(d.gov).registerComposition(1n, [NATIVE_COMPONENT]);
    const deadline = (await now()) + 86_400n;
    await d.hub.connect(d.user).lockLocal(1n, 1n, 3n, deadline, { value: 3n * ONE });
    await d.hub.connect(d.user).approvePending(1n, []);
    const token = HubToken__factory.connect(await d.hub.token(), d.user);
    await d.hub.connect(d.other).exerciseMint(await d.user.getAddress(), 1n);
    expect(await token.balanceOf(await d.user.getAddress())).to.equal(3n * 10n ** 18n);
  });

  it("rejects a second lockLocal for the same (user, nonce), and refunds native + ERC20 legs on expiry", async function () {
    const d = await deploy();
    const mock = await new MockERC20__factory(d.gov).deploy("Mock", "MOCK");
    await mock.waitForDeployment();
    const mockAddr = await mock.getAddress();
    await mock.mint(await d.user.getAddress(), ethers.parseUnits("1000", 18));
    await mock.connect(d.user).approve(await d.hub.getAddress(), ethers.parseUnits("1000", 18));

    const TOKEN_COMPONENT = { networkId: SELF_NETWORK_ID, tokenId: mockAddr, amountPerUnit: ethers.parseUnits("2", 18) };
    await d.hub.connect(d.gov).registerComposition(1n, [NATIVE_COMPONENT, TOKEN_COMPONENT, REMOTE_COMPONENT_1]);
    const deadline = (await now()) + 86_400n;
    await d.hub.connect(d.user).lockLocal(1n, 1n, 2n, deadline, { value: 2n * ONE });
    await expect(d.hub.connect(d.user).lockLocal(1n, 1n, 2n, deadline, { value: 2n * ONE })).to.be.revertedWithCustomError(d.hub, "PendingExists");

    expect(await mock.balanceOf(await d.hub.getAddress())).to.equal(2n * ethers.parseUnits("2", 18)); // 2 units * 2/unit = 4
    await warp(86_400 + 601);
    const ethBefore = await ethers.provider.getBalance(await d.user.getAddress());
    const mockBefore = await mock.balanceOf(await d.user.getAddress());
    await d.hub.connect(d.other).expirePending(await d.user.getAddress(), 1n);
    const ethAfter = await ethers.provider.getBalance(await d.user.getAddress());
    const mockAfter = await mock.balanceOf(await d.user.getAddress());
    expect(ethAfter - ethBefore).to.equal(2n * ONE);
    expect(mockAfter - mockBefore).to.equal(2n * ethers.parseUnits("2", 18));
  });

  it("expirePending is blocked while claimed by a live operator, allowed once that operator is dead", async function () {
    const d = await deploy();
    await d.hub.connect(d.gov).registerComposition(1n, [NATIVE_COMPONENT, REMOTE_COMPONENT_1]);
    const deadline = (await now()) + 86_400n;
    const userAddr = await d.user.getAddress();
    await d.hub.connect(d.user).lockLocal(1n, 1n, 1n, deadline, { value: ONE });
    await d.hub.connect(d.user).approvePending(1n, [42n]);
    await d.process(OP_ID, [GENESIS, 0], stmtMint(1n, 0, 42n, userAddr, 1n, 1n, deadline), d.op, 1);

    await warp(86_400 + 601);
    await expect(d.hub.connect(d.other).expirePending(userAddr, 1n)).to.be.revertedWithCustomError(d.hub, "QueuedByLiveOperator");

    // Retire the operator (unbond) then it becomes "dead" — expire should now succeed.
    await d.hub.connect(d.op).requestUnbond(OP_ID);
    await warp(61);
    await d.hub.connect(d.op).withdrawBond(OP_ID);
    await d.hub.connect(d.other).expirePending(userAddr, 1n);
  });
});

describe("BetaHub: _processRemoteMint predicate", function () {
  async function withComposition() {
    const d = await deploy();
    await d.hub.connect(d.gov).registerComposition(1n, [NATIVE_COMPONENT, REMOTE_COMPONENT_1, REMOTE_COMPONENT_2]);
    return d;
  }

  it("a true MINT queues exactly the right component, no others", async function () {
    const d = await withComposition();
    const deadline = (await now()) + 86_400n;
    const userAddr = await d.user.getAddress();
    await d.hub.connect(d.user).lockLocal(1n, 1n, 2n, deadline, { value: 2n * ONE });
    await d.hub.connect(d.user).approvePending(1n, [11n, 22n]);

    const { txid } = await d.process(OP_ID, [GENESIS, 0], stmtMint(1n, 0, 11n, userAddr, 1n, 2n, deadline), d.op, 1);
    const anchor = await d.hub.anchors(txid);
    expect(anchor.status).to.equal(1); // Queued
    const p = await d.hub.pending(ethers.solidityPackedKeccak256(["address", "uint64"], [userAddr, 1n]));
    expect(p.queuedBy).to.equal(OP_ID);
  });

  it("a false MINT (wrong lockId) slashes the operator to the named victim, not insurance", async function () {
    const d = await withComposition();
    const deadline = (await now()) + 86_400n;
    const userAddr = await d.user.getAddress();
    await d.hub.connect(d.user).lockLocal(1n, 1n, 2n, deadline, { value: 2n * ONE });
    await d.hub.connect(d.user).approvePending(1n, [11n, 22n]);

    const victimBefore = await ethers.provider.getBalance(userAddr);
    const insuranceBefore = await d.hub.insurance();
    const { tx } = await d.process(OP_ID, [GENESIS, 0], stmtMint(1n, 0, 999n, userAddr, 1n, 2n, deadline), d.op, 1);
    await expect(tx).to.emit(d.hub, "Slashed");
    const victimAfter = await ethers.provider.getBalance(userAddr);
    const insuranceAfter = await d.hub.insurance();
    // bountyBps=1000 (10%) goes to the submitter (also op here, since op
    // submitted its own false claim) — the victim gets the other 90% of
    // (2 units * slashWeiPerUnit).
    const slashed = 2n * PARAMS.slashWeiPerUnit;
    const bounty = (slashed * PARAMS.bountyBps) / 10_000n;
    expect(victimAfter - victimBefore).to.equal(slashed - bounty);
    expect(insuranceAfter).to.equal(insuranceBefore); // untouched — this slash had a named victim
    const p = await d.hub.parties(OP_ID);
    expect(p.dead).to.equal(true);
  });

  it("takeover after operator retirement resets every leg", async function () {
    const d = await withComposition();
    const deadline = (await now()) + 86_400n;
    const userAddr = await d.user.getAddress();
    await d.hub.connect(d.user).lockLocal(1n, 1n, 1n, deadline, { value: ONE });
    await d.hub.connect(d.user).approvePending(1n, [11n, 22n]);

    await d.process(OP_ID, [GENESIS, 0], stmtMint(1n, 0, 11n, userAddr, 1n, 1n, deadline), d.op, 1);
    await d.hub.connect(d.op).requestUnbond(OP_ID);
    await warp(61);
    await d.hub.connect(d.op).withdrawBond(OP_ID);

    // op2 takes over — must restart from component 0 again (a fresh claim).
    // A different salt than op's own first anchor: same statement bytes plus
    // the same [GENESIS,0] prev would otherwise hash to the identical txid
    // (Bitcoin txid depends only on tx bytes, not which party "owns" it).
    const { txid } = await d.process(OP2_ID, [GENESIS, 0], stmtMint(1n, 0, 11n, userAddr, 1n, 1n, deadline), d.op2, 2);
    const pendingId = ethers.solidityPackedKeccak256(["address", "uint64"], [userAddr, 1n]);
    const p = await d.hub.pending(pendingId);
    expect(p.queuedBy).to.equal(OP2_ID);
    const remoteAnchorTxid = await d.hub.pendingRemoteAnchorTxid(pendingId);
    expect(remoteAnchorTxid[0]).to.equal(txid);
    expect(remoteAnchorTxid[1]).to.equal(ZERO32);
  });
});

describe("BetaHub: exerciseMint", function () {
  it("refuses with an incomplete remote leg, mints exactly units * 1e18 once every leg is ready", async function () {
    const d = await deploy();
    await d.hub.connect(d.gov).registerComposition(1n, [NATIVE_COMPONENT, REMOTE_COMPONENT_1, REMOTE_COMPONENT_2]);
    const deadline = (await now()) + 86_400n;
    const userAddr = await d.user.getAddress();
    await d.hub.connect(d.user).lockLocal(1n, 1n, 2n, deadline, { value: 2n * ONE });
    await d.hub.connect(d.user).approvePending(1n, [11n, 22n]);

    // Only component 0 submitted — component 0 itself isn't past its own
    // challenge window yet, so that's the failure reported first (checked
    // in component order), even though component 1 is also missing.
    const { txid: txid1 } = await d.process(OP_ID, [GENESIS, 0], stmtMint(1n, 0, 11n, userAddr, 1n, 2n, deadline), d.op, 1);
    await expect(d.hub.connect(d.other).exerciseMint(userAddr, 1n)).to.be.revertedWithCustomError(d.hub, "ComponentHeldOrNotReady");

    // Past component 0's window now — but component 1 was never submitted.
    await warp(Number(PARAMS.tChallengeSecs) + 1);
    await expect(d.hub.connect(d.other).exerciseMint(userAddr, 1n)).to.be.revertedWithCustomError(d.hub, "IncompleteRemoteComponents");

    // Submit component 1 (chaining OP_ID's own statement chain from its
    // first anchor's own txid, not GENESIS again) — it gets its own fresh
    // challenge window starting now, so it isn't ready immediately either.
    await d.process(OP_ID, [txid1, 0], stmtMint(1n, 1, 22n, userAddr, 1n, 2n, deadline), d.op, 1);
    await expect(d.hub.connect(d.other).exerciseMint(userAddr, 1n)).to.be.revertedWithCustomError(d.hub, "ComponentHeldOrNotReady");

    await warp(Number(PARAMS.tChallengeSecs) + 1);
    const tokenAddr = await d.hub.token();
    const token = HubToken__factory.connect(tokenAddr, d.user);
    await d.hub.connect(d.other).exerciseMint(userAddr, 1n);
    expect(await token.balanceOf(userAddr)).to.equal(2n * 10n ** 18n);
  });

  it("an ATTESTed component lets exerciseMint proceed before its challenge window closes", async function () {
    const d = await deploy();
    await d.hub.connect(d.gov).registerComposition(1n, [NATIVE_COMPONENT, REMOTE_COMPONENT_1]);
    const deadline = (await now()) + 86_400n;
    const userAddr = await d.user.getAddress();
    await d.hub.connect(d.user).lockLocal(1n, 1n, 1n, deadline, { value: ONE });
    await d.hub.connect(d.user).approvePending(1n, [11n]);
    const { txid } = await d.process(OP_ID, [GENESIS, 0], stmtMint(1n, 0, 11n, userAddr, 1n, 1n, deadline), d.op, 1);

    await d.process(AUD_ID, [GENESIS, 0], stmtTarget(KIND_ATTEST, txid), d.aud, 1);
    const tokenAddr = await d.hub.token();
    const token = HubToken__factory.connect(tokenAddr, d.user);
    await d.hub.connect(d.other).exerciseMint(userAddr, 1n); // succeeds despite being well within the challenge window
    expect(await token.balanceOf(userAddr)).to.equal(1n * 10n ** 18n);
  });
});

describe("BetaHub: VETO / CLEAR hold a queued component", function () {
  it("a VETO holds a queued component; CLEAR lifts it; exerciseMint refuses while held", async function () {
    const d = await deploy();
    await d.hub.connect(d.gov).registerComposition(1n, [NATIVE_COMPONENT, REMOTE_COMPONENT_1]);
    const deadline = (await now()) + 86_400n;
    const userAddr = await d.user.getAddress();
    await d.hub.connect(d.user).lockLocal(1n, 1n, 1n, deadline, { value: ONE });
    await d.hub.connect(d.user).approvePending(1n, [11n]);
    const { txid } = await d.process(OP_ID, [GENESIS, 0], stmtMint(1n, 0, 11n, userAddr, 1n, 1n, deadline), d.op, 1);

    const { txid: vetoTxid } = await d.process(AUD_ID, [GENESIS, 0], stmtVeto(OP_ID, txid), d.aud, 1);
    let anchor = await d.hub.anchors(txid);
    expect(anchor.held).to.equal(true);
    await warp(Number(PARAMS.tChallengeSecs) + 1);
    await expect(d.hub.connect(d.other).exerciseMint(userAddr, 1n)).to.be.revertedWithCustomError(d.hub, "IncompleteRemoteComponents");

    // Continuing AUD_ID's own statement chain from its previous anchor (the
    // VETO), not from the target's txid.
    await d.process(AUD_ID, [vetoTxid, 0], stmtTarget(KIND_CLEAR, txid), d.aud, 2);
    anchor = await d.hub.anchors(txid);
    expect(anchor.held).to.equal(false);
    const tokenAddr = await d.hub.token();
    const token = HubToken__factory.connect(tokenAddr, d.user);
    await d.hub.connect(d.other).exerciseMint(userAddr, 1n);
    expect(await token.balanceOf(userAddr)).to.equal(1n * 10n ** 18n);
  });

  it("settleMint un-queues a held-and-never-cleared component so a new claimant can retry", async function () {
    const d = await deploy();
    await d.hub.connect(d.gov).registerComposition(1n, [NATIVE_COMPONENT, REMOTE_COMPONENT_1]);
    const deadline = (await now()) + 86_400n;
    const userAddr = await d.user.getAddress();
    await d.hub.connect(d.user).lockLocal(1n, 1n, 1n, deadline, { value: ONE });
    await d.hub.connect(d.user).approvePending(1n, [11n]);
    const { txid } = await d.process(OP_ID, [GENESIS, 0], stmtMint(1n, 0, 11n, userAddr, 1n, 1n, deadline), d.op, 1);
    await d.process(AUD_ID, [GENESIS, 0], stmtVeto(OP_ID, txid), d.aud, 1);

    await warp(Number(PARAMS.tChallengeSecs) + 1);
    await d.hub.connect(d.other).settleMint(txid);
    const anchor = await d.hub.anchors(txid);
    expect(anchor.status).to.equal(5); // Cancelled
    const pendingId = ethers.solidityPackedKeccak256(["address", "uint64"], [userAddr, 1n]);
    const p = await d.hub.pending(pendingId);
    expect(p.queuedBy).to.equal(ZERO32);
    const remoteAnchorTxid = await d.hub.pendingRemoteAnchorTxid(pendingId);
    expect(remoteAnchorTxid[0]).to.equal(ZERO32);
  });
});

describe("BetaHub: KIND_ALIVE dead-party veto", function () {
  it("a true ALIVE claim about a live operator is a no-op", async function () {
    const d = await deploy();
    const { tx } = await d.process(AUD_ID, [GENESIS, 0], stmtTarget(KIND_ALIVE, OP_ID), d.aud, 1);
    await expect(tx).to.not.emit(d.hub, "Slashed");
    expect((await d.hub.parties(AUD_ID)).dead).to.equal(false);
  });

  it("slashes a submitter who falsely claims a now-dead operator is still alive", async function () {
    const d = await deploy();
    await d.hub.connect(d.op).requestUnbond(OP_ID);
    await warp(61);
    await d.hub.connect(d.op).withdrawBond(OP_ID); // OP_ID is now genuinely dead.

    const insuranceBefore = await d.hub.insurance();
    const { tx } = await d.process(AUD_ID, [GENESIS, 0], stmtTarget(KIND_ALIVE, OP_ID), d.aud, 1);
    await expect(tx).to.emit(d.hub, "Slashed");
    const insuranceAfter = await d.hub.insurance();
    expect(insuranceAfter).to.be.greaterThan(insuranceBefore); // no named victim for this one — parked in insurance
    expect((await d.hub.parties(AUD_ID)).dead).to.equal(true);
  });
});

describe("BetaHub: end-to-end, an EVM hub backed by other EVM spokes", function () {
  it("a 3-component composition (1 local + 2 remote BetaVault-backed) mints via two independent statement chains", async function () {
    const d = await deploy();
    await d.hub.connect(d.gov).registerComposition(1n, [NATIVE_COMPONENT, REMOTE_COMPONENT_1, REMOTE_COMPONENT_2]);

    // Two independent BetaVault deployments play the role of remote spokes
    // — proving BetaVault.sol needs zero modification to back an EVM hub's
    // mint (Lock.solUser is opaque bytes32, here set to the hub-user's
    // address, never compared except byte-for-byte).
    const vault1 = await new BetaVault__factory(d.gov).deploy(await d.gov.getAddress(), d.ipowAddr, {
      ethWeiPerUnit: ONE, tFinSecs: 1800n, tChallengeSecs: 604800n, tSkipSecs: 3600n, refundMarginSecs: 600n, unbondDelaySecs: 60n,
      minOperatorBond: ethers.parseEther("5"), minAuditorBond: ONE, vetoSlashWei: ethers.parseEther("0.5"), vetoRewardWei: ethers.parseEther("0.1"), bountyBps: 1000n,
    });
    await vault1.waitForDeployment();
    const vault2 = await new BetaVault__factory(d.gov).deploy(await d.gov.getAddress(), d.ipowAddr, {
      ethWeiPerUnit: ONE, tFinSecs: 1800n, tChallengeSecs: 604800n, tSkipSecs: 3600n, refundMarginSecs: 600n, unbondDelaySecs: 60n,
      minOperatorBond: ethers.parseEther("5"), minAuditorBond: ONE, vetoSlashWei: ethers.parseEther("0.5"), vetoRewardWei: ethers.parseEther("0.1"), bountyBps: 1000n,
    });
    await vault2.waitForDeployment();
    await vault1.connect(d.gov).approveOperator(OP_ID, await d.op.getAddress());
    await vault1.connect(d.op).registerParty(OP_ID, 0, GENESIS, 0, { value: ethers.parseEther("10") });
    await vault2.connect(d.gov).approveOperator(OP_ID, await d.op.getAddress());
    await vault2.connect(d.op).registerParty(OP_ID, 0, GENESIS, 0, { value: ethers.parseEther("10") });

    const userAddr = await d.user.getAddress();
    const hubUserAsBytes32 = addrToBytes32(userAddr);
    const deadline = (await now()) + 86_400n;
    const lock1 = await vault1.connect(d.user).deposit.staticCall(ethers.ZeroAddress, hubUserAsBytes32, 1n, 1n, deadline, { value: ONE });
    await vault1.connect(d.user).deposit(ethers.ZeroAddress, hubUserAsBytes32, 1n, 1n, deadline, { value: ONE });
    const lock2 = await vault2.connect(d.user).deposit.staticCall(ethers.ZeroAddress, hubUserAsBytes32, 1n, 1n, deadline, { value: ONE });
    await vault2.connect(d.user).deposit(ethers.ZeroAddress, hubUserAsBytes32, 1n, 1n, deadline, { value: ONE });

    await d.hub.connect(d.user).lockLocal(1n, 1n, 1n, deadline, { value: ONE });
    await d.hub.connect(d.user).approvePending(1n, [lock1, lock2]);

    const { txid: firstTxid } = await d.process(OP_ID, [GENESIS, 0], stmtMint(1n, 0, lock1, userAddr, 1n, 1n, deadline), d.op, 1);
    await d.process(OP_ID, [firstTxid, 0], stmtMint(1n, 1, lock2, userAddr, 1n, 1n, deadline), d.op, 2);

    await warp(Number(PARAMS.tChallengeSecs) + 1);
    const tokenAddr = await d.hub.token();
    const token = HubToken__factory.connect(tokenAddr, d.user);
    await d.hub.connect(d.other).exerciseMint(userAddr, 1n);
    expect(await token.balanceOf(userAddr)).to.equal(1n * 10n ** 18n);
  });
});
