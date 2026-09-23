use anchor_lang::prelude::*;
use anchor_spl::associated_token::{self, AssociatedToken};
use anchor_spl::token::{self, Mint, Token, TokenAccount};

use crate::errors::FactoryError;
use crate::state::{Composition, FactoryConfig, Pending};
use crate::utils::transfer_from_signer;

/// User X locks every local (Solana) leg of `units` BETA against a
/// registered `composition_id` (DESIGN_V2 §8/§8.12) — native SOL if the
/// composition names it, plus an SPL transfer per local non-native token
/// (up to `MAX_LOCAL_COMPONENTS - 1` of them, `local_spl_0/1/2`, in
/// composition order) — plus an optional `attest_fee`, what they're
/// willing to pay whoever accelerates the mint instead of the free
/// week-long path (§7). Also creates X's BETA token account now so
/// `exercise_mint` later needs no `init`.
pub fn handler(ctx: Context<LockSol>, nonce: u64, composition_id: u64, units: u64, deadline: i64, attest_fee: u64) -> Result<()> {
    require!(!ctx.accounts.config.paused, FactoryError::Paused);
    require!(units > 0, FactoryError::InvalidParams);
    let now = Clock::get()?.unix_timestamp;
    require!(deadline > now, FactoryError::InvalidParams);
    let comp = &ctx.accounts.composition;
    require!(comp.id == composition_id, FactoryError::AccountMismatch);

    let locals: Vec<_> = comp.components.iter().filter(|c| c.network_id == 0).copied().collect();
    require!(!locals.is_empty(), FactoryError::InvalidParams);
    let remote_count = comp.components.len() - locals.len();

    let spl_mints = [&ctx.accounts.local_spl_0_mint, &ctx.accounts.local_spl_1_mint, &ctx.accounts.local_spl_2_mint];
    let spl_users = [&ctx.accounts.local_spl_0_user, &ctx.accounts.local_spl_1_user, &ctx.accounts.local_spl_2_user];
    let spl_vaults = [&ctx.accounts.local_spl_0_vault, &ctx.accounts.local_spl_1_vault, &ctx.accounts.local_spl_2_vault];
    let mut native_lamports: u64 = 0;
    let mut spl_slot = 0usize;
    for c in &locals {
        let amount = units.checked_mul(c.amount_per_unit).ok_or(FactoryError::Overflow)?;
        if c.token_id == [0u8; 32] {
            transfer_from_signer(
                amount,
                &ctx.accounts.user.to_account_info(),
                &ctx.accounts.vault.to_account_info(),
                &ctx.accounts.system_program.to_account_info(),
            )?;
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
            associated_token::create_idempotent(CpiContext::new(
                ctx.accounts.associated_token_program.key(),
                associated_token::Create {
                    payer: ctx.accounts.user.to_account_info(),
                    associated_token: vault_ai.to_account_info(),
                    authority: ctx.accounts.token_vault_authority.to_account_info(),
                    mint: mint_ai.to_account_info(),
                    system_program: ctx.accounts.system_program.to_account_info(),
                    token_program: ctx.accounts.token_program.to_account_info(),
                },
            ))?;
            token::transfer(
                CpiContext::new(
                    ctx.accounts.token_program.key(),
                    token::Transfer {
                        from: user_ai.to_account_info(),
                        to: vault_ai.to_account_info(),
                        authority: ctx.accounts.user.to_account_info(),
                    },
                ),
                amount,
            )?;
            spl_slot += 1;
        }
    }

    transfer_from_signer(
        attest_fee,
        &ctx.accounts.user.to_account_info(),
        &ctx.accounts.fees.to_account_info(),
        &ctx.accounts.system_program.to_account_info(),
    )?;
    let c = &mut ctx.accounts.config;
    c.pending_lamports = c.pending_lamports.checked_add(native_lamports).ok_or(FactoryError::Overflow)?;
    let p = &mut ctx.accounts.pending;
    p.user = ctx.accounts.user.key();
    p.nonce = nonce;
    p.composition_id = composition_id;
    p.units = units;
    p.deadline = deadline;
    p.approved = false;
    p.remote_lock_id = vec![0u64; remote_count];
    p.remote_anchor_txid = vec![[0u8; 32]; remote_count];
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
    pub composition: Account<'info, Composition>,
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

    /// CHECK: owning authority of every SPL local-leg vault ATA; never
    /// itself signs a `lock_sol` CPI (only the depositing user does).
    #[account(seeds = [b"token_vault_authority"], bump)]
    pub token_vault_authority: UncheckedAccount<'info>,
    // --- up to MAX_LOCAL_COMPONENTS - 1 non-native local (Solana) legs,
    // matched to the composition's local components in order, skipping
    // whichever one (if any) is native SOL. ---
    /// CHECK: verified against the composition's declared local component in the handler.
    pub local_spl_0_mint: Option<UncheckedAccount<'info>>,
    /// CHECK: verified to be the user's ATA for `local_spl_0_mint`.
    #[account(mut)]
    pub local_spl_0_user: Option<UncheckedAccount<'info>>,
    /// CHECK: verified to be the vault's ATA for `local_spl_0_mint`; created idempotently if needed.
    #[account(mut)]
    pub local_spl_0_vault: Option<UncheckedAccount<'info>>,
    /// CHECK: verified against the composition's declared local component in the handler.
    pub local_spl_1_mint: Option<UncheckedAccount<'info>>,
    /// CHECK: verified to be the user's ATA for `local_spl_1_mint`.
    #[account(mut)]
    pub local_spl_1_user: Option<UncheckedAccount<'info>>,
    /// CHECK: verified to be the vault's ATA for `local_spl_1_mint`; created idempotently if needed.
    #[account(mut)]
    pub local_spl_1_vault: Option<UncheckedAccount<'info>>,
    /// CHECK: verified against the composition's declared local component in the handler.
    pub local_spl_2_mint: Option<UncheckedAccount<'info>>,
    /// CHECK: verified to be the user's ATA for `local_spl_2_mint`.
    #[account(mut)]
    pub local_spl_2_user: Option<UncheckedAccount<'info>>,
    /// CHECK: verified to be the vault's ATA for `local_spl_2_mint`; created idempotently if needed.
    #[account(mut)]
    pub local_spl_2_vault: Option<UncheckedAccount<'info>>,
}
