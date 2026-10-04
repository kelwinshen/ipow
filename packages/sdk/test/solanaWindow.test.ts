// When a tunnel's sell on Solana was paid outside its window (T1): the
// rule the page offers the refund by, the same as the program's.

import { test } from "node:test";
import assert from "node:assert/strict";

import { solanaPaidOutsideWindow, type SolanaSwapView } from "../src/index.ts";

const sell = (over: Partial<SolanaSwapView>): SolanaSwapView => ({
  id: 1n,
  side: "Sell",
  state: "Open",
  user: "u",
  amount: 1n,
  sats: 1n,
  address: null,
  jobId: 1n,
  stage: "Proven",
  operator: "o",
  times: { opened: 0, auctionEnd: 0, deadline: 0, proven: 1, lockEnd: 2 },
  payBlocks: null,
  payWindow: { first: 101, last: 112 },
  provenTxid: "00",
  provenHeight: 101,
  ...over,
});

test("a payment in the window's first or last block counts; one block either side does not", () => {
  assert.equal(solanaPaidOutsideWindow(sell({ provenHeight: 101 })), false);
  assert.equal(solanaPaidOutsideWindow(sell({ provenHeight: 112 })), false);
  assert.equal(solanaPaidOutsideWindow(sell({ provenHeight: 100 })), true);
  assert.equal(solanaPaidOutsideWindow(sell({ provenHeight: 113 })), true);
  assert.equal(solanaPaidOutsideWindow(sell({ provenHeight: 113, stage: "Settled" })), true);
});

test("only a proven sell with a window", () => {
  assert.equal(solanaPaidOutsideWindow(sell({ provenHeight: 113, payWindow: null })), false);
  assert.equal(solanaPaidOutsideWindow(sell({ provenHeight: null, provenTxid: null })), false);
  assert.equal(solanaPaidOutsideWindow(sell({ provenHeight: 113, stage: "Assigned" })), false);
  assert.equal(solanaPaidOutsideWindow(sell({ provenHeight: 113, side: "Buy" })), false);
});
