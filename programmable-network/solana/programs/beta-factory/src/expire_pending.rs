use anchor_lang::prelude::*;
use anchor_spl::associated_token;
use anchor_spl::token;

use crate::errors::FactoryError;
use crate::state::{Composition, FactoryConfig, Party, Pending};
use crate::utils::transfer_from_pda;

/// Permissionless. Past the deadline and no operator has claimed this
/// mint (`queued_by` still zero): every local leg (native SOL and/or SPL
/// tokens, §8.10) goes back to X and the account closes to X.
pub fn handler(ctx: Context<ExpirePending>, _nonce: u64) -> Result<()> {
    let p = &ctx.accounts.pending;
    if p.queued_by != [0u8; 32] {
        // Claimed by an operator: may only be cancelled if that operator
        // has since been retired (its anchors can never be exercised).
        let prior = ctx.accounts.prior_party.as_ref().ok_or(FactoryError::PendingQueued)?;
        require!(prior.party_id == p.queued_by && prior.dead, FactoryError::PendingQueued);
    }
    let now = Clock::get()?.unix_timestamp;
    require!(now > p.deadline, FactoryError::PendingNotExpired);
    let comp = &ctx.accounts.composition;
    require!(comp.id == p.composition_id, FactoryError::AccountMismatch);
    let locals: Vec<_> = comp.components.iter().filter(|c| c.network_id == 0).copied().collect();
    require!(!locals.is_empty(), FactoryError::InvalidParams);

    let bump = ctx.accounts.config.vault_bump;
    let vault_seeds: &[&[u8]] = &[b"vault", &[bump]];
    let authority_bump = ctx.bumps.token_vault_authority;
    let authority_seeds: &[&[u8]] = &[b"token_vault_authority", &[authority_bump]];

    let spl_mints = [&ctx.accounts.local_spl_0_mint, &ctx.accounts.local_spl_1_mint, &ctx.accounts.local_spl_2_mint];
    let spl_users = [&ctx.accounts.local_spl_0_user, &ctx.accounts.local_spl_1_user, &ctx.accounts.local_spl_2_user];
    let spl_vaults = [&ctx.accounts.local_spl_0_vault, &ctx.accounts.local_spl_1_vault, &ctx.accounts.local_spl_2_vault];
    let mut native_lamports: u64 = 0;
    let mut spl_slot = 0usize;
    for c in &locals {
        let amount = p.units.checked_mul(c.amount_per_unit).ok_or(FactoryError::Overflow)?;
        if c.token_id == [0u8; 32] {
            native_lamports = native_lamports.checked_add(amount).ok_or(FactoryError::Overflow)?;
        } else {
            require!(spl_slot < 3, FactoryError::InvalidParams);
            let mint_ai = spl_mints[spl_slot].as_ref().ok_or(FactoryError::MissingAccount)?;
            let user_ai = spl_users[spl_slot].as_ref().ok_or(FactoryError::MissingAccount)?;
            let vault_ai = spl_vaults[spl_slot].as_ref().ok_or(FactoryError::MissingAccount)?;
            require!(mint_ai.key() == Pubkey::new_from_array(c.token_id), FactoryError::AccountMismatch);
            require!(
                user_ai.key() == associated_token::get_associated_token_address(&ctx.accounts.user.key(), &mint_ai.key()),
                FactoryError::AccountMismatch
            );
            require!(
                vault_ai.key() == associated_token::get_associated_token_address(&ctx.accounts.token_vault_authority.key(), &mint_ai.key()),
                FactoryError::AccountMismatch
            );
            token::transfer(
                CpiContext::new_with_signer(
                    ctx.accounts.token_program.key(),
                    token::Transfer {
                        from: vault_ai.to_account_info(),
                        to: user_ai.to_account_info(),
                        authority: ctx.accounts.token_vault_authority.to_account_info(),
                    },
                    &[authority_seeds],
                ),
                amount,
            )?;
            spl_slot += 1;
        }
    }

    let c = &mut ctx.accounts.config;
    c.pending_lamports = c.pending_lamports.checked_sub(native_lamports).ok_or(FactoryError::Overflow)?;
    transfer_from_pda(
        native_lamports,
        &ctx.accounts.vault.to_account_info(),
        &ctx.accounts.user.to_account_info(),
        &ctx.accounts.system_program.to_account_info(),
        &[vault_seeds],
    )
}

#[derive(Accounts)]
#[instruction(nonce: u64)]
pub struct ExpirePending<'info> {
    #[account(mut, seeds = [b"config"], bump)]
    pub config: Account<'info, FactoryConfig>,
    #[account(mut, close = user, seeds = [b"pending", user.key().as_ref(), nonce.to_le_bytes().as_ref()], bump, has_one = user)]
    pub pending: Account<'info, Pending>,
    pub composition: Account<'info, Composition>,
    #[account(mut, seeds = [b"vault"], bump = config.vault_bump)]
    pub vault: SystemAccount<'info>,
    /// CHECK: constrained by `pending.user` via `has_one`.
    #[account(mut)]
    pub user: UncheckedAccount<'info>,
    pub token_program: Program<'info, anchor_spl::token::Token>,
    pub system_program: Program<'info, System>,
    /// Required only to cancel a slot some operator already claimed.
    pub prior_party: Option<Account<'info, Party>>,

    /// CHECK: owning authority of every SPL local-leg vault ATA; signs
    /// the refund transfer back to the user.
    #[account(seeds = [b"token_vault_authority"], bump)]
    pub token_vault_authority: UncheckedAccount<'info>,
    // --- up to MAX_LOCAL_COMPONENTS - 1 non-native local (Solana) legs,
    // same composition-order convention as `lock_sol`. ---
    /// CHECK: verified against the composition's declared local component in the handler.
    pub local_spl_0_mint: Option<UncheckedAccount<'info>>,
    /// CHECK: verified to be the user's ATA for `local_spl_0_mint`.
    #[account(mut)]
    pub local_spl_0_user: Option<UncheckedAccount<'info>>,
    /// CHECK: verified to be the vault's ATA for `local_spl_0_mint`.
    #[account(mut)]
    pub local_spl_0_vault: Option<UncheckedAccount<'info>>,
    /// CHECK: verified against the composition's declared local component in the handler.
    pub local_spl_1_mint: Option<UncheckedAccount<'info>>,
    /// CHECK: verified to be the user's ATA for `local_spl_1_mint`.
    #[account(mut)]
    pub local_spl_1_user: Option<UncheckedAccount<'info>>,
    /// CHECK: verified to be the vault's ATA for `local_spl_1_mint`.
    #[account(mut)]
    pub local_spl_1_vault: Option<UncheckedAccount<'info>>,
    /// CHECK: verified against the composition's declared local component in the handler.
    pub local_spl_2_mint: Option<UncheckedAccount<'info>>,
    /// CHECK: verified to be the user's ATA for `local_spl_2_mint`.
    #[account(mut)]
    pub local_spl_2_user: Option<UncheckedAccount<'info>>,
    /// CHECK: verified to be the vault's ATA for `local_spl_2_mint`.
    #[account(mut)]
    pub local_spl_2_vault: Option<UncheckedAccount<'info>>,
}
