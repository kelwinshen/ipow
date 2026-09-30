use anchor_lang::prelude::*;
use anchor_spl::token_interface::{Mint, TokenAccount, TokenInterface};
use ipow_protocol::duty::{deadline, status, Status};
use ipow_protocol::state::Job;

use crate::constants::{CLOSE_PERIOD, PAY_BLOCKS, SWAP_SEED};
use crate::errors::ConversionError;
use crate::jobs::{lock_ended, now};
use crate::money::{pay, Token};
use crate::state::{Side, Swap, SwapState};

/// Buy: gives the operator its coin back when the user did not pay. Either
/// its close was mined after the payment blocks and its lock has ended with
/// no challenge won, or 36 hours have passed since the job's deadline.
/// Until then the user may still prove a payment. Anyone may call.
pub fn handler(ctx: Context<Reclaim>) -> Result<()> {
    let a = &ctx.accounts;
    require!(a.swap.side == Side::Buy && a.swap.state == SwapState::Funded, ConversionError::WrongState);
    let job = &a.job;
    let t = now()?;
    let s = status(job, t);
    let closed = (s == Status::Proven || s == Status::Settled)
        && job.proof_block.height > job.anchor.height + PAY_BLOCKS
        && lock_ended(job)?;
    let late = t >= deadline(job)? + CLOSE_PERIOD as i64;
    require!(closed || late, ConversionError::NotReclaimable);
    require_keys_eq!(a.operator.key(), job.operator, ConversionError::WrongAccount);
    let token = Token { mint: &a.mint, escrow: &a.escrow, other: &a.to, token_program: &a.token_program, associated_token_program: &None };
    pay(&a.swap, &a.operator.to_account_info(), &token)?;
    ctx.accounts.swap.state = SwapState::Reclaimed;
    Ok(())
}

#[derive(Accounts)]
pub struct Reclaim<'info> {
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
