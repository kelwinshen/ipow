import { sha256 } from "ethers";

// Real Bitcoin mainnet block headers, used as fixtures wherever a test needs to pass
// iPoW's genuine proof-of-work check.
//
// GENESIS_HEADER_HEX is the raw 80-byte mainnet genesis block header (height 0):
// version=1, prevHash=0x00..00, time=1231006505, bits=0x1d00ffff, nonce=2083236893.
// Its hash is NOT hand-transcribed here — computeHeaderHashLE() below derives it from
// the header bytes at test time via ethers' own sha256, so there's no hardcoded hash
// value that could be mistyped. The header hex itself has been independently verified
// (decoded to exactly 80 bytes, double-SHA256 hash satisfies its own encoded
// difficulty target, and matches the well-known genesis block hash).
export const GENESIS_HEADER_HEX =
  "0x0100000000000000000000000000000000000000000000000000000000000000000000003ba3edfd7a7b12b27ac72c3e67768f617fc81bc3888a51323a9fb8aa4b1e5e4a29ab5f49ffff001d1dac2b7c";

export const GENESIS_HEIGHT = 0;

/** Computes double-SHA256 of a hex-encoded byte string (contract's hashLE convention). */
export function sha256d(hex: string): string {
  return sha256(sha256(hex));
}

/** The little-endian block hash iPoW stores/compares, derived (not hardcoded). */
export function computeHeaderHashLE(headerHex: string): string {
  return sha256d(headerHex);
}
