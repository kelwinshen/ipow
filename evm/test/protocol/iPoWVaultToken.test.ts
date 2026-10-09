import { expect } from "chai";
import { network } from "hardhat";

import { tokenNetwork } from "../helpers/tokenNetwork.ts";
import { makeParts } from "../helpers/vaultParts.ts";

const { ethers } = await network.create();

// The vault's token build (D136, D137), as Tempo's (8) vault paired with
// Ethereum (1): its coin is PathUSD, asset 0. Its logic is the native
// build's, tested in iPoWVault.test.ts; this file tests what differs.

const USD = 10n ** 6n;
const SCALE = 10n ** 12n;
const TEMPO = 8;
const ETHEREUM = 1;
const DEPOSIT = USD / 10n;
const GAS = 3_000_000n;

/** A token-build vault for the pair Tempo-Ethereum, its parts made first. */
async function tokenVault(protocol: any, coin: string, minEscrow: bigint) {
  const hf = await ethers.deployContract("VaultHomeFactory", [18]);
  const rf = await ethers.deployContract("VaultReceiptsFactory");
  const parts = await makeParts(ethers, hf, rf, TEMPO, ETHEREUM, coin);
  const args = [await protocol.getAddress(), TEMPO, ETHEREUM, ethers.zeroPadValue("0x11", 32), DEPOSIT, minEscrow, await hf.getAddress(), await rf.getAddress(), parts.home, parts.receipts, coin];
  return { args, parts };
}

async function setup() {
  const [, user, operator] = await ethers.getSigners();
  const lightClient = await ethers.deployContract("iPoWLightClient", [0]);
  const coin = await ethers.deployContract("MockToken", ["PathUSD", "pathUSD", 6]);
  const protocol = await ethers.deployContract("iPoWProtocolToken", [await lightClient.getAddress(), await coin.getAddress(), SCALE]);
  const { args } = await tokenVault(protocol, await coin.getAddress(), 10n * USD);
  const vault = await ethers.deployContract("iPoWVaultToken", args);
  const home = await ethers.getContractAt("VaultHome", await vault.home());
  for (const who of [user, operator]) {
    await coin.mint(who.address, 1_000n * USD);
    await coin.connect(who).approve(await vault.getAddress(), ethers.MaxUint256);
    await coin.connect(who).approve(await home.getAddress(), ethers.MaxUint256);
  }
  return { coin, protocol, vault, home, user, operator };
}

describe("iPoWVaultToken", function () {
  it("makes the network's coin asset 0, in its own decimals", async function () {
    const { coin, vault, home } = await setup();
    const a = await home.getAsset(0);
    expect([a.token, a.decimals, a.recordDecimals, a.unit]).to.deep.equal([await coin.getAddress(), 6n, 6n, 1n]);
    expect(await vault.coin()).to.equal(await coin.getAddress());
    expect([await vault.here(), await vault.peer()]).to.deep.equal([BigInt(TEMPO), BigInt(ETHEREUM)]);
  });

  it("takes a checkpoint's fees in the coin and pays them to the protocol, and refuses native value", async function () {
    const { coin, protocol, vault, user } = await setup();
    const fees = USD;
    await ethers.provider.send("hardhat_setNextBlockBaseFeePerGas", ["0x" + (2n * 10n ** 10n).toString(16)]);
    await vault.connect(user).openCheckpoint(6, fees, { gasLimit: GAS });
    expect(await coin.balanceOf(await protocol.getAddress())).to.equal(fees);
    expect(await coin.balanceOf(await vault.getAddress())).to.equal(0n);
    await expect(vault.connect(user).openCheckpoint(6, fees, { value: 1n, gasLimit: GAS })).to.be.revertedWithCustomError(vault, "WrongValue");
  });

  it("locks the coin, asset 0, in record units of the token", async function () {
    const { coin, home, user } = await setup();
    const recipient = ethers.zeroPadValue(user.address, 32);
    await home.connect(user).lock(0, recipient, 5n * USD, 1_000n, 0n);
    expect(await coin.balanceOf(await home.getAddress())).to.equal(5n * USD + 1_000n);
    const l = await home.getLock(1);
    expect([l.asset, l.amount, l.fee]).to.deep.equal([0n, 5n * USD, 1_000n]);
    // A lock of the coin sends no native value.
    await expect(home.connect(user).lock(0, recipient, USD, 0n, 0n, { value: 1n })).to.be.revertedWithCustomError(home, "WrongValue");
  });

  it("refuses a coin other than its protocol's, and the coin as a second asset", async function () {
    const { coin, protocol, home } = await setup();
    const other = await ethers.deployContract("MockToken", ["Other", "OTH", 6]);
    const { args } = await tokenVault(protocol, await other.getAddress(), 10n * USD);
    const factory = await ethers.getContractFactory("iPoWVaultToken");
    await expect(factory.deploy(...args)).to.be.revertedWithCustomError(factory, "WrongCoin");
    // Its parts made with the native coin as asset 0, the vault naming its
    // protocol's coin: the home does not hold the coin.
    const native = await tokenVault(protocol, ethers.ZeroAddress, 10n * USD);
    const nativeArgs = [...native.args.slice(0, -1), await coin.getAddress()];
    await expect(factory.deploy(...nativeArgs)).to.be.revertedWithCustomError(factory, "BadNetworks");
    await expect(home.registerAsset(await coin.getAddress())).to.be.revertedWithCustomError(home, "AssetExists");
  });

  it("takes the share of a slashed checkpoint job into the backing of the coin, in its own units", async function () {
    const { coin, protocol, stranger, atTempoPrice, slash } = await tokenNetwork(ethers);
    const { args } = await tokenVault(protocol, await coin.getAddress(), USD);
    const vault = await ethers.deployContract("iPoWVaultToken", args);
    const home = await ethers.getContractAt("VaultHome", await vault.home());
    await coin.connect(stranger).approve(await vault.getAddress(), ethers.MaxUint256);
    await atTempoPrice();
    await vault.connect(stranger).openCheckpoint(6, 200_000n, { gasLimit: GAS });
    const escrow = await slash(await protocol.jobCount());
    const share = (escrow * 8000n) / 10_000n;
    await vault.collectProtocolCredit();
    expect(await home.reserve(0)).to.equal(share);
    expect(await coin.balanceOf(await home.getAddress())).to.equal(share);
    expect(await coin.balanceOf(await vault.getAddress())).to.equal(0n);
  });
});
