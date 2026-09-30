use anchor_lang::prelude::*;
use ipow_protocol::duty::{status, Status};
use ipow_protocol::state::Job;

use crate::constants::*;
use crate::errors::VaultError;
use crate::state::{Config, Real};
use crate::util::now;

/// Records the proof block of a job as real: its escrow is at least the
/// minimum (D118), and its lock has ended with no challenge won and no
/// slash (D108). Anyone may call.
pub fn handler(ctx: Context<RecordRealFromJob>, _job_id: u64) -> Result<()> {
    let job = &ctx.accounts.job;
    require!(job.escrow >= ctx.accounts.config.min_certifying_escrow, VaultError::EscrowTooLow);
    let t = now()?;
    let s = status(job, t);
    let certified = s == Status::Settled || (s == Status::Proven && t >= job.lock_end && job.open_challenges == 0);
    require!(certified, VaultError::NotCertified);
    ctx.accounts.real.bump = ctx.bumps.real;
    Ok(())
}

#[derive(Accounts)]
#[instruction(job_id: u64)]
pub struct RecordRealFromJob<'info> {
    #[account(seeds = [CONFIG_SEED], bump = config.bump)]
    pub config: Account<'info, Config>,
    #[account(seeds = [b"job", &job_id.to_le_bytes()], bump = job.bump, seeds::program = ipow_protocol::ID)]
    pub job: Account<'info, Job>,
    #[account(
        init,
        payer = payer,
        space = 8 + Real::INIT_SPACE,
        seeds = [REAL_SEED, job.proof_block.hash.as_ref(), &job.proof_block.height.to_le_bytes(), &job.proof_block.epoch_time.to_le_bytes()],
        bump
    )]
    pub real: Account<'info, Real>,
    #[account(mut)]
    pub payer: Signer<'info>,
    pub system_program: Program<'info, System>,
}
