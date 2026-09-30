use anchor_lang::prelude::*;

use crate::constants::*;
use crate::errors::ProtocolError;
use crate::fees::commitment_fee_for;
use crate::money::pay_in;
use crate::state::{Application, Job, Protocol, TagRecord};

/// Opens a job. Only a registered application may (D83). `paid` lamports
/// are taken from the funder: the two fees, and what is above them is kept
/// for the operator as commitment fee (D79). The tag can be used once by an
/// application (D80).
#[allow(clippy::too_many_arguments)]
pub fn handler(
    ctx: Context<OpenJob>,
    tag: [u8; 32],
    escrow: u64,
    escrow_fee_bps: u16,
    confirmations: u16,
    claim_kind: u16,
    payer: Pubkey,
    paid: u64,
) -> Result<()> {
    let app = &ctx.accounts.application;
    require_keys_neq!(payer, Pubkey::default(), ProtocolError::ZeroAddress);
    require!(
        (claim_kind as usize) <= app.challenge_periods.len(),
        ProtocolError::UnknownClaimKind
    );
    require!(escrow_fee_bps as u64 <= BPS, ProtocolError::EscrowFeeOutOfRange);

    let commitment_fee = commitment_fee_for(confirmations)?;
    // D55, D56.
    require!(
        escrow > 0 && escrow >= commitment_fee.checked_mul(MIN_ESCROW_MULTIPLE).ok_or(ProtocolError::Overflow)?,
        ProtocolError::EscrowTooLow
    );
    // D33: on x, never on a bid.
    let escrow_fee = ((escrow as u128 * escrow_fee_bps as u128) / BPS as u128) as u64;
    let fees = commitment_fee.checked_add(escrow_fee).ok_or(ProtocolError::Overflow)?;
    require!(paid >= fees, ProtocolError::FeesNotPaid);
    pay_in(
        &ctx.accounts.funder.to_account_info(),
        &ctx.accounts.vault.to_account_info(),
        &ctx.accounts.system_program.to_account_info(),
        paid,
    )?;

    let protocol = &mut ctx.accounts.protocol;
    protocol.job_count += 1;
    let job = &mut ctx.accounts.job;
    job.id = protocol.job_count;
    job.application = app.key;
    job.payer = payer;
    job.tag = tag;
    job.escrow = escrow;
    // D79.
    job.commitment_fee = paid - escrow_fee;
    job.escrow_fee = escrow_fee;
    job.confirmations = confirmations;
    job.claim_kind = claim_kind;
    job.opened_at = Clock::get()?.unix_timestamp;
    job.bump = ctx.bumps.job;
    ctx.accounts.tag_record.job_id = job.id;
    ctx.accounts.tag_record.bump = ctx.bumps.tag_record;

    emit!(crate::JobOpened {
        job_id: job.id,
        application: job.application,
        tag,
        escrow,
        commitment_fee: job.commitment_fee,
        escrow_fee,
        confirmations,
        claim_kind,
        payer,
    });
    Ok(())
}

#[derive(Accounts)]
#[instruction(tag: [u8; 32])]
pub struct OpenJob<'info> {
    #[account(mut, seeds = [PROTOCOL_SEED], bump = protocol.bump)]
    pub protocol: Account<'info, Protocol>,
    #[account(seeds = [APPLICATION_SEED, key.key().as_ref()], bump = application.bump)]
    pub application: Account<'info, Application>,
    #[account(
        init,
        payer = funder,
        space = 8 + Job::INIT_SPACE,
        seeds = [JOB_SEED, &(protocol.job_count + 1).to_le_bytes()],
        bump
    )]
    pub job: Account<'info, Job>,
    #[account(
        init,
        payer = funder,
        space = 8 + TagRecord::INIT_SPACE,
        seeds = [TAG_SEED, key.key().as_ref(), &tag],
        bump
    )]
    pub tag_record: Account<'info, TagRecord>,
    /// CHECK: the protocol's vault.
    #[account(mut, seeds = [VAULT_SEED], bump = protocol.vault_bump)]
    pub vault: UncheckedAccount<'info>,
    /// The application's own key.
    pub key: Signer<'info>,
    #[account(mut)]
    pub funder: Signer<'info>,
    pub system_program: Program<'info, System>,
}
