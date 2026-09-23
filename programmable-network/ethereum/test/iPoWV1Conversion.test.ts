import { expect } from "chai";
import { network } from "hardhat";
import {
  sha256,
  getBytes,
  hexlify,
  concat,
  toBeHex,
  zeroPadValue,
  AbiCoder,
  keccak256,
} from "ethers";

import { IPoWV1__factory } from "../types/ethers-contracts/index.ts";
import { IPoWV1Conversion__factory } from "../types/ethers-contracts/index.ts";
import { MockERC20__factory } from "../types/ethers-contracts/index.ts";
import { COMMIT_FEE_BPS, NATIVE_DECIMALS, SELF_NETWORK_ID } from "./helpers/deploy.ts";

const { ethers } = await network.create();

function doubleSha256(data: Uint8Array): Uint8Array {
  return getBytes(sha256(getBytes(sha256(data))));
}

/// `iPoWV1.commitGlobalBitcoinHeader80` requires real proof-of-work capped
/// at `BitcoinPrimitives._powLimit()` (~2^224, genuine Bitcoin-difficulty
/// scale) — brute-forcing a nonce against that in a JS test would need
/// ~4 billion hashes, impractical for a fast unit test. Instead this seeds
/// `globalHeightToHashLE`/`globalHeaders` directly via Hardhat's storage
/// cheat-code, mirroring how the Solana test suite bypasses full
/// instruction flow to seed account state directly for setup
/// (`seed_message_commitment` in `test_message_commitment.rs`). Slot
/// layout verified against `iPoWV1`'s actual declaration order — see the
/// inline comments below for how each slot number and struct offset was
/// derived. Only `merkleRootLE` and `set` are populated: those are the
/// only two fields `iPoWV1Conversion` ever reads back.
async function seedHeader(ipowAddress: string, height: bigint, headerHashLE: string, merkleRootLE: string) {
  const abi = AbiCoder.defaultAbiCoder();

  // `globalHeightToHashLE` is `iPoWV1`'s 10th declared storage variable
  // (0-indexed slot 9): operator(0), commitFeeBps(1), nativeLiquidity(2),
  // totalLockedDeposits(3), totalReservedNative(4), totalHeldCommitFees(5),
  // minAnchorHeight(6), nextTxId(7), conversions(8),
  // globalHeightToHashLE(9). `NATIVE_DECIMALS`/`SELF_NETWORK_ID` are
  // `immutable` (no storage slot); `ReentrancyGuard` uses OZ 5.x's
  // ERC-7201 namespaced storage (a fixed, hash-derived slot), not a
  // sequential one, so it doesn't shift anything here.
  const heightSlot = keccak256(abi.encode(["uint256", "uint256"], [height, 9n]));
  await ethers.provider.send("hardhat_setStorageAt", [ipowAddress, heightSlot, headerHashLE]);

  // `globalHeaders` is slot 10. `GlobalHeaderMeta` packs as: prevHashLE
  // (its own full slot, offset 0), merkleRootLE (its own full slot, offset
  // 1), then nBits(uint32)+timestamp(uint32)+set(bool)+arrivalTime(uint64)
  // packed together into one slot (offset 2) — Solidity packs
  // earlier-declared fields into lower-order bits, so within that packed
  // slot: nBits occupies bits [0,32), timestamp [32,64), set [64,72),
  // arrivalTime [72,136).
  const baseSlot = BigInt(keccak256(abi.encode(["bytes32", "uint256"], [headerHashLE, 10n])));
  const merkleSlot = zeroPadValue(toBeHex(baseSlot + 1n), 32);
  await ethers.provider.send("hardhat_setStorageAt", [ipowAddress, merkleSlot, merkleRootLE]);

  const packedSlot = zeroPadValue(toBeHex(1n << 64n), 32); // set = true, everything else 0
  const packedSlotAddr = zeroPadValue(toBeHex(baseSlot + 2n), 32);
  await ethers.provider.send("hardhat_setStorageAt", [ipowAddress, packedSlotAddr, packedSlot]);
}

/// A minimal single-input, single-output legacy transaction paying
/// `valueSats` to `program`.
function txPaying(program: Uint8Array, valueSats: bigint): string {
  const parts: Uint8Array[] = [];
  parts.push(getBytes(zeroPadValue(toBeHex(1), 4)).reverse()); // version
  parts.push(Uint8Array.of(0x01)); // inCount
  parts.push(new Uint8Array(32)); // prevout txid
  parts.push(new Uint8Array(4)); // prevout vout
  parts.push(Uint8Array.of(0x00)); // scriptSig len
  parts.push(new Uint8Array(4).fill(0xff)); // sequence
  parts.push(Uint8Array.of(0x01)); // outCount
  const valueLE = getBytes(zeroPadValue(toBeHex(valueSats), 8)).reverse();
  parts.push(valueLE);
  parts.push(Uint8Array.of(program.length));
  parts.push(program);
  return hexlify(concat(parts));
}

// iPoWV1 deployed purely as the header-relay source `iPoWV1Conversion`
// reads cross-contract — its own Conversion logic is unused in these tests.
async function deployBoth(admin: any) {
  const ipow = await new IPoWV1__factory(admin).deploy(
    NATIVE_DECIMALS,
    SELF_NETWORK_ID,
    await admin.getAddress(),
    COMMIT_FEE_BPS
  );
  await ipow.waitForDeployment();

  const conversion = await new IPoWV1Conversion__factory(admin).deploy(
    await admin.getAddress(),
    await ipow.getAddress(),
    COMMIT_FEE_BPS
  );
  await conversion.waitForDeployment();

  return { ipow, conversion };
}

describe("iPoWV1Conversion: permissionless auction", function () {
  it("a claimant wins the auction and completes a bitcoin->native conversion", async function () {
    const [admin, user, claimant] = await ethers.getSigners();
    const { ipow, conversion } = await deployBoth(admin);

    const BITCOIN_AMOUNT = 100_000n;
    const NATIVE_AMOUNT = ethers.parseEther("1");
    const REQUIRED_BOND = ethers.parseEther("0.5");

    const userProgram = getBytes("0x76a914" + "11".repeat(20) + "88ac");

    await conversion
      .connect(user)
      .commitBitcoinToToken(BITCOIN_AMOUNT, NATIVE_AMOUNT, userProgram, REQUIRED_BOND, ethers.ZeroAddress, {
        value: (NATIVE_AMOUNT * COMMIT_FEE_BPS) / 10_000n,
      });

    const txId = 1n;

    // Claimant wins the auction — permissionless, no fixed operator gate.
    // Bitcoin->native requires the claimant's own receive script here
    // (checked later against the real Bitcoin payment); `userProgram` set
    // at commit time only ever matters as a fallback if the claimant
    // doesn't supply one. `msg.value` carries both the stake and (no more
    // separate governance-funded pool) the claimant's own self-escrow of
    // the real payout.
    const claimantProgram = getBytes("0x76a914" + "22".repeat(20) + "88ac");
    await conversion
      .connect(claimant)
      .proposeClaimConversion(txId, NATIVE_AMOUNT, 3600n, claimantProgram, { value: REQUIRED_BOND + NATIVE_AMOUNT });

    // Quiet period (1 minute) must pass before the claim locks in.
    await ethers.provider.send("evm_increaseTime", [61]);
    await ethers.provider.send("evm_mine", []);

    await conversion.connect(claimant).finalizeClaimConversion(txId);

    const conv = await conversion.conversions(txId);
    expect(conv.responsibleOperator).to.equal(await claimant.getAddress());
    expect(conv.reservedNative).to.equal(NATIVE_AMOUNT);

    // Craft a Bitcoin tx paying the claimed output script, and seed a
    // header (bypassing real PoW mining — see seedHeader's own comment)
    // whose merkle root is that transaction's own txid.
    const txRaw = txPaying(claimantProgram, BITCOIN_AMOUNT + 1_000n);
    const txidLE = doubleSha256(getBytes(txRaw));
    const merkleRootLE = hexlify(txidLE); // single-tx block: merkle root == the one txid
    const headerHashLE = keccak256(ethers.toUtf8Bytes("test-header-0"));

    await seedHeader(await ipow.getAddress(), 0n, headerHashLE, merkleRootLE);

    const escrowBalanceBefore = await ethers.provider.getBalance(await conversion.getAddress());
    const claimantBalanceBefore = await ethers.provider.getBalance(await claimant.getAddress());

    // The user themselves proves the real Bitcoin payment, releasing the
    // operator's already-reserved payout to the user.
    await conversion
      .connect(user)
      .submitBitcoinMerkleProofWithTx(txId, txRaw, 0n, 0n, [], 0n);

    const convAfter = await conversion.conversions(txId);
    expect(convAfter.proofVerified).to.equal(true);

    // Unlike Solana's separate stake-escrow PDA, this contract holds both
    // the value it moves and auction stakes in the same balance — so the
    // drop here is the native payout + commit fee + the claimant's own
    // stake being returned to them, all at once.
    const escrowBalanceAfter = await ethers.provider.getBalance(await conversion.getAddress());
    expect(escrowBalanceBefore - escrowBalanceAfter).to.equal(
      NATIVE_AMOUNT + (NATIVE_AMOUNT * COMMIT_FEE_BPS) / 10_000n + REQUIRED_BOND
    );

    // Claimant gets their stake back plus the commit fee, for actually
    // completing the duty (not a fee-payer in this tx, so uncontaminated).
    const claimantBalanceAfter = await ethers.provider.getBalance(await claimant.getAddress());
    expect(claimantBalanceAfter).to.be.greaterThan(claimantBalanceBefore);
  });

  it("a claimant wins the auction and completes a native->bitcoin conversion", async function () {
    const [admin, user, claimant] = await ethers.getSigners();
    const { ipow, conversion } = await deployBoth(admin);

    const BITCOIN_AMOUNT = 100_000n;
    const NATIVE_AMOUNT = ethers.parseEther("1");
    const REQUIRED_BOND = ethers.parseEther("0.5");
    const COMMIT_FEE = (NATIVE_AMOUNT * COMMIT_FEE_BPS) / 10_000n;

    // User's own Bitcoin destination script, fixed at commit time — this is
    // what the auction winner must pay.
    const userProgram = getBytes("0x76a914" + "33".repeat(20) + "88ac");

    await conversion
      .connect(user)
      .commitTokenToBitcoin(NATIVE_AMOUNT, BITCOIN_AMOUNT, userProgram, REQUIRED_BOND, ethers.ZeroAddress, [], {
        value: COMMIT_FEE,
      });

    const txId = 1n;

    // Claimant wins the auction — no receive-program of their own needed
    // here (the user already fixed the destination script at commit time).
    await conversion.connect(claimant).proposeClaimConversion(txId, BITCOIN_AMOUNT, 3600n, "0x", {
      value: REQUIRED_BOND,
    });

    await ethers.provider.send("evm_increaseTime", [61]);
    await ethers.provider.send("evm_mine", []);

    await conversion.connect(claimant).finalizeClaimConversion(txId);

    // User deposits real value only now that a real claimant is locked in.
    await conversion.connect(user).depositApprovedConversion(txId, { value: NATIVE_AMOUNT });

    const txRaw = txPaying(userProgram, BITCOIN_AMOUNT + 1_000n);
    const txidLE = doubleSha256(getBytes(txRaw));
    const merkleRootLE = hexlify(txidLE);
    const headerHashLE = keccak256(ethers.toUtf8Bytes("test-header-native-to-bitcoin"));
    await seedHeader(await ipow.getAddress(), 0n, headerHashLE, merkleRootLE);

    const claimantBalanceBefore = await ethers.provider.getBalance(await claimant.getAddress());
    const escrowBalanceBefore = await ethers.provider.getBalance(await conversion.getAddress());

    // The claimant themselves proves they paid the user's Bitcoin address,
    // releasing the user's own deposit to themselves.
    await conversion
      .connect(claimant)
      .submitBitcoinMerkleProofWithTx(txId, txRaw, 0n, 0n, [], 0n);

    const conv = await conversion.conversions(txId);
    expect(conv.proofVerified).to.equal(true);

    // Escrow releases the user's deposit + fee, and separately refunds the
    // claimant's own stake — all from the same balance.
    const escrowBalanceAfter = await ethers.provider.getBalance(await conversion.getAddress());
    expect(escrowBalanceBefore - escrowBalanceAfter).to.equal(NATIVE_AMOUNT + COMMIT_FEE + REQUIRED_BOND);

    // Claimant is not a fee-payer in this tx (only gas, tracked separately
    // from the transfer), so their net gain should exceed the payout itself.
    const claimantBalanceAfter = await ethers.provider.getBalance(await claimant.getAddress());
    expect(claimantBalanceAfter).to.be.greaterThan(claimantBalanceBefore + NATIVE_AMOUNT - ethers.parseEther("0.01"));
  });

  it("reclaim forfeits stake into bounty and lets a new claimant finish", async function () {
    const [admin, user, claimantA, claimantB] = await ethers.getSigners();
    const { ipow, conversion } = await deployBoth(admin);

    const BITCOIN_AMOUNT = 100_000n;
    const NATIVE_AMOUNT = ethers.parseEther("1");
    const REQUIRED_BOND = ethers.parseEther("0.5");

    const userProgram = getBytes("0x76a914" + "11".repeat(20) + "88ac");
    await conversion
      .connect(user)
      .commitBitcoinToToken(BITCOIN_AMOUNT, NATIVE_AMOUNT, userProgram, REQUIRED_BOND, ethers.ZeroAddress, {
        value: (NATIVE_AMOUNT * COMMIT_FEE_BPS) / 10_000n,
      });

    const txId = 1n;

    // claimantA self-escrows NATIVE_AMOUNT as part of this first claim.
    const claimantAProgram = getBytes("0x76a914" + "aa".repeat(20) + "88ac");
    await conversion
      .connect(claimantA)
      .proposeClaimConversion(txId, NATIVE_AMOUNT, 3600n, claimantAProgram, { value: REQUIRED_BOND + NATIVE_AMOUNT });

    await ethers.provider.send("evm_increaseTime", [61]);
    await ethers.provider.send("evm_mine", []);
    await conversion.connect(claimantA).finalizeClaimConversion(txId);

    let conv = await conversion.conversions(txId);
    expect(conv.reservedNative).to.equal(NATIVE_AMOUNT);

    // claimantA goes dark — duty window (3600s) plus a margin passes with
    // no proof ever submitted.
    await ethers.provider.send("evm_increaseTime", [3601]);
    await ethers.provider.send("evm_mine", []);

    const stakeHolderBalanceBefore = await ethers.provider.getBalance(await conversion.getAddress());

    // Permissionless — the user themselves reclaims, not claimantA.
    await conversion.connect(user).reclaimExpiredConversion(txId);

    conv = await conversion.conversions(txId);
    // Forfeited (not refunded): the contract's own balance is unchanged (no
    // outbound transfer happened), the conversion's own bounty absorbed it.
    expect(await ethers.provider.getBalance(await conversion.getAddress())).to.equal(stakeHolderBalanceBefore);
    expect(conv.bounty).to.equal(REQUIRED_BOND);
    expect(conv.requiredBond).to.equal(REQUIRED_BOND);
    expect(conv.responsibleOperator).to.equal(ethers.ZeroAddress);
    expect(conv.operatorDutyExpiresAt).to.equal(0n);
    // Reservation persists — claimantB doesn't need the pool to re-reserve.
    expect(conv.reservedNative).to.equal(NATIVE_AMOUNT);

    // claimantB picks up the exact same reservation and finishes it,
    // earning the forfeited bounty on top of their own returned stake.
    // `windowStarted` is already true (set at the first finalize above), so
    // this claim does *not* re-touch the escrow — only the stake.
    const claimantBProgram = getBytes("0x76a914" + "bb".repeat(20) + "88ac");
    await conversion
      .connect(claimantB)
      .proposeClaimConversion(txId, NATIVE_AMOUNT, 3600n, claimantBProgram, { value: REQUIRED_BOND });

    await ethers.provider.send("evm_increaseTime", [61]);
    await ethers.provider.send("evm_mine", []);
    await conversion.connect(claimantB).finalizeClaimConversion(txId);

    // Reservation must not have doubled from the second finalize.
    conv = await conversion.conversions(txId);
    expect(conv.reservedNative).to.equal(NATIVE_AMOUNT);

    const txRaw = txPaying(claimantBProgram, BITCOIN_AMOUNT + 1_000n);
    const txidLE = doubleSha256(getBytes(txRaw));
    const merkleRootLE = hexlify(txidLE);
    const headerHashLE = keccak256(ethers.toUtf8Bytes("test-header-reclaim"));
    await seedHeader(await ipow.getAddress(), 0n, headerHashLE, merkleRootLE);

    const userBalanceBefore = await ethers.provider.getBalance(await user.getAddress());

    await conversion.connect(user).submitBitcoinMerkleProofWithTx(txId, txRaw, 0n, 0n, [], 0n);

    conv = await conversion.conversions(txId);
    expect(conv.proofVerified).to.equal(true);

    // User pays this tx's own gas, so check against the payout minus a
    // margin rather than an exact equality.
    const userBalanceAfter = await ethers.provider.getBalance(await user.getAddress());
    expect(userBalanceAfter).to.be.greaterThan(userBalanceBefore + NATIVE_AMOUNT - ethers.parseEther("0.01"));

    // claimantB earns their own stake back *plus* the bounty claimantA
    // forfeited.
    const claimantBBalance = await ethers.provider.getBalance(await claimantB.getAddress());
    expect(claimantBBalance).to.be.greaterThan(REQUIRED_BOND + conv.bounty);
  });

  it("an ERC20 bitcoin->native conversion pays out the real token", async function () {
    const [admin, user, claimant] = await ethers.getSigners();
    const { ipow, conversion } = await deployBoth(admin);

    const token = await new MockERC20__factory(admin).deploy("Mock", "MOCK");
    await token.waitForDeployment();
    const tokenAddr = await token.getAddress();

    const BITCOIN_AMOUNT = 100_000n;
    const TOKEN_AMOUNT = ethers.parseUnits("5", 18);
    const REQUIRED_BOND = ethers.parseEther("0.5"); // stake is always native

    // The claimant is minted exactly what they'll self-escrow at propose
    // time — no separate governance-funded pool anymore.
    await token.mint(await claimant.getAddress(), TOKEN_AMOUNT);
    await token.connect(claimant).approve(await conversion.getAddress(), TOKEN_AMOUNT);

    const claimantProgram = getBytes("0x76a914" + "cc".repeat(20) + "88ac");

    await conversion
      .connect(user)
      .commitBitcoinToToken(BITCOIN_AMOUNT, TOKEN_AMOUNT, getBytes("0x76a914" + "11".repeat(20) + "88ac"), REQUIRED_BOND, tokenAddr, {
        value: (TOKEN_AMOUNT * COMMIT_FEE_BPS) / 10_000n,
      });

    const txId = 1n;

    // Claimant self-escrows the real TOKEN_AMOUNT of the ERC20 here — the
    // stake (always native ETH) still rides on `msg.value`.
    await conversion
      .connect(claimant)
      .proposeClaimConversion(txId, TOKEN_AMOUNT, 3600n, claimantProgram, { value: REQUIRED_BOND });

    await ethers.provider.send("evm_increaseTime", [61]);
    await ethers.provider.send("evm_mine", []);
    await conversion.connect(claimant).finalizeClaimConversion(txId);

    const conv = await conversion.conversions(txId);
    expect(conv.reservedNative).to.equal(TOKEN_AMOUNT);
    expect(conv.tokenAddr).to.equal(tokenAddr);

    const txRaw = txPaying(claimantProgram, BITCOIN_AMOUNT + 1_000n);
    const txidLE = doubleSha256(getBytes(txRaw));
    const merkleRootLE = hexlify(txidLE);
    const headerHashLE = keccak256(ethers.toUtf8Bytes("test-header-erc20"));
    await seedHeader(await ipow.getAddress(), 0n, headerHashLE, merkleRootLE);

    await conversion.connect(user).submitBitcoinMerkleProofWithTx(txId, txRaw, 0n, 0n, [], 0n);

    const convAfter = await conversion.conversions(txId);
    expect(convAfter.proofVerified).to.equal(true);

    // The user's token balance reflects the real payout — the commit fee
    // and stake both moved native ETH throughout, never touching the
    // ERC20 at all. The contract held exactly what the claimant
    // self-escrowed — fully paid out, nothing left over.
    expect(await token.balanceOf(await user.getAddress())).to.equal(TOKEN_AMOUNT);
    expect(await token.balanceOf(await conversion.getAddress())).to.equal(0n);
  });

  it("openBundleTunnel: single-token happy path pays out via submitBitcoinMerkleProofWithTx", async function () {
    const [admin, opener, dest] = await ethers.getSigners();
    const { ipow, conversion } = await deployBoth(admin);

    const NETWORK_ID = 7n;
    await conversion.connect(admin).addNetwork(NETWORK_ID, 1, 32);

    const BITCOIN_AMOUNT = 100_000n;
    const NATIVE_AMOUNT = ethers.parseEther("1");
    const openerProgram = getBytes("0x76a914" + "99".repeat(20) + "88ac");
    const networkAddress = getBytes("0x" + "11".repeat(20));

    await conversion
      .connect(opener)
      .openBundleTunnel(
        NATIVE_AMOUNT,
        BITCOIN_AMOUNT,
        ethers.ZeroAddress,
        [],
        await dest.getAddress(),
        NETWORK_ID,
        networkAddress,
        3600n,
        openerProgram,
        { value: NATIVE_AMOUNT }
      );

    const txId = 1n;
    const conv = await conversion.conversions(txId);
    expect(conv.responsibleOperator).to.equal(await opener.getAddress());
    expect(conv.windowStarted).to.equal(true);
    expect(conv.reservedNative).to.equal(NATIVE_AMOUNT);
    expect(conv.user).to.equal(await dest.getAddress());

    const txRaw = txPaying(openerProgram, BITCOIN_AMOUNT + 1_000n);
    const txidLE = doubleSha256(getBytes(txRaw));
    const merkleRootLE = hexlify(txidLE);
    const headerHashLE = keccak256(ethers.toUtf8Bytes("test-header-tunnel-single"));
    await seedHeader(await ipow.getAddress(), 0n, headerHashLE, merkleRootLE);

    const escrowBalanceBefore = await ethers.provider.getBalance(await conversion.getAddress());

    // Bitcoin->token: the recipient (`dest`) is the one who proves payment.
    await conversion.connect(dest).submitBitcoinMerkleProofWithTx(txId, txRaw, 0n, 0n, [], 0n);

    const convAfter = await conversion.conversions(txId);
    expect(convAfter.proofVerified).to.equal(true);

    const escrowBalanceAfter = await ethers.provider.getBalance(await conversion.getAddress());
    expect(escrowBalanceBefore - escrowBalanceAfter).to.equal(NATIVE_AMOUNT);
  });

  it("openBundleTunnel: bundle pays out primary and extra token", async function () {
    const [admin, opener, dest] = await ethers.getSigners();
    const { ipow, conversion } = await deployBoth(admin);

    const NETWORK_ID = 7n;
    await conversion.connect(admin).addNetwork(NETWORK_ID, 1, 32);

    const token = await new MockERC20__factory(admin).deploy("Mock", "MOCK");
    await token.waitForDeployment();
    const tokenAddr = await token.getAddress();

    const BITCOIN_AMOUNT = 100_000n;
    const NATIVE_AMOUNT = ethers.parseEther("1");
    const EXTRA_AMOUNT = ethers.parseUnits("2", 18);

    // The opener self-funds the whole bundle atomically at open time.
    await token.mint(await opener.getAddress(), EXTRA_AMOUNT);
    await token.connect(opener).approve(await conversion.getAddress(), EXTRA_AMOUNT);

    const openerProgram = getBytes("0x76a914" + "aa".repeat(20) + "88ac");
    const networkAddress = getBytes("0x" + "11".repeat(20));

    await conversion
      .connect(opener)
      .openBundleTunnel(
        NATIVE_AMOUNT,
        BITCOIN_AMOUNT,
        ethers.ZeroAddress,
        [[tokenAddr, EXTRA_AMOUNT]],
        await dest.getAddress(),
        NETWORK_ID,
        networkAddress,
        3600n,
        openerProgram,
        { value: NATIVE_AMOUNT }
      );

    const txId = 1n;
    expect(await token.balanceOf(await conversion.getAddress())).to.equal(EXTRA_AMOUNT);

    const txRaw = txPaying(openerProgram, BITCOIN_AMOUNT + 1_000n);
    const txidLE = doubleSha256(getBytes(txRaw));
    const merkleRootLE = hexlify(txidLE);
    const headerHashLE = keccak256(ethers.toUtf8Bytes("test-header-tunnel-bundle"));
    await seedHeader(await ipow.getAddress(), 0n, headerHashLE, merkleRootLE);

    // Exercises the `submitBitcoinMerkleProofWithTx` bitcoin->token bundle
    // payout fix — before it, only `nativeAmount` would have paid out.
    await conversion.connect(dest).submitBitcoinMerkleProofWithTx(txId, txRaw, 0n, 0n, [], 0n);

    const conv = await conversion.conversions(txId);
    expect(conv.proofVerified).to.equal(true);
    expect(await token.balanceOf(await dest.getAddress())).to.equal(EXTRA_AMOUNT);
    expect(await token.balanceOf(await conversion.getAddress())).to.equal(0n);
  });

  it("openBundleTunnel: guaranteed resolution force-claims all tokens", async function () {
    const [admin, opener, dest] = await ethers.getSigners();
    const { conversion } = await deployBoth(admin);

    const NETWORK_ID = 7n;
    await conversion.connect(admin).addNetwork(NETWORK_ID, 1, 32);

    const token = await new MockERC20__factory(admin).deploy("Mock", "MOCK");
    await token.waitForDeployment();
    const tokenAddr = await token.getAddress();

    const BITCOIN_AMOUNT = 100_000n;
    const NATIVE_AMOUNT = ethers.parseEther("1");
    const EXTRA_AMOUNT = ethers.parseUnits("3", 18);
    const DUTY_WINDOW_SECONDS = 3600;

    await token.mint(await opener.getAddress(), EXTRA_AMOUNT);
    await token.connect(opener).approve(await conversion.getAddress(), EXTRA_AMOUNT);

    const openerProgram = getBytes("0x76a914" + "bb".repeat(20) + "88ac");
    const networkAddress = getBytes("0x" + "11".repeat(20));

    await conversion
      .connect(opener)
      .openBundleTunnel(
        NATIVE_AMOUNT,
        BITCOIN_AMOUNT,
        ethers.ZeroAddress,
        [[tokenAddr, EXTRA_AMOUNT]],
        await dest.getAddress(),
        NETWORK_ID,
        networkAddress,
        BigInt(DUTY_WINDOW_SECONDS),
        openerProgram,
        { value: NATIVE_AMOUNT }
      );

    const txId = 1n;

    // Duty window expires with no proof ever submitted.
    await ethers.provider.send("evm_increaseTime", [DUTY_WINDOW_SECONDS + 1]);
    await ethers.provider.send("evm_mine", []);

    const escrowBalanceBefore = await ethers.provider.getBalance(await conversion.getAddress());

    // Permissionless — anyone can call it, not just opener or dest.
    await conversion.connect(admin).claimNativeOperatorExpired(txId);

    const escrowBalanceAfter = await ethers.provider.getBalance(await conversion.getAddress());
    expect(escrowBalanceBefore - escrowBalanceAfter).to.equal(NATIVE_AMOUNT);

    // Exercises the `claimNativeOperatorExpired` bundle-loop fix — before
    // it, only `reservedNative` (the primary) would have resolved.
    expect(await token.balanceOf(await dest.getAddress())).to.equal(EXTRA_AMOUNT);
    expect(await token.balanceOf(await conversion.getAddress())).to.equal(0n);
  });

  it("openBundleTunnel: rejects more than three extra tokens", async function () {
    const [admin, opener, dest] = await ethers.getSigners();
    const { conversion } = await deployBoth(admin);

    const NETWORK_ID = 7n;
    await conversion.connect(admin).addNetwork(NETWORK_ID, 1, 32);

    const extraTokens = Array.from({ length: 4 }, (_, i) => [
      ethers.getAddress("0x" + (i + 1).toString(16).padStart(40, "0")),
      1_000n,
    ]);
    const openerProgram = getBytes("0x76a914" + "cc".repeat(20) + "88ac");

    await expect(
      conversion
        .connect(opener)
        .openBundleTunnel(
          ethers.parseEther("1"),
          100_000n,
          ethers.ZeroAddress,
          extraTokens,
          await dest.getAddress(),
          NETWORK_ID,
          getBytes("0x" + "11".repeat(20)),
          3600n,
          openerProgram,
          { value: ethers.parseEther("1") }
        )
    ).to.be.revertedWithCustomError(conversion, "TooManyTokens");
  });

  it("openBundleTunnel: rejects networkId == 0", async function () {
    const [admin, opener, dest] = await ethers.getSigners();
    const { conversion } = await deployBoth(admin);

    const openerProgram = getBytes("0x76a914" + "dd".repeat(20) + "88ac");

    await expect(
      conversion
        .connect(opener)
        .openBundleTunnel(
          ethers.parseEther("1"),
          100_000n,
          ethers.ZeroAddress,
          [],
          await dest.getAddress(),
          0n,
          "0x",
          3600n,
          openerProgram,
          { value: ethers.parseEther("1") }
        )
    ).to.be.revertedWithCustomError(conversion, "IncorrectNetwork");
  });
});
