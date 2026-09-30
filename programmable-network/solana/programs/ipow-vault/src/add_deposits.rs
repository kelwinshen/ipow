use anchor_lang::prelude::*;
use anchor_lang::system_program::{transfer, Transfer};

use crate::constants::*;
use crate::errors::VaultError;
use crate::state::{Chain, Config};

/// Adds lamports for the flat deposits of the caller's claims, apart from
/// its bond (section 11.2).
pub fn handler(ctx: Context<AddDeposits>, amount: u64) -> Result<()> {
    require!(amount > 0, VaultError::ZeroAmount);
    transfer(
        CpiContext::new(
            ctx.accounts.system_program.key(),
            Transfer { from: ctx.accounts.operator.to_account_info(), to: ctx.accounts.config.to_account_info() },
        ),
        amount,
    )?;
    let c = &mut ctx.accounts.chain;
    c.deposits = c.deposits.checked_add(amount).ok_or(VaultError::Overflow)?;
    Ok(())
}

#[derive(Accounts)]
pub struct AddDeposits<'info> {
    #[account(mut, seeds = [CHAIN_SEED, operator.key().as_ref()], bump = chain.bump)]
    pub chain: Account<'info, Chain>,
    #[account(mut, seeds = [CONFIG_SEED], bump = config.bump)]
    pub config: Account<'info, Config>,
    #[account(mut)]
    pub operator: Signer<'info>,
    pub system_program: Program<'info, System>,
}
