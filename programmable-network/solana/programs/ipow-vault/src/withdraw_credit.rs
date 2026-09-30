use anchor_lang::prelude::*;
use anchor_spl::token::{Mint, Token, TokenAccount};

use crate::constants::*;
use crate::errors::VaultError;
use crate::state::{Config, Credit};
use crate::util::{pay_lamports, Veth};

/// Withdraws the caller's lamports and vETH. `to` receives the vETH and is
/// needed only when there is some.
pub fn handler(ctx: Context<WithdrawCredit>) -> Result<()> {
    let lamports = std::mem::take(&mut ctx.accounts.credit.lamports);
    let veth_amount = std::mem::take(&mut ctx.accounts.credit.veth);
    require!(lamports > 0 || veth_amount > 0, VaultError::ZeroAmount);
    let a = &ctx.accounts;
    if lamports > 0 {
        pay_lamports(&a.config.to_account_info(), &a.owner.to_account_info(), lamports)?;
    }
    if veth_amount > 0 {
        let to = a.to.as_ref().ok_or(VaultError::WrongAccount)?;
        let veth = Veth {
            config: &a.config.to_account_info(),
            config_bump: a.config.bump,
            mint: &a.mint.to_account_info(),
            holding: &a.holding.to_account_info(),
            token_program: &a.token_program,
        };
        veth.pay_held(&to.to_account_info(), veth_amount)?;
    }
    Ok(())
}

#[derive(Accounts)]
pub struct WithdrawCredit<'info> {
    #[account(mut, seeds = [CREDIT_SEED, owner.key().as_ref()], bump = credit.bump)]
    pub credit: Account<'info, Credit>,
    #[account(mut, seeds = [CONFIG_SEED], bump = config.bump)]
    pub config: Account<'info, Config>,
    #[account(seeds = [MINT_SEED], bump)]
    pub mint: Account<'info, Mint>,
    #[account(mut, seeds = [HOLDING_SEED], bump)]
    pub holding: Account<'info, TokenAccount>,
    #[account(mut, token::mint = mint)]
    pub to: Option<Account<'info, TokenAccount>>,
    #[account(mut)]
    pub owner: Signer<'info>,
    pub token_program: Program<'info, Token>,
}
