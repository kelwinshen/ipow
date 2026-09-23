import { expect } from "chai";
import { network } from "hardhat";

import { BPS_DENOM, COMMIT_FEE_BPS, deployIPoWV1 } from "./helpers/deploy.ts";

const { ethers } = await network.create();

const REMOTE_NETWORK_ID = 777n;

function feeFor(nativeAmount: bigint) {
  return (nativeAmount * COMMIT_FEE_BPS) / BPS_DENOM;
}

const btcProgram = "0x76a914" + "11".repeat(20) + "88ac"; // arbitrary 25-byte P2PKH-shaped script

describe("iPoWV1: commitNativeToBitcoin validation", function () {
  const nativeAmount = ethers.parseEther("1");
  const bitcoinAmount = 100_000n;

  it("reverts IncorrectCommitFee when msg.value doesn't match the quoted fee", async function () {
    const [operator, user] = await ethers.getSigners();
    const c = await deployIPoWV1(operator);

    await expect(
      c
        .connect(user)
        .commitNativeToBitcoin(
          nativeAmount,
          bitcoinAmount,
          0n,
          "0x",
          btcProgram,
          {
            value: feeFor(nativeAmount) + 1n,
          }
        )
    ).to.be.revertedWithCustomError(c, "IncorrectCommitFee");
  });

  it("reverts ZeroValue when nativeAmount is zero", async function () {
    const [operator, user] = await ethers.getSigners();
    const c = await deployIPoWV1(operator);

    await expect(
      c
        .connect(user)
        .commitNativeToBitcoin(0n, bitcoinAmount, 0n, "0x", btcProgram, {
          value: 0n,
        })
    ).to.be.revertedWithCustomError(c, "ZeroValue");
  });

  it("reverts ZeroValue when bitcoinAmount is zero", async function () {
    const [operator, user] = await ethers.getSigners();
    const c = await deployIPoWV1(operator);

    await expect(
      c
        .connect(user)
        .commitNativeToBitcoin(nativeAmount, 0n, 0n, "0x", btcProgram, {
          value: feeFor(nativeAmount),
        })
    ).to.be.revertedWithCustomError(c, "ZeroValue");
  });

  it("reverts BadBitcoinProgram when userProgram is empty for a direct-Bitcoin conversion", async function () {
    const [operator, user] = await ethers.getSigners();
    const c = await deployIPoWV1(operator);

    await expect(
      c
        .connect(user)
        .commitNativeToBitcoin(nativeAmount, bitcoinAmount, 0n, "0x", "0x", {
          value: feeFor(nativeAmount),
        })
    ).to.be.revertedWithCustomError(c, "BadBitcoinProgram");
  });

  it("reverts BadBitcoinProgram when userProgram exceeds 80 bytes", async function () {
    const [operator, user] = await ethers.getSigners();
    const c = await deployIPoWV1(operator);
    const tooLong = "0x" + "ab".repeat(81);

    await expect(
      c
        .connect(user)
        .commitNativeToBitcoin(nativeAmount, bitcoinAmount, 0n, "0x", tooLong, {
          value: feeFor(nativeAmount),
        })
    ).to.be.revertedWithCustomError(c, "BadBitcoinProgram");
  });

  it("reverts NetworkAddressNotAllowed when a networkAddress is given for direct-Bitcoin (networkId 0)", async function () {
    const [operator, user] = await ethers.getSigners();
    const c = await deployIPoWV1(operator);

    await expect(
      c
        .connect(user)
        .commitNativeToBitcoin(
          nativeAmount,
          bitcoinAmount,
          0n,
          "0x1234",
          btcProgram,
          {
            value: feeFor(nativeAmount),
          }
        )
    ).to.be.revertedWithCustomError(c, "NetworkAddressNotAllowed");
  });

  it("reverts IncorrectNetwork when networkId refers to a network that isn't enabled", async function () {
    const [operator, user] = await ethers.getSigners();
    const c = await deployIPoWV1(operator);

    await expect(
      c
        .connect(user)
        .commitNativeToBitcoin(
          nativeAmount,
          bitcoinAmount,
          REMOTE_NETWORK_ID,
          "0x1234",
          "0x",
          {
            value: feeFor(nativeAmount),
          }
        )
    ).to.be.revertedWithCustomError(c, "IncorrectNetwork");
  });

  it("reverts UserBitcoinProgramNotAllowed when a userProgram is given for a routed (non-zero network) conversion", async function () {
    const [operator, user] = await ethers.getSigners();
    const c = await deployIPoWV1(operator);
    await c.connect(operator).addNetwork(REMOTE_NETWORK_ID, 20, 32);

    await expect(
      c
        .connect(user)
        .commitNativeToBitcoin(
          nativeAmount,
          bitcoinAmount,
          REMOTE_NETWORK_ID,
          "0x" + "11".repeat(20),
          btcProgram,
          {
            value: feeFor(nativeAmount),
          }
        )
    ).to.be.revertedWithCustomError(c, "UserBitcoinProgramNotAllowed");
  });

  it("reverts IncorrectNetworkAddress when the destination address length is out of the network's bounds", async function () {
    const [operator, user] = await ethers.getSigners();
    const c = await deployIPoWV1(operator);
    await c.connect(operator).addNetwork(REMOTE_NETWORK_ID, 20, 32);

    await expect(
      c
        .connect(user)
        .commitNativeToBitcoin(
          nativeAmount,
          bitcoinAmount,
          REMOTE_NETWORK_ID,
          "0x1234",
          "0x",
          {
            value: feeFor(nativeAmount),
          }
        )
    ).to.be.revertedWithCustomError(c, "IncorrectNetworkAddress");
  });

  it("succeeds for a valid direct-Bitcoin commit: stores the conversion, holds the fee, emits ConversionCommitted", async function () {
    const [operator, user] = await ethers.getSigners();
    const c = await deployIPoWV1(operator);
    const fee = feeFor(nativeAmount);

    const txId = await c.nextTxId();
    await expect(
      c
        .connect(user)
        .commitNativeToBitcoin(
          nativeAmount,
          bitcoinAmount,
          0n,
          "0x",
          btcProgram,
          { value: fee }
        )
    )
      .to.emit(c, "ConversionCommitted")
      .withArgs(txId, user.address, true);

    expect(await c.nextTxId()).to.equal(txId + 1n);
    expect(await c.totalHeldCommitFees()).to.equal(fee);

    const stored = await c.conversions(txId);
    expect(stored.user).to.equal(user.address);
    expect(stored.isNativeToBitcoin).to.equal(true);
    expect(stored.nativeAmount).to.equal(nativeAmount);
    expect(stored.bitcoinAmount).to.equal(bitcoinAmount);
    expect(stored.approved).to.equal(false);
  });

  it("succeeds for a valid routed (remote-network) commit", async function () {
    const [operator, user] = await ethers.getSigners();
    const c = await deployIPoWV1(operator);
    await c.connect(operator).addNetwork(REMOTE_NETWORK_ID, 20, 32);
    const fee = feeFor(nativeAmount);
    const remoteAddr = "0x" + "11".repeat(20);

    const txId = await c.nextTxId();
    await expect(
      c
        .connect(user)
        .commitNativeToBitcoin(
          nativeAmount,
          bitcoinAmount,
          REMOTE_NETWORK_ID,
          remoteAddr,
          "0x",
          { value: fee }
        )
    )
      .to.emit(c, "ConversionCommitted")
      .withArgs(txId, user.address, true);

    const stored = await c.conversions(txId);
    expect(stored.networkId).to.equal(REMOTE_NETWORK_ID);
    expect(stored.networkAddress).to.equal(remoteAddr);
  });
});

describe("iPoWV1: commitBitcoinToNative validation (user path)", function () {
  const nativeAmount = ethers.parseEther("1");
  const bitcoinAmount = 100_000n;

  it("reverts ZeroValue when bitcoinAmount is zero", async function () {
    const [operator, user] = await ethers.getSigners();
    const c = await deployIPoWV1(operator);

    await expect(
      c
        .connect(user)
        .commitBitcoinToNative(
          0n,
          nativeAmount,
          0n,
          btcProgram,
          ethers.ZeroAddress,
          "0x",
          0n,
          "0x",
          0n,
          {
            value: feeFor(nativeAmount),
          }
        )
    ).to.be.revertedWithCustomError(c, "ZeroValue");
  });

  it("reverts IncorrectCommitFee when msg.value doesn't match the quoted fee", async function () {
    const [operator, user] = await ethers.getSigners();
    const c = await deployIPoWV1(operator);

    await expect(
      c
        .connect(user)
        .commitBitcoinToNative(
          bitcoinAmount,
          nativeAmount,
          0n,
          btcProgram,
          ethers.ZeroAddress,
          "0x",
          0n,
          "0x",
          0n,
          {
            value: feeFor(nativeAmount) + 1n,
          }
        )
    ).to.be.revertedWithCustomError(c, "IncorrectCommitFee");
  });

  it("reverts NetworkNotAllowed when a non-operator specifies a non-zero networkId", async function () {
    const [operator, user] = await ethers.getSigners();
    const c = await deployIPoWV1(operator);

    await expect(
      c
        .connect(user)
        .commitBitcoinToNative(
          bitcoinAmount,
          nativeAmount,
          REMOTE_NETWORK_ID,
          btcProgram,
          ethers.ZeroAddress,
          "0x",
          0n,
          "0x",
          0n,
          { value: feeFor(nativeAmount) }
        )
    ).to.be.revertedWithCustomError(c, "NetworkNotAllowed");
  });

  it("reverts BadBitcoinProgram when userProgram is empty", async function () {
    const [operator, user] = await ethers.getSigners();
    const c = await deployIPoWV1(operator);

    await expect(
      c
        .connect(user)
        .commitBitcoinToNative(
          bitcoinAmount,
          nativeAmount,
          0n,
          "0x",
          ethers.ZeroAddress,
          "0x",
          0n,
          "0x",
          0n,
          {
            value: feeFor(nativeAmount),
          }
        )
    ).to.be.revertedWithCustomError(c, "BadBitcoinProgram");
  });

  it("succeeds and stores an unapproved conversion, emits ConversionCommitted", async function () {
    const [operator, user] = await ethers.getSigners();
    const c = await deployIPoWV1(operator);
    const fee = feeFor(nativeAmount);

    const txId = await c.nextTxId();
    await expect(
      c
        .connect(user)
        .commitBitcoinToNative(
          bitcoinAmount,
          nativeAmount,
          0n,
          btcProgram,
          ethers.ZeroAddress,
          "0x",
          0n,
          "0x",
          0n,
          {
            value: fee,
          }
        )
    )
      .to.emit(c, "ConversionCommitted")
      .withArgs(txId, user.address, false);

    const stored = await c.conversions(txId);
    expect(stored.isNativeToBitcoin).to.equal(false);
    expect(stored.approved).to.equal(false);
    expect(await c.totalHeldCommitFees()).to.equal(fee);
  });
});

describe("iPoWV1: commitBitcoinToNative validation (operator tunnel path)", function () {
  const nativeAmount = ethers.parseEther("1");
  const bitcoinAmount = 100_000n;
  const paradappProgram = "0x76a914" + "22".repeat(20) + "88ac";

  it("reverts NeedDestAddress when destAddress is the zero address", async function () {
    const [operator] = await ethers.getSigners();
    const c = await deployIPoWV1(operator);
    await c.connect(operator).addNetwork(REMOTE_NETWORK_ID, 20, 32);

    await expect(
      c
        .connect(operator)
        .commitBitcoinToNative(
          bitcoinAmount,
          nativeAmount,
          REMOTE_NETWORK_ID,
          "0x",
          ethers.ZeroAddress,
          "0x" + "11".repeat(20),
          3600n,
          paradappProgram,
          0n
        )
    ).to.be.revertedWithCustomError(c, "NeedDestAddress");
  });

  it("reverts UnexpectedValue when the operator sends native value along with a tunnel commit", async function () {
    const [operator, destUser] = await ethers.getSigners();
    const c = await deployIPoWV1(operator);
    await c.connect(operator).addNetwork(REMOTE_NETWORK_ID, 20, 32);

    await expect(
      c
        .connect(operator)
        .commitBitcoinToNative(
          bitcoinAmount,
          nativeAmount,
          REMOTE_NETWORK_ID,
          "0x",
          destUser.address,
          "0x" + "11".repeat(20),
          3600n,
          paradappProgram,
          0n,
          { value: 1n }
        )
    ).to.be.revertedWithCustomError(c, "UnexpectedValue");
  });

  it("reverts IncorrectNetwork when networkId is zero for the operator tunnel path", async function () {
    const [operator, destUser] = await ethers.getSigners();
    const c = await deployIPoWV1(operator);

    await expect(
      c
        .connect(operator)
        .commitBitcoinToNative(
          bitcoinAmount,
          nativeAmount,
          0n,
          "0x",
          destUser.address,
          "0x",
          3600n,
          paradappProgram,
          0n
        )
    ).to.be.revertedWithCustomError(c, "IncorrectNetwork");
  });

  it("reverts NeedDutyWindow when dutyWindowSeconds is zero", async function () {
    const [operator, destUser] = await ethers.getSigners();
    const c = await deployIPoWV1(operator);
    await c.connect(operator).addNetwork(REMOTE_NETWORK_ID, 20, 32);

    await expect(
      c
        .connect(operator)
        .commitBitcoinToNative(
          bitcoinAmount,
          nativeAmount,
          REMOTE_NETWORK_ID,
          "0x",
          destUser.address,
          "0x" + "11".repeat(20),
          0n,
          paradappProgram,
          0n
        )
    ).to.be.revertedWithCustomError(c, "NeedDutyWindow");
  });

  it("reverts UserBitcoinProgramNotAllowed when a userProgram is supplied on the operator path", async function () {
    const [operator, destUser] = await ethers.getSigners();
    const c = await deployIPoWV1(operator);
    await c.connect(operator).addNetwork(REMOTE_NETWORK_ID, 20, 32);

    await expect(
      c
        .connect(operator)
        .commitBitcoinToNative(
          bitcoinAmount,
          nativeAmount,
          REMOTE_NETWORK_ID,
          btcProgram,
          destUser.address,
          "0x" + "11".repeat(20),
          3600n,
          paradappProgram,
          0n
        )
    ).to.be.revertedWithCustomError(c, "UserBitcoinProgramNotAllowed");
  });

  it("reverts InvalidAnchorHeight when lockedAnchorHeight is beyond the current (zero) global tip", async function () {
    const [operator, destUser] = await ethers.getSigners();
    const c = await deployIPoWV1(operator);
    await c.connect(operator).addNetwork(REMOTE_NETWORK_ID, 20, 32);

    await expect(
      c.connect(operator).commitBitcoinToNative(
        bitcoinAmount,
        nativeAmount,
        REMOTE_NETWORK_ID,
        "0x",
        destUser.address,
        "0x" + "11".repeat(20),
        3600n,
        paradappProgram,
        1n // globalTipHeight is 0 on a fresh contract, so height 1 is out of range
      )
    ).to.be.revertedWithCustomError(c, "InvalidAnchorHeight");
  });
});

describe("iPoWV1: refundIfNotApproved", function () {
  const nativeAmount = ethers.parseEther("1");
  const bitcoinAmount = 100_000n;

  async function commitOne(c: any, user: any) {
    const fee = feeFor(nativeAmount);
    const txId = await c.nextTxId();
    await c
      .connect(user)
      .commitNativeToBitcoin(
        nativeAmount,
        bitcoinAmount,
        0n,
        "0x",
        btcProgram,
        { value: fee }
      );
    return { txId, fee };
  }

  it("refunds the commit fee to the user and marks the conversion refunded", async function () {
    const [operator, user] = await ethers.getSigners();
    const c = await deployIPoWV1(operator);
    const { txId, fee } = await commitOne(c, user);

    await expect(c.connect(user).refundIfNotApproved(txId))
      .to.emit(c, "ConversionRefunded")
      .withArgs(txId, 0n, true);

    const stored = await c.conversions(txId);
    expect(stored.refunded).to.equal(true);
    expect(stored.commitFee).to.equal(fee);
  });

  it("reverts Unauthorized when called by someone other than the conversion's user", async function () {
    const [operator, user, stranger] = await ethers.getSigners();
    const c = await deployIPoWV1(operator);
    const { txId } = await commitOne(c, user);

    await expect(
      c.connect(stranger).refundIfNotApproved(txId)
    ).to.be.revertedWithCustomError(c, "Unauthorized");
  });

  it("reverts BadTxId for a txId that doesn't exist", async function () {
    const [operator, user] = await ethers.getSigners();
    const c = await deployIPoWV1(operator);

    await expect(
      c.connect(user).refundIfNotApproved(1n)
    ).to.be.revertedWithCustomError(c, "BadTxId");
  });

  it("reverts BadState when called twice on the same conversion", async function () {
    const [operator, user] = await ethers.getSigners();
    const c = await deployIPoWV1(operator);
    const { txId } = await commitOne(c, user);

    await c.connect(user).refundIfNotApproved(txId);
    await expect(
      c.connect(user).refundIfNotApproved(txId)
    ).to.be.revertedWithCustomError(c, "BadState");
  });
});
