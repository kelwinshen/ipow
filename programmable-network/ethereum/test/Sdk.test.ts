import { expect } from "chai";
import { network } from "hardhat";

import { deployNetwork } from "../deploy/deploy.ts";
import { swapsOf, creditOf, expireJob, withdrawCredit, buy, cancelBuy, getSwap, quoteSwap, refundSell, sell, burn, burnBeta, collectOwed, createBasket, getBasket, mintBeta, quoteMint, displayTxid, encodeRecipient, getJob, getLock, homeAssets, lock, openCheckpoint, quoteCheckpoint, quoteLock, locksOf, burnsOf, receiptMark, addressToScript, scriptToAddress } from "../../../packages/sdk/src/index.ts";

const { ethers } = await network.create();

// The SDK (packages/sdk) against a deployment on a local chain, as on
// Sepolia: quote a checkpoint, open it, and follow its stages.

async function deployed() {
  const [signer] = await ethers.getSigners();
  const d = await deployNetwork(signer, { network: "ethereum", env: "testnet", minHeight: 0, maxSats: 100_000n, pairs: [{ peer: 2, peerVault: "0x" + "ab".repeat(32) }], local: {} });
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

  it("refuses Bitcoin addresses whose outputs anyone could spend", async function () {
    // Segwit v0 and taproot (v1, 32 bytes) are fine.
    expect(addressToScript("bc1qw508d6qejxtdg4y5r3zarvary0c5xw7kv8f3t4")).to.equal("0x0014751e76e8199196d454941c45d1b3a323f1433bd6");
    expect(addressToScript("bc1p0xlxvlhemja6c4dqv22uapctqupfhlxm9h8z3k2e72q4k9hcz7vqzk5jj0")).to.match(/^0x5120/);
    // Well-formed addresses of a version 2 program and of a 20-byte version
    // 1 program, written from their scripts so their checksums are right.
    const program = "751e76e8199196d454941c45d1b3a323f1433bd6";
    for (const script of ["0x5214" + program, "0x5114" + program]) {
      const address = scriptToAddress(script)!;
      expect(address).to.match(/^bc1/);
      expect(() => addressToScript(address)).to.throw(/not a Bitcoin mainnet address/);
    }
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

    // The owner's locks, newest first and at most the limit; none for
    // another, and no burns yet. A lock from the other network that nothing
    // has touched has no mark here.
    const [, other] = await ethers.getSigners();
    const second = await lock(user, d, await quoteLock(ethers.provider, d, { amount: 10n ** 15n }), solana);
    await lock(other, d, await quoteLock(ethers.provider, d, { amount: 10n ** 15n }), solana);
    expect((await locksOf(ethers.provider, d, user.address)).map((x) => x.lockId)).to.deep.equal([second.lockId, lockId]);
    expect((await locksOf(ethers.provider, d, user.address, 1)).map((x) => x.lockId)).to.deep.equal([second.lockId]);
    expect((await locksOf(ethers.provider, d, other.address)).map((x) => x.owner)).to.deep.equal([other.address]);
    expect(await burnsOf(ethers.provider, d, user.address)).to.deep.equal([]);
    expect(await receiptMark(ethers.provider, d, 1)).to.equal(null);
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

  it("makes a BETA basket of ETH and a token, mints it, and burns it with a part deferred", async function () {
    const d = await deployed();
    const [user] = await ethers.getSigners();
    const usd = await ethers.deployContract("MockToken", ["USD", "USD", 6]);
    await usd.mint(user.address, 10n ** 12n);
    const ONE = 10n ** 9n; // one whole BETA
    // One BETA holds 0.001 ETH and 2 USD; 0.5% to mint and to burn.
    const { key, beta } = await createBasket(user, d, { id: 1, name: "ETH and USD", symbol: "EUSD", uri: "", parts: [{ token: ethers.ZeroAddress, amount: 10n ** 15n }, { token: await usd.getAddress(), amount: 2_000_000n }], mintFeeBps: 50, burnFeeBps: 50 });

    const q = await quoteMint(ethers.provider, d, key, 3n * ONE);
    expect(q.need).to.deep.equal([3n * 10n ** 15n, 6_000_000n]);
    expect(q.fee).to.deep.equal([(3n * 10n ** 15n * 50n) / 10_000n, 30_000n]);
    expect(q.value).to.equal(q.need[0] + q.fee[0]);
    await mintBeta(user, d, q);
    let b = await getBasket(ethers.provider, d, key);
    expect([b.beta, b.supply, b.parts.map((p) => p.held)]).to.deep.equal([beta, 3n * ONE, [3n * 10n ** 15n, 6_000_000n]]);

    // Burn 1 BETA, the token part owed rather than paid now.
    const usdBefore = await usd.balanceOf(user.address);
    await burnBeta(user, d, key, ONE, { defer: [1] });
    expect(await usd.balanceOf(user.address)).to.equal(usdBefore);
    b = await getBasket(ethers.provider, d, key);
    expect(b.supply).to.equal(2n * ONE);
    expect(b.parts[1].owed).to.be.greaterThan(0n);
    await collectOwed(user, d, key, 1);
    // 2 USD, less the 0.5% burn fee taken when collected.
    expect((await usd.balanceOf(user.address)) - usdBefore).to.equal(2_000_000n - 10_000n);
  });

  it("sells ETH for BTC to a Bitcoin address, and refunds it when no operator takes it; cancels a buy the same way", async function () {
    const d = await deployed();
    const [user] = await ethers.getSigners();
    const to = "bc1q7hyq44nph46tjv9m0jzwtgqld029tuem7u5syx";
    const q = await quoteSwap(ethers.provider, d);
    expect(q.fees).to.equal(q.commitmentFee + q.margin + q.escrowFee);
    const amount = 10n ** 16n;
    const before = await ethers.provider.getBalance(user.address);
    const { swapId, jobId } = await sell(user, d, q, { amount, sats: 50_000n, to });
    let s = await getSwap(ethers.provider, d, swapId);
    expect([s.side, s.state, s.amount, s.sats, s.address, s.job.id, s.job.stage]).to.deep.equal(["Sell", "Open", amount, 50_000n, to, jobId, "Auction"]);
    expect(s.waitingFor).to.match(/^an operator to pay at least 50000 sats to bc1q7hyq/);

    // No operator in 15 minutes: the job expires and the coin comes back.
    await ethers.provider.send("evm_increaseTime", [16 * 60]);
    await ethers.provider.send("evm_mine", []);
    s = await getSwap(ethers.provider, d, swapId);
    expect(s.job.stage).to.equal("Expired");
    expect(s.waitingFor).to.match(/found no operator/);
    await refundSell(user, d, swapId);
    expect((await getSwap(ethers.provider, d, swapId)).state).to.equal("Refunded");
    // The fees are not in the refund: they return once the job is expired
    // (D61), then withdrawn.
    // The refund expired the job itself: the fees are in the user's credit,
    // and expiring it again is refused.
    expect(await creditOf(ethers.provider, d, user.address)).to.equal(q.fees);
    await expect(expireJob(user, d, jobId)).to.be.revert(ethers);
    await withdrawCredit(user, d);
    // Back to where the user began, but for gas.
    expect(before - (await ethers.provider.getBalance(user.address))).to.be.lessThan(10n ** 15n);

    const bought = await buy(user, d, await quoteSwap(ethers.provider, d), { amount, sats: 50_000n });
    s = await getSwap(ethers.provider, d, bought.swapId);
    expect([s.side, s.state, s.address]).to.deep.equal(["Buy", "Open", null]);
    expect(s.waitingFor).to.match(/^an operator to lock the coin/);
    await ethers.provider.send("evm_increaseTime", [16 * 60]);
    await ethers.provider.send("evm_mine", []);
    await cancelBuy(user, d, bought.swapId);
    expect((await getSwap(ethers.provider, d, bought.swapId)).state).to.equal("Cancelled");
    // The user's swaps, newest first; another user has none.
    const mine = await swapsOf(ethers.provider, d, user.address);
    expect(mine.map((x) => [x.id, x.side])).to.deep.equal([[bought.swapId, "Buy"], [swapId, "Sell"]]);
    const [, other] = await ethers.getSigners();
    expect(await swapsOf(ethers.provider, d, other.address)).to.deep.equal([]);
  });
});

