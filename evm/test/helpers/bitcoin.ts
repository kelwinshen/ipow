import { createHash } from "node:crypto";

// Bitcoin helpers for the light client tests, written independently of the
// contract so that a test compares two implementations.

export const EPOCH_BLOCKS = 2016;
export const TARGET_TIMESPAN = 14 * 24 * 60 * 60;

export function sha256d(data: Buffer): Buffer {
  const once = createHash("sha256").update(data).digest();
  return createHash("sha256").update(once).digest();
}

function toBuffer(hex: string): Buffer {
  return Buffer.from(hex.replace(/^0x/, ""), "hex");
}

/** The block hash in the byte order a header uses (what the contract stores). */
export function headerHashLE(headerHex: string): string {
  return "0x" + sha256d(toBuffer(headerHex)).toString("hex");
}

/** An explorer prints hashes the other way round. */
export function reverseHex(hex: string): string {
  return "0x" + Buffer.from(toBuffer(hex)).reverse().toString("hex");
}

export function headerTime(headerHex: string): number {
  return toBuffer(headerHex).readUInt32LE(68);
}

export function headerBits(headerHex: string): number {
  return toBuffer(headerHex).readUInt32LE(72);
}

export function headerPrevLE(headerHex: string): string {
  return "0x" + toBuffer(headerHex).subarray(4, 36).toString("hex");
}

export function targetFromBits(bits: number): bigint {
  const exp = BigInt(bits >>> 24);
  const mant = BigInt(bits & 0x007fffff);
  return exp <= 3n ? mant >> (8n * (3n - exp)) : mant << (8n * (exp - 3n));
}

export function bitsFromTarget(target: bigint): number {
  let size = 0n;
  for (let t = target; t > 0n; t >>= 8n) size++;
  let compact =
    size <= 3n ? target << (8n * (3n - size)) : target >> (8n * (size - 3n));
  if ((compact & 0x00800000n) !== 0n) {
    compact >>= 8n;
    size++;
  }
  return Number(compact | (size << 24n));
}

/** Bitcoin's rule for the difficulty of the first block of a new epoch. */
export function retarget(
  lastBits: number,
  firstTime: number,
  lastTime: number,
  powLimit: bigint
): number {
  let timespan = Math.max(lastTime - firstTime, 0);
  timespan = Math.max(timespan, TARGET_TIMESPAN / 4);
  timespan = Math.min(timespan, TARGET_TIMESPAN * 4);
  let next =
    (targetFromBits(lastBits) * BigInt(timespan)) / BigInt(TARGET_TIMESPAN);
  if (next > powLimit) next = powLimit;
  return bitsFromTarget(next);
}

export function concat(headers: string[]): string {
  return "0x" + headers.map((h) => h.replace(/^0x/, "")).join("");
}

let merkleCounter = 0;

/**
 * Mines a header for a test. Only usable with a low difficulty, which the test
 * harness allows and the real contract does not.
 */
export function mine(opts: {
  prevLE: string;
  time: number;
  bits: number;
  merkleRootLE?: string;
}): string {
  const header = Buffer.alloc(80);
  header.writeUInt32LE(0x20000000, 0);
  toBuffer(opts.prevLE).copy(header, 4);
  if (opts.merkleRootLE) {
    toBuffer(opts.merkleRootLE).copy(header, 36);
  } else {
    // A different Merkle root for every mined block, so that two blocks on
    // the same parent are different blocks.
    createHash("sha256")
      .update(`test-merkle-${merkleCounter++}`)
      .digest()
      .copy(header, 36);
  }
  header.writeUInt32LE(opts.time, 68);
  header.writeUInt32LE(opts.bits, 72);

  const target = targetFromBits(opts.bits);
  for (let nonce = 0; nonce < 0xffffffff; nonce++) {
    header.writeUInt32LE(nonce, 76);
    const hash = sha256d(header);
    const value = BigInt("0x" + Buffer.from(hash).reverse().toString("hex"));
    if (value <= target) return "0x" + header.toString("hex");
  }
  throw new Error("no nonce found");
}

/** Mines `count` headers that name each other in order. */
export function mineChain(opts: {
  prevLE: string;
  firstTime: number;
  bits: number;
  count: number;
  spacing?: number;
}): string[] {
  const out: string[] = [];
  let prevLE = opts.prevLE;
  for (let i = 0; i < opts.count; i++) {
    const header = mine({
      prevLE,
      time: opts.firstTime + i * (opts.spacing ?? 600),
      bits: opts.bits,
    });
    out.push(header);
    prevLE = headerHashLE(header);
  }
  return out;
}

// ----------------------------------------------------------------------
// Transactions
// ----------------------------------------------------------------------

function varInt(n: number): Buffer {
  if (n < 0xfd) return Buffer.from([n]);
  const b = Buffer.alloc(3);
  b[0] = 0xfd;
  b.writeUInt16LE(n, 1);
  return b;
}

/** A pay-to-witness-public-key-hash script: a coin that can be spent. */
export const COIN_SCRIPT = "0x0014" + "11".repeat(20);

/** `OP_RETURN` followed by 32 bytes. */
export function tagScript(tag: string): string {
  return "0x6a20" + tag.replace(/^0x/, "");
}

/** A transaction without witness data. Its double SHA-256 is the txid. */
export function buildTx(opts: {
  inputs: { txidLE: string; vout: number }[];
  outputs: { value: bigint; script: string }[];
}): string {
  const parts: Buffer[] = [];
  const version = Buffer.alloc(4);
  version.writeUInt32LE(2, 0);
  parts.push(version, varInt(opts.inputs.length));
  for (const input of opts.inputs) {
    const vout = Buffer.alloc(4);
    vout.writeUInt32LE(input.vout, 0);
    parts.push(
      toBuffer(input.txidLE),
      vout,
      varInt(0),
      Buffer.from("ffffffff", "hex")
    );
  }
  parts.push(varInt(opts.outputs.length));
  for (const output of opts.outputs) {
    const value = Buffer.alloc(8);
    value.writeBigUInt64LE(output.value, 0);
    const script = toBuffer(output.script);
    parts.push(value, varInt(script.length), script);
  }
  parts.push(Buffer.alloc(4));
  return "0x" + Buffer.concat(parts).toString("hex");
}

export function txidLE(rawHex: string): string {
  return "0x" + sha256d(toBuffer(rawHex)).toString("hex");
}

function pairHash(a: Buffer, b: Buffer): Buffer {
  return sha256d(Buffer.concat([a, b]));
}

/** The Merkle root of a block's transactions and the proof of one of them. */
export function merkle(
  txidsLE: string[],
  index: number
): { rootLE: string; siblings: string[] } {
  let level = txidsLE.map(toBuffer);
  const siblings: string[] = [];
  let at = index;
  while (level.length > 1) {
    // Bitcoin repeats the last one when a level has an odd number.
    if (level.length % 2 === 1) level.push(level[level.length - 1]);
    siblings.push("0x" + level[at ^ 1].toString("hex"));
    const next: Buffer[] = [];
    for (let i = 0; i < level.length; i += 2) {
      next.push(pairHash(level[i], level[i + 1]));
    }
    level = next;
    at = Math.floor(at / 2);
  }
  return { rootLE: "0x" + level[0].toString("hex"), siblings };
}
