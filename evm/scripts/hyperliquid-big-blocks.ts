// Switches the deployer's address on HyperEVM testnet to big blocks (30M
// gas, about one a minute), which deploying the protocol needs: it takes
// about 5.4M gas, and small blocks hold 3M (measured on 2026-10-02). This is
// a HyperCore action, `evmUserModify`, signed with the address's key, not an
// EVM transaction. Run from evm:
//
//   node scripts/hyperliquid-big-blocks.ts [--check | --activate | --off]
//
// With --check it only reads whether the address uses big blocks. HyperCore
// refuses the action from an address it has no account for; --activate
// first moves 0.01 HYPE from HyperEVM to the same address on HyperCore,
// through the bridge address 0x2222...2222, which opens that account. The
// key and endpoint come from networks/hyperliquid/.env. With
// --off it switches the address back to small blocks.

import { FetchRequest, JsonRpcProvider, Signature, Wallet, concat, keccak256, parseEther, toBeHex, zeroPadValue } from "ethers";
import { readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const here = dirname(fileURLToPath(import.meta.url));
const vars: Record<string, string> = {};
for (const line of readFileSync(join(here, "..", "..", "networks", "hyperliquid", ".env"), "utf8").split("\n")) {
  const m = line.match(/^\s*([A-Z0-9_]+)\s*=\s*"?([^"\n]*)"?/);
  if (m) vars[m[1]] = m[2].trim();
}
const request = new FetchRequest(vars.HYPEREVM_TESTNET_RPC_URL);
request.setHeader("user-agent", "ipow-deploy");
const provider = new JsonRpcProvider(request, undefined, { staticNetwork: true });
const key = vars.HYPEREVM_TESTNET_PRIVATE_KEY;
const wallet = new Wallet(key.startsWith("0x") ? key : "0x" + key);

const usingBigBlocks = async () => (await provider.send("eth_usingBigBlocks", [wallet.address])) as boolean;
console.log(wallet.address, "uses big blocks:", await usingBigBlocks());
if (process.argv.includes("--check")) process.exit(0);

if (process.argv.includes("--activate")) {
  // HYPE sent to this address on HyperEVM arrives on HyperCore.
  const BRIDGE = "0x2222222222222222222222222222222222222222";
  const tx = await wallet.connect(provider).sendTransaction({ to: BRIDGE, value: parseEther("0.01") });
  console.log("moved 0.01 HYPE to HyperCore:", tx.hash, "status", (await tx.wait())?.status);
  // HyperCore credits it shortly after the HyperEVM block.
  await new Promise((r) => setTimeout(r, 10_000));
}

// The action, msgpack-encoded as Hyperliquid hashes it: a map of two
// entries in this order, {"type": "evmUserModify", "usingBigBlocks": true}.
// Strings here are under 32 bytes, msgpack's short form.
const str = (s: string) => {
  if (s.length >= 32) throw new Error("only short strings");
  return concat([new Uint8Array([0xa0 | s.length]), new TextEncoder().encode(s)]);
};
const on = !process.argv.includes("--off");
const action = { type: "evmUserModify", usingBigBlocks: on };
const packed = concat([new Uint8Array([0x82]), str("type"), str(action.type), str("usingBigBlocks"), new Uint8Array([on ? 0xc3 : 0xc2])]);
const nonce = Date.now();
// The L1 action's hash: the action, the nonce in 8 bytes, and no vault.
const connectionId = keccak256(concat([packed, zeroPadValue(toBeHex(nonce), 8), new Uint8Array([0])]));
// Signed as Hyperliquid's "phantom agent": source "b" on testnet.
const signature = Signature.from(
  await wallet.signTypedData(
    { name: "Exchange", version: "1", chainId: 1337, verifyingContract: "0x0000000000000000000000000000000000000000" },
    { Agent: [{ name: "source", type: "string" }, { name: "connectionId", type: "bytes32" }] },
    { source: "b", connectionId }
  )
);
const response = await fetch("https://api.hyperliquid-testnet.xyz/exchange", {
  method: "POST",
  headers: { "content-type": "application/json" },
  body: JSON.stringify({ action, nonce, signature: { r: signature.r, s: signature.s, v: signature.v } }),
});
const answer = await response.text();
console.log("exchange answered:", response.status, answer);
const now = await usingBigBlocks();
console.log(wallet.address, "uses big blocks:", now);
if (JSON.parse(answer).status !== "ok" || now !== on) process.exit(1);
