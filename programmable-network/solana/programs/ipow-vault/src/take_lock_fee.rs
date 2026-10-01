use anchor_lang::prelude::*;
use anchor_spl::token_interface::{Mint, TokenAccount, TokenInterface};

use crate::constants::*;
use crate::errors::VaultError;
use crate::state::{Config, HomeAsset, HomeLock};
use crate::util::pay_home;

/// The operator of the first true message carrying a lock here takes its
/// fee (D113): lamports to itself, or tokens to its account `to`.
pub fn handler<'info>(ctx: Context<'info, TakeLockFee<'info>>, _lock_id: u64) -> Result<()> {
    let l = &mut ctx.accounts.lock;
    require!(l.fee_paid && !l.fee_taken && l.fee_to == ctx.accounts.operator.key() && l.fee != 0, VaultError::NothingToCollect);
    l.fee_taken = true;
    let fee = l.fee;
    let a = &ctx.accounts;
    let to = match &a.to {
        Some(t) => t.to_account_info(),
        None => a.operator.to_account_info(),
    };
    pay_home(
        &a.asset,
        fee,
        &a.config.to_account_info(),
        a.config.peer,
        a.config.bump,
        &to,
        a.tokens.as_ref().map(|t| t.to_account_info()).as_ref(),
        a.mint.as_ref().map(|m| m.to_account_info()).as_ref(),
        a.token_program.as_ref().map(|p| p.to_account_info()).as_ref(),
    )
}

#[derive(Accounts)]
#[instruction(lock_id: u64)]
pub struct TakeLockFee<'info> {
    #[account(mut, seeds = [CONFIG_SEED, &[config.peer]], bump = config.bump)]
    pub config: Box<Account<'info, Config>>,
    #[account(mut, seeds = [HOME_LOCK_SEED, config.key().as_ref(), &lock_id.to_le_bytes()], bump = lock.bump)]
    pub lock: Box<Account<'info, HomeLock>>,
    #[account(seeds = [ASSET_SEED, config.key().as_ref(), &lock.asset.to_le_bytes()], bump = asset.bump)]
    pub asset: Box<Account<'info, HomeAsset>>,
    #[account(mut, token::authority = operator)]
    pub to: Option<Box<InterfaceAccount<'info, TokenAccount>>>,
    #[account(mut, seeds = [HOME_TOKENS_SEED, config.key().as_ref(), asset.mint.as_ref()], bump)]
    pub tokens: Option<Box<InterfaceAccount<'info, TokenAccount>>>,
    pub mint: Option<Box<InterfaceAccount<'info, Mint>>>,
    pub token_program: Option<Interface<'info, TokenInterface>>,
    #[account(mut)]
    pub operator: Signer<'info>,
}
