/// The challenge period of a close, the application's one kind of claim.
pub const CLOSE_PERIOD: u32 = 36 * 60 * 60;
pub const CLOSE: u16 = 1;
/// Buy: how long the operator has to lock the coin, from the end of the
/// auction.
pub const FUNDING_TIME: i64 = 30 * 60;
/// Buy: the user pays in one of the 12 blocks after the anchor. A close
/// counts only when mined after them.
pub const PAY_BLOCKS: u32 = 12;
/// Buy: the operator may lock the coin, and so reveal the script the user
/// pays, only while its anchor is at most this old (seconds).
pub const ANCHOR_AGE: i64 = 30 * 60;
pub const ESCROW_FEE_BPS: u16 = 50;
pub const MAX_SCRIPT_LENGTH: usize = 100;
/// Buy: the largest payment, in satoshis (0.1 BTC). A user who proves their
/// own payment on made-up blocks must mine at Bitcoin's full difficulty on
/// top of the anchor; this keeps that unprofitable. Fixed in the program:
/// no key changes it (D59).
pub const MAX_SATS: u64 = 10_000_000;

pub const CONFIG_SEED: &[u8] = b"config";
pub const SWAP_SEED: &[u8] = b"swap";
pub const SCRIPT_SEED: &[u8] = b"script";
