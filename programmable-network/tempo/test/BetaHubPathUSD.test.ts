import { expect } from "chai";
import { network } from "hardhat";
import { sha256, getBytes, hexlify, concat, toBeHex, zeroPadValue, AbiCoder, keccak256 } from "ethers";

import { IPoW__factory, BetaHubPathUSD__factory, HubToken__factory, MockERC20__factory } from "../types/ethers-contracts/index.ts";
import { COMMIT_FEE_BPS, NATIVE_DECIMALS, SELF_NETWORK_ID } from "./helpers/deploy.ts";

const { ethers } = await network.create();

// Same test infrastructure as test/BetaHub.test.ts (raw Bitcoin
// statement-chain tx builders, iPoW storage cheat-codes) — this suite
// mirrors that one's cases exactly, swapping every native-value bond/
// payout for a MockERC20 standing in for PathUSD, since BetaHubPathUSD's
// whole point is that bonds/payouts move that token instead of msg.value.
// Composition *local-leg* deposits (lockLocal's own msg.value handling)
// are untouched by that fix and stay native here too, exactly like
// BetaHub.test.ts — this local Hardhat network has no Tempo-style native-
// value rejection, so testing that path unchanged is the right mirror.
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
const BOND = ethers.parseUnits("1000", 18); // generous MockERC20 bond, decimals irrelevant to the logic under test
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

async function deploy() {
  const [gov, op, op2, aud, user, other] = await ethers.getSigners();
  const ipow = await new IPoW__factory(gov).deploy(NATIVE_DECIMALS, SELF_NETWORK_ID, await gov.getAddress(), COMMIT_FEE_BPS);
  await ipow.waitForDeployment();

  const bond = await new MockERC20__factory(gov).deploy("Mock PathUSD", "mUSD");
  await bond.waitForDeployment();
  const bondAddr = await bond.getAddress();

  const hub = await new BetaHubPathUSD__factory(gov).deploy(await gov.getAddress(), await ipow.getAddress(), bondAddr, SELF_NETWORK_ID, PARAMS, "iBETA", "iBETA");
  await hub.waitForDeployment();
  const hubAddr = await hub.getAddress();

  async function fundAndApprove(signer: any, amount: bigint) {
    await bond.mint(await signer.getAddress(), amount);
    await bond.connect(signer).approve(hubAddr, amount);
  }

  await hub.connect(gov).approveOperator(OP_ID, await op.getAddress());
  await fundAndApprove(op, ethers.parseEther("10"));
  await hub.connect(op)["registerParty(bytes32,uint8,bytes32,uint32,uint256)"](OP_ID, 0, GENESIS, 0, ethers.parseEther("10"));

  await hub.connect(gov).approveOperator(OP2_ID, await op2.getAddress());
  await fundAndApprove(op2, ethers.parseEther("10"));
  await hub.connect(op2)["registerParty(bytes32,uint8,bytes32,uint32,uint256)"](OP2_ID, 0, GENESIS, 0, ethers.parseEther("10"));

  await fundAndApprove(aud, ethers.parseEther("2"));
  await hub.connect(aud)["registerParty(bytes32,uint8,bytes32,uint32,uint256)"](AUD_ID, 1, GENESIS, 0, ethers.parseEther("2"));

  // Fund the veto-reward pool with PathUSD (mirrors BetaHub.test.ts's own
  // implicit reliance on lockLocal's native deposits sitting in the hub's
  // balance) so KIND_ALIVE/VETO reward payouts (_pay, now PathUSD) have
  // real balance to draw from.
  await fundAndApprove(gov, ethers.parseEther("5"));
  await hub.connect(gov).fundRewards(ethers.parseEther("5"));

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
  return { gov, op, op2, aud, user, other, ipow, hub, bond, ipowAddr, process };
}

const NATIVE_COMPONENT = { networkId: SELF_NETWORK_ID, tokenId: ethers.ZeroAddress, amountPerUnit: ONE };
const REMOTE_COMPONENT_1 = { networkId: 5n, tokenId: ethers.ZeroAddress, amountPerUnit: ONE }; // Base
const REMOTE_COMPONENT_2 = { networkId: 6n, tokenId: ethers.ZeroAddress, amountPerUnit: ONE }; // Robinhood Chain

describe("BetaHubPathUSD: composition registry", function () {
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

describe("BetaHubPathUSD: registerParty (PathUSD bond, the actual fix under test)", function () {
  it("the inherited native-value registerParty is present but permanently unusable (BondTooSmall for any zero-value call)", async function () {
    const d = await deploy();
    await d.hub.connect(d.gov).approveOperator(ethers.zeroPadValue("0x09", 32), await d.other.getAddress());
    await expect(
      d.hub.connect(d.other)["registerParty(bytes32,uint8,bytes32,uint32)"](ethers.zeroPadValue("0x09", 32), 0, GENESIS, 0),
    ).to.be.revertedWithCustomError(d.hub, "BondTooSmall"); // msg.value defaults to 0 -- exactly what Tempo forces
  });

  it("the new PathUSD-taking overload pulls the exact bond via transferFrom and registers the party", async function () {
    const d = await deploy();
    const before = await d.bond.balanceOf(await d.hub.getAddress());
    const p = await d.hub.parties(OP_ID);
    expect(p.exists).to.equal(true);
    expect(p.bond).to.equal(ethers.parseEther("10"));
    expect(await d.bond.balanceOf(await d.hub.getAddress())).to.equal(before); // already registered in deploy(); just confirming it landed
  });

  it("topUpBond(partyId, amount) adds PathUSD to an existing party's bond", async function () {
    const d = await deploy();
    await d.bond.mint(await d.op.getAddress(), ethers.parseEther("1"));
    await d.bond.connect(d.op).approve(await d.hub.getAddress(), ethers.parseEther("1"));
    await d.hub.connect(d.op)["topUpBond(bytes32,uint256)"](OP_ID, ethers.parseEther("1"));
    const p = await d.hub.parties(OP_ID);
    expect(p.bond).to.equal(ethers.parseEther("11"));
  });

  it("withdrawBond pays out PathUSD, not native value", async function () {
    const d = await deploy();
    await d.hub.connect(d.op).requestUnbond(OP_ID);
    await warp(61);
    const before = await d.bond.balanceOf(await d.op.getAddress());
    await d.hub.connect(d.op).withdrawBond(OP_ID);
    const after = await d.bond.balanceOf(await d.op.getAddress());
    expect(after - before).to.equal(ethers.parseEther("10"));
    const p = await d.hub.parties(OP_ID);
    expect(p.dead).to.equal(true);
    expect(p.bond).to.equal(0n);
  });
});

describe("BetaHubPathUSD: lockLocal / approvePending / expirePending", function () {
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

  it("rejects a second lockLocal for the same (user, nonce), and refunds two ERC20 legs on expiry", async function () {
    // Unlike BetaHub.test.ts's own version of this test, this one uses two
    // ERC20 local components, not one native + one ERC20 -- a native
    // (address(0)) local component is realistic on plain BetaHub but never
    // on Tempo: nothing could ever fund it there (Tempo hard-rejects any
    // nonzero-value transaction), so BetaHubPathUSD's _payOut(address(0),
    // ...) branch (which now routes through the PathUSD-only _pay) is a
    // provably-unreachable dead path there, the same "dead path, not a
    // live footgun" choice BetaVaultPathUSD.sol's own native branch makes
    // -- not something worth exercising as if it were a real scenario.
    const d = await deploy();
    const mock = await new MockERC20__factory(d.gov).deploy("Mock", "MOCK");
    await mock.waitForDeployment();
    const mockAddr = await mock.getAddress();
    await mock.mint(await d.user.getAddress(), ethers.parseUnits("1000", 18));
    await mock.connect(d.user).approve(await d.hub.getAddress(), ethers.parseUnits("1000", 18));

    const mock2 = await new MockERC20__factory(d.gov).deploy("Mock2", "MOCK2");
    await mock2.waitForDeployment();
    const mock2Addr = await mock2.getAddress();
    await mock2.mint(await d.user.getAddress(), ethers.parseUnits("1000", 18));
    await mock2.connect(d.user).approve(await d.hub.getAddress(), ethers.parseUnits("1000", 18));

    const TOKEN_COMPONENT = { networkId: SELF_NETWORK_ID, tokenId: mockAddr, amountPerUnit: ethers.parseUnits("2", 18) };
    const TOKEN_COMPONENT_2 = { networkId: SELF_NETWORK_ID, tokenId: mock2Addr, amountPerUnit: ethers.parseUnits("1", 18) };
    await d.hub.connect(d.gov).registerComposition(1n, [TOKEN_COMPONENT, TOKEN_COMPONENT_2, REMOTE_COMPONENT_1]);
    const deadline = (await now()) + 86_400n;
    await d.hub.connect(d.user).lockLocal(1n, 1n, 2n, deadline);
    await expect(d.hub.connect(d.user).lockLocal(1n, 1n, 2n, deadline)).to.be.revertedWithCustomError(d.hub, "PendingExists");

    expect(await mock.balanceOf(await d.hub.getAddress())).to.equal(2n * ethers.parseUnits("2", 18)); // 2 units * 2/unit = 4
    expect(await mock2.balanceOf(await d.hub.getAddress())).to.equal(2n * ethers.parseUnits("1", 18)); // 2 units * 1/unit = 2
    await warp(86_400 + 601);
    const mockBefore = await mock.balanceOf(await d.user.getAddress());
    const mock2Before = await mock2.balanceOf(await d.user.getAddress());
    await d.hub.connect(d.other).expirePending(await d.user.getAddress(), 1n);
    const mockAfter = await mock.balanceOf(await d.user.getAddress());
    const mock2After = await mock2.balanceOf(await d.user.getAddress());
    expect(mockAfter - mockBefore).to.equal(2n * ethers.parseUnits("2", 18));
    expect(mock2After - mock2Before).to.equal(2n * ethers.parseUnits("1", 18));
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

    await d.hub.connect(d.op).requestUnbond(OP_ID);
    await warp(61);
    await d.hub.connect(d.op).withdrawBond(OP_ID);
    await d.hub.connect(d.other).expirePending(userAddr, 1n);
  });
});

describe("BetaHubPathUSD: _processRemoteMint predicate", function () {
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

  it("a false MINT (wrong lockId) slashes the operator to the named victim in PathUSD, not native value, not insurance", async function () {
    const d = await withComposition();
    const deadline = (await now()) + 86_400n;
    const userAddr = await d.user.getAddress();
    await d.hub.connect(d.user).lockLocal(1n, 1n, 2n, deadline, { value: 2n * ONE });
    await d.hub.connect(d.user).approvePending(1n, [11n, 22n]);

    const victimBefore = await d.bond.balanceOf(userAddr);
    const insuranceBefore = await d.hub.insurance();
    const { tx } = await d.process(OP_ID, [GENESIS, 0], stmtMint(1n, 0, 999n, userAddr, 1n, 2n, deadline), d.op, 1);
    await expect(tx).to.emit(d.hub, "Slashed");
    const victimAfter = await d.bond.balanceOf(userAddr);
    const insuranceAfter = await d.hub.insurance();
    const slashed = 2n * PARAMS.slashWeiPerUnit;
    const bounty = (slashed * PARAMS.bountyBps) / 10_000n;
    expect(victimAfter - victimBefore).to.equal(slashed - bounty);
    expect(insuranceAfter).to.equal(insuranceBefore); // untouched -- this slash had a named victim
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

    const { txid } = await d.process(OP2_ID, [GENESIS, 0], stmtMint(1n, 0, 11n, userAddr, 1n, 1n, deadline), d.op2, 2);
    const pendingId = ethers.solidityPackedKeccak256(["address", "uint64"], [userAddr, 1n]);
    const p = await d.hub.pending(pendingId);
    expect(p.queuedBy).to.equal(OP2_ID);
    const remoteAnchorTxid = await d.hub.pendingRemoteAnchorTxid(pendingId);
    expect(remoteAnchorTxid[0]).to.equal(txid);
    expect(remoteAnchorTxid[1]).to.equal(ZERO32);
  });
});

describe("BetaHubPathUSD: exerciseMint", function () {
  it("refuses with an incomplete remote leg, mints exactly units * 1e18 once every leg is ready", async function () {
    const d = await deploy();
    await d.hub.connect(d.gov).registerComposition(1n, [NATIVE_COMPONENT, REMOTE_COMPONENT_1, REMOTE_COMPONENT_2]);
    const deadline = (await now()) + 86_400n;
    const userAddr = await d.user.getAddress();
    await d.hub.connect(d.user).lockLocal(1n, 1n, 2n, deadline, { value: 2n * ONE });
    await d.hub.connect(d.user).approvePending(1n, [11n, 22n]);

    const { txid: txid1 } = await d.process(OP_ID, [GENESIS, 0], stmtMint(1n, 0, 11n, userAddr, 1n, 2n, deadline), d.op, 1);
    await expect(d.hub.connect(d.other).exerciseMint(userAddr, 1n)).to.be.revertedWithCustomError(d.hub, "ComponentHeldOrNotReady");

    await warp(Number(PARAMS.tChallengeSecs) + 1);
    await expect(d.hub.connect(d.other).exerciseMint(userAddr, 1n)).to.be.revertedWithCustomError(d.hub, "IncompleteRemoteComponents");

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
    await d.hub.connect(d.other).exerciseMint(userAddr, 1n);
    expect(await token.balanceOf(userAddr)).to.equal(1n * 10n ** 18n);
  });
});

describe("BetaHubPathUSD: VETO / CLEAR hold a queued component", function () {
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

describe("BetaHubPathUSD: KIND_ALIVE dead-party veto", function () {
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
    expect(insuranceAfter).to.be.greaterThan(insuranceBefore); // no named victim for this one -- parked in insurance
    expect((await d.hub.parties(AUD_ID)).dead).to.equal(true);
  });

  it("a true dead-veto (KIND_VETO, zero target txid) pays the vetoReward in PathUSD, not native value", async function () {
    // The reward payout lives in _processVeto's dead-veto branch (a
    // KIND_VETO statement with targetTxid == 0, a shorthand "is this
    // operator dead?" claim), not in KIND_ALIVE's true-claim branch (which
    // has no payout at all -- KIND_ALIVE only pays out via _slash, on the
    // FALSE-claim path already covered by the "slashes a submitter..." case
    // above).
    const d = await deploy();
    await d.hub.connect(d.op).requestUnbond(OP_ID);
    await warp(61);
    await d.hub.connect(d.op).withdrawBond(OP_ID); // OP_ID is now genuinely dead.

    const before = await d.bond.balanceOf(await d.aud.getAddress());
    const rewardPoolBefore = await d.hub.rewardPool();
    const { tx } = await d.process(AUD_ID, [GENESIS, 0], stmtVeto(OP_ID, ZERO32), d.aud, 1);
    await expect(tx).to.not.emit(d.hub, "Slashed");
    const after = await d.bond.balanceOf(await d.aud.getAddress());
    const rewardPoolAfter = await d.hub.rewardPool();
    expect(after - before).to.equal(PARAMS.vetoRewardWei);
    expect(rewardPoolBefore - rewardPoolAfter).to.equal(PARAMS.vetoRewardWei);
  });
});

describe("BetaHubPathUSD: fundRewards", function () {
  it("pulls PathUSD via transferFrom and credits rewardPool exactly", async function () {
    const d = await deploy();
    const before = await d.hub.rewardPool();
    await d.bond.mint(await d.other.getAddress(), ethers.parseEther("2"));
    await d.bond.connect(d.other).approve(await d.hub.getAddress(), ethers.parseEther("2"));
    await d.hub.connect(d.other).fundRewards(ethers.parseEther("2"));
    expect((await d.hub.rewardPool()) - before).to.equal(ethers.parseEther("2"));
  });
});
