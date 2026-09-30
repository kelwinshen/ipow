use anchor_lang::prelude::*;

use crate::constants::{JOB_SEED, NOTE_SEED, OPERATOR_SEED};
use crate::duty::{deadline, note_for, slash, status, SlashAccounts, Status};
use crate::errors::ProtocolError;
use crate::state::{Job, Note, Operator};

/// The first of a guardian's two steps (D47): a sealed note. Nobody can read
/// from it what the guardian will show.
pub fn seal(ctx: Context<SealNote>, _note: [u8; 32]) -> Result<()> {
    let record = &mut ctx.accounts.record;
    record.slot = Clock::get()?.slot;
    record.bump = ctx.bumps.record;
    Ok(())
}

/// Checks and uses up a guardian's note: sealed for this guardian, this job
/// and this evidence, in an earlier slot.
pub fn require_note(record: &Note, note: &[u8; 32], guardian: &Pubkey, job_id: u64, evidence: &[u8; 32], salt: &[u8; 32]) -> Result<()> {
    require!(*note == note_for(guardian, job_id, evidence, salt), ProtocolError::NoNote);
    require!(record.slot < Clock::get()?.slot, ProtocolError::NoNote);
    Ok(())
}

/// The second step, for a missed duty: the deadline has passed and no proof
/// was accepted. The full escrow is slashed (D53), and both fees return to
/// the user (D62). The evidence of a missed duty is empty.
pub fn report_missed_duty(ctx: Context<ReportMissedDuty>, note: [u8; 32], salt: [u8; 32]) -> Result<()> {
    let job = &mut ctx.accounts.job;
    let now = Clock::get()?.unix_timestamp;
    require!(status(job, now) == Status::Assigned, ProtocolError::NotAssigned);
    require!(now > deadline(job)?, ProtocolError::DeadlineNotPassed);
    let guardian = ctx.accounts.guardian.key();
    require_note(&ctx.accounts.note_record, &note, &guardian, job.id, &[0u8; 32], &salt)?;

    let funder = ctx.accounts.guardian.to_account_info();
    let system = ctx.accounts.system_program.to_account_info();
    slash(
        job,
        &guardian,
        SlashAccounts {
            operator: &mut ctx.accounts.operator,
            attester: None,
            application_credit: &ctx.accounts.application_credit.to_account_info(),
            guardian_credit: &ctx.accounts.guardian_credit.to_account_info(),
            payer_credit: &ctx.accounts.payer_credit.to_account_info(),
            funder: &funder,
            system_program: &system,
        },
    )
}

#[derive(Accounts)]
#[instruction(note: [u8; 32])]
pub struct SealNote<'info> {
    #[account(init, payer = guardian, space = 8 + Note::INIT_SPACE, seeds = [NOTE_SEED, &note], bump)]
    pub record: Account<'info, Note>,
    #[account(mut)]
    pub guardian: Signer<'info>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
#[instruction(note: [u8; 32])]
pub struct ReportMissedDuty<'info> {
    #[account(mut, seeds = [JOB_SEED, &job.id.to_le_bytes()], bump = job.bump)]
    pub job: Account<'info, Job>,
    #[account(mut, seeds = [OPERATOR_SEED, job.operator.as_ref()], bump = operator.bump)]
    pub operator: Account<'info, Operator>,
    /// The note is used up; its rent goes back to the guardian.
    #[account(mut, close = guardian, seeds = [NOTE_SEED, &note], bump = note_record.bump)]
    pub note_record: Account<'info, Note>,
    /// CHECK: credit accounts, checked and created in `credit_to`.
    #[account(mut)]
    pub application_credit: UncheckedAccount<'info>,
    /// CHECK: see above.
    #[account(mut)]
    pub guardian_credit: UncheckedAccount<'info>,
    /// CHECK: see above.
    #[account(mut)]
    pub payer_credit: UncheckedAccount<'info>,
    #[account(mut)]
    pub guardian: Signer<'info>,
    pub system_program: Program<'info, System>,
}
