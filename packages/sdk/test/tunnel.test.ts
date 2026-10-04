// The tunnel's helpers that need no network: the message a registration is
// signed with (the node's `tunnel_message` has the same words), amounts in
// smallest units, and estimates at an operator's prices.

import { test } from "node:test";
import assert from "node:assert/strict";

import { coinFor, fromUnits, satsFor, toUnits, tunnelMessage, type TunnelAsset } from "../src/index.ts";

test("the registration message is the node's, word for word", () => {
  assert.equal(
    tunnelMessage({ to: "ethereum-sepolia", swapId: 8n, from: "solana-devnet", fromToken: "native", amountIn: 60000000n }),
    "iPoW tunnel: buy 8 on ethereum-sepolia; sell 60000000 of native on solana-devnet"
  );
});

test("amounts to and from smallest units, without floating point", () => {
  assert.equal(toUnits("0.06", 9), 60000000n);
  assert.equal(toUnits("1", 18), 10n ** 18n);
  assert.equal(toUnits("0.123456789123", 9), 123456789n);
  assert.equal(fromUnits(60000000n, 9), 0.06);
  assert.throws(() => toUnits("1e3", 9));
});

test("estimates stay inside the operator's prices", () => {
  const sol: TunnelAsset = { network: "solana-devnet", token: "native", symbol: "SOL", decimals: 9, paySats: 15000n, askSats: 15000n };
  // 0.06 SOL at 15,000 sats a SOL is 900 sats; the estimate keeps 1% back.
  assert.equal(satsFor(sol, "0.06"), 891n);
  assert.ok(Math.abs(coinFor(sol, 900)! - 0.0594) < 1e-12);
  assert.equal(coinFor({ ...sol, askSats: 0n }, 900), null);
});
