// The Bitcoin side of a job, read from public explorers. Most of a job's
// time is Bitcoin's (the first live run waited 58 minutes for one block),
// so an app shows it: the transaction, its confirmations of those needed,
// and a link. Explorers time out, so each read falls back to the next.

export const EXPLORERS = ["https://mempool.space/api", "https://blockstream.info/api"];

export type BitcoinTx = {
  /** As explorers show it. */
  txid: string;
  confirmed: boolean;
  height: number | null;
  /** When its block was mined, in seconds. */
  time: number | null;
  confirmations: number;
  link: string;
};

export class Bitcoin {
  readonly explorers: string[];
  readonly timeoutMs: number;

  constructor(explorers: string[] = EXPLORERS, timeoutMs = 15_000) {
    this.explorers = explorers;
    this.timeoutMs = timeoutMs;
  }

  private async get(path: string, json = true): Promise<any> {
    let last: unknown;
    for (const base of this.explorers) {
      try {
        const r = await fetch(base + path, { signal: AbortSignal.timeout(this.timeoutMs) });
        if (r.status === 404) return null;
        if (!r.ok) throw new Error(`${base}${path}: ${r.status}`);
        return json ? await r.json() : (await r.text()).trim();
      } catch (e) {
        last = e;
      }
    }
    throw last;
  }

  async tip(): Promise<number> {
    return Number(await this.get("/blocks/tip/height", false));
  }

  async tx(txid: string): Promise<BitcoinTx | null> {
    const status = await this.get(`/tx/${txid}/status`);
    if (!status) return null;
    const tip = status.confirmed ? await this.tip() : 0;
    return {
      txid,
      confirmed: status.confirmed,
      height: status.block_height ?? null,
      time: status.block_time ?? null,
      confirmations: status.confirmed ? tip - status.block_height + 1 : 0,
      link: `https://mempool.space/tx/${txid}`,
    };
  }

  /** The transaction that spends an output, if any. */
  async spender(txid: string, vout: number): Promise<string | null> {
    const s = await this.get(`/tx/${txid}/outspend/${vout}`);
    return s?.spent ? s.txid : null;
  }

  /** A transaction's outputs' scripts, in hex. */
  async outputScripts(txid: string): Promise<string[]> {
    const t = await this.get(`/tx/${txid}`);
    return (t?.vout ?? []).map((o: any) => o.scriptpubkey as string);
  }
}

/** A txid as a contract stores it (Bitcoin's internal order) to how
 *  explorers show it, and back: the bytes reversed. */
export function displayTxid(stored: string): string {
  const hex = stored.replace(/^0x/, "");
  return hex.match(/../g)!.reverse().join("");
}
