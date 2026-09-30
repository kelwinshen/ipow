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

import { IPoW__factory, IPoWConversionPathUSD__factory, MockERC20__factory } from "../types/ethers-contracts/index.ts";
import { COMMIT_FEE_BPS, NATIVE_DECIMALS, SELF_NETWORK_ID } from "./helpers/deploy.ts";

const { ethers } = await network.create();

// Mirrors test/iPoWConversion.test.ts's own cases exactly (same
// infrastructure: raw Bitcoin tx builders, iPoW storage cheat-codes) —
// only the commit fee, auction stake, and bounty move PathUSD here
// instead of msg.value; the conversion's own value (tokenAddr-denominated)
// is untouched, so tests that use tokenAddr = address(0) for it are still
// meaningful structural-parity coverage (that native branch stays present,
// just unreachable on real Tempo — see the contract's own header comment),
// not a claim that real Tempo usage would ever choose it.
function doubleSha256(data: Uint8Array): Uint8Array {
  return getBytes(sha256(getBytes(sha256(data))));
}

async function seedHeader(ipowAddress: string, height: bigint, headerHashLE: string, merkleRootLE: string) {
  const abi = AbiCoder.defaultAbiCoder();
  const heightSlot = keccak256(abi.encode(["uint256", "uint256"], [height, 9n]));
  await ethers.provider.send("hardhat_setStorageAt", [ipowAddress, heightSlot, headerHashLE]);
  const baseSlot = BigInt(keccak256(abi.encode(["bytes32", "uint256"], [headerHashLE, 10n])));
  const merkleSlot = zeroPadValue(toBeHex(baseSlot + 1n), 32);
  await ethers.provider.send("hardhat_setStorageAt", [ipowAddress, merkleSlot, merkleRootLE]);
  const packedSlot = zeroPadValue(toBeHex(1n << 64n), 32);
  const packedSlotAddr = zeroPadValue(toBeHex(baseSlot + 2n), 32);
  await ethers.provider.send("hardhat_setStorageAt", [ipowAddress, packedSlotAddr, packedSlot]);
}

function txPaying(program: Uint8Array, valueSats: bigint): string {
  const parts: Uint8Array[] = [];
  parts.push(getBytes(zeroPadValue(toBeHex(1), 4)).reverse());
  parts.push(Uint8Array.of(0x01));
  parts.push(new Uint8Array(32));
  parts.push(new Uint8Array(4));
  parts.push(Uint8Array.of(0x00));
  parts.push(new Uint8Array(4).fill(0xff));
  parts.push(Uint8Array.of(0x01));
  const valueLE = getBytes(zeroPadValue(toBeHex(valueSats), 8)).reverse();
  parts.push(valueLE);
  parts.push(Uint8Array.of(program.length));
  parts.push(program);
  return hexlify(concat(parts));
}

async function deployAll(admin: any) {
  const ipow = await new IPoW__factory(admin).deploy(
    NATIVE_DECIMALS,
    SELF_NETWORK_ID,
    await admin.getAddress(),
    COMMIT_FEE_BPS
  );
  await ipow.waitForDeployment();

  const bond = await new MockERC20__factory(admin).deploy("Mock PathUSD", "mUSD");
  await bond.waitForDeployment();

  const conversion = await new IPoWConversionPathUSD__factory(admin).deploy(
    await admin.getAddress(),
    await ipow.getAddress(),
    await bond.getAddress(),
    COMMIT_FEE_BPS
  );
  await conversion.waitForDeployment();

  async function fundAndApprove(signer: any, amount: bigint) {
    if (amount === 0n) return;
    await bond.mint(await signer.getAddress(), amount);
    await bond.connect(signer).approve(await conversion.getAddress(), amount);
  }

  return { ipow, bond, conversion, fundAndApprove };
}

describe("iPoWConversionPathUSD: permissionless auction", function () {
  it("a claimant wins the auction and completes a bitcoin->native conversion", async function () {
    const [admin, user, claimant] = await ethers.getSigners();
    const { ipow, bond, conversion, fundAndApprove } = await deployAll(admin);

    const BITCOIN_AMOUNT = 100_000n;
    const NATIVE_AMOUNT = ethers.parseEther("1");
    const REQUIRED_BOND = ethers.parseEther("0.5");
    const COMMIT_FEE = (NATIVE_AMOUNT * COMMIT_FEE_BPS) / 10_000n;

    const userProgram = getBytes("0x76a914" + "11".repeat(20) + "88ac");

    await fundAndApprove(user, COMMIT_FEE);
    await conversion
      .connect(user)
      .commitBitcoinToToken(BITCOIN_AMOUNT, NATIVE_AMOUNT, userProgram, REQUIRED_BOND, ethers.ZeroAddress);

    const txId = 1n;

    // tokenAddr == address(0) here, so the self-escrow portion of the
    // conversion's own value still moves native msg.value -- only the
    // stake (now an explicit param) moves PathUSD.
    const claimantProgram = getBytes("0x76a914" + "22".repeat(20) + "88ac");
    await fundAndApprove(claimant, REQUIRED_BOND);
    await conversion
      .connect(claimant)
      .proposeClaimConversion(txId, NATIVE_AMOUNT, 3600n, claimantProgram, REQUIRED_BOND, { value: NATIVE_AMOUNT });

    await ethers.provider.send("evm_increaseTime", [61]);
    await ethers.provider.send("evm_mine", []);

    await conversion.connect(claimant).finalizeClaimConversion(txId);

    const conv = await conversion.conversions(txId);
    expect(conv.responsibleOperator).to.equal(await claimant.getAddress());
    expect(conv.reservedNative).to.equal(NATIVE_AMOUNT);

    const txRaw = txPaying(claimantProgram, BITCOIN_AMOUNT + 1_000n);
    const txidLE = doubleSha256(getBytes(txRaw));
    const merkleRootLE = hexlify(txidLE);
    const headerHashLE = keccak256(ethers.toUtf8Bytes("test-header-0"));
    await seedHeader(await ipow.getAddress(), 0n, headerHashLE, merkleRootLE);

    const escrowNativeBefore = await ethers.provider.getBalance(await conversion.getAddress());
    const escrowBondBefore = await bond.balanceOf(await conversion.getAddress());
    const claimantBondBefore = await bond.balanceOf(await claimant.getAddress());

    await conversion.connect(user).submitBitcoinMerkleProofWithTx(txId, txRaw, 0n, 0n, [], 0n);

    const convAfter = await conversion.conversions(txId);
    expect(convAfter.proofVerified).to.equal(true);

    // Native drop is just the conversion's own value payout now.
    const escrowNativeAfter = await ethers.provider.getBalance(await conversion.getAddress());
    expect(escrowNativeBefore - escrowNativeAfter).to.equal(NATIVE_AMOUNT);

    // PathUSD drop is the fee + stake, both released to the claimant.
    const escrowBondAfter = await bond.balanceOf(await conversion.getAddress());
    expect(escrowBondBefore - escrowBondAfter).to.equal(COMMIT_FEE + REQUIRED_BOND);
    const claimantBondAfter = await bond.balanceOf(await claimant.getAddress());
    expect(claimantBondAfter - claimantBondBefore).to.equal(COMMIT_FEE + REQUIRED_BOND);
  });

  it("a claimant wins the auction and completes a native->bitcoin conversion", async function () {
    const [admin, user, claimant] = await ethers.getSigners();
    const { ipow, bond, conversion, fundAndApprove } = await deployAll(admin);

    const BITCOIN_AMOUNT = 100_000n;
    const NATIVE_AMOUNT = ethers.parseEther("1");
    const REQUIRED_BOND = ethers.parseEther("0.5");
    const COMMIT_FEE = (NATIVE_AMOUNT * COMMIT_FEE_BPS) / 10_000n;

    const userProgram = getBytes("0x76a914" + "33".repeat(20) + "88ac");

    await fundAndApprove(user, COMMIT_FEE);
    await conversion
      .connect(user)
      .commitTokenToBitcoin(NATIVE_AMOUNT, BITCOIN_AMOUNT, userProgram, REQUIRED_BOND, ethers.ZeroAddress, []);

    const txId = 1n;

    // native->bitcoin: selfEscrow is false (c.isNativeToBitcoin == true),
    // so msg.value carries nothing here -- only the stake, now explicit.
    await fundAndApprove(claimant, REQUIRED_BOND);
    await conversion.connect(claimant).proposeClaimConversion(txId, BITCOIN_AMOUNT, 3600n, "0x", REQUIRED_BOND);

    await ethers.provider.send("evm_increaseTime", [61]);
    await ethers.provider.send("evm_mine", []);

    await conversion.connect(claimant).finalizeClaimConversion(txId);

    // depositApprovedConversion is unchanged -- still native for
    // tokenAddr == address(0).
    await conversion.connect(user).depositApprovedConversion(txId, { value: NATIVE_AMOUNT });

    const txRaw = txPaying(userProgram, BITCOIN_AMOUNT + 1_000n);
    const txidLE = doubleSha256(getBytes(txRaw));
    const merkleRootLE = hexlify(txidLE);
    const headerHashLE = keccak256(ethers.toUtf8Bytes("test-header-native-to-bitcoin"));
    await seedHeader(await ipow.getAddress(), 0n, headerHashLE, merkleRootLE);

    const claimantNativeBefore = await ethers.provider.getBalance(await claimant.getAddress());
    const claimantBondBefore = await bond.balanceOf(await claimant.getAddress());
    const escrowNativeBefore = await ethers.provider.getBalance(await conversion.getAddress());
    const escrowBondBefore = await bond.balanceOf(await conversion.getAddress());

    await conversion
      .connect(claimant)
      .submitBitcoinMerkleProofWithTx(txId, txRaw, 0n, 0n, [], 0n);

    const conv = await conversion.conversions(txId);
    expect(conv.proofVerified).to.equal(true);

    // Native drop is just the user's deposited value, released to claimant.
    const escrowNativeAfter = await ethers.provider.getBalance(await conversion.getAddress());
    expect(escrowNativeBefore - escrowNativeAfter).to.equal(NATIVE_AMOUNT);
    // PathUSD drop is fee + stake, both to claimant.
    const escrowBondAfter = await bond.balanceOf(await conversion.getAddress());
    expect(escrowBondBefore - escrowBondAfter).to.equal(COMMIT_FEE + REQUIRED_BOND);

    const claimantNativeAfter = await ethers.provider.getBalance(await claimant.getAddress());
    expect(claimantNativeAfter).to.be.greaterThan(claimantNativeBefore + NATIVE_AMOUNT - ethers.parseEther("0.01"));
    const claimantBondAfter = await bond.balanceOf(await claimant.getAddress());
    expect(claimantBondAfter - claimantBondBefore).to.equal(COMMIT_FEE + REQUIRED_BOND);
  });

  it("reclaim forfeits stake into bounty (PathUSD) and lets a new claimant finish", async function () {
    const [admin, user, claimantA, claimantB] = await ethers.getSigners();
    const { ipow, bond, conversion, fundAndApprove } = await deployAll(admin);

    const BITCOIN_AMOUNT = 100_000n;
    const NATIVE_AMOUNT = ethers.parseEther("1");
    const REQUIRED_BOND = ethers.parseEther("0.5");
    const COMMIT_FEE = (NATIVE_AMOUNT * COMMIT_FEE_BPS) / 10_000n;

    const userProgram = getBytes("0x76a914" + "11".repeat(20) + "88ac");
    await fundAndApprove(user, COMMIT_FEE);
    await conversion
      .connect(user)
      .commitBitcoinToToken(BITCOIN_AMOUNT, NATIVE_AMOUNT, userProgram, REQUIRED_BOND, ethers.ZeroAddress);

    const txId = 1n;

    const claimantAProgram = getBytes("0x76a914" + "aa".repeat(20) + "88ac");
    await fundAndApprove(claimantA, REQUIRED_BOND);
    await conversion
      .connect(claimantA)
      .proposeClaimConversion(txId, NATIVE_AMOUNT, 3600n, claimantAProgram, REQUIRED_BOND, { value: NATIVE_AMOUNT });

    await ethers.provider.send("evm_increaseTime", [61]);
    await ethers.provider.send("evm_mine", []);
    await conversion.connect(claimantA).finalizeClaimConversion(txId);

    let conv = await conversion.conversions(txId);
    expect(conv.reservedNative).to.equal(NATIVE_AMOUNT);

    await ethers.provider.send("evm_increaseTime", [3601]);
    await ethers.provider.send("evm_mine", []);

    const bondPoolBefore = await bond.balanceOf(await conversion.getAddress());

    await conversion.connect(user).reclaimExpiredConversion(txId);

    conv = await conversion.conversions(txId);
    // Forfeited (not refunded): the PathUSD balance is unchanged (no
    // outbound transfer), the conversion's own bounty absorbed it.
    expect(await bond.balanceOf(await conversion.getAddress())).to.equal(bondPoolBefore);
    expect(conv.bounty).to.equal(REQUIRED_BOND);
    expect(conv.requiredBond).to.equal(REQUIRED_BOND);
    expect(conv.responsibleOperator).to.equal(ethers.ZeroAddress);
    expect(conv.operatorDutyExpiresAt).to.equal(0n);
    expect(conv.reservedNative).to.equal(NATIVE_AMOUNT);

    // windowStarted is already true, so this claim doesn't re-touch the
    // native escrow -- only the (PathUSD) stake.
    const claimantBProgram = getBytes("0x76a914" + "bb".repeat(20) + "88ac");
    await fundAndApprove(claimantB, REQUIRED_BOND);
    await conversion
      .connect(claimantB)
      .proposeClaimConversion(txId, NATIVE_AMOUNT, 3600n, claimantBProgram, REQUIRED_BOND);

    await ethers.provider.send("evm_increaseTime", [61]);
    await ethers.provider.send("evm_mine", []);
    await conversion.connect(claimantB).finalizeClaimConversion(txId);

    conv = await conversion.conversions(txId);
    expect(conv.reservedNative).to.equal(NATIVE_AMOUNT);

    const txRaw = txPaying(claimantBProgram, BITCOIN_AMOUNT + 1_000n);
    const txidLE = doubleSha256(getBytes(txRaw));
    const merkleRootLE = hexlify(txidLE);
    const headerHashLE = keccak256(ethers.toUtf8Bytes("test-header-reclaim"));
    await seedHeader(await ipow.getAddress(), 0n, headerHashLE, merkleRootLE);

    const userNativeBefore = await ethers.provider.getBalance(await user.getAddress());
    const claimantBBondBefore = await bond.balanceOf(await claimantB.getAddress());
    // Capture bounty *before* the proof call -- submitBitcoinMerkleProofWithTx
    // zeroes c.bounty out as part of paying it, so reading conversions(txId)
    // afterward would always see 0 here.
    const expectedBounty = conv.bounty;

    await conversion.connect(user).submitBitcoinMerkleProofWithTx(txId, txRaw, 0n, 0n, [], 0n);

    conv = await conversion.conversions(txId);
    expect(conv.proofVerified).to.equal(true);

    const userNativeAfter = await ethers.provider.getBalance(await user.getAddress());
    expect(userNativeAfter).to.be.greaterThan(userNativeBefore + NATIVE_AMOUNT - ethers.parseEther("0.01"));

    // claimantB earns their own stake back plus the bounty claimantA
    // forfeited, plus the commit fee released on successful proof -- all
    // PathUSD now.
    const claimantBBondAfter = await bond.balanceOf(await claimantB.getAddress());
    expect(claimantBBondAfter - claimantBBondBefore).to.equal(COMMIT_FEE + REQUIRED_BOND + expectedBounty);
  });

  it("an ERC20 bitcoin->native conversion pays out the real token, fee/stake still PathUSD", async function () {
    const [admin, user, claimant] = await ethers.getSigners();
    const { ipow, bond, conversion, fundAndApprove } = await deployAll(admin);

    const token = await new MockERC20__factory(admin).deploy("Mock", "MOCK");
    await token.waitForDeployment();
    const tokenAddr = await token.getAddress();

    const BITCOIN_AMOUNT = 100_000n;
    const TOKEN_AMOUNT = ethers.parseUnits("5", 18);
    const REQUIRED_BOND = ethers.parseEther("0.5");
    const COMMIT_FEE = (TOKEN_AMOUNT * COMMIT_FEE_BPS) / 10_000n;

    await token.mint(await claimant.getAddress(), TOKEN_AMOUNT);
    await token.connect(claimant).approve(await conversion.getAddress(), TOKEN_AMOUNT);

    const claimantProgram = getBytes("0x76a914" + "cc".repeat(20) + "88ac");

    await fundAndApprove(user, COMMIT_FEE);
    await conversion
      .connect(user)
      .commitBitcoinToToken(BITCOIN_AMOUNT, TOKEN_AMOUNT, getBytes("0x76a914" + "11".repeat(20) + "88ac"), REQUIRED_BOND, tokenAddr);

    const txId = 1n;

    // tokenAddr != address(0), so selfEscrow of the conversion's own value
    // goes through _pullToken (the real MOCK token) -- the stake is a
    // separate, explicit PathUSD param, no msg.value at all here.
    await fundAndApprove(claimant, REQUIRED_BOND);
    await conversion
      .connect(claimant)
      .proposeClaimConversion(txId, TOKEN_AMOUNT, 3600n, claimantProgram, REQUIRED_BOND);

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

    expect(await token.balanceOf(await user.getAddress())).to.equal(TOKEN_AMOUNT);
    expect(await token.balanceOf(await conversion.getAddress())).to.equal(0n);
    // Fee + stake both released to the claimant in PathUSD.
    expect(await bond.balanceOf(await conversion.getAddress())).to.equal(0n);
    expect(await bond.balanceOf(await claimant.getAddress())).to.equal(COMMIT_FEE + REQUIRED_BOND);
  });

  it("openBundleTunnel: single-token happy path pays out via submitBitcoinMerkleProofWithTx", async function () {
    const [admin, opener, dest] = await ethers.getSigners();
    const { ipow, conversion } = await deployAll(admin);

    const NETWORK_ID = 7n;
    await conversion.connect(admin).addNetwork(NETWORK_ID, 1, 32);

    const BITCOIN_AMOUNT = 100_000n;
    const NATIVE_AMOUNT = ethers.parseEther("1");
    const openerProgram = getBytes("0x76a914" + "99".repeat(20) + "88ac");
    const networkAddress = getBytes("0x" + "11".repeat(20));

    // openBundleTunnel is entirely unchanged -- no commit fee, and its own
    // value movement already follows tokenAddr correctly.
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

    await conversion.connect(dest).submitBitcoinMerkleProofWithTx(txId, txRaw, 0n, 0n, [], 0n);

    const convAfter = await conversion.conversions(txId);
    expect(convAfter.proofVerified).to.equal(true);

    const escrowBalanceAfter = await ethers.provider.getBalance(await conversion.getAddress());
    expect(escrowBalanceBefore - escrowBalanceAfter).to.equal(NATIVE_AMOUNT);
  });

  it("openBundleTunnel: bundle pays out primary and extra token", async function () {
    const [admin, opener, dest] = await ethers.getSigners();
    const { ipow, conversion } = await deployAll(admin);

    const NETWORK_ID = 7n;
    await conversion.connect(admin).addNetwork(NETWORK_ID, 1, 32);

    const token = await new MockERC20__factory(admin).deploy("Mock", "MOCK");
    await token.waitForDeployment();
    const tokenAddr = await token.getAddress();

    const BITCOIN_AMOUNT = 100_000n;
    const NATIVE_AMOUNT = ethers.parseEther("1");
    const EXTRA_AMOUNT = ethers.parseUnits("2", 18);

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

    await conversion.connect(dest).submitBitcoinMerkleProofWithTx(txId, txRaw, 0n, 0n, [], 0n);

    const conv = await conversion.conversions(txId);
    expect(conv.proofVerified).to.equal(true);
    expect(await token.balanceOf(await dest.getAddress())).to.equal(EXTRA_AMOUNT);
    expect(await token.balanceOf(await conversion.getAddress())).to.equal(0n);
  });

  it("openBundleTunnel: guaranteed resolution force-claims all tokens", async function () {
    const [admin, opener, dest] = await ethers.getSigners();
    const { conversion } = await deployAll(admin);

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

    await ethers.provider.send("evm_increaseTime", [DUTY_WINDOW_SECONDS + 1]);
    await ethers.provider.send("evm_mine", []);

    const escrowBalanceBefore = await ethers.provider.getBalance(await conversion.getAddress());

    await conversion.connect(admin).claimNativeOperatorExpired(txId);

    const escrowBalanceAfter = await ethers.provider.getBalance(await conversion.getAddress());
    expect(escrowBalanceBefore - escrowBalanceAfter).to.equal(NATIVE_AMOUNT);

    expect(await token.balanceOf(await dest.getAddress())).to.equal(EXTRA_AMOUNT);
    expect(await token.balanceOf(await conversion.getAddress())).to.equal(0n);
  });

  it("openBundleTunnel: rejects more than three extra tokens", async function () {
    const [admin, opener, dest] = await ethers.getSigners();
    const { conversion } = await deployAll(admin);

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
    const { conversion } = await deployAll(admin);

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

describe("iPoWConversionPathUSD: commit fee / stake are PathUSD, not native", function () {
  it("commitTokenToBitcoin and commitBitcoinToToken are not payable and pull the exact fee via transferFrom", async function () {
    const [admin, user] = await ethers.getSigners();
    const { bond, conversion, fundAndApprove } = await deployAll(admin);

    const NATIVE_AMOUNT = ethers.parseEther("1");
    const COMMIT_FEE = (NATIVE_AMOUNT * COMMIT_FEE_BPS) / 10_000n;
    const userProgram = getBytes("0x76a914" + "11".repeat(20) + "88ac");

    await fundAndApprove(user, COMMIT_FEE);
    const before = await bond.balanceOf(await conversion.getAddress());
    await conversion
      .connect(user)
      .commitTokenToBitcoin(NATIVE_AMOUNT, 100_000n, userProgram, ethers.parseEther("0.5"), ethers.ZeroAddress, []);
    const after = await bond.balanceOf(await conversion.getAddress());
    expect(after - before).to.equal(COMMIT_FEE);

    const conv = await conversion.conversions(1n);
    expect(conv.commitFee).to.equal(COMMIT_FEE);
  });

  it("proposeClaimConversion requires an explicit stakeAmount >= requiredBond, pulled as PathUSD", async function () {
    const [admin, user, claimant] = await ethers.getSigners();
    const { bond, conversion, fundAndApprove } = await deployAll(admin);

    const NATIVE_AMOUNT = ethers.parseEther("1");
    const REQUIRED_BOND = ethers.parseEther("0.5");
    const COMMIT_FEE = (NATIVE_AMOUNT * COMMIT_FEE_BPS) / 10_000n;
    const userProgram = getBytes("0x76a914" + "11".repeat(20) + "88ac");

    await fundAndApprove(user, COMMIT_FEE);
    await conversion
      .connect(user)
      .commitBitcoinToToken(100_000n, NATIVE_AMOUNT, userProgram, REQUIRED_BOND, ethers.ZeroAddress);

    const claimantProgram = getBytes("0x76a914" + "22".repeat(20) + "88ac");
    await fundAndApprove(claimant, REQUIRED_BOND - 1n);
    await expect(
      conversion
        .connect(claimant)
        .proposeClaimConversion(1n, NATIVE_AMOUNT, 3600n, claimantProgram, REQUIRED_BOND - 1n, { value: NATIVE_AMOUNT }),
    ).to.be.revertedWithCustomError(conversion, "BelowRequiredBond");
  });
});
