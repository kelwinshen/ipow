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

  /** A transaction's outputs: each one's script (hex, no 0x) and sats. */
  async outputs(txid: string): Promise<{ script: string; sats: bigint }[]> {
    const t = await this.get(`/tx/${txid}`);
    return (t?.vout ?? []).map((o: any) => ({ script: o.scriptpubkey as string, sats: BigInt(o.value) }));
  }

  /** What a transaction spends: each input's previous output, its script
   *  (hex, no 0x) and sats. */
  async inputs(txid: string): Promise<{ txid: string; vout: number; script: string; sats: bigint }[]> {
    const t = await this.get(`/tx/${txid}`);
    return (t?.vin ?? []).map((i: any) => ({ txid: i.txid, vout: i.vout, script: i.prevout?.scriptpubkey ?? "", sats: BigInt(i.prevout?.value ?? 0) }));
  }

  /** The best chain's block at a height: its hash as explorers show it. */
  async blockHashAt(height: number): Promise<string> {
    return this.get(`/block-height/${height}`, false);
  }

  /** A block's 80-byte header, hex, by its hash as explorers show it. */
  async header(blockHash: string): Promise<string> {
    return this.get(`/block/${blockHash}/header`, false);
  }

  /** A block's transactions' ids, in order, as explorers show them. */
  async blockTxids(blockHash: string): Promise<string[]> {
    return this.get(`/block/${blockHash}/txids`);
  }

  /** Where a transaction was mined: its block's hash and height; null while
   *  in the mempool or unknown. */
  async minedIn(txid: string): Promise<{ blockHash: string; height: number } | null> {
    const s = await this.get(`/tx/${txid}/status`);
    return s?.confirmed ? { blockHash: s.block_hash, height: s.block_height } : null;
  }

  /** A transaction as contracts take it: without witness data, 0x hex. */
  async rawTx(txid: string): Promise<string> {
    const hex = await this.get(`/tx/${txid}/hex`, false);
    if (!hex) throw new Error(`no transaction ${txid}`);
    return stripWitness(hex);
  }

  /** Transactions paying `address`, mempool included, newest first. */
  async paymentsTo(address: string): Promise<{ txid: string; confirmed: boolean }[]> {
    const txs = (await this.get(`/address/${address}/txs`)) ?? [];
    return txs
      .filter((t: any) => t.vout.some((o: any) => o.scriptpubkey_address === address))
      .map((t: any) => ({ txid: t.txid as string, confirmed: Boolean(t.status?.confirmed) }));
  }
}

/**
 * A transaction without its witness data (BIP144's marker, flag and
 * witnesses taken out), as a txid is computed and contracts take it. One
 * that has none is returned as it is. In and out: hex, with or without 0x.
 */
export function stripWitness(hex: string): string {
  const b = Uint8Array.from((hex.replace(/^0x/, "").match(/../g) ?? []).map((x) => parseInt(x, 16)));
  if (b.length < 10 || b[4] !== 0 || b[5] === 0) return "0x" + hex.replace(/^0x/, "");
  let at = 6;
  const varint = (): number => {
    const first = b[at++];
    if (first < 0xfd) return first;
    const size = first === 0xfd ? 2 : first === 0xfe ? 4 : 8;
    let n = 0;
    for (let i = 0; i < size; i++) n += b[at + i] * 2 ** (8 * i);
    at += size;
    return n;
  };
  // Skips a length-prefixed field. (`at += varint()` would read `at`
  // before the varint moved it.)
  const skipField = () => {
    const n = varint();
    at += n;
  };
  const start = at;
  const inputs = varint();
  for (let i = 0; i < inputs; i++) {
    at += 36;
    skipField();
    at += 4;
  }
  const outputs = varint();
  for (let i = 0; i < outputs; i++) {
    at += 8;
    skipField();
  }
  const body = b.subarray(start, at);
  for (let i = 0; i < inputs; i++) {
    const items = varint();
    for (let j = 0; j < items; j++) skipField();
  }
  const locktime = b.subarray(at, at + 4);
  if (locktime.length !== 4) throw new Error("not a transaction");
  const out = new Uint8Array(4 + body.length + 4);
  out.set(b.subarray(0, 4), 0);
  out.set(body, 4);
  out.set(locktime, 4 + body.length);
  return "0x" + Array.from(out, (x) => x.toString(16).padStart(2, "0")).join("");
}

/** A txid as a contract stores it (Bitcoin's internal order) to how
 *  explorers show it, and back: the bytes reversed. */
export function displayTxid(stored: string): string {
  const hex = stored.replace(/^0x/, "");
  return hex.match(/../g)!.reverse().join("");
}
