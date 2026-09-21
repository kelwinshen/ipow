use anchor_lang::prelude::*;

use crate::errors::FactoryError;
use crate::state::{FactoryConfig, Party};
use crate::utils::transfer_from_pda;

/// After the unbond delay the whole remaining bond leaves and the party is
/// retired (`dead`), so a withdrawn party can't keep making statements
/// with nothing at stake.
pub fn handler(ctx: Context<WithdrawBond>) -> Result<()> {
    let party = &mut ctx.accounts.party;
    require!(!party.dead, FactoryError::PartyDead);
    require!(party.unbond_requested_at > 0, FactoryError::UnbondNotRequested);
    let now = Clock::get()?.unix_timestamp;
    require!(
        now >= party.unbond_requested_at + ctx.accounts.config.params.unbond_delay_secs,
        FactoryError::UnbondNotReady
    );
    let amount = party.bond;
    party.bond = 0;
    party.dead = true;
    let bump = ctx.accounts.config.bond_escrow_bump;
    let seeds: &[&[u8]] = &[b"bond_escrow", &[bump]];
    transfer_from_pda(
        amount,
        &ctx.accounts.bond_escrow.to_account_info(),
        &ctx.accounts.owner.to_account_info(),
        &ctx.accounts.system_program.to_account_info(),
        &[seeds],
    )
}

#[derive(Accounts)]
pub struct WithdrawBond<'info> {
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
