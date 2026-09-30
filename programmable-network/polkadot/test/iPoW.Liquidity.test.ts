import { expect } from "chai";
import { network } from "hardhat";

import { deployIPoW } from "./helpers/deploy.ts";

const { ethers } = await network.create();

describe("iPoW: liquidity management", function () {
  it("addNativeLiquidity increases nativeLiquidity and emits LiquidityUpdated", async function () {
    const [operator] = await ethers.getSigners();
    const c = await deployIPoW(operator);

    await expect(
      c.connect(operator).addNativeLiquidity({ value: ethers.parseEther("1") })
    )
      .to.emit(c, "LiquidityUpdated")
      .withArgs(ethers.parseEther("1"));

    expect(await c.nativeLiquidity()).to.equal(ethers.parseEther("1"));
  });

  it("reverts ZeroValue when adding liquidity with msg.value == 0", async function () {
    const [operator] = await ethers.getSigners();
    const c = await deployIPoW(operator);

    await expect(
      c.connect(operator).addNativeLiquidity({ value: 0n })
    ).to.be.revertedWithCustomError(c, "ZeroValue");
  });

  it("reverts Unauthorized when a non-operator tries to add liquidity", async function () {
    const [operator, stranger] = await ethers.getSigners();
    const c = await deployIPoW(operator);

    await expect(
      c.connect(stranger).addNativeLiquidity({ value: ethers.parseEther("1") })
    ).to.be.revertedWithCustomError(c, "Unauthorized");
  });

  it("removableNative reflects the full balance when nothing is locked/reserved/held", async function () {
    const [operator] = await ethers.getSigners();
    const c = await deployIPoW(operator);

    await c
      .connect(operator)
      .addNativeLiquidity({ value: ethers.parseEther("2") });
    expect(await c.removableNative()).to.equal(ethers.parseEther("2"));
  });

  it("removeNativeLiquidity withdraws funds, decreases nativeLiquidity, and pays the operator", async function () {
    const [operator] = await ethers.getSigners();
    const c = await deployIPoW(operator);

    await c
      .connect(operator)
      .addNativeLiquidity({ value: ethers.parseEther("2") });

    const contractAddress = await c.getAddress();
    const balanceBefore = await ethers.provider.getBalance(contractAddress);

    await (
      await c.connect(operator).removeNativeLiquidity(ethers.parseEther("1"))
    ).wait();

    const balanceAfter = await ethers.provider.getBalance(contractAddress);
    expect(balanceBefore - balanceAfter).to.equal(ethers.parseEther("1"));
    expect(await c.nativeLiquidity()).to.equal(ethers.parseEther("1"));
  });

  it("reverts ExceedsRemovable when withdrawing more than what's removable", async function () {
    const [operator] = await ethers.getSigners();
    const c = await deployIPoW(operator);

    await c
      .connect(operator)
      .addNativeLiquidity({ value: ethers.parseEther("1") });

    await expect(
      c.connect(operator).removeNativeLiquidity(ethers.parseEther("2"))
    ).to.be.revertedWithCustomError(c, "ExceedsRemovable");
  });

  it("reverts Unauthorized when a non-operator tries to remove liquidity", async function () {
    const [operator, stranger] = await ethers.getSigners();
    const c = await deployIPoW(operator);

    await c
      .connect(operator)
      .addNativeLiquidity({ value: ethers.parseEther("1") });

    await expect(
      c.connect(stranger).removeNativeLiquidity(ethers.parseEther("1"))
    ).to.be.revertedWithCustomError(c, "Unauthorized");
  });

  it("removableNative is capped by tracked nativeLiquidity even if the contract holds a larger balance", async function () {
    const [operator, other] = await ethers.getSigners();
    const c = await deployIPoW(operator);

    await c
      .connect(operator)
      .addNativeLiquidity({ value: ethers.parseEther("1") });
    // Send extra ETH directly via the receive() fallback — not tracked as nativeLiquidity.
    await other.sendTransaction({
      to: await c.getAddress(),
      value: ethers.parseEther("5"),
    });

    expect(await c.removableNative()).to.equal(ethers.parseEther("1"));
  });
});
