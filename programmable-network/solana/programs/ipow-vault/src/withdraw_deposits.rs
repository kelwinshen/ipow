use anchor_lang::prelude::*;

use crate::constants::*;
use crate::errors::VaultError;
use crate::state::{Chain, Config};
use crate::util::pay_lamports;

/// Withdraws deposit lamports not put down in a claim.
pub fn handler(ctx: Context<WithdrawDeposits>, amount: u64) -> Result<()> {
    require!(amount > 0, VaultError::ZeroAmount);
    let c = &mut ctx.accounts.chain;
    require!(amount <= c.deposits, VaultError::BondNotFree);
    c.deposits -= amount;
    pay_lamports(&ctx.accounts.config.to_account_info(), &ctx.accounts.operator.to_account_info(), amount)
}

#[derive(Accounts)]
pub struct WithdrawDeposits<'info> {
    #[account(mut, seeds = [CHAIN_SEED, operator.key().as_ref()], bump = chain.bump)]
    pub chain: Account<'info, Chain>,
    #[account(mut, seeds = [CONFIG_SEED], bump = config.bump)]
    pub config: Account<'info, Config>,
    #[account(mut)]
    pub operator: Signer<'info>,
}
