import { expect } from "chai";
import { network } from "hardhat";

import { tokenNetwork } from "./helpers/tokenNetwork.ts";

const { ethers } = await network.create();

// Conversion on a network whose coin is a token (D136), such as Tempo: the
// protocol is its token build (D137), and a swap's job fees are named and
// taken in the coin (D138). Swaps themselves are tested in
// Conversion.test.ts.

const USD = 10n ** 6n;
const SCALE = 10n ** 12n;
const TEMPO_BASE_FEE = 2n * 10n ** 10n;
const GAS = 2_000_000n;
const SCRIPT = "0x0014" + "11".repeat(20);

async function setup() {
  const [, user] = await ethers.getSigners();
  const lightClient = await ethers.deployContract("iPoWLightClient", [0]);
  const coin = await ethers.deployContract("MockToken", ["PathUSD", "pathUSD", 6]);
  const protocol = await ethers.deployContract("iPoWProtocolToken", [await lightClient.getAddress(), await coin.getAddress(), SCALE]);
  const conversion = await ethers.deployContract("Conversion", [await protocol.getAddress(), 10_000_000n, await coin.getAddress()]);
  await coin.mint(user.address, 1_000n * USD);
  await coin.connect(user).approve(await conversion.getAddress(), ethers.MaxUint256);
  return { coin, protocol, conversion, user };
}

async function atTempoPrice() {
  await ethers.provider.send("hardhat_setNextBlockBaseFeePerGas", ["0x" + TEMPO_BASE_FEE.toString(16)]);
}

describe("Conversion on a token network", function () {
  it("takes the coin sold and the job's fees in the coin, and the protocol takes the fees from it", async function () {
    const { coin, protocol, conversion, user } = await setup();
    const fees = 200_000n; // 0.2 PathUSD: above the fees at Tempo's price
    const before = await coin.balanceOf(user.address);
    await atTempoPrice();
    await conversion.connect(user).sell(await coin.getAddress(), 10n * USD, 5_000_000n, SCRIPT, 6, fees, { gasLimit: GAS });
    expect(before - (await coin.balanceOf(user.address))).to.equal(10n * USD + fees);
    // The amount stays with Conversion; the fees went to the protocol.
    expect(await coin.balanceOf(await conversion.getAddress())).to.equal(10n * USD);
    expect(await coin.balanceOf(await protocol.getAddress())).to.equal(fees);
    const job = await protocol.getJob(await protocol.jobCount());
    expect(job.commitmentFee + job.escrowFee).to.equal(fees);
  });

  it("refuses the native coin and native value, and takes a buy's fees in the coin", async function () {
    const { coin, protocol, conversion, user } = await setup();
    await atTempoPrice();
    await expect(
      conversion.connect(user).sell(ethers.ZeroAddress, USD, 5_000_000n, SCRIPT, 6, 200_000n, { value: USD, gasLimit: GAS })
    ).to.be.revertedWithCustomError(conversion, "InvalidAmount");
    await atTempoPrice();
    await expect(conversion.connect(user).buy(ethers.ZeroAddress, USD, 5_000_000n, "0x", "0x", 6, 200_000n, { gasLimit: GAS })).to.be.revertedWithCustomError(
      conversion,
      "InvalidAmount"
    );
    await atTempoPrice();
    await expect(
      conversion.connect(user).buy(await coin.getAddress(), USD, 5_000_000n, "0x", "0x", 6, 200_000n, { value: 1n, gasLimit: GAS })
    ).to.be.revertedWithCustomError(conversion, "InvalidAmount");
    await atTempoPrice();
    await conversion.connect(user).buy(await coin.getAddress(), USD, 5_000_000n, "0x", "0x", 6, 200_000n, { gasLimit: GAS });
    expect(await coin.balanceOf(await protocol.getAddress())).to.equal(200_000n);
  });

  it("refuses a coin other than its protocol's", async function () {
    const { protocol } = await setup();
    const other = await ethers.deployContract("MockToken", ["Other", "OTH", 6]);
    const factory = await ethers.getContractFactory("Conversion");
    await expect(factory.deploy(await protocol.getAddress(), 10_000_000n, await other.getAddress())).to.be.revertedWithCustomError(
      factory,
      "WrongCoin"
    );
  });

  it("refunds a sell whose job was slashed and passes on the escrow share in the coin", async function () {
    const net = await tokenNetwork(ethers);
    const { coin, protocol, user, atTempoPrice, slash } = net;
    const conversion = await ethers.deployContract("Conversion", [await protocol.getAddress(), 10_000_000n, await coin.getAddress()]);
    await coin.connect(user).approve(await conversion.getAddress(), ethers.MaxUint256);
    await atTempoPrice();
    await conversion.connect(user).sell(await coin.getAddress(), 10n * USD, 5_000_000n, SCRIPT, 6, 200_000n, { gasLimit: GAS });
    const escrow = await slash((await conversion.getSwap(1n)).jobId);
    const before = await coin.balanceOf(user.address);
    await conversion.refundSell(1n, "0x");
    // The amount sold, the application's share of the escrow, and the
    // fees: the user paid them, so the protocol credits the user.
    expect((await coin.balanceOf(user.address)) - before).to.equal(10n * USD + (escrow * 8000n) / 10_000n);
    expect(await protocol.credit(user.address)).to.equal(200_000n);
    expect(await coin.balanceOf(await conversion.getAddress())).to.equal(0n);
  });
});
