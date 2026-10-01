use anchor_lang::prelude::*;
use anchor_spl::token::{self, Burn, Mint, Token, TokenAccount, Transfer};

use crate::constants::*;
use crate::errors::VaultError;
use crate::state::{Config, Request};
use crate::util::{is_eth_address, now};

/// Burns receipts of an asset of Ethereum for the asset there: request
/// number `request_count + 1`. The fee, in receipts, goes to the first
/// operator whose message carrying the request is judged true here (D113).
/// The fast fee is burned with the amount and paid on Ethereum, to an
/// attester who paid at once, or to `to` (D122). `to` is an Ethereum address
/// in 32 bytes.
pub fn handler(ctx: Context<MakeRequest>, asset: u32, amount: u64, to: [u8; 32], fee: u64, fast_fee: u64) -> Result<()> {
    require!(amount > 0, VaultError::ZeroAmount);
    require!(is_eth_address(&to), VaultError::ZeroAddress);
    let a = &ctx.accounts;
    token::burn(
        CpiContext::new(
            a.token_program.key(),
            Burn { mint: a.mint.to_account_info(), from: a.from.to_account_info(), authority: a.user.to_account_info() },
        ),
        amount.checked_add(fast_fee).ok_or(VaultError::Overflow)?,
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
    r.asset = asset;
    r.owner = ctx.accounts.user.key();
    r.amount = amount;
    r.to = to;
    r.fee = fee;
    r.fast_fee = fast_fee;
    r.requested_at = now()?;
    r.fee_to = Pubkey::default();
    r.fee_paid = false;
    r.fee_taken = false;
    r.bump = ctx.bumps.request;
    Ok(())
}

#[derive(Accounts)]
#[instruction(asset: u32)]
pub struct MakeRequest<'info> {
    #[account(mut, seeds = [CONFIG_SEED], bump = config.bump)]
    pub config: Box<Account<'info, Config>>,
    #[account(
        init,
        payer = user,
        space = 8 + Request::INIT_SPACE,
        seeds = [REQUEST_SEED, &(config.request_count + 1).to_le_bytes()],
        bump
    )]
    pub request: Box<Account<'info, Request>>,
    #[account(mut, seeds = [RECEIPT_SEED, &asset.to_le_bytes()], bump)]
    pub mint: Box<Account<'info, Mint>>,
    #[account(mut, token::mint = mint, token::authority = user)]
    pub from: Box<Account<'info, TokenAccount>>,
    #[account(mut, seeds = [HOLDING_SEED, &asset.to_le_bytes()], bump)]
    pub holding: Box<Account<'info, TokenAccount>>,
    #[account(mut)]
    pub user: Signer<'info>,
    pub token_program: Program<'info, Token>,
    pub system_program: Program<'info, System>,
}
