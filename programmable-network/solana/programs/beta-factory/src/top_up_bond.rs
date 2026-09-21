use anchor_lang::prelude::*;

use crate::errors::FactoryError;
use crate::state::{FactoryConfig, Party};
use crate::utils::transfer_from_signer;

pub fn handler(ctx: Context<TopUpBond>, amount: u64) -> Result<()> {
    transfer_from_signer(
        amount,
        &ctx.accounts.owner.to_account_info(),
        &ctx.accounts.bond_escrow.to_account_info(),
        &ctx.accounts.system_program.to_account_info(),
    )?;
    let party = &mut ctx.accounts.party;
    party.bond = party.bond.checked_add(amount).ok_or(FactoryError::Overflow)?;
    Ok(())
}

#[derive(Accounts)]
pub struct TopUpBond<'info> {
    #[account(seeds = [b"config"], bump)]
    pub config: Account<'info, FactoryConfig>,
    #[account(mut, has_one = owner @ FactoryError::Unauthorized)]
    pub party: Account<'info, Party>,
    #[account(mut, seeds = [b"bond_escrow"], bump = config.bond_escrow_bump)]
    pub bond_escrow: SystemAccount<'info>,
    #[account(mut)]
    pub owner: Signer<'info>,
    pub system_program: Program<'info, System>,
}
