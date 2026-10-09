use anchor_lang::prelude::*;
use ipow_protocol::auction::auction_end;
use ipow_protocol::duty::{status, Status};
use ipow_protocol::state::Job;

use crate::constants::{FUNDING_TIME, SWAP_SEED};
use crate::errors::ConversionError;
use crate::jobs::now;
use crate::state::{Side, Swap, SwapState};

/// Buy: ends a swap whose operator did not lock the coin in time, or that
/// nobody took. The user has paid nothing on Bitcoin. Anyone may call.
pub fn handler(mut ctx: Context<Cancel>) -> Result<()> {
    let a = &mut ctx.accounts;
    require!(a.swap.side == Side::Buy && a.swap.state == SwapState::Open, ConversionError::WrongState);
    let t = now()?;
    let s = status(&a.job, t);
    require!(s != Status::Auction, ConversionError::FundingTimeNotOver);
    require!(s == Status::Expired || t > auction_end(&a.job) + FUNDING_TIME, ConversionError::FundingTimeNotOver);
    a.swap.state = SwapState::Cancelled;
    Ok(())
}

#[derive(Accounts)]
pub struct Cancel<'info> {
    #[account(mut, seeds = [SWAP_SEED, &swap.id.to_le_bytes()], bump = swap.bump)]
    pub swap: Account<'info, Swap>,
    #[account(seeds = [b"job", &swap.job_id.to_le_bytes()], bump = job.bump, seeds::program = ipow_protocol::ID)]
    pub job: Account<'info, Job>,
}
