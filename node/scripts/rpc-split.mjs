// A local JSON-RPC endpoint that splits one network's calls between two
// upstreams: sends (and the nonces they need) to one, everything else to
// the other. For HyperEVM testnet, whose Alchemy endpoint reads well but
// refuses every transaction ("Unable to send transaction", 2026-10-10),
// while its public RPC sends but rate-limits a node's many reads.
// run-testnet.sh starts it and points the node at it. No dependencies:
//
//   READS=<url> SENDS=<url> PORT=8546 node scripts/rpc-split.mjs
//
// Batches are split call by call and answered in their order. Upstream URLs
// (with their keys) are never logged.

import http from "node:http";

const READS = process.env.READS;
const SENDS = process.env.SENDS;
const PORT = Number(process.env.PORT ?? 8546);
if (!READS || !SENDS) throw new Error("READS and SENDS must be set");

/** The calls that go to SENDS: a send, what a sender reads to build one,
 *  and how it follows it (on the endpoint that took it, which knows it first). */
const TO_SENDS = new Set(["eth_sendRawTransaction", "eth_sendTransaction", "eth_getTransactionCount", "eth_getTransactionReceipt", "eth_getTransactionByHash"]);

async function call(url, body) {
  const r = await fetch(url, { method: "POST", headers: { "content-type": "application/json" }, body: JSON.stringify(body), signal: AbortSignal.timeout(30_000) });
  return r.json();
}

async function one(req) {
  try {
    return await call(TO_SENDS.has(req.method) ? SENDS : READS, req);
  } catch (e) {
    return { jsonrpc: "2.0", id: req.id ?? null, error: { code: -32603, message: `upstream unreachable: ${e?.name ?? "error"}` } };
  }
}

http
  .createServer((req, res) => {
    let raw = "";
    req.on("data", (c) => (raw += c));
    req.on("end", async () => {
      let body;
      try {
        body = JSON.parse(raw);
      } catch {
        res.writeHead(400, { "content-type": "application/json" });
        return res.end(JSON.stringify({ jsonrpc: "2.0", id: null, error: { code: -32700, message: "parse error" } }));
      }
      const out = Array.isArray(body) ? await Promise.all(body.map(one)) : await one(body);
      res.writeHead(200, { "content-type": "application/json" });
      res.end(JSON.stringify(out));
    });
  })
  .listen(PORT, "127.0.0.1", () => console.log(`rpc-split on 127.0.0.1:${PORT}`));
