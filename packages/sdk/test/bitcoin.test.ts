// Bitcoin helpers that need no network: a transaction without its witness
// data, as contracts take it, checked against a real mainnet transaction.

import { test } from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { getBytes, sha256 } from "ethers";

import { displayTxid, stripWitness } from "../src/index.ts";

// A segwit transaction from Bitcoin mainnet, as Blockstream serves it.
const fixture = JSON.parse(readFileSync(new URL("./fixtures/segwit-tx.json", import.meta.url), "utf8")) as { txid: string; hex: string };
const txidOf = (raw: string) => displayTxid(sha256(sha256(getBytes(raw))));

test("takes a segwit transaction's witness out, leaving the bytes its txid is of", () => {
  const raw = stripWitness(fixture.hex);
  assert.ok(raw.length < fixture.hex.length + 2);
  assert.equal(txidOf(raw), fixture.txid);
  // The full transaction's hash is its wtxid, not its txid.
  assert.notEqual(txidOf("0x" + fixture.hex), fixture.txid);
});

test("leaves a transaction without witness data as it is", () => {
  const raw = stripWitness(fixture.hex);
  assert.equal(stripWitness(raw), raw);
});
