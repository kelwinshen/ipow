use anchor_lang::prelude::*;
use anchor_spl::token_interface::{Mint, TokenAccount, TokenInterface};
use ipow_protocol::state::Job;

use crate::constants::SWAP_SEED;
use crate::errors::ConversionError;
use crate::jobs::{lock_ended, require_proven};
use crate::money::{pay, Token};
use crate::payment::pays_at_least;
use crate::state::{Side, Swap, SwapState};

/// Sell: pays the operator once its transaction, which pays the user
/// enough, is proven and the proof's lock has ended with no challenge won:
/// a proof on made-up blocks never takes the user's coin. Anyone may call.
pub fn handler(ctx: Context<CompleteSell>, raw_tx: Vec<u8>) -> Result<()> {
    let a = &ctx.accounts;
    require!(a.swap.side == Side::Sell && a.swap.state == SwapState::Open, ConversionError::WrongState);
    require_proven(&a.job, &raw_tx)?;
    require!(lock_ended(&a.job)?, ConversionError::LockNotEnded);
    require!(!a.swap.outside_window(a.job.proof_block.height), ConversionError::PaidOutsideWindow);
    require!(pays_at_least(&raw_tx, &a.swap.script, a.swap.sats)?, ConversionError::NotPaid);
    require_keys_eq!(a.operator.key(), a.job.operator, ConversionError::WrongAccount);
    let token = Token { mint: &a.mint, escrow: &a.escrow, other: &a.to, token_program: &a.token_program, associated_token_program: &None };
    pay(&a.swap, &a.operator.to_account_info(), &token)?;
    ctx.accounts.swap.state = SwapState::Done;
    Ok(())
}

#[derive(Accounts)]
pub struct CompleteSell<'info> {
    #[account(mut, seeds = [SWAP_SEED, &swap.id.to_le_bytes()], bump = swap.bump)]
    pub swap: Account<'info, Swap>,
    #[account(seeds = [b"job", &swap.job_id.to_le_bytes()], bump = job.bump, seeds::program = ipow_protocol::ID)]
    pub job: Account<'info, Job>,
    /// CHECK: the job's operator; checked in the handler.
    #[account(mut)]
    pub operator: UncheckedAccount<'info>,
    pub mint: Option<InterfaceAccount<'info, Mint>>,
    /// CHECK: the swap's associated token account; checked when used.
    #[account(mut)]
    pub escrow: Option<UncheckedAccount<'info>>,
    #[account(mut)]
    pub to: Option<InterfaceAccount<'info, TokenAccount>>,
    pub token_program: Option<Interface<'info, TokenInterface>>,
}
