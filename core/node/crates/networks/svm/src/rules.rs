//! The program's own rules for what a job's state is. Solana has no views,
//! so the adapter computes them the way the program does: the same code as
//! `programs/ipow-protocol/src/{auction,duty,fees,challenge}.rs`.

use crate::programs::ipow_protocol::accounts::Job;

const AUCTION_DURATION: i64 = 15 * 60;
const AUCTION_QUIET: i64 = 60;
const BID_STEP_DIVISOR: u64 = 1000;
const PROOF_RANGE: i64 = 25;
const DEADLINE_PER_BLOCK: i64 = 48 * 60;

/// `auction::auction_end`.
pub fn auction_end(job: &Job) -> i64 {
    let end = job.opened_at + AUCTION_DURATION;
    if job.has_operator { end.min(job.last_bid_at + AUCTION_QUIET) } else { end }
}

/// `duty::deadline`: the auction's end plus 48 minutes per block of the
/// window (D60, D69).
pub fn deadline(job: &Job) -> i64 {
    let window = PROOF_RANGE + job.confirmations as i64 - 1;
    auction_end(job) + window * DEADLINE_PER_BLOCK
}

/// `bid.rs`: x for the first bid, 0.1% above the best bid after (D32, D76).
pub fn minimum_bid(job: &Job) -> u64 {
    if !job.has_operator {
        return job.escrow;
    }
    job.bid.saturating_add((job.bid / BID_STEP_DIVISOR).max(1))
}

/// `challenge::parent_deposit` (D91).
pub fn parent_deposit(job: &Job) -> u64 {
    job.commitment_fee.saturating_mul(job.parents_shown as u64 + 1)
}
