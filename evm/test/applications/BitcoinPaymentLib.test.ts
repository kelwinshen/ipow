import { expect } from "chai";
import { network } from "hardhat";

import { buildTx } from "../helpers/bitcoin.ts";

const { ethers } = await network.create();

const A = "0x0014" + "aa".repeat(20);
const B = "0x0014" + "bb".repeat(20);
const COIN = { txidLE: "0x" + "11".repeat(32), vout: 3 };

describe("BitcoinPaymentLib", function () {
  async function lib() {
    return ethers.deployContract("BitcoinPaymentLibHarness");
  }
  const tx = buildTx({ inputs: [COIN], outputs: [{ value: 5000n, script: A }, { value: 7000n, script: B }] });

  it("finds an output that pays a script at least an amount, in one output", async function () {
    const l = await lib();
    expect(await l.paysAtLeast(tx, A, 5000n)).to.equal(true);
    expect(await l.paysAtLeast(tx, A, 5001n)).to.equal(false);
    expect(await l.paysAtLeast(tx, B, 7000n)).to.equal(true);
    // Two outputs to one script do not add up.
    const split = buildTx({ inputs: [COIN], outputs: [{ value: 3000n, script: A }, { value: 3000n, script: A }] });
    expect(await l.paysAtLeast(split, A, 5000n)).to.equal(false);
    // An empty script is never a payment.
    expect(await l.paysAtLeast(tx, "0x", 0n)).to.equal(false);
  });

  it("checks one output by its number, and a number past the last is no payment", async function () {
    const l = await lib();
    expect(await l.outputPays(tx, 1, B, 7000n)).to.equal(true);
    expect(await l.outputPays(tx, 0, B, 7000n)).to.equal(false);
    expect(await l.outputPays(tx, 2, B, 1n)).to.equal(false);
  });

  it("finds the coin an input spends", async function () {
    const l = await lib();
    expect(await l.spends(tx, COIN.txidLE, 3)).to.equal(true);
    expect(await l.spends(tx, COIN.txidLE, 2)).to.equal(false);
  });

  it("refuses a transaction cut short, with bytes after it, with witness data, or a long varint", async function () {
    const l = await lib();
    await expect(l.paysAtLeast(tx.slice(0, -2), A, 1n)).to.be.revertedWithCustomError(l, "MalformedTx");
    await expect(l.paysAtLeast(tx + "00", A, 1n)).to.be.revertedWithCustomError(l, "MalformedTx");
    const witness = "0x" + tx.slice(2, 10) + "0001" + tx.slice(10);
    await expect(l.paysAtLeast(witness, A, 1n)).to.be.revertedWithCustomError(l, "WitnessSerialization");
    // One input written as fd 01 00 instead of 01.
    const long = "0x" + tx.slice(2, 10) + "fd0100" + tx.slice(12);
    await expect(l.paysAtLeast(long, A, 1n)).to.be.revertedWithCustomError(l, "MalformedTx");
  });
});
