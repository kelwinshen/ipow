import { expect } from "chai";
import { network } from "hardhat";

import { deployNetwork } from "../deploy/deploy.ts";
import { displayTxid, getJob, openCheckpoint, quoteCheckpoint } from "../../../packages/sdk/src/index.ts";

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
});
