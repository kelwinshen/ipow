/// BETA has 9 decimals so one whole BETA is `UNIT` raw token units. Every
/// statement and every reserve calculation is in whole BETA ("units"); the
/// composition (`sol_per_unit` lamports, `eth_gwei_per_unit`) lives in
/// `FactoryConfig`.
pub const UNIT: u64 = 1_000_000_000;

/// OP_RETURN payload layout on every anchor: `ver(1) | kind(1) | sha256(statement)(32)`.
pub const ANCHOR_VERSION: u8 = 1;
pub const ANCHOR_PAYLOAD_LEN: usize = 34;

pub const KIND_MINT: u8 = 1;
pub const KIND_RELEASE: u8 = 2;
pub const KIND_VETO: u8 = 3;
pub const KIND_CANCEL: u8 = 4;
pub const KIND_ATTEST: u8 = 5;
pub const KIND_CLEAR: u8 = 6;
pub const KIND_ALIVE: u8 = 7;

/// `Party.paused_until` sentinel: paused until an ALIVE statement (v3 — no expiry).
pub const PAUSED_FOREVER: i64 = i64::MAX;

pub const BPS_DENOM: u64 = 10_000;
