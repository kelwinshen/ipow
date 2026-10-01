use anchor_lang::prelude::*;
use anchor_spl::token::{Mint, Token, TokenAccount};

use crate::constants::*;
use crate::errors::VaultError;
use crate::state::{Config, Request};
use crate::util::Receipt;

/// The operator of the first true message carrying a burn here takes its
/// fee, in receipts (D113).
pub fn handler(ctx: Context<TakeRequestFee>, _request_id: u64) -> Result<()> {
    let r = &mut ctx.accounts.request;
    require!(r.fee_paid && !r.fee_taken && r.fee_to == ctx.accounts.operator.key() && r.fee != 0, VaultError::NothingToCollect);
    r.fee_taken = true;
    let fee = r.fee;
    let a = &ctx.accounts;
    let receipt = Receipt {
        config: &a.config.to_account_info(),
        config_peer: a.config.peer,
        config_bump: a.config.bump,
        mint: &a.mint.to_account_info(),
        holding: &a.holding.to_account_info(),
        token_program: &a.token_program,
    };
    receipt.pay_held(&a.to.to_account_info(), fee)
}

#[derive(Accounts)]
#[instruction(request_id: u64)]
pub struct TakeRequestFee<'info> {
    #[account(seeds = [CONFIG_SEED, &[config.peer]], bump = config.bump)]
    pub config: Box<Account<'info, Config>>,
    #[account(mut, seeds = [REQUEST_SEED, config.key().as_ref(), &request_id.to_le_bytes()], bump = request.bump)]
    pub request: Box<Account<'info, Request>>,
    #[account(seeds = [RECEIPT_SEED, config.key().as_ref(), &request.asset.to_le_bytes()], bump)]
    pub mint: Box<Account<'info, Mint>>,
    #[account(mut, seeds = [HOLDING_SEED, config.key().as_ref(), &request.asset.to_le_bytes()], bump)]
    pub holding: Box<Account<'info, TokenAccount>>,
    #[account(mut, token::mint = mint)]
    pub to: Box<Account<'info, TokenAccount>>,
    pub operator: Signer<'info>,
    pub token_program: Program<'info, Token>,
}
