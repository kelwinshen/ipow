use crate::constants::{AUCTION_DURATION, AUCTION_QUIET};
use crate::state::Job;

/// D37, D82: bidding ends 15 minutes after the job was opened, or 1 minute
/// after the last bid when that is earlier.
pub fn auction_end(job: &Job) -> i64 {
    let end = job.opened_at + AUCTION_DURATION;
    if job.has_operator {
        end.min(job.last_bid_at + AUCTION_QUIET)
    } else {
        end
    }
}
