use anchor_lang::prelude::*;
use anchor_spl::token::{self, Burn, Mint, Token, TokenAccount, Transfer};

use crate::constants::*;
use crate::errors::VaultError;
use crate::state::{Config, Request};

/// Burns vETH for ETH on Ethereum: request number `request_count + 1`. The
/// fee, in vETH, goes to the first operator whose message carrying the
/// request is judged true here (D113). Never to address zero.
pub fn handler(ctx: Context<MakeRequest>, amount: u64, to: [u8; 20], fee: u64) -> Result<()> {
    require!(amount > 0, VaultError::ZeroAmount);
    require!(to != [0u8; 20], VaultError::ZeroAddress);
    let a = &ctx.accounts;
    token::burn(
        CpiContext::new(
            a.token_program.key(),
            Burn { mint: a.mint.to_account_info(), from: a.from.to_account_info(), authority: a.user.to_account_info() },
        ),
        amount,
    )?;
    if fee > 0 {
        token::transfer(
            CpiContext::new(
                a.token_program.key(),
                Transfer { from: a.from.to_account_info(), to: a.holding.to_account_info(), authority: a.user.to_account_info() },
            ),
            fee,
        )?;
    }
    let id = ctx.accounts.config.request_count + 1;
    ctx.accounts.config.request_count = id;
    let r = &mut ctx.accounts.request;
    r.id = id;
    r.owner = ctx.accounts.user.key();
    r.amount = amount;
    r.to = to;
    r.fee = fee;
    r.bump = ctx.bumps.request;
    Ok(())
}

#[derive(Accounts)]
pub struct MakeRequest<'info> {
    #[account(mut, seeds = [CONFIG_SEED], bump = config.bump)]
    pub config: Account<'info, Config>,
    #[account(
        init,
        payer = user,
        space = 8 + Request::INIT_SPACE,
        seeds = [REQUEST_SEED, &(config.request_count + 1).to_le_bytes()],
        bump
    )]
    pub request: Account<'info, Request>,
    #[account(mut, seeds = [MINT_SEED], bump)]
    pub mint: Account<'info, Mint>,
    #[account(mut, token::mint = mint, token::authority = user)]
    pub from: Account<'info, TokenAccount>,
    #[account(mut, seeds = [HOLDING_SEED], bump)]
    pub holding: Account<'info, TokenAccount>,
    #[account(mut)]
    pub user: Signer<'info>,
    pub token_program: Program<'info, Token>,
    pub system_program: Program<'info, System>,
}
