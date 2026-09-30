use anchor_lang::prelude::*;
use anchor_spl::token::{self, Mint, Token, TokenAccount, Transfer};

use crate::constants::*;
use crate::errors::VaultError;
use crate::state::Chain;

/// Adds vETH to the caller's bond (D110).
pub fn handler(ctx: Context<AddBond>, amount: u64) -> Result<()> {
    require!(amount > 0, VaultError::ZeroAmount);
    require!(!ctx.accounts.chain.slashed, VaultError::ChainEnded);
    token::transfer(
        CpiContext::new(
            ctx.accounts.token_program.key(),
            Transfer {
                from: ctx.accounts.from.to_account_info(),
                to: ctx.accounts.holding.to_account_info(),
                authority: ctx.accounts.operator.to_account_info(),
            },
        ),
        amount,
    )?;
    let c = &mut ctx.accounts.chain;
    c.bond = c.bond.checked_add(amount).ok_or(VaultError::Overflow)?;
    Ok(())
}

#[derive(Accounts)]
pub struct AddBond<'info> {
    #[account(mut, seeds = [CHAIN_SEED, operator.key().as_ref()], bump = chain.bump)]
    pub chain: Account<'info, Chain>,
    #[account(seeds = [MINT_SEED], bump)]
    pub mint: Account<'info, Mint>,
    #[account(mut, token::mint = mint, token::authority = operator)]
    pub from: Account<'info, TokenAccount>,
    #[account(mut, seeds = [HOLDING_SEED], bump)]
    pub holding: Account<'info, TokenAccount>,
    pub operator: Signer<'info>,
    pub token_program: Program<'info, Token>,
}
