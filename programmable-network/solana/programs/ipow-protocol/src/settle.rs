use anchor_lang::prelude::*;

use crate::constants::{ATTESTER_SHARE_BPS, BPS, JOB_SEED, OPERATOR_SEED};
use crate::duty::{credit_to, status, Status};
use crate::errors::ProtocolError;
use crate::state::{Job, Operator};

/// Pays the operator and frees its bond when the lock has ended and no
/// challenge is open (D23, D50, D67). With an attester, the attester's lock
/// ends and it earns 40% of the escrow fee for the part of the lock it
/// covered (D42, D96). Anyone may call.
pub fn handler(ctx: Context<Settle>) -> Result<()> {
    let job = &mut ctx.accounts.job;
    let now = Clock::get()?.unix_timestamp;
    require!(status(job, now) == Status::Proven, ProtocolError::NotProven);
    require!(now >= job.lock_end, ProtocolError::LockNotEnded);
    require!(job.open_challenges == 0, ProtocolError::ChallengeOpen);

    job.settled = true;
    job.fees_returned = true;
    let mut paid = job.commitment_fee + job.escrow_fee;
    let funder = ctx.accounts.funder.to_account_info();
    let system = ctx.accounts.system_program.to_account_info();

    if job.has_attester {
        // The operator may be its own attester (D95); its record is then
        // passed once, as the operator.
        let att = if job.attester == job.operator {
            &mut ctx.accounts.operator
        } else {
            ctx.accounts.attester.as_mut().ok_or(ProtocolError::WrongAccount)?
        };
        require_keys_eq!(att.owner, job.attester, ProtocolError::WrongAccount);
        att.locked -= job.escrow;
        let covered = (job.lock_end - job.attested_at) as u128;
        let whole = (job.lock_end - job.proven_at) as u128;
        let share = (job.escrow_fee as u128 * ATTESTER_SHARE_BPS as u128 * covered / (BPS as u128 * whole)) as u64;
        paid -= share;
        let account = if job.attester == job.operator {
            ctx.accounts.operator_credit.to_account_info()
        } else {
            ctx.accounts.attester_credit.as_ref().ok_or(ProtocolError::WrongAccount)?.to_account_info()
        };
        credit_to(&account, &job.attester, share, &funder, &system)?;
    } else {
        let op = &mut ctx.accounts.operator;
        require_keys_eq!(op.owner, job.operator, ProtocolError::WrongAccount);
        op.locked -= job.bid;
    }
    credit_to(&ctx.accounts.operator_credit.to_account_info(), &job.operator, paid, &funder, &system)?;
    emit!(crate::JobSettled { job_id: job.id, operator: job.operator, paid });
    Ok(())
}

#[derive(Accounts)]
pub struct Settle<'info> {
    #[account(mut, seeds = [JOB_SEED, &job.id.to_le_bytes()], bump = job.bump)]
    pub job: Account<'info, Job>,
    #[account(mut, seeds = [OPERATOR_SEED, job.operator.as_ref()], bump = operator.bump)]
    pub operator: Account<'info, Operator>,
    /// CHECK: the operator's credit account; checked and created in `credit_to`.
    #[account(mut)]
    pub operator_credit: UncheckedAccount<'info>,
    /// The attester's record, when the job has one.
    #[account(mut)]
    pub attester: Option<Account<'info, Operator>>,
    /// CHECK: the attester's credit account; checked and created in `credit_to`.
    #[account(mut)]
    pub attester_credit: Option<UncheckedAccount<'info>>,
    #[account(mut)]
    pub funder: Signer<'info>,
    pub system_program: Program<'info, System>,
}
