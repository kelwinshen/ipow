use anchor_lang::prelude::*;
use anchor_spl::token_interface::{Mint, TokenAccount, TokenInterface};
use ipow_light_client::bitcoin::sha256d;
use ipow_protocol::state::Job;

use crate::constants::SWAP_SEED;
use crate::errors::ConversionError;
use crate::jobs::require_proven;
use crate::money::{pay, Token};
use crate::payment::{output_pays, spends};
use crate::state::{Side, Swap, SwapState};

/// Buy: gives the user the coin once the operator's proven transaction
/// spends the user's payment: the receipt. Anyone may call.
pub fn handler(ctx: Context<CompleteBuy>, receipt_raw: Vec<u8>, payment_raw: Vec<u8>, vout: u32) -> Result<()> {
    let a = &ctx.accounts;
    require!(a.swap.side == Side::Buy && a.swap.state == SwapState::Funded, ConversionError::WrongState);
    require_proven(&a.job, &receipt_raw)?;
    require!(spends(&receipt_raw, &sha256d(&payment_raw), vout)?, ConversionError::WrongTransaction);
    require!(output_pays(&payment_raw, vout, &a.swap.script, a.swap.sats)?, ConversionError::NotPaid);
    require_keys_eq!(a.user.key(), a.swap.user, ConversionError::WrongAccount);
    let token = Token { mint: &a.mint, escrow: &a.escrow, other: &a.to, token_program: &a.token_program, associated_token_program: &None };
    pay(&a.swap, &a.user.to_account_info(), &token)?;
    ctx.accounts.swap.state = SwapState::Done;
    Ok(())
}

#[derive(Accounts)]
pub struct CompleteBuy<'info> {
    #[account(mut, seeds = [SWAP_SEED, &swap.id.to_le_bytes()], bump = swap.bump)]
    pub swap: Account<'info, Swap>,
    #[account(seeds = [b"job", &swap.job_id.to_le_bytes()], bump = job.bump, seeds::program = ipow_protocol::ID)]
    pub job: Account<'info, Job>,
    /// CHECK: the swap's user; checked in the handler.
    #[account(mut)]
    pub user: UncheckedAccount<'info>,
    pub mint: Option<InterfaceAccount<'info, Mint>>,
    /// CHECK: the swap's associated token account; checked when used.
    #[account(mut)]
    pub escrow: Option<UncheckedAccount<'info>>,
    #[account(mut)]
    pub to: Option<InterfaceAccount<'info, TokenAccount>>,
    pub token_program: Option<Interface<'info, TokenInterface>>,
}
