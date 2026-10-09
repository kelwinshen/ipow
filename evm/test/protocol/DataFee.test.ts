import { expect } from "chai";
import { network } from "hardhat";

const { ethers } = await network.create();

// D135: on a rollup the commitment fee adds the cost of posting the work's
// data to the parent network, read from the rollup's own price. The
// rollup's oracle is stood in for by a mock whose code is placed at the
// oracle's real address.

const OP_ORACLE = "0x420000000000000000000000000000000000000F";
const ARB_GAS_INFO = "0x000000000000000000000000000000000000006C";
const GWEI = 10n ** 9n;
const GAS = 1_000_000n;

async function placeAt(name: string, address: string) {
  const mock = await ethers.deployContract(name);
  await ethers.provider.send("hardhat_setCode", [address, await ethers.provider.getCode(await mock.getAddress())]);
  return ethers.getContractAt(name, address);
}

async function protocolWith(dataFeeName: string | null) {
  const lightClient = await ethers.deployContract("iPoWLightClient", [0]);
  const dataFee = dataFeeName ? await ethers.deployContract(dataFeeName) : null;
  const protocol = await ethers.deployContract("iPoWProtocolNative", [
    await lightClient.getAddress(),
    dataFee ? await dataFee.getAddress() : ethers.ZeroAddress,
  ]);
  const [, application, user] = await ethers.getSigners();
  await protocol.connect(application).registerApplication([]);
  return { protocol, dataFee, application, user };
}

/** (work gas x price + work bytes x data price) x 1.5 */
function feeFor(price: bigint, perByte: bigint, confirmations: number) {
  const window = BigInt(24 + confirmations);
  const gas = 110_000n * window + 520_000n;
  const bytes = 350n * window + 1_800n;
  return ((gas * price + bytes * perByte) * 3n) / 2n;
}

describe("A rollup's data fee (D135)", function () {
  it("adds nothing on a network without a data price", async function () {
    const { protocol } = await protocolWith(null);
    expect(await protocol.dataFee()).to.equal(ethers.ZeroAddress);
    expect(await protocol.commitmentFeeAt(6, GWEI)).to.equal(feeFor(GWEI, 0n, 6));
  });

  it("adds the data fee of an OP Stack rollup, read from its GasPriceOracle", async function () {
    const oracle: any = await placeAt("MockGasPriceOracle", OP_ORACLE);
    const { protocol, dataFee } = await protocolWith("OpDataFee");
    await oracle.setPerByte(30n * GWEI);
    expect(await dataFee!.dataFee(1_000n)).to.equal(30_000n * GWEI);
    expect(await protocol.commitmentFeeAt(6, GWEI / 100n)).to.equal(feeFor(GWEI / 100n, 30n * GWEI, 6));
    // It follows the oracle: nobody sets it.
    await oracle.setPerByte(60n * GWEI);
    expect(await protocol.commitmentFeeAt(6, GWEI / 100n)).to.equal(feeFor(GWEI / 100n, 60n * GWEI, 6));
  });

  it("adds the data fee of an Arbitrum chain, read from ArbGasInfo", async function () {
    const info: any = await placeAt("MockArbGasInfo", ARB_GAS_INFO);
    const { protocol } = await protocolWith("ArbDataFee");
    await info.setPerByte(7n * GWEI);
    expect(await protocol.commitmentFeeAt(12, GWEI / 10n)).to.equal(feeFor(GWEI / 10n, 7n * GWEI, 12));
  });

  it("asks a job's fees with the data fee, and refuses less", async function () {
    const oracle: any = await placeAt("MockGasPriceOracle", OP_ORACLE);
    const { protocol, application, user } = await protocolWith("OpDataFee");
    await oracle.setPerByte(30n * GWEI);
    const price = GWEI / 100n;
    const fee = feeFor(price, 30n * GWEI, 6);
    const escrow = 10n * fee;
    await ethers.provider.send("hardhat_setNextBlockBaseFeePerGas", ["0x" + price.toString(16)]);
    await expect(
      protocol.connect(application).openJob(ethers.id("a"), escrow, 0, 6, 0, user.address, fee - 1n, { value: fee - 1n, gasLimit: GAS })
    ).to.be.revertedWithCustomError(protocol, "FeesNotPaid");
    // The minimum escrow is 5 times the fee with its data part (D56).
    await ethers.provider.send("hardhat_setNextBlockBaseFeePerGas", ["0x" + price.toString(16)]);
    await expect(
      protocol.connect(application).openJob(ethers.id("a"), 5n * fee - 1n, 0, 6, 0, user.address, fee, { value: fee, gasLimit: GAS })
    ).to.be.revertedWithCustomError(protocol, "EscrowTooLow");
    await ethers.provider.send("hardhat_setNextBlockBaseFeePerGas", ["0x" + price.toString(16)]);
    await protocol.connect(application).openJob(ethers.id("a"), escrow, 0, 6, 0, user.address, fee, { value: fee, gasLimit: GAS });
    expect((await protocol.getJob(1n)).commitmentFee).to.equal(fee);
  });

  it("refuses at deployment a data fee reader that cannot read", async function () {
    const lightClient = await ethers.deployContract("iPoWLightClient", [0]);
    // No oracle at the predeploy's address on this chain.
    await ethers.provider.send("hardhat_setCode", [OP_ORACLE, "0x"]);
    const reader = await ethers.deployContract("OpDataFee");
    const factory = await ethers.getContractFactory("iPoWProtocolNative");
    await expect(factory.deploy(await lightClient.getAddress(), await reader.getAddress())).to.be.revert(ethers);
  });

  it("counts no data fee when the oracle stops reading, so jobs still open", async function () {
    const oracle: any = await placeAt("MockGasPriceOracle", OP_ORACLE);
    const { protocol, application, user } = await protocolWith("OpDataFee");
    const price = GWEI / 100n;
    // The oracle reverts.
    await oracle.setPerByte(ethers.MaxUint256);
    expect(await protocol.commitmentFeeAt(6, price)).to.equal(feeFor(price, 0n, 6));
    // The oracle is gone.
    await ethers.provider.send("hardhat_setCode", [OP_ORACLE, "0x"]);
    expect(await protocol.commitmentFeeAt(6, price)).to.equal(feeFor(price, 0n, 6));
    const fee = feeFor(price, 0n, 6);
    await ethers.provider.send("hardhat_setNextBlockBaseFeePerGas", ["0x" + price.toString(16)]);
    await protocol.connect(application).openJob(ethers.id("a"), 10n * fee, 0, 6, 0, user.address, fee, { value: fee, gasLimit: GAS });
    expect((await protocol.getJob(1n)).commitmentFee).to.equal(fee);
  });

  it("counts no data fee for an answer that is not one word", async function () {
    const { protocol } = await protocolWith("LongDataFee");
    expect(await protocol.commitmentFeeAt(6, GWEI)).to.equal(feeFor(GWEI, 0n, 6));
  });

  it("gives the reader a fixed gas, so one that burns it all still lets jobs open", async function () {
    const { protocol, dataFee, application, user } = await protocolWith("BurningDataFee");
    await (dataFee as any).burn();
    const price = GWEI / 100n;
    const fee = feeFor(price, 0n, 6);
    await ethers.provider.send("hardhat_setNextBlockBaseFeePerGas", ["0x" + price.toString(16)]);
    await protocol.connect(application).openJob(ethers.id("a"), 10n * fee, 0, 6, 0, user.address, fee, { value: fee, gasLimit: 1_000_000n });
    expect((await protocol.getJob(1n)).commitmentFee).to.equal(fee);
  });

  it("refuses a call without gas enough to give the reader all of its gas", async function () {
    const oracle: any = await placeAt("MockGasPriceOracle", OP_ORACLE);
    const { protocol, application, user } = await protocolWith("OpDataFee");
    await oracle.setPerByte(30n * GWEI);
    await expect(protocol.commitmentFeeAt.estimateGas(6, GWEI, { gasLimit: 120_000n })).to.be.revertedWithCustomError(
      protocol,
      "DataFeeGas"
    );
    const price = GWEI / 100n;
    const fee = feeFor(price, 30n * GWEI, 6);
    await ethers.provider.send("hardhat_setNextBlockBaseFeePerGas", ["0x" + price.toString(16)]);
    await expect(
      protocol.connect(application).openJob(ethers.id("a"), 10n * fee, 0, 6, 0, user.address, fee, { value: fee, gasLimit: 120_000n })
    ).to.be.revertedWithCustomError(protocol, "DataFeeGas");
  });

  it("refuses at deployment a reader that cannot read within its gas", async function () {
    const lightClient = await ethers.deployContract("iPoWLightClient", [0]);
    const reader = await ethers.deployContract("BurningDataFee");
    await reader.burn();
    const factory = await ethers.getContractFactory("iPoWProtocolNative");
    await expect(factory.deploy(await lightClient.getAddress(), await reader.getAddress())).to.be.revert(ethers);
  });
});
