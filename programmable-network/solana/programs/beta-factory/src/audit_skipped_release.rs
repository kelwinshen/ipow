use anchor_lang::prelude::*;

use crate::constants::*;
use crate::errors::FactoryError;
use crate::state::{AnchorStatus, Burn, FactoryConfig, Party, ProcessedAnchor};
use crate::statement::Statement;
use crate::utils::slash;

/// A skipped RELEASE whose preimage later surfaces is judged exactly as it
/// would have been at processing time. (Skipped MINTs need no late audit:
/// they were never exercised here, and Ethereum only finalizes a MINT
/// anchor revealed within its own window — DESIGN_V2 §6.2.)
pub fn handler(ctx: Context<AuditSkippedRelease>, _txid_le: [u8; 32], statement: Vec<u8>) -> Result<()> {
    let pa = &mut ctx.accounts.processed;
    require!(pa.status == AnchorStatus::Skipped, FactoryError::BadAnchorState);
    require!(pa.kind == KIND_RELEASE, FactoryError::BadAnchorState);
    require!(Statement::hash(&statement) == pa.statement_hash, FactoryError::StatementHashMismatch);
    let (burn_id, to_eth, units) = match Statement::decode(&statement)? {
        Statement::Release { burn_id, to_eth, units, .. } => (burn_id, to_eth, units),
        _ => return Err(error!(FactoryError::KindMismatch)),
    };
    let party = &mut ctx.accounts.party;
    require!(party.party_id == pa.party_id, FactoryError::AccountMismatch);
    let (expected_burn, _) = Pubkey::find_program_address(&[b"burn", &burn_id.to_le_bytes()], ctx.program_id);
    let predicate_true = match ctx.accounts.burn.as_ref() {
        Some(b) if b.key() == expected_burn => !b.claimed && b.units == units && b.to_eth == to_eth,
        _ => false,
    };
    if predicate_true {
        ctx.accounts.burn.as_mut().unwrap().claimed = true;
        pa.status = AnchorStatus::Exercised;
    } else {
        let params = ctx.accounts.config.params;
        let bump = ctx.accounts.config.bond_escrow_bump;
        let seeds: &[&[u8]] = &[b"bond_escrow", &[bump]];
        slash(
            party,
            units.checked_mul(params.comp_lamports_per_unit).ok_or(FactoryError::Overflow)?,
            params.bounty_bps,
            &ctx.accounts.bond_escrow.to_account_info(),
            &ctx.accounts.submitter.to_account_info(),
            &ctx.accounts.insurance.to_account_info(),
            &ctx.accounts.system_program.to_account_info(),
            &[seeds],
        )?;
        pa.status = AnchorStatus::Slashed;
    }
    Ok(())
}

#[derive(Accounts)]
#[instruction(txid_le: [u8; 32])]
pub struct AuditSkippedRelease<'info> {
    #[account(seeds = [b"config"], bump)]
    pub config: Box<Account<'info, FactoryConfig>>,
    #[account(mut)]
    pub party: Box<Account<'info, Party>>,
    #[account(mut, seeds = [b"anchor", txid_le.as_ref()], bump)]
    pub processed: Box<Account<'info, ProcessedAnchor>>,
    #[account(mut, seeds = [b"bond_escrow"], bump = config.bond_escrow_bump)]
    pub bond_escrow: SystemAccount<'info>,
    #[account(mut, seeds = [b"insurance"], bump = config.insurance_bump)]
    pub insurance: SystemAccount<'info>,
    #[account(mut)]
    pub submitter: Signer<'info>,
    pub system_program: Program<'info, System>,
    #[account(mut)]
    pub burn: Option<Box<Account<'info, Burn>>>,
}
