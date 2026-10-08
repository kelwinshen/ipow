// A JSON-RPC provider that waits out an endpoint's rate limit: a reply
// "rate limited" (-32005, as HyperEVM's own endpoint gives, 2026-10-08) or
// an HTTP 429 is sent again after a growing pause, and requests go one at a
// time rather than in batches, so a deploy is not stopped halfway.

import { FetchRequest, JsonRpcProvider, type JsonRpcPayload, type JsonRpcResult } from "ethers";

const sleep = (ms: number) => new Promise((r) => setTimeout(r, ms));
const limited = (e: unknown) => /rate limit|429|-32005|Too Many Requests/i.test(String((e as any)?.message ?? e) + JSON.stringify((e as any)?.error ?? ""));

export class RetryProvider extends JsonRpcProvider {
  constructor(url: string, userAgent = "ipow-deploy") {
    const request = new FetchRequest(url);
    request.setHeader("user-agent", userAgent);
    super(request, undefined, { staticNetwork: true, batchMaxCount: 1 });
  }

  override async _send(payload: JsonRpcPayload | JsonRpcPayload[]): Promise<JsonRpcResult[]> {
    for (let attempt = 0; ; attempt++) {
      try {
        const results = await super._send(payload);
        const busy = results.some((r: any) => r?.error && (r.error.code === -32005 || /rate limit/i.test(String(r.error.message))));
        if (!busy || attempt >= 8) return results;
      } catch (e) {
        if (!limited(e) || attempt >= 8) throw e;
      }
      await sleep(Math.min(1_000 * 2 ** attempt, 30_000));
    }
  }
}
