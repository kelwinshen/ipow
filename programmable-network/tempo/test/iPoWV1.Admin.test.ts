import { expect } from "chai";
import { network } from "hardhat";

import {
  BPS_DENOM,
  COMMIT_FEE_BPS,
  NATIVE_DECIMALS,
  SELF_NETWORK_ID,
  deployIPoWV1,
} from "./helpers/deploy.ts";

const { ethers } = await network.create();

describe("iPoWV1: constructor", function () {
  it("sets immutables, operator, and commit fee from constructor args", async function () {
    const [operator] = await ethers.getSigners();
    const c = await deployIPoWV1(operator);
    expect(await c.NATIVE_DECIMALS()).to.equal(NATIVE_DECIMALS);
    expect(await c.SELF_NETWORK_ID()).to.equal(SELF_NETWORK_ID);
    expect(await c.operator()).to.equal(operator.address);
    expect(await c.commitFeeBps()).to.equal(COMMIT_FEE_BPS);
    expect(await c.nextTxId()).to.equal(1n);
  });

  it("reverts InvalidConstructor when selfNetworkId is zero", async function () {
    const [operator] = await ethers.getSigners();
    const factory = await ethers.getContractFactory("iPoWV1");
    await expect(
      factory.deploy(NATIVE_DECIMALS, 0n, operator.address, COMMIT_FEE_BPS)
    ).to.be.revertedWithCustomError(factory, "InvalidConstructor");
  });

  it("reverts InvalidConstructor when operator is the zero address", async function () {
    const factory = await ethers.getContractFactory("iPoWV1");
    await expect(
      factory.deploy(
        NATIVE_DECIMALS,
        SELF_NETWORK_ID,
        ethers.ZeroAddress,
        COMMIT_FEE_BPS
      )
    ).to.be.revertedWithCustomError(factory, "InvalidConstructor");
  });

  it("reverts InvalidConstructor when commitFeeBps exceeds BPS_DENOM", async function () {
    const [operator] = await ethers.getSigners();
    const factory = await ethers.getContractFactory("iPoWV1");
    await expect(
      factory.deploy(
        NATIVE_DECIMALS,
        SELF_NETWORK_ID,
        operator.address,
        BPS_DENOM + 1n
      )
    ).to.be.revertedWithCustomError(factory, "InvalidConstructor");
  });

  it("allows commitFeeBps exactly equal to BPS_DENOM (100%)", async function () {
    const [operator] = await ethers.getSigners();
    const factory = await ethers.getContractFactory("iPoWV1");
    const c = await factory.deploy(
      NATIVE_DECIMALS,
      SELF_NETWORK_ID,
      operator.address,
      BPS_DENOM
    );
    expect(await c.commitFeeBps()).to.equal(BPS_DENOM);
  });
});

describe("iPoWV1: setOperator", function () {
  it("transfers the operator role and emits OperatorChanged", async function () {
    const [operator, newOperator] = await ethers.getSigners();
    const c = await deployIPoWV1(operator);

    await expect(c.connect(operator).setOperator(newOperator.address))
      .to.emit(c, "OperatorChanged")
      .withArgs(newOperator.address);

    expect(await c.operator()).to.equal(newOperator.address);
  });

  it("reverts Unauthorized when called by a non-operator", async function () {
    const [operator, , stranger] = await ethers.getSigners();
    const c = await deployIPoWV1(operator);

    await expect(
      c.connect(stranger).setOperator(stranger.address)
    ).to.be.revertedWithCustomError(c, "Unauthorized");
  });
});

describe("iPoWV1: setFees", function () {
  it("updates commitFeeBps and emits FeesUpdated", async function () {
    const [operator] = await ethers.getSigners();
    const c = await deployIPoWV1(operator);

    await expect(c.connect(operator).setFees(123n))
      .to.emit(c, "FeesUpdated")
      .withArgs(123n);
    expect(await c.commitFeeBps()).to.equal(123n);
  });

  it("reverts Unauthorized when called by a non-operator", async function () {
    const [operator, stranger] = await ethers.getSigners();
    const c = await deployIPoWV1(operator);

    await expect(
      c.connect(stranger).setFees(100n)
    ).to.be.revertedWithCustomError(c, "Unauthorized");
  });

  it("reverts InvalidFeeConfig when the new fee exceeds BPS_DENOM", async function () {
    const [operator] = await ethers.getSigners();
    const c = await deployIPoWV1(operator);

    await expect(
      c.connect(operator).setFees(BPS_DENOM + 1n)
    ).to.be.revertedWithCustomError(c, "InvalidFeeConfig");
  });
});

describe("iPoWV1: addNetwork / removeNetwork", function () {
  const NETWORK_ID = 999n;

  it("registers a new network with the given address-length bounds", async function () {
    const [operator] = await ethers.getSigners();
    const c = await deployIPoWV1(operator);

    await c.connect(operator).addNetwork(NETWORK_ID, 20, 32);
    const cfg = await c.networkConfigs(NETWORK_ID);
    expect(cfg.enabled).to.equal(true);
    expect(cfg.minAddrLen).to.equal(20);
    expect(cfg.maxAddrLen).to.equal(32);
  });

  it("reverts Unauthorized when called by a non-operator", async function () {
    const [operator, stranger] = await ethers.getSigners();
    const c = await deployIPoWV1(operator);

    await expect(
      c.connect(stranger).addNetwork(NETWORK_ID, 20, 32)
    ).to.be.revertedWithCustomError(c, "Unauthorized");
  });

  it("reverts InvalidNetworkConfig for networkId == 0", async function () {
    const [operator] = await ethers.getSigners();
    const c = await deployIPoWV1(operator);

    await expect(
      c.connect(operator).addNetwork(0n, 20, 32)
    ).to.be.revertedWithCustomError(c, "InvalidNetworkConfig");
  });

  it("reverts InvalidNetworkConfig for networkId == SELF_NETWORK_ID", async function () {
    const [operator] = await ethers.getSigners();
    const c = await deployIPoWV1(operator);

    await expect(
      c.connect(operator).addNetwork(SELF_NETWORK_ID, 20, 32)
    ).to.be.revertedWithCustomError(c, "InvalidNetworkConfig");
  });

  it("reverts InvalidNetworkConfig when minAddrLen > maxAddrLen", async function () {
    const [operator] = await ethers.getSigners();
    const c = await deployIPoWV1(operator);

    await expect(
      c.connect(operator).addNetwork(NETWORK_ID, 40, 32)
    ).to.be.revertedWithCustomError(c, "InvalidNetworkConfig");
  });

  it("reverts InvalidNetworkConfig when the network is already enabled", async function () {
    const [operator] = await ethers.getSigners();
    const c = await deployIPoWV1(operator);

    await c.connect(operator).addNetwork(NETWORK_ID, 20, 32);
    await expect(
      c.connect(operator).addNetwork(NETWORK_ID, 20, 32)
    ).to.be.revertedWithCustomError(c, "InvalidNetworkConfig");
  });

  it("removes a previously enabled network", async function () {
    const [operator] = await ethers.getSigners();
    const c = await deployIPoWV1(operator);

    await c.connect(operator).addNetwork(NETWORK_ID, 20, 32);
    await c.connect(operator).removeNetwork(NETWORK_ID);

    const cfg = await c.networkConfigs(NETWORK_ID);
    expect(cfg.enabled).to.equal(false);
  });

  it("reverts InvalidNetworkConfig when removing a network that isn't enabled", async function () {
    const [operator] = await ethers.getSigners();
    const c = await deployIPoWV1(operator);

    await expect(
      c.connect(operator).removeNetwork(NETWORK_ID)
    ).to.be.revertedWithCustomError(c, "InvalidNetworkConfig");
  });

  it("reverts Unauthorized on removeNetwork when called by a non-operator", async function () {
    const [operator, stranger] = await ethers.getSigners();
    const c = await deployIPoWV1(operator);
    await c.connect(operator).addNetwork(NETWORK_ID, 20, 32);

    await expect(
      c.connect(stranger).removeNetwork(NETWORK_ID)
    ).to.be.revertedWithCustomError(c, "Unauthorized");
  });
});
