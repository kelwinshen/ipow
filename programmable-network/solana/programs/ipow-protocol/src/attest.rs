use anchor_lang::prelude::*;

use crate::constants::{APPLICATION_SEED, JOB_SEED, OPERATOR_SEED};
use crate::duty::{challenge_period_of, status, Status};
use crate::errors::ProtocolError;
use crate::state::{Application, Job, Operator};

/// An attester takes the operator's place: it locks x from its own bond, and
/// the operator's bond for the job becomes free (D42, D73). It is paid when
/// the lock ends, for the part of the lock it covered (D96), and slashed in
/// the operator's place if the proof is proven fake. Anyone with a bond may
/// attest, the operator too (D95).
pub fn attest(ctx: Context<Attest>) -> Result<()> {
    let job = &mut ctx.accounts.job;
    let now = Clock::get()?.unix_timestamp;
    require!(status(job, now) == Status::Proven, ProtocolError::NotProven);
    require!(now < job.lock_end, ProtocolError::LockEnded);
    require!(!job.has_attester, ProtocolError::AlreadyAttested);

    let me = ctx.accounts.owner.key();
    let bid = job.bid;
    let escrow = job.escrow;
    if me == job.operator {
        // The same record: the bid is freed and x is locked instead.
        // As on Ethereum: x must be free while the bid is still locked.
        let att = &mut ctx.accounts.attester;
        require!(escrow <= att.bond - att.locked, ProtocolError::BondNotFree);
        att.locked = att.locked - bid + escrow;
    } else {
        let att = &mut ctx.accounts.attester;
        require!(escrow <= att.bond - att.locked, ProtocolError::BondNotFree);
        att.locked += escrow;
        let op = ctx.accounts.operator.as_mut().ok_or(ProtocolError::WrongAccount)?;
        require_keys_eq!(op.owner, job.operator, ProtocolError::WrongAccount);
        op.locked -= bid;
    }
    job.attester = me;
    job.has_attester = true;
    job.attested_at = now;
    emit!(crate::Attested { job_id: job.id, attester: me, amount: escrow });
    Ok(())
}

/// Fails unless the job's message is official (D11, D97): a settlement once
/// its proof is accepted; a claim once attested, or once its lock has ended
/// with no challenge open. Never after a slash. An application calls this
/// (by CPI) before it acts on a message.
pub fn require_official(ctx: Context<RequireOfficial>) -> Result<()> {
    let job = &ctx.accounts.job;
    let now = Clock::get()?.unix_timestamp;
    let s = status(job, now);
    let official = match s {
        Status::Proven | Status::Settled => {
            if job.claim_kind == 0 || job.has_attester {
                true
            } else {
                now >= job.lock_end && job.open_challenges == 0
            }
        }
        _ => false,
    };
    let _ = challenge_period_of(job, &ctx.accounts.application)?;
    require!(official, ProtocolError::NotOfficial);
    Ok(())
}

#[derive(Accounts)]
pub struct Attest<'info> {
    #[account(mut, seeds = [JOB_SEED, &job.id.to_le_bytes()], bump = job.bump)]
    pub job: Account<'info, Job>,
    #[account(mut, seeds = [OPERATOR_SEED, owner.key().as_ref()], bump = attester.bump)]
    pub attester: Account<'info, Operator>,
    /// The operator's record, when the attester is someone else.
    #[account(mut)]
    pub operator: Option<Account<'info, Operator>>,
    pub owner: Signer<'info>,
}

#[derive(Accounts)]
pub struct RequireOfficial<'info> {
    #[account(seeds = [JOB_SEED, &job.id.to_le_bytes()], bump = job.bump)]
    pub job: Account<'info, Job>,
    #[account(seeds = [APPLICATION_SEED, job.application.as_ref()], bump = application.bump)]
    pub application: Account<'info, Application>,
}
