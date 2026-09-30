use anchor_lang::prelude::*;

use crate::auction::auction_end;
use crate::constants::{CREDIT_SEED, JOB_SEED};
use crate::errors::ProtocolError;
use crate::money::credit;
use crate::state::{Credit, Job};

/// D61: a job with no bid when the auction closes expires, and its fees
/// return to the payer. Anyone may call.
pub fn handler(ctx: Context<Expire>) -> Result<()> {
    let job = &mut ctx.accounts.job;
    require!(Clock::get()?.unix_timestamp >= auction_end(job), ProtocolError::AuctionOpen);
    require!(!job.has_operator, ProtocolError::JobHasBid);
    require!(!job.fees_returned, ProtocolError::FeesAlreadyReturned);
    job.fees_returned = true;
    let amount = job.commitment_fee + job.escrow_fee;
    let record = &mut ctx.accounts.payer_credit;
    record.bump = ctx.bumps.payer_credit;
    credit(record, job.payer, amount)?;
    emit!(crate::JobExpired { job_id: job.id });
    Ok(())
}

#[derive(Accounts)]
pub struct Expire<'info> {
    #[account(mut, seeds = [JOB_SEED, &job.id.to_le_bytes()], bump = job.bump)]
    pub job: Account<'info, Job>,
    #[account(
        init_if_needed,
        payer = funder,
        space = 8 + Credit::INIT_SPACE,
        seeds = [CREDIT_SEED, job.payer.as_ref()],
        bump
    )]
    pub payer_credit: Account<'info, Credit>,
    #[account(mut)]
    pub funder: Signer<'info>,
    pub system_program: Program<'info, System>,
}
