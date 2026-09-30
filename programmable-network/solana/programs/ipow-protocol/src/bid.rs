use anchor_lang::prelude::*;

use crate::auction::auction_end;
use crate::constants::{BID_STEP_DIVISOR, JOB_SEED, OPERATOR_SEED};
use crate::errors::ProtocolError;
use crate::state::{Job, Operator};

/// The lowest amount the next bid must lock: x for the first bid (D32), and
/// 0.1% above the best bid after that (D76).
pub fn minimum_bid(job: &Job) -> u64 {
    if !job.has_operator {
        return job.escrow;
    }
    let step = (job.bid / BID_STEP_DIVISOR).max(1);
    job.bid.saturating_add(step)
}

/// Bids for a job by locking bond (D29, D32). The operator that was outbid
/// gets its bond back at once; its record is `previous`, left out when the
/// bidder raises its own bid or there is no bid yet.
pub fn handler(ctx: Context<Bid>, amount: u64) -> Result<()> {
    let job = &mut ctx.accounts.job;
    let now = Clock::get()?.unix_timestamp;
    require!(now < auction_end(job), ProtocolError::AuctionClosed);
    require!(amount >= job.escrow && amount >= minimum_bid(job), ProtocolError::BidTooLow);

    let me = ctx.accounts.owner.key();
    let operator = &mut ctx.accounts.operator;
    let mut locked = operator.locked;
    if job.has_operator {
        if job.operator == me {
            locked -= job.bid;
        } else {
            let previous = ctx.accounts.previous.as_mut().ok_or(ProtocolError::BidTooLow)?;
            require_keys_eq!(previous.owner, job.operator, ProtocolError::BidTooLow);
            previous.locked -= job.bid;
        }
    }
    // D19: an operator needs that much free bond.
    require!(amount <= operator.bond - locked, ProtocolError::BondNotFree);
    operator.locked = locked + amount;

    job.operator = me;
    job.has_operator = true;
    job.bid = amount;
    job.last_bid_at = now;
    emit!(crate::BidPlaced { job_id: job.id, operator: me, amount });
    Ok(())
}

#[derive(Accounts)]
pub struct Bid<'info> {
    #[account(mut, seeds = [JOB_SEED, &job.id.to_le_bytes()], bump = job.bump)]
    pub job: Account<'info, Job>,
    #[account(mut, seeds = [OPERATOR_SEED, owner.key().as_ref()], bump = operator.bump)]
    pub operator: Account<'info, Operator>,
    /// The operator that held the best bid, when it is someone else.
    #[account(mut)]
    pub previous: Option<Account<'info, Operator>>,
    pub owner: Signer<'info>,
}
