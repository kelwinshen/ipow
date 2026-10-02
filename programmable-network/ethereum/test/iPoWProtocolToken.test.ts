import { expect } from "chai";
import { network } from "hardhat";

import { tokenNetwork } from "./helpers/tokenNetwork.ts";

const { ethers } = await network.create();

// The protocol's token build (D136, D137): for a network without a native
// coin, such as Tempo, whose fees are paid in PathUSD (6 decimals) and whose
// gas price is in attodollars (10^-18 USD) per gas. The logic is the same
// source as the native build, tested in iPoWProtocol*.test.ts; this file
// tests what differs: how money moves, and the unit of the price.

const USD = 10n ** 6n; // one PathUSD
const SCALE = 10n ** 12n; // attodollars per microdollar
const TEMPO_BASE_FEE = 2n * 10n ** 10n; // attodollars per gas (TIP-1010)
const WORK_PER_BLOCK = 110_000n;
const WORK_FIXED = 520_000n;
const GAS = 1_000_000n;

async function setup() {
  const [, application, user, operator] = await ethers.getSigners();
  const lightClient = await ethers.deployContract("iPoWLightClient", [0]);
  const coin = await ethers.deployContract("MockToken", ["PathUSD", "pathUSD", 6]);
  const protocol = await ethers.deployContract("iPoWProtocolToken", [await lightClient.getAddress(), await coin.getAddress(), SCALE]);
  for (const who of [application, operator]) {
    await coin.mint(who.address, 1_000n * USD);
    await coin.connect(who).approve(await protocol.getAddress(), ethers.MaxUint256);
  }
  await protocol.connect(application).registerApplication([]);
  return { protocol, coin, application, user, operator };
}

async function setPrice(price: bigint) {
  await ethers.provider.send("hardhat_setNextBlockBaseFeePerGas", ["0x" + price.toString(16)]);
}

/** amount of work x price x 1.5, in attodollars: divided by 10^12 for PathUSD. */
function feeFor(price: bigint, confirmations: number) {
  const work = WORK_PER_BLOCK * BigInt(24 + confirmations) + WORK_FIXED;
  return (work * price * 3n) / 2n / SCALE;
}

describe("iPoWProtocolToken", function () {
  it("locks a bond in the token, pulled with the operator's approval, and pays it back in the token", async function () {
    const { protocol, coin, operator } = await setup();
    const p = await protocol.getAddress();
    await protocol.connect(operator).lockBond(10n * USD);
    expect(await coin.balanceOf(p)).to.equal(10n * USD);
    expect((await protocol.bondOf(operator.address))[0]).to.equal(10n * USD);
    // A network without a native coin takes none.
    await expect(protocol.connect(operator).lockBond(USD, { value: 1n })).to.be.revertedWithCustomError(protocol, "WrongValue");
    await expect(protocol.connect(operator).lockBond(0)).to.be.revertedWithCustomError(protocol, "ZeroAmount");
    const before = await coin.balanceOf(operator.address);
    await protocol.connect(operator).withdrawBond(4n * USD);
    expect(await coin.balanceOf(operator.address)).to.equal(before + 4n * USD);
    expect([await protocol.coin(), await protocol.priceScale()]).to.deep.equal([await coin.getAddress(), SCALE]);
  });

  it("prices the commitment fee in the token: work x price x 1.5, the price in attodollars", async function () {
    const { protocol } = await setup();
    await setPrice(TEMPO_BASE_FEE);
    await ethers.provider.send("evm_mine", []);
    // 3,820,000 gas for 6 confirmations at 2x10^10 attodollars, x 1.5: 0.1146
    // PathUSD.
    expect(feeFor(TEMPO_BASE_FEE, 6)).to.equal(114_600n);
    // In the coin, as the native build gives it in wei: an application asks
    // it the same way on every network (V14).
    expect(await protocol.commitmentFeeAt(6, TEMPO_BASE_FEE)).to.equal(feeFor(TEMPO_BASE_FEE, 6));
  });

  it("takes exactly the amount a job names as paid, keeping what is above the fees for the operator (D79, D138)", async function () {
    const { protocol, coin, application, user } = await setup();
    const p = await protocol.getAddress();
    const escrow = USD;
    const escrowFee = (escrow * 50n) / 10_000n;
    const fee = feeFor(TEMPO_BASE_FEE, 6);
    // Less than the fees: refused.
    await setPrice(TEMPO_BASE_FEE);
    await expect(
      protocol.connect(application).openJob(ethers.id("a"), escrow, 50, 6, 0, user.address, fee + escrowFee - 1n, { gasLimit: GAS })
    ).to.be.revertedWithCustomError(protocol, "FeesNotPaid");
    // Native value is refused.
    await setPrice(TEMPO_BASE_FEE);
    await expect(
      protocol.connect(application).openJob(ethers.id("a"), escrow, 50, 6, 0, user.address, fee + escrowFee, { value: 1n, gasLimit: GAS })
    ).to.be.revertedWithCustomError(protocol, "WrongValue");
    // The fees and 0.01 more: all taken, the extra part of the commitment fee.
    const extra = USD / 100n;
    const before = await coin.balanceOf(application.address);
    await setPrice(TEMPO_BASE_FEE);
    await protocol.connect(application).openJob(ethers.id("a"), escrow, 50, 6, 0, user.address, fee + escrowFee + extra, { gasLimit: GAS });
    expect(before - (await coin.balanceOf(application.address))).to.equal(fee + escrowFee + extra);
    expect(await coin.balanceOf(p)).to.equal(fee + escrowFee + extra);
    const job = await protocol.getJob(await protocol.jobCount());
    expect([job.commitmentFee, job.escrowFee]).to.deep.equal([fee + extra, escrowFee]);
  });

  it("returns an expired job's fees to the payer in the coin", async function () {
    const { protocol, coin, application, user } = await setup();
    const fees = feeFor(TEMPO_BASE_FEE, 6) + USD / 100n;
    await setPrice(TEMPO_BASE_FEE);
    await protocol.connect(application).openJob(ethers.id("a"), USD, 0, 6, 0, user.address, fees, { gasLimit: GAS });
    await ethers.provider.send("evm_increaseTime", [16 * 60]);
    await protocol.expire(1n);
    expect(await protocol.credit(user.address)).to.equal(fees);
    const before = await coin.balanceOf(user.address);
    await protocol.connect(user).withdrawCredit();
    expect((await coin.balanceOf(user.address)) - before).to.equal(fees);
    expect(await coin.balanceOf(await protocol.getAddress())).to.equal(0n);
  });

  it("slashes in the coin: the escrow to the application and the guardian, the fees back to the payer", async function () {
    const { protocol, coin, application, user, guardian, operator, atTempoPrice, slash } = await tokenNetwork(ethers);
    await protocol.connect(application).registerApplication([]);
    const fees = feeFor(TEMPO_BASE_FEE, 6) + 5_000n;
    await atTempoPrice();
    await protocol.connect(application).openJob(ethers.id("s"), USD, 0, 6, 0, user.address, fees, { gasLimit: GAS });
    const bondBefore = (await protocol.bondOf(operator.address))[0];
    const escrow = await slash(1n);
    const toApplication = (escrow * 8000n) / 10_000n;
    expect(await protocol.credit(application.address)).to.equal(toApplication);
    expect(await protocol.credit(guardian.address)).to.equal(escrow - toApplication);
    expect(await protocol.credit(user.address)).to.equal(fees);
    expect((await protocol.bondOf(operator.address))[0]).to.equal(bondBefore - escrow);
    for (const [who, amount] of [[application, toApplication], [guardian, escrow - toApplication], [user, fees]] as const) {
      const before = await coin.balanceOf(who.address);
      await protocol.connect(who).withdrawCredit();
      expect((await coin.balanceOf(who.address)) - before).to.equal(amount);
    }
  });
});
