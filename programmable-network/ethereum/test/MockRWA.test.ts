import { expect } from "chai";
import { network } from "hardhat";

const { ethers } = await network.create();

// The test networks' stand-in for a tokenized asset: the deployer mints,
// anyone uses the faucet once per cooldown, and the vault takes it as an
// ordinary token.

describe("MockRWA", function () {
  it("lets only the deployer mint, and anyone take the faucet once per cooldown", async function () {
    const [deployer, user] = await ethers.getSigners();
    const one = 10n ** 18n;
    const aapl = await ethers.deployContract("MockRWA", ["Apple (test)", "AAPL", 5n * one]);
    expect([await aapl.symbol(), await aapl.decimals(), await aapl.minter()]).to.deep.equal(["AAPL", 18n, deployer.address]);

    await aapl.mint(user.address, 100n * one);
    await expect(aapl.connect(user).mint(user.address, 1n)).to.be.revertedWithCustomError(aapl, "NotMinter");

    await aapl.connect(user).faucet();
    expect(await aapl.balanceOf(user.address)).to.equal(105n * one);
    await expect(aapl.connect(user).faucet()).to.be.revertedWithCustomError(aapl, "FaucetCoolingDown");
    await ethers.provider.send("evm_increaseTime", [6 * 3600]);
    await ethers.provider.send("evm_mine", []);
    await aapl.connect(user).faucet();
    expect(await aapl.balanceOf(user.address)).to.equal(110n * one);
  });
});
