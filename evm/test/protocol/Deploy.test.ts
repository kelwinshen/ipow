import { expect } from "chai";
import { network } from "hardhat";

import { codeMatches, deployNetwork, refusal, settingsError } from "../../deploy/deploy.ts";
import { NETWORKS } from "../../deploy/networks.ts";

const { ethers } = await network.create();

// Each network is a deployment of the same contracts, differing only in its
// settings (D134). Deployed here on a local chain, standing in for each
// kind of network: a native coin, a rollup that reads its price of data,
// and a token coin.

const SOLANA = 2;
const SOLANA_VAULT = "0x" + "ab".repeat(32);

describe("Deployment from a network's settings (D134)", function () {
  it("deploys Ethereum: the native build, no data fee, and a vault paired with Solana", async function () {
    const [signer] = await ethers.getSigners();
    const d = await deployNetwork(signer, { network: "ethereum", env: "testnet", minHeight: 0, maxSats: 10_000_000n, pairs: [{ peer: SOLANA, peerVault: SOLANA_VAULT }], local: {} });
    expect(d.dataFee).to.equal(null);
    const protocol = await ethers.getContractAt("iPoWProtocolNative", d.protocol);
    expect(await protocol.dataFee()).to.equal(ethers.ZeroAddress);
    const vault = await ethers.getContractAt("iPoWVaultNative", d.vaults[0].vault);
    expect([await vault.here(), await vault.peer()]).to.deep.equal([1n, 2n]);
    expect(await vault.deposit()).to.equal(NETWORKS.ethereum.vault.testnet!.deposit);
    expect(await vault.homeFactory()).to.equal(d.homeFactory);
  });

  it("deploys Base with the reader of its price of data, and Robinhood with Arbitrum's", async function () {
    const [signer] = await ethers.getSigners();
    // The rollups' oracles must answer for the protocol to accept the reader.
    for (const [name, at] of [["MockGasPriceOracle", "0x420000000000000000000000000000000000000F"], ["MockArbGasInfo", "0x000000000000000000000000000000000000006C"]]) {
      const mock = await ethers.deployContract(name);
      await ethers.provider.send("hardhat_setCode", [at, await ethers.provider.getCode(await mock.getAddress())]);
    }
    const base = await deployNetwork(signer, { network: "base", env: "testnet", minHeight: 0, maxSats: 1n, pairs: [], local: {} });
    expect(await (await ethers.getContractAt("OpDataFee", base.dataFee!)).ORACLE()).to.equal("0x420000000000000000000000000000000000000F");
    expect(await (await ethers.getContractAt("iPoWProtocolNative", base.protocol)).dataFee()).to.equal(base.dataFee);
    const rh = await deployNetwork(signer, { network: "robinhood", env: "testnet", minHeight: 0, maxSats: 1n, pairs: [], local: {} });
    expect(await (await ethers.getContractAt("ArbDataFee", rh.dataFee!)).GAS_INFO()).to.equal("0x000000000000000000000000000000000000006C");
  });

  it("deploys Tempo: the token builds, PathUSD as the coin and asset 0", async function () {
    const [signer] = await ethers.getSigners();
    const coin = await ethers.deployContract("MockToken", ["PathUSD", "pathUSD", 6]);
    const d = await deployNetwork(signer, { network: "tempo", env: "testnet", minHeight: 0, maxSats: 1n, pairs: [{ peer: SOLANA, peerVault: SOLANA_VAULT }], local: { coin: await coin.getAddress() } });
    const protocol = await ethers.getContractAt("iPoWProtocolToken", d.protocol);
    expect([await protocol.coin(), await protocol.priceScale()]).to.deep.equal([await coin.getAddress(), 10n ** 12n]);
    expect(await (await ethers.getContractAt("Conversion", d.conversion)).coin()).to.equal(await coin.getAddress());
    const home = await ethers.getContractAt("VaultHome", d.vaults[0].home);
    expect((await home.getAsset(0)).token).to.equal(await coin.getAddress());
  });

  it("deploys Hedera: the price from the transaction's gas price, and HBAR in 8 decimals (D139)", async function () {
    const [signer] = await ethers.getSigners();
    const d = await deployNetwork(signer, { network: "hedera", env: "testnet", minHeight: 0, maxSats: 1n, pairs: [{ peer: SOLANA, peerVault: SOLANA_VAULT }], local: {} });
    const protocol = await ethers.getContractAt("iPoWProtocolGasPrice", d.protocol);
    expect(await protocol.dataFee()).to.equal(ethers.ZeroAddress);
    // The fee follows the price the transaction pays, not the base fee.
    const fee = await protocol.commitmentFeeFor.staticCall(6, { gasPrice: 80n });
    expect(fee).to.equal(((110_000n * 30n + 520_000n) * 80n * 3n) / 2n);
    const a = await (await ethers.getContractAt("VaultHome", d.vaults[0].home)).getAsset(0);
    expect([a.token, a.decimals, a.recordDecimals, a.unit]).to.deep.equal([ethers.ZeroAddress, 8n, 8n, 1n]);
  });

  it("deploys Polkadot: the price of work scaled by its measured gas, 1/8 of Ethereum's (D140)", async function () {
    const [signer] = await ethers.getSigners();
    const d = await deployNetwork(signer, { network: "polkadot", env: "testnet", minHeight: 0, maxSats: 1n, pairs: [], local: {} });
    const protocol = await ethers.getContractAt("iPoWProtocolPolkadot", d.protocol);
    expect(await protocol.WORK_SCALE()).to.equal(8n);
    // A job opened at a base fee pays the work at it, divided by 8 last, so
    // a base fee that is not a multiple of 8 loses nothing to rounding.
    const [, application, user] = await ethers.getSigners();
    await protocol.connect(application).registerApplication([]);
    const price = 10n ** 12n + 7n;
    const fee = ((110_000n * 30n + 520_000n) * price * 3n) / 2n / 8n;
    expect(fee).to.be.gt(((110_000n * 30n + 520_000n) * (price / 8n) * 3n) / 2n);
    await ethers.provider.send("hardhat_setNextBlockBaseFeePerGas", ["0x" + price.toString(16)]);
    await protocol.connect(application).openJob(ethers.id("p"), 5n * fee, 0, 6, 0, user.address, fee, { value: fee, gasLimit: 1_000_000n });
    expect((await protocol.getJob(1n)).commitmentFee).to.equal(fee);
    // Asked with the base fee as it is, the fee is the same.
    expect(await protocol.commitmentFeeAt(6, price)).to.equal(fee);
    // One unit less is refused: the scale is not rounded in the payer's favour.
    await ethers.provider.send("hardhat_setNextBlockBaseFeePerGas", ["0x" + price.toString(16)]);
    await expect(
      protocol.connect(application).openJob(ethers.id("q"), 5n * fee, 0, 6, 0, user.address, fee - 1n, { value: fee - 1n, gasLimit: 1_000_000n })
    ).to.be.revertedWithCustomError(protocol, "FeesNotPaid");
  });

  it("locks HBAR on Hedera in its 8 decimals: a record unit is a tinybar", async function () {
    const [signer, user] = await ethers.getSigners();
    const d = await deployNetwork(signer, { network: "hedera", env: "testnet", minHeight: 0, maxSats: 1n, pairs: [{ peer: SOLANA, peerVault: SOLANA_VAULT }], local: {} });
    const home = await ethers.getContractAt("VaultHome", d.vaults[0].home);
    const HBAR = 10n ** 8n;
    // 2 HBAR and a fee of 1,000 tinybars: exactly that much is sent.
    await home.connect(user).lock(0, ethers.zeroPadValue(user.address, 32), 2n * HBAR, 1_000n, 0n, { value: 2n * HBAR + 1_000n });
    const l = await home.getLock(1);
    expect([l.amount, l.fee]).to.deep.equal([2n * HBAR, 1_000n]);
    await expect(home.connect(user).lock(0, ethers.zeroPadValue(user.address, 32), HBAR, 0n, 0n, { value: HBAR + 1n })).to.be.revertedWithCustomError(home, "WrongValue");
  });

  it("refuses settings that no build serves, and a native coin's decimals outside 1 to 18", async function () {
    const native = (price: "baseFee" | "gasPrice") => ({ kind: "native" as const, decimals: 18, price });
    const base = { number: 9, chainId: { testnet: 1, mainnet: 1 }, rpcEnv: { testnet: "", mainnet: "" }, vault: { testnet: null, mainnet: null } };
    expect(settingsError({ ...base, coin: native("gasPrice"), dataFee: "op" })).to.match(/read no price of data/);
    expect(settingsError({ ...base, coin: native("baseFee"), dataFee: "arb", scaledBuild: { name: "iPoWProtocolPolkadot", workScale: 8n } })).to.match(/read no price of data/);
    expect(settingsError({ ...base, coin: native("gasPrice"), dataFee: "none", scaledBuild: { name: "iPoWProtocolPolkadot", workScale: 8n } })).to.match(/only for a native coin priced by the base fee/);
    for (const n of Object.values(NETWORKS)) expect(settingsError(n)).to.equal(null);
    const factory = await ethers.getContractFactory("VaultHomeFactory");
    for (const d of [0, 19]) await expect(factory.deploy(d)).to.be.revertedWithCustomError(factory, "BadDecimals");
  });

  it("refuses a blocked network, unset amounts, an unset chain, and the wrong chain", async function () {
    expect(refusal("base", "mainnet")).to.match(/amounts are not set/);
    expect(refusal("robinhood", "mainnet")).to.match(/amounts are not set/);
    expect(refusal("ethereum", "mainnet")).to.equal(null);
    expect(refusal("nowhere", "testnet")).to.match(/unknown/);
    const [signer] = await ethers.getSigners();
    // The local chain is not Sepolia.
    let error = "";
    try {
      await deployNetwork(signer, { network: "ethereum", env: "testnet", minHeight: 0, maxSats: 1n, pairs: [] });
    } catch (e: any) {
      error = e.message;
    }
    expect(error).to.match(/connected to chain 31337, expected 11155111/);
    // Tempo's transactions need a sender ethers is not.
    expect(refusal("tempo", "testnet")).to.match(/Tempo sender/);
    expect(refusal("tempo", "testnet", true)).to.equal(null);
  });

  it("refuses a peer vault that is not an address when the peer is an EVM network", async function () {
    const [signer] = await ethers.getSigners();
    let error = "";
    try {
      await deployNetwork(signer, { network: "base", env: "testnet", minHeight: 0, maxSats: 1n, pairs: [{ peer: 1, peerVault: SOLANA_VAULT }], local: {} });
    } catch (e: any) {
      error = e.message;
    }
    expect(error).to.match(/must be an address in 32 bytes/);
    error = "";
    try {
      await deployNetwork(signer, { network: "base", env: "testnet", minHeight: 0, maxSats: 1n, pairs: [{ peer: 1, peerVault: "0x" + "00".repeat(32) }], local: {} });
    } catch (e: any) {
      error = e.message;
    }
    expect(error).to.match(/must be an address in 32 bytes/);
  });

  it("compares code with the artifact's apart from its immutables", async function () {
    const art = "0x6000" + "00".repeat(4) + "6001";
    const refs = { "1": [{ start: 2, length: 4 }] };
    expect(codeMatches("0x6000" + "deadbeef" + "6001", art, refs)).to.equal(true);
    // A byte outside the immutable differs.
    expect(codeMatches("0x6000" + "deadbeef" + "6002", art, refs)).to.equal(false);
    // Another contract's code, of another length.
    expect(codeMatches("0x6000", art, refs)).to.equal(false);
    expect(codeMatches("0x6000" + "00".repeat(4) + "6001", art, {})).to.equal(true);
  });
});
