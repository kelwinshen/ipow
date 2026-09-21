use anchor_lang::prelude::*;

use crate::errors::FactoryError;
use crate::state::{FactoryConfig, Party, PartyKind};
use crate::utils::transfer_from_signer;

/// Auditors register permissionlessly. Operators need governance's
/// co-signature (DESIGN_V2 §6.7: governance registers/replaces the
/// operator; it never judges statements).
pub fn handler(
    ctx: Context<RegisterParty>,
    party_id: [u8; 32],
    kind: PartyKind,
    anchor_txid_le: [u8; 32],
    anchor_vout: u32,
    bond: u64,
) -> Result<()> {
    let p = &ctx.accounts.config.params;
    match kind {
        PartyKind::Operator => {
            let gov = ctx
                .accounts
                .governance
                .as_ref()
                .ok_or(FactoryError::Unauthorized)?;
            require!(
                gov.key() == ctx.accounts.config.governance,
                FactoryError::Unauthorized
            );
            require!(bond >= p.min_operator_bond, FactoryError::BondTooSmall);
        }
        PartyKind::Auditor => {
            require!(bond >= p.min_auditor_bond, FactoryError::BondTooSmall);
        }
    }
    transfer_from_signer(
        bond,
        &ctx.accounts.owner.to_account_info(),
        &ctx.accounts.bond_escrow.to_account_info(),
        &ctx.accounts.system_program.to_account_info(),
    )?;
    let party = &mut ctx.accounts.party;
    party.party_id = party_id;
    party.owner = ctx.accounts.owner.key();
    party.kind = kind;
    party.anchor_txid_le = anchor_txid_le;
    party.anchor_vout = anchor_vout;
    party.seq = 0;
    party.bond = bond;
    party.dead = false;
    party.paused_until = 0;
    party.unbond_requested_at = 0;
    Ok(())
}

#[derive(Accounts)]
#[instruction(party_id: [u8; 32])]
pub struct RegisterParty<'info> {
    #[account(seeds = [b"config"], bump)]
    pub config: Account<'info, FactoryConfig>,
    #[account(init, payer = owner, space = 8 + Party::INIT_SPACE, seeds = [b"party", party_id.as_ref()], bump)]
    pub party: Account<'info, Party>,
    #[account(mut, seeds = [b"bond_escrow"], bump = config.bond_escrow_bump)]
    pub bond_escrow: SystemAccount<'info>,
    #[account(mut)]
    pub owner: Signer<'info>,
    pub governance: Option<Signer<'info>>,
    pub system_program: Program<'info, System>,
}
