//! Rules fixed by the protocol. The same numbers as `iPoWProtocol.sol`.

/// D41: confirmations a job requires by default, and at least.
pub const DEFAULT_CONFIRMATIONS: u16 = 6;
/// D15: the proof is in blocks 1 to 25 of the window.
pub const PROOF_RANGE: u16 = 25;
/// D70: a window is at most 100 blocks.
pub const MAX_WINDOW: u16 = 100;
/// D69: 48 minutes for each block of the window, 1 day for 30 blocks (D14).
pub const DEADLINE_PER_BLOCK: i64 = 48 * 60;

/// D37: bidding is open for 15 minutes.
pub const AUCTION_DURATION: i64 = 15 * 60;
/// D37: the winner is locked in once 1 minute passes with no better bid.
pub const AUCTION_QUIET: i64 = 60;
/// D76: a better bid is at least 0.1% above the best bid.
pub const BID_STEP_DIVISOR: u64 = 1000;

/// D25: the escrow fee is 0.5% of x by default. D77: at most 100%.
pub const DEFAULT_ESCROW_FEE_BPS: u16 = 50;
pub const BPS: u64 = 10_000;
/// D56: the minimum escrow is 5 times the commitment fee.
pub const MIN_ESCROW_MULTIPLE: u64 = 5;
/// D24: the safety margin of the commitment fee is 1.5.
pub const MARGIN_NUMERATOR: u64 = 3;
pub const MARGIN_DENOMINATOR: u64 = 2;

/// D58 on Solana. A transaction pays 5,000 lamports per signature, fixed by
/// the network. The work of one block of the window is one signature. The
/// fixed part is 20 signatures: the jump, naming the anchor, the walks and
/// the proof. The rent of stored blocks is not counted: it goes back to
/// whoever stored them when the blocks are closed after 8 weeks (D101).
pub const SIGNATURE_FEE: u64 = 5_000;
pub const FIXED_SIGNATURES: u64 = 20;

/// D39: an anchor may be at most 2 hours old.
pub const MAX_ANCHOR_AGE: i64 = 2 * 60 * 60;
/// D50: the escrow stays locked 36 hours after a job.
pub const MIN_LOCK: i64 = 36 * 60 * 60;
/// D71: the lock and the challenge period are at least 1.5 times the deadline.
pub const LOCK_NUMERATOR: i64 = 3;
pub const LOCK_DENOMINATOR: i64 = 2;
/// D81: the time to show a parent. D99: no challenge opens in the last 12
/// hours of the lock.
pub const RESPONSE_TIME: i64 = 12 * 60 * 60;
/// D92: at most 2,016 questions for a parent per job.
pub const MAX_PARENT_QUESTIONS: u32 = 2016;
/// D35, D36: 80% of a slash to the application, 20% to the guardian.
pub const APPLICATION_SHARE_BPS: u64 = 8000;
/// D42, D96: an attester earns up to 40% of the escrow fee.
pub const ATTESTER_SHARE_BPS: u64 = 4000;

/// D28: the challenge period is between 36 hours and 7 days.
pub const MIN_CHALLENGE_PERIOD: u32 = 36 * 60 * 60;
pub const MAX_CHALLENGE_PERIOD: u32 = 7 * 24 * 60 * 60;
/// D78: an application registers at most 32 kinds of claim.
pub const MAX_CLAIM_KINDS: usize = 32;

pub const PROTOCOL_SEED: &[u8] = b"protocol";
pub const VAULT_SEED: &[u8] = b"vault";
pub const OPERATOR_SEED: &[u8] = b"operator";
pub const APPLICATION_SEED: &[u8] = b"application";
pub const JOB_SEED: &[u8] = b"job";
pub const TAG_SEED: &[u8] = b"tag";
pub const CREDIT_SEED: &[u8] = b"credit";
pub const USED_TX_SEED: &[u8] = b"used_tx";
pub const NOTE_SEED: &[u8] = b"note";
pub const CHALLENGE_SEED: &[u8] = b"challenge";
pub const CHECKPOINT_SEED: &[u8] = b"checkpoint";
