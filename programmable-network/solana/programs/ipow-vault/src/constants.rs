//! The vault's rules. Fixed in the program: no key changes them (D59).

/// D111: a claim is decided when its last objection or answer has stood for
/// 7 days.
pub const OBJECTION_WINDOW: i64 = 7 * 24 * 60 * 60;
/// D110: a chain's open claims are at most 80% of its bond.
pub const COVER_BPS: u64 = 8000;
/// D109: 80% of a slash backs the receipt, 20% goes to whoever submitted the
/// message.
pub const BACKING_SHARE_BPS: u64 = 8000;
pub const BPS: u64 = 10_000;
/// vETH has 9 decimals: its smallest unit is one gwei.
pub const VETH_DECIMALS: u8 = 9;


/// The largest part of a message this vault acts on, in bytes: ten LOCK
/// records. A message with more acting records opens no claim; they can be
/// carried again.
pub const MAX_ACTING_BYTES: usize = 730;

/// Section 11.7: an attester of a lock locks 1.25 times its amount in vETH.
/// A claim carrying the same LOCK record must open within 7 days of the
/// attest; otherwise, or when that claim is refused, the amount is burned
/// and the rest goes to whoever burns it (D123).
pub const FAST_COLLATERAL_BPS: u64 = 12_500;
pub const FAST_OPEN_WINDOW: i64 = 7 * 24 * 60 * 60;
/// D124: an attester's share of a fast fee falls to nothing 8 days after the
/// lock or burn, about the slow path.
pub const FAST_FEE_DEADLINE: i64 = 8 * 24 * 60 * 60;

/// A message is false when its Bitcoin transaction is longer than this, its
/// batch is longer than this, or it carries more REQUEST and CANCEL records
/// than this (section 11.3): every message that counts can then be judged
/// on every network. A large message is uploaded to a buffer first.
pub const MAX_RAW_TX: usize = 1024;
pub const MAX_BATCH: usize = 2048;
pub const MAX_HOME_RECORDS: usize = 32;

/// The networks of the pair, as records name them.
pub const ETHEREUM: u8 = 1;
pub const SOLANA: u8 = 2;

pub const LOCK: u8 = 1;
pub const REQUEST: u8 = 2;
pub const CANCEL: u8 = 3;
pub const BOND: u8 = 4;
pub const EXIT: u8 = 5;
pub const LOCK_LEN: usize = 73;
pub const REQUEST_LEN: usize = 61;
pub const CANCEL_LEN: usize = 9;
pub const BOND_LEN: usize = 10;

pub const CONFIG_SEED: &[u8] = b"config";
pub const MINT_SEED: &[u8] = b"veth";
pub const HOLDING_SEED: &[u8] = b"holding";
pub const CHAIN_SEED: &[u8] = b"chain";
pub const CLAIM_SEED: &[u8] = b"claim";
pub const STAKE_SEED: &[u8] = b"stake";
pub const REAL_SEED: &[u8] = b"real";
pub const REQUEST_SEED: &[u8] = b"request";
pub const LOCK_SEED: &[u8] = b"lock";
pub const CREDIT_SEED: &[u8] = b"credit";
pub const BUFFER_SEED: &[u8] = b"buffer";
pub const MESSAGE_SEED: &[u8] = b"message";
/// An attest of a lock, by its number (section 11.7).
pub const FAST_SEED: &[u8] = b"fast";
/// How long a message's published batch stays before its operator may close
/// it: the time guardians have to bring a message hidden from the other
/// network there (section 11.3).
pub const MESSAGE_KEPT: i64 = 30 * 24 * 60 * 60;
