use anchor_lang::prelude::*;

use crate::constants::{JOB_SEED, MAX_ANCHOR_AGE};
use crate::duty::{deadline, read_node, status, Status};
use crate::errors::ProtocolError;
use crate::state::{BlockRef, Job};

/// The operator of a job names its anchor (N26). The window of the job is
/// the blocks that follow it. D39 applies to this job at this moment.
pub fn handler(ctx: Context<AnchorJob>, anchor: BlockRef) -> Result<()> {
    let job = &mut ctx.accounts.job;
    let now = Clock::get()?.unix_timestamp;
    require!(status(job, now) == Status::Assigned, ProtocolError::NotAssigned);
    require_keys_eq!(ctx.accounts.owner.key(), job.operator, ProtocolError::NotOperator);
    require!(job.anchored_at == 0, ProtocolError::AlreadyAnchored);
    require!(now <= deadline(job)?, ProtocolError::DeadlinePassed);

    let node = read_node(&ctx.accounts.node, &anchor)?;
    require!(now <= node.time as i64 + MAX_ANCHOR_AGE, ProtocolError::AnchorTooOld);

    job.anchor = anchor;
    job.anchored_at = now;
    emit!(crate::JobAnchored { job_id: job.id, anchor_hash: anchor.hash, height: anchor.height });
    Ok(())
}

#[derive(Accounts)]
pub struct AnchorJob<'info> {
    #[account(mut, seeds = [JOB_SEED, &job.id.to_le_bytes()], bump = job.bump)]
    pub job: Account<'info, Job>,
    /// CHECK: the light client's block named as the anchor; checked in `read_node`.
    pub node: UncheckedAccount<'info>,
    pub owner: Signer<'info>,
}
