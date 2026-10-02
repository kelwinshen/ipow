// Bitcoin mainnet addresses to the output script a contract stores, and
// back: users give and see addresses, contracts hold scripts. Segwit v0
// (bc1q…, BIP173 bech32), taproot (bc1p…, BIP350 bech32m), and the older
// base58 kinds (1…, 3…). The light client only accepts real Bitcoin, so
// mainnet only.

import { sha256 } from "ethers";

const CHARSET = "qpzry9x8gf2tvdw0s3jn54khce6mua7l";
const BECH32 = 1;
const BECH32M = 0x2bc830a3;

function polymod(values: number[]): number {
  const G = [0x3b6a57b2, 0x26508e6d, 0x1ea119fa, 0x3d4233dd, 0x2a1462b3];
  let chk = 1;
  for (const v of values) {
    const top = chk >>> 25;
    chk = ((chk & 0x1ffffff) << 5) ^ v;
    for (let i = 0; i < 5; i++) if ((top >>> i) & 1) chk ^= G[i];
  }
  return chk >>> 0;
}

const hrpExpand = (hrp: string) => [...hrp].map((c) => c.charCodeAt(0) >> 5).concat([0], [...hrp].map((c) => c.charCodeAt(0) & 31));

function convertBits(data: number[], from: number, to: number, pad: boolean): number[] | null {
  let acc = 0;
  let bits = 0;
  const out: number[] = [];
  const max = (1 << to) - 1;
  for (const v of data) {
    if (v < 0 || v >> from) return null;
    acc = (acc << from) | v;
    bits += from;
    while (bits >= to) {
      bits -= to;
      out.push((acc >> bits) & max);
    }
  }
  if (pad) {
    if (bits) out.push((acc << (to - bits)) & max);
  } else if (bits >= from || ((acc << (to - bits)) & max)) return null;
  return out;
}

const hex = (b: number[] | Uint8Array) => Buffer.from(b).toString("hex");

function segwitScript(address: string): string | null {
  const a = address.toLowerCase();
  if (address !== a && address !== address.toUpperCase()) return null;
  const sep = a.lastIndexOf("1");
  if (a.slice(0, sep) !== "bc" || a.length > 90) return null;
  const data = [...a.slice(sep + 1)].map((c) => CHARSET.indexOf(c));
  if (data.some((d) => d < 0) || data.length < 7) return null;
  const check = polymod([...hrpExpand("bc"), ...data]);
  const version = data[0];
  if (check !== (version === 0 ? BECH32 : BECH32M)) return null;
  const program = convertBits(data.slice(1, -6), 5, 8, false);
  if (!program || program.length < 2 || program.length > 40 || version > 16) return null;
  if (version === 0 && program.length !== 20 && program.length !== 32) return null;
  const op = version === 0 ? 0 : 0x50 + version;
  return "0x" + hex([op, program.length, ...program]);
}

const B58 = "123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz";

function base58Script(address: string): string | null {
  let n = 0n;
  for (const c of address) {
    const d = B58.indexOf(c);
    if (d < 0) return null;
    n = n * 58n + BigInt(d);
  }
  const raw = Buffer.from(n.toString(16).padStart(50, "0"), "hex");
  if (raw.length !== 25) return null;
  const body = raw.subarray(0, 21);
  const sum = Buffer.from(sha256(sha256(body)).slice(2), "hex").subarray(0, 4);
  if (!sum.equals(raw.subarray(21))) return null;
  const h = hex(body.subarray(1));
  if (body[0] === 0x00) return `0x76a914${h}88ac`; // P2PKH
  if (body[0] === 0x05) return `0xa914${h}87`; // P2SH
  return null;
}

/** The output script that pays a Bitcoin mainnet address. */
export function addressToScript(address: string): string {
  const script = address.toLowerCase().startsWith("bc1") ? segwitScript(address) : base58Script(address);
  if (!script) throw new Error(`not a Bitcoin mainnet address: ${address}`);
  return script;
}

/** The address an output script pays, or null for another kind. */
export function scriptToAddress(script: string): string | null {
  const s = Buffer.from(script.replace(/^0x/, ""), "hex");
  const version = s[0] === 0 ? 0 : s[0] >= 0x51 && s[0] <= 0x60 ? s[0] - 0x50 : -1;
  if (version >= 0 && s.length === s[1] + 2 && s[1] >= 2 && s[1] <= 40) {
    const data = [version, ...convertBits([...s.subarray(2)], 8, 5, true)!];
    const mod = polymod([...hrpExpand("bc"), ...data, 0, 0, 0, 0, 0, 0]) ^ (version === 0 ? BECH32 : BECH32M);
    const check = [0, 1, 2, 3, 4, 5].map((i) => (mod >>> (5 * (5 - i))) & 31);
    return "bc1" + [...data, ...check].map((d) => CHARSET[d]).join("");
  }
  const base58 = (prefix: number, h: Buffer) => {
    const body = Buffer.concat([Buffer.from([prefix]), h]);
    const full = Buffer.concat([body, Buffer.from(sha256(sha256(body)).slice(2), "hex").subarray(0, 4)]);
    let n = BigInt("0x" + full.toString("hex"));
    let out = "";
    while (n > 0n) {
      out = B58[Number(n % 58n)] + out;
      n /= 58n;
    }
    for (const b of full) {
      if (b !== 0) break;
      out = "1" + out;
    }
    return out;
  };
  if (s.length === 25 && s[0] === 0x76 && s[1] === 0xa9 && s[2] === 20 && s[23] === 0x88 && s[24] === 0xac) return base58(0x00, s.subarray(3, 23));
  if (s.length === 23 && s[0] === 0xa9 && s[1] === 20 && s[22] === 0x87) return base58(0x05, s.subarray(2, 22));
  return null;
}
