use anchor_lang::prelude::*;

use crate::errors::FactoryError;
use crate::state::{FactoryConfig, Party, Pending};
use crate::utils::transfer_from_pda;

/// Permissionless. Past the deadline and not queued: the SOL goes back to
/// X and the account closes to X.
pub fn handler(ctx: Context<ExpirePending>, _nonce: u64) -> Result<()> {
    let p = &ctx.accounts.pending;
    if p.queued {
        // A queued slot may only be cancelled if the party that queued it
        // has been retired (its anchor can never be exercised).
        let prior = ctx.accounts.prior_party.as_ref().ok_or(FactoryError::PendingQueued)?;
        require!(prior.party_id == p.queued_by && prior.dead, FactoryError::PendingQueued);
    }
    let now = Clock::get()?.unix_timestamp;
    require!(now > p.deadline, FactoryError::PendingNotExpired);
    let lamports = p
        .units
        .checked_mul(ctx.accounts.config.params.sol_per_unit)
        .ok_or(FactoryError::Overflow)?;
    let c = &mut ctx.accounts.config;
    c.pending_lamports = c.pending_lamports.checked_sub(lamports).ok_or(FactoryError::Overflow)?;
    let bump = c.vault_bump;
    let seeds: &[&[u8]] = &[b"vault", &[bump]];
    transfer_from_pda(
        lamports,
        &ctx.accounts.vault.to_account_info(),
        &ctx.accounts.user.to_account_info(),
        &ctx.accounts.system_program.to_account_info(),
        &[seeds],
    )
}

#[derive(Accounts)]
#[instruction(nonce: u64)]
pub struct ExpirePending<'info> {
    #[account(mut, seeds = [b"config"], bump)]
    pub config: Account<'info, FactoryConfig>,
    #[account(mut, close = user, seeds = [b"pending", user.key().as_ref(), nonce.to_le_bytes().as_ref()], bump, has_one = user)]
    pub pending: Account<'info, Pending>,
    #[account(mut, seeds = [b"vault"], bump = config.vault_bump)]
    pub vault: SystemAccount<'info>,
    /// CHECK: constrained by `pending.user` via `has_one`.
    #[account(mut)]
    pub user: UncheckedAccount<'info>,
    pub system_program: Program<'info, System>,
    /// Required only to cancel a queued slot whose queuing party is dead.
    pub prior_party: Option<Account<'info, Party>>,
}
