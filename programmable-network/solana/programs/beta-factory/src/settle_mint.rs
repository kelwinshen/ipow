use anchor_lang::prelude::*;

use crate::constants::*;
use crate::errors::FactoryError;
use crate::state::{AnchorStatus, FactoryConfig, Party, Pending, ProcessedAnchor};
use crate::utils::transfer_from_pda;

/// Permissionless. Settles a MINT anchor once its challenge window has
/// closed (DESIGN_V2 §7.3):
/// - not held → the attester's escrow (if any) returns to its bond; the
///   mint is final (and may still be exercised now if it never was).
/// - held → the escrow is forfeited to insurance (it backs the unit that
///   should not exist). If the mint was never exercised it is cancelled
///   and its pending slot un-queued, so the user can re-anchor or expire.
pub fn handler(ctx: Context<SettleMint>, _txid_le: [u8; 32]) -> Result<()> {
    let now = Clock::get()?.unix_timestamp;
    let pa = &mut ctx.accounts.processed;
    require!(pa.kind == KIND_MINT, FactoryError::BadAnchorState);
    require!(!pa.settled, FactoryError::AlreadySettled);
    require!(now >= pa.challenge_until, FactoryError::ChallengeOpen);

    let escrow_bump = ctx.accounts.config.bond_escrow_bump;
    let seeds: &[&[u8]] = &[b"bond_escrow", &[escrow_bump]];
    if pa.escrow > 0 {
        let attester = ctx.accounts.attester.as_mut().ok_or(FactoryError::AttesterRequired)?;
        require!(attester.party_id == pa.attested_by, FactoryError::AccountMismatch);
        if pa.held {
            transfer_from_pda(
                pa.escrow,
                &ctx.accounts.bond_escrow.to_account_info(),
                &ctx.accounts.insurance.to_account_info(),
                &ctx.accounts.system_program.to_account_info(),
                &[seeds],
            )?;
        } else {
            attester.bond = attester.bond.checked_add(pa.escrow).ok_or(FactoryError::Overflow)?;
        }
        pa.escrow = 0;
    }
    if pa.held && pa.status == AnchorStatus::Queued {
        let p = ctx.accounts.pending.as_mut().ok_or(FactoryError::MissingAccount)?;
        require!(p.user == pa.sol_user && p.nonce == pa.nonce, FactoryError::AccountMismatch);
        p.queued = false;
        p.queued_by = [0u8; 32];
        pa.status = AnchorStatus::Cancelled;
    }
    pa.settled = true;
    Ok(())
}

#[derive(Accounts)]
#[instruction(txid_le: [u8; 32])]
pub struct SettleMint<'info> {
    #[account(seeds = [b"config"], bump)]
    pub config: Box<Account<'info, FactoryConfig>>,
    #[account(mut, seeds = [b"anchor", txid_le.as_ref()], bump)]
    pub processed: Box<Account<'info, ProcessedAnchor>>,
    #[account(mut, seeds = [b"bond_escrow"], bump = config.bond_escrow_bump)]
    pub bond_escrow: SystemAccount<'info>,
    #[account(mut, seeds = [b"insurance"], bump = config.insurance_bump)]
    pub insurance: SystemAccount<'info>,
    pub system_program: Program<'info, System>,
    /// The attesting party (required when an escrow is outstanding).
    #[account(mut)]
    pub attester: Option<Box<Account<'info, Party>>>,
    /// The pending slot (required when cancelling a never-exercised mint).
    #[account(mut)]
    pub pending: Option<Box<Account<'info, Pending>>>,
}
