use anchor_lang::prelude::*;
use anchor_spl::token::{Mint, Token, TokenAccount};

use crate::constants::*;
use crate::errors::VaultError;
use crate::state::{Chain, Config};
use crate::util::Veth;

/// Withdraws free bond: all of it once the chain has exited and every claim
/// of it here has ended (D112).
pub fn handler(ctx: Context<WithdrawBond>, amount: u64) -> Result<()> {
    require!(amount > 0, VaultError::ZeroAmount);
    let c = &mut ctx.accounts.chain;
    let free = if c.exited && c.open_claims == 0 { c.bond } else { c.bond - c.stated };
    require!(amount <= free, VaultError::BondNotFree);
    c.bond -= amount;
    if c.stated > c.bond {
        c.stated = c.bond;
    }
    let a = &ctx.accounts;
    let veth = Veth {
        config: &a.config.to_account_info(),
        config_bump: a.config.bump,
        mint: &a.mint.to_account_info(),
        holding: &a.holding.to_account_info(),
        token_program: &a.token_program,
    };
    veth.pay_held(&a.to.to_account_info(), amount)
}

#[derive(Accounts)]
pub struct WithdrawBond<'info> {
    #[account(mut, seeds = [CHAIN_SEED, operator.key().as_ref()], bump = chain.bump)]
    pub chain: Account<'info, Chain>,
    #[account(seeds = [CONFIG_SEED], bump = config.bump)]
    pub config: Account<'info, Config>,
    #[account(seeds = [MINT_SEED], bump)]
    pub mint: Account<'info, Mint>,
    #[account(mut, seeds = [HOLDING_SEED], bump)]
    pub holding: Account<'info, TokenAccount>,
    #[account(mut, token::mint = mint)]
    pub to: Account<'info, TokenAccount>,
    pub operator: Signer<'info>,
    pub token_program: Program<'info, Token>,
}
