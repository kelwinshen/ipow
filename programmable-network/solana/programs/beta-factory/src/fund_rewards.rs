use anchor_lang::prelude::*;

use crate::utils::transfer_from_signer;

/// Anyone (in practice governance) tops up the pool that pays veto rewards.
/// Kept separate from insurance so rewards never eat into the backing of
/// units minted against lies.
pub fn handler(ctx: Context<FundRewards>, amount: u64) -> Result<()> {
    transfer_from_signer(
        amount,
        &ctx.accounts.funder.to_account_info(),
        &ctx.accounts.reward_pool.to_account_info(),
        &ctx.accounts.system_program.to_account_info(),
    )
}

#[derive(Accounts)]
pub struct FundRewards<'info> {
    #[account(mut, seeds = [b"rewards"], bump)]
    pub reward_pool: SystemAccount<'info>,
    #[account(mut)]
    pub funder: Signer<'info>,
    pub system_program: Program<'info, System>,
}
