use anchor_lang::prelude::*;
use anchor_spl::associated_token::AssociatedToken;
use anchor_spl::token::{Mint, Token, TokenAccount};

use crate::errors::FactoryError;
use crate::state::{FactoryConfig, Pending};
use crate::utils::transfer_from_signer;

/// User X locks the SOL half of `units` BETA, plus an optional
/// `attest_fee` — what they're willing to pay whoever accelerates this
/// mint instead of the free week-long path (DESIGN_V2 §7). Also creates
/// X's BETA token account now so `exercise_mint` later needs no `init`.
pub fn handler(ctx: Context<LockSol>, nonce: u64, units: u64, deadline: i64, attest_fee: u64) -> Result<()> {
    require!(!ctx.accounts.config.paused, FactoryError::Paused);
    require!(units > 0, FactoryError::InvalidParams);
    let now = Clock::get()?.unix_timestamp;
    require!(deadline > now, FactoryError::InvalidParams);
    let lamports = units
        .checked_mul(ctx.accounts.config.params.sol_per_unit)
        .ok_or(FactoryError::Overflow)?;
    transfer_from_signer(
        lamports,
        &ctx.accounts.user.to_account_info(),
        &ctx.accounts.vault.to_account_info(),
        &ctx.accounts.system_program.to_account_info(),
    )?;
    transfer_from_signer(
        attest_fee,
        &ctx.accounts.user.to_account_info(),
        &ctx.accounts.fees.to_account_info(),
        &ctx.accounts.system_program.to_account_info(),
    )?;
    let c = &mut ctx.accounts.config;
    c.pending_lamports = c.pending_lamports.checked_add(lamports).ok_or(FactoryError::Overflow)?;
    let p = &mut ctx.accounts.pending;
    p.user = ctx.accounts.user.key();
    p.nonce = nonce;
    p.units = units;
    p.deadline = deadline;
    p.eth_lock_id = 0;
    p.approved = false;
    p.queued = false;
    p.queued_by = [0u8; 32];
    p.attest_fee = attest_fee;
    p.created_at = now;
    Ok(())
}

#[derive(Accounts)]
#[instruction(nonce: u64)]
pub struct LockSol<'info> {
    #[account(mut, seeds = [b"config"], bump)]
    pub config: Account<'info, FactoryConfig>,
    #[account(init, payer = user, space = 8 + Pending::INIT_SPACE, seeds = [b"pending", user.key().as_ref(), nonce.to_le_bytes().as_ref()], bump)]
    pub pending: Account<'info, Pending>,
    #[account(mut, seeds = [b"vault"], bump = config.vault_bump)]
    pub vault: SystemAccount<'info>,
    /// Holds every posted-but-not-yet-paid-or-refunded acceleration fee.
    #[account(mut, seeds = [b"fees"], bump)]
    pub fees: SystemAccount<'info>,
    #[account(address = config.beta_mint)]
    pub beta_mint: Account<'info, Mint>,
    #[account(init_if_needed, payer = user, associated_token::mint = beta_mint, associated_token::authority = user)]
    pub user_beta: Account<'info, TokenAccount>,
    #[account(mut)]
    pub user: Signer<'info>,
    pub token_program: Program<'info, Token>,
    pub associated_token_program: Program<'info, AssociatedToken>,
    pub system_program: Program<'info, System>,
}
