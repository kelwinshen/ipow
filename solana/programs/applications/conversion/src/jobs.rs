//! The application's side of its jobs: the tag, opening a job in the
//! protocol, and reading what the protocol accepted.

use anchor_lang::prelude::*;
use ipow_protocol::duty::{status, Status};
use ipow_protocol::state::Job;
use ipow_light_client::bitcoin::sha256d;
use sha2::{Digest, Sha256};

use crate::constants::{CONFIG_SEED, ESCROW_FEE_BPS};
use crate::errors::ConversionError;

/// The tag of a swap's job (D80).
pub fn tag_of(swap_id: u64) -> [u8; 32] {
    let mut h = Sha256::new();
    h.update(b"iPoW conversion");
    h.update(crate::ID.as_ref());
    h.update(swap_id.to_le_bytes());
    h.finalize().into()
}

pub struct Open<'a, 'info> {
    pub protocol: &'a Account<'info, ipow_protocol::state::Protocol>,
    pub application: &'a UncheckedAccount<'info>,
    pub job: &'a UncheckedAccount<'info>,
    pub tag_record: &'a UncheckedAccount<'info>,
    pub protocol_vault: &'a UncheckedAccount<'info>,
    pub config: AccountInfo<'info>,
    pub config_bump: u8,
    pub funder: AccountInfo<'info>,
    pub system_program: AccountInfo<'info>,
}

/// Opens the swap's job, with the protocol's lowest escrow: the swap's
/// value is kept safe by this program, not by the escrow. The user pays
/// `paid` (the fees, and what is above them goes to the operator, D79) and
/// receives the fees back if the operator fails (D62). Returns the job's id.
pub fn open_job(o: Open, swap_id: u64, confirmations: u16, claim_kind: u16, user: Pubkey, paid: u64) -> Result<u64> {
    let fee = ipow_protocol::fees::commitment_fee_for(confirmations)?;
    let escrow = fee.checked_mul(ipow_protocol::constants::MIN_ESCROW_MULTIPLE).ok_or(ConversionError::Overflow)?;
    let job_id = o.protocol.job_count + 1;
    let seeds: &[&[u8]] = &[CONFIG_SEED, &[o.config_bump]];
    ipow_protocol::cpi::open_job(
        CpiContext::new_with_signer(
            ipow_protocol::ID,
            ipow_protocol::cpi::accounts::OpenJob {
                protocol: o.protocol.to_account_info(),
                application: o.application.to_account_info(),
                job: o.job.to_account_info(),
                tag_record: o.tag_record.to_account_info(),
                vault: o.protocol_vault.to_account_info(),
                key: o.config,
                funder: o.funder,
                system_program: o.system_program,
            },
            &[seeds],
        ),
        tag_of(swap_id),
        escrow,
        ESCROW_FEE_BPS,
        confirmations,
        claim_kind,
        user,
        paid,
    )?;
    Ok(job_id)
}

pub fn now() -> Result<i64> {
    Ok(Clock::get()?.unix_timestamp)
}

/// Checks the job is proven (or settled) and `raw_tx` is its transaction.
pub fn require_proven(job: &Job, raw_tx: &[u8]) -> Result<()> {
    let s = status(job, now()?);
    require!(s == Status::Proven || s == Status::Settled, ConversionError::NotProven);
    require!(sha256d(raw_tx) == job.txid, ConversionError::WrongTransaction);
    Ok(())
}

/// Whether the proof's lock has ended with no challenge open and no slash:
/// the proof can no longer be shown false.
pub fn lock_ended(job: &Job) -> Result<bool> {
    Ok(!job.slashed && now()? >= job.lock_end && job.open_challenges == 0)
}
