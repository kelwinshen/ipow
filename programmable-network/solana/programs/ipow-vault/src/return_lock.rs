use anchor_lang::prelude::*;
use anchor_spl::token_interface::{Mint, TokenAccount, TokenInterface};

use crate::constants::*;
use crate::errors::VaultError;
use crate::state::{Claim, Config, HomeAsset, HomeLock};
use crate::util::{pay_home, u64_at};

/// Returns a lock here whose CANCEL an accepted claim carries, with its fast
/// fee, and its fee if no message earned it: lamports to its owner, or
/// tokens to its owner's account `to`. Anyone may call.
pub fn handler<'info>(ctx: Context<'info, ReturnLock<'info>>, _claim_id: u64, _lock_id: u64, record: Vec<u8>) -> Result<()> {
    require!(record.len() == CANCEL_LEN && record[0] == CANCEL && record[1] == ETHEREUM, VaultError::WrongRecord);
    let claim = &ctx.accounts.claim;
    require!(claim.accepted && claim.carries(&record), VaultError::NotAccepted);
    let l = &mut ctx.accounts.lock;
    require!(u64_at(&record, 2) == l.id, VaultError::WrongRecord);
    require!(!l.returned, VaultError::AlreadyDone);
    let backing = l.amount.checked_add(l.fast_fee).ok_or(VaultError::Overflow)?;
    let a = &mut ctx.accounts.asset;
    require!(backing <= a.reserve, VaultError::Underfunded);
    a.reserve -= backing;
    l.returned = true;
    let mut total = backing;
    if !l.fee_paid {
        // The fee is settled: no later message can earn it.
        l.fee_paid = true;
        l.fee_taken = true;
        total = total.checked_add(l.fee).ok_or(VaultError::Overflow)?;
    }
    let a = &ctx.accounts;
    require!(a.asset.number != 0 || a.to.is_none(), VaultError::WrongAccount);
    let to = match &a.to {
        Some(t) => {
            require_keys_eq!(t.owner, a.lock.owner, VaultError::WrongAccount);
            t.to_account_info()
        }
        None => {
            require_keys_eq!(a.owner.key(), a.lock.owner, VaultError::WrongAccount);
            a.owner.to_account_info()
        }
    };
    pay_home(
        &a.asset,
        total,
        &a.config.to_account_info(),
        a.config.bump,
        &to,
        a.tokens.as_ref().map(|t| t.to_account_info()).as_ref(),
        a.mint.as_ref().map(|m| m.to_account_info()).as_ref(),
        a.token_program.as_ref().map(|p| p.to_account_info()).as_ref(),
    )
}

#[derive(Accounts)]
#[instruction(claim_id: u64, lock_id: u64)]
pub struct ReturnLock<'info> {
    #[account(mut, seeds = [CONFIG_SEED], bump = config.bump)]
    pub config: Box<Account<'info, Config>>,
    #[account(seeds = [CLAIM_SEED, &claim_id.to_le_bytes()], bump = claim.bump)]
    pub claim: Box<Account<'info, Claim>>,
    #[account(mut, seeds = [HOME_LOCK_SEED, &lock_id.to_le_bytes()], bump = lock.bump)]
    pub lock: Box<Account<'info, HomeLock>>,
    #[account(mut, seeds = [ASSET_SEED, &lock.asset.to_le_bytes()], bump = asset.bump)]
    pub asset: Box<Account<'info, HomeAsset>>,
    /// CHECK: the lock's owner, for SOL; checked in the handler.
    #[account(mut)]
    pub owner: UncheckedAccount<'info>,
    #[account(mut)]
    pub to: Option<Box<InterfaceAccount<'info, TokenAccount>>>,
    #[account(mut, seeds = [HOME_TOKENS_SEED, asset.mint.as_ref()], bump)]
    pub tokens: Option<Box<InterfaceAccount<'info, TokenAccount>>>,
    pub mint: Option<Box<InterfaceAccount<'info, Mint>>>,
    pub token_program: Option<Interface<'info, TokenInterface>>,
}
