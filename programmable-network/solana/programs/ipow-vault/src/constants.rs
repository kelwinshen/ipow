//! The vault's rules. Fixed in the program: no key changes them (D59).

/// D111: a claim is decided when its last objection or answer has stood for
/// 7 days.
pub const OBJECTION_WINDOW: i64 = 7 * 24 * 60 * 60;
/// D110, D129: a chain's open claims in an asset are at most 80% of its
/// bond in that asset.
pub const COVER_BPS: u64 = 8000;
/// D109: 80% of a slash backs the receipt, 20% goes to whoever submitted the
/// message.
pub const BACKING_SHARE_BPS: u64 = 8000;
pub const BPS: u64 = 10_000;

/// A message acts on at most this many LOCK records here; with more, it
/// opens no claim and they can be carried again (section 11.9).
pub const MAX_LOCKS: usize = 10;
/// The assets a chain keeps bonds in, and a claim moves, at most.
pub const MAX_ASSETS: usize = 8;

/// Section 11.7: an attester of a lock locks 1.25 times its amount in its
/// receipt. A claim carrying the same LOCK record must open within 7 days of
/// the attest (D123), and a lock takes attests for 7 days from its first
/// (D126).
pub const FAST_COLLATERAL_BPS: u64 = 12_500;
pub const FAST_OPEN_WINDOW: i64 = 7 * 24 * 60 * 60;
/// D124: an attester's share of a fast fee falls to nothing 8 days after the
/// lock or burn.
pub const FAST_FEE_DEADLINE: i64 = 8 * 24 * 60 * 60;

/// D119: a message is false when its Bitcoin transaction is longer than
/// this, its batch is longer than this, or it carries more LOCK, REQUEST,
/// CANCEL and ASSET records, on both networks together, than this: most
/// need an account here. A large message is uploaded to a buffer first.
pub const MAX_RAW_TX: usize = 1024;
pub const MAX_BATCH: usize = 2048;
pub const MAX_RECORDS: usize = 32;

/// A receipt has its token's decimals, at most 9 (section 11.9).
pub const MAX_DECIMALS: u8 = 9;

/// The networks of the pair, as records name them.
pub const ETHEREUM: u8 = 1;
pub const SOLANA: u8 = 2;

pub const LOCK: u8 = 1;
pub const REQUEST: u8 = 2;
pub const CANCEL: u8 = 3;
pub const BOND: u8 = 4;
pub const EXIT: u8 = 5;
pub const ASSET: u8 = 6;
pub const LOCK_LEN: usize = 78;
pub const REQUEST_LEN: usize = 78;
pub const CANCEL_LEN: usize = 10;
pub const BOND_LEN: usize = 15;
pub const ASSET_LEN: usize = 39;

pub const CONFIG_SEED: &[u8] = b"config";
pub const CHAIN_SEED: &[u8] = b"chain";
pub const CLAIM_SEED: &[u8] = b"claim";
pub const STAKE_SEED: &[u8] = b"stake";
pub const REAL_SEED: &[u8] = b"real";
pub const BUFFER_SEED: &[u8] = b"buffer";
pub const MESSAGE_SEED: &[u8] = b"message";
pub const CREDIT_SEED: &[u8] = b"credit";
/// An asset whose home is Solana, by its number: SOL is 0.
pub const ASSET_SEED: &[u8] = b"asset";
/// The vault's account of a token whose home is Solana, by its number.
pub const HOME_TOKENS_SEED: &[u8] = b"home_tokens";
/// A lock of an asset whose home is Solana, by its number.
pub const HOME_LOCK_SEED: &[u8] = b"home_lock";
/// A burn on Ethereum of a receipt of an asset here, once paid.
pub const PAID_SEED: &[u8] = b"paid";
/// A burn on Ethereum paid here at once, by its number and stated record.
pub const FAST_PAY_SEED: &[u8] = b"fast_pay";
/// The receipt here of an asset whose home is Ethereum, by its number, and
/// the vault's account of it.
pub const RECEIPT_SEED: &[u8] = b"receipt";
pub const HOLDING_SEED: &[u8] = b"holding";
/// A burn here of a receipt, by its number.
pub const REQUEST_SEED: &[u8] = b"request";
/// What became of a lock on Ethereum here, by its number.
pub const LOCK_SEED: &[u8] = b"lock";
/// An attest of a lock on Ethereum, by its number (section 11.7).
pub const FAST_SEED: &[u8] = b"fast";
/// How long a message's published batch stays before anyone may close it:
/// the time guardians have to bring a message hidden from the other network
/// there (D120).
pub const MESSAGE_KEPT: i64 = 30 * 24 * 60 * 60;
