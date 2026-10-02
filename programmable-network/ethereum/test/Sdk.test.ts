import { expect } from "chai";
import { network } from "hardhat";

import { deployNetwork } from "../deploy/deploy.ts";
import { burn, displayTxid, encodeRecipient, getJob, getLock, homeAssets, lock, openCheckpoint, quoteCheckpoint, quoteLock } from "../../../packages/sdk/src/index.ts";

const { ethers } = await network.create();

// The SDK (packages/sdk) against a deployment on a local chain, as on
// Sepolia: quote a checkpoint, open it, and follow its stages.

async function deployed() {
  const [signer] = await ethers.getSigners();
  const d = await deployNetwork(signer, { network: "ethereum", env: "testnet", minHeight: 0, maxSats: 1n, pairs: [{ peer: 2, peerVault: "0x" + "ab".repeat(32) }], local: {} });
  // The SDK's shape of a deployment.
  return { name: "local", network: "ethereum", number: 1, chainId: 31337, coin: { kind: "native", decimals: 18, price: "baseFee" }, workScale: null, dataFee: "none", ...d, source: "local" } as any;
}

describe("The SDK", function () {
  it("quotes a checkpoint with its margin stated, opens it, and names its stages", async function () {
    const d = await deployed();
    const [user, , operator] = await ethers.getSigners();
    const q = await quoteCheckpoint(ethers.provider, d);
    expect(q.escrow).to.equal(10n ** 16n); // the vault's least certifying escrow on a test network
    expect(q.margin).to.equal(q.commitmentFee); // 100% by default
    expect(q.pay).to.equal(q.commitmentFee + q.margin + q.escrowFee);
    expect(q.value).to.equal(q.pay);

    const { jobId } = await openCheckpoint(user, d, q);
    let job = await getJob(ethers.provider, d, jobId);
    expect([job.stage, job.operator, job.escrow]).to.deep.equal(["Auction", null, q.escrow]);
    expect(job.waitingFor).to.match(/^an operator's bid, until /);
    expect(job.commitmentFee).to.equal(q.commitmentFee + q.margin); // D79: the margin goes to the operator

    const protocol = await ethers.getContractAt("iPoWProtocolNative", d.protocol);
    await protocol.connect(operator).lockBond(10n ** 17n, { value: 10n ** 17n });
    await protocol.connect(operator).bid(jobId, await protocol.minimumBidOf(jobId));
    job = await getJob(ethers.provider, d, jobId);
    // Bidding stays open a minute after the last bid (D37).
    expect([job.stage, job.operator]).to.deep.equal(["Auction", operator.address]);
    expect(job.waitingFor).to.match(/an operator has bid$/);
    await ethers.provider.send("evm_increaseTime", [61]);
    await ethers.provider.send("evm_mine", []);
    job = await getJob(ethers.provider, d, jobId);
    expect([job.stage, job.operator]).to.deep.equal(["Assigned", operator.address]);
    expect(job.waitingFor).to.match(/^the operator to anchor the job/);
    expect(job.times.lastBid).to.be.a("number");
    expect(job.times.deadline).to.be.greaterThan(job.times.opened);
  });

  it("shows a stored txid as explorers do", async function () {
    expect(displayTxid("0x1201426b6f3cd7ffdf9b0a2fa906bf87273a15b0615133e41e9d22d749404dd2")).to.equal(
      "d24d4049d7229d1ee4335161b0153a2787bf06a92f0a9bdfffd73c6f6b420112"
    );
  });

  it("locks ETH for its receipt on Solana, in record units, and follows the lock", async function () {
    const d = await deployed();
    const [user] = await ethers.getSigners();
    const GWEI = 10n ** 9n;
    // A Solana address, decoded as the vault takes it.
    const solana = "5Ks1r25VGX5S7dUMf8c5oP6qzXfFvckQ67wgtqywnDAN";
    expect(encodeRecipient(2, solana)).to.equal("0x4043c1013b9ac5a7b0d6e41df72999295abc02c0b7e824f17a79154e9ab76bb7");
    const q = await quoteLock(ethers.provider, d, { amount: 10n ** 16n, fee: 1000n * GWEI, fastFee: 0n });
    expect([q.amount, q.fee, q.asset.unit, q.total, q.value]).to.deep.equal([10n ** 7n, 1000n, GWEI, 10n ** 16n + 1000n * GWEI, 10n ** 16n + 1000n * GWEI]);
    // Not a whole number of record units: refused before anything is sent.
    let error = "";
    await quoteLock(ethers.provider, d, { amount: 10n ** 16n + 1n }).catch((e) => (error = e.message));
    expect(error).to.match(/whole number of record units/);

    const before = await ethers.provider.getBalance(d.vaults[0].home);
    const { lockId } = await lock(user, d, q, solana);
    expect((await ethers.provider.getBalance(d.vaults[0].home)) - before).to.equal(q.total);
    const l = await getLock(ethers.provider, d, lockId);
    expect([l.stage, l.owner, l.amount, l.fee, l.recipient]).to.deep.equal(["Locked", user.address, q.amount, q.fee, encodeRecipient(2, solana)]);
  });

  it("locks a token with its approval, and burns only a receipt that exists", async function () {
    const d = await deployed();
    const [user] = await ethers.getSigners();
    const token = await ethers.deployContract("MockToken", ["USD", "USD", 6]);
    const home = await ethers.getContractAt("VaultHome", d.vaults[0].home);
    await home.registerAsset(await token.getAddress());
    await token.mint(user.address, 10n ** 9n);
    const assets = await homeAssets(ethers.provider, d);
    expect(assets.map((a) => [a.number, a.decimals, a.unit])).to.deep.equal([[0, 18, 10n ** 9n], [1, 6, 1n]]);
    const q = await quoteLock(ethers.provider, d, { asset: 1, amount: 5_000_000n, fee: 1_000n });
    expect([q.value, q.total]).to.deep.equal([0n, 5_001_000n]);
    const { lockId } = await lock(user, d, q, "5Ks1r25VGX5S7dUMf8c5oP6qzXfFvckQ67wgtqywnDAN");
    expect((await getLock(ethers.provider, d, lockId)).asset).to.equal(1);
    expect(await token.balanceOf(user.address)).to.equal(10n ** 9n - 5_001_000n);
    // No receipt of Solana's SOL exists here yet: its burn is refused.
    const receipts = await ethers.getContractAt("VaultReceipts", d.vaults[0].receipts);
    await expect(burn(user, d, { asset: 0, to: "5Ks1r25VGX5S7dUMf8c5oP6qzXfFvckQ67wgtqywnDAN", amount: 1n })).to.be.revertedWithCustomError(receipts, "UnknownAsset");
  });
});

