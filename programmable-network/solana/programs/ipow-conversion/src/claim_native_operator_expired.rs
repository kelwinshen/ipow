use anchor_lang::prelude::*;

use crate::errors::ConversionError;
use crate::state::{Conversion, ConversionStatus, Pool};
use crate::spl_accounts::{SplAccounts, SplAccountsBumps, __client_accounts_spl_accounts, __cpi_client_accounts_spl_accounts};
use crate::utils::transfer_value_out;

/// Force-claim, bitcoin->native only: if the winning claimant never streamed
/// their duty in time, the user takes the *already-reserved* real pool
/// capital directly — permissionless, unchanged trigger/condition from the
/// original design. The claimant's forfeited stake is handled separately by
/// `reclaim_expired_conversion`. Pays native or SPL depending on
/// `conversion.token_mint`.
pub fn handler<'info>(ctx: Context<'info, ClaimNativeOperatorExpired<'info>>) -> Result<()> {
    let conversion = &mut ctx.accounts.conversion;

    require!(
        !conversion.is_native_to_bitcoin,
        ConversionError::WrongConversionType
    );
    require!(
        conversion.status == ConversionStatus::Approved,
        ConversionError::BadState
    );

    let now = Clock::get()?.unix_timestamp;
    require!(
        conversion.operator_duty_expires_at > 0 && now > conversion.operator_duty_expires_at,
        ConversionError::DutyNotExpired
    );

    conversion.status = ConversionStatus::Completed;

    let token_mint = conversion.token_mint;
    let amount_to_claim = conversion.reserved_native;
    ctx.accounts.pool.total_reserved =
        ctx.accounts.pool.total_reserved.checked_sub(amount_to_claim).unwrap();

    let escrow_bump = ctx.accounts.pool.escrow_bump;
    let escrow_seeds = &[b"escrow".as_ref(), token_mint.as_ref(), &[escrow_bump]];
    let escrow_signer_seeds = &[&escrow_seeds[..]];

    transfer_value_out(
        token_mint,
        amount_to_claim,
        &ctx.accounts.user.to_account_info(),
        &ctx.accounts.user_token_account.to_account_info(),
        &ctx.accounts.escrow_vault.to_account_info(),
        &ctx.accounts.escrow_ata.to_account_info(),
        &ctx.accounts.escrow_vault.to_account_info(),
        &ctx.accounts.payer.to_account_info(),
        &ctx.accounts.system_program.to_account_info(),
        &ctx.accounts.spl,
        escrow_signer_seeds,
    )?;

    // Bundle: only ever populated by `open_bundle_tunnel` (no auctioned
    // bitcoin->token commit can carry one). Same `remaining_accounts`
    // shape and manual PDA re-validation as `deposit_conversion.rs`'s own
    // loop — without this, a defaulted bundle tunnel's extra tokens would
    // never resolve even after duty expiry.
    const ACCOUNTS_PER_EXTRA: usize = 5;
    require!(
        ctx.remaining_accounts.len() == conversion.extra_tokens.len() * ACCOUNTS_PER_EXTRA,
        ConversionError::InvalidRemainingAccount
    );
    let user_info = ctx.accounts.user.to_account_info();
    let payer_info = ctx.accounts.payer.to_account_info();
    let system_program_info = ctx.accounts.system_program.to_account_info();
    for (i, extra) in conversion.extra_tokens.iter().enumerate() {
        let base = i * ACCOUNTS_PER_EXTRA;
        let mint: &AccountInfo = &ctx.remaining_accounts[base];
        let token_program: &AccountInfo = &ctx.remaining_accounts[base + 1];
        let escrow_vault: &AccountInfo = &ctx.remaining_accounts[base + 2];
        let escrow_ata: &AccountInfo = &ctx.remaining_accounts[base + 3];
        let user_extra_token_account: &AccountInfo = &ctx.remaining_accounts[base + 4];

        require_keys_eq!(mint.key(), extra.mint, ConversionError::InvalidRemainingAccount);
        let (expected_escrow_vault, expected_bump) =
            Pubkey::find_program_address(&[b"escrow", extra.mint.as_ref()], ctx.program_id);
        require_keys_eq!(
            escrow_vault.key(),
            expected_escrow_vault,
            ConversionError::InvalidRemainingAccount
        );
        let extra_escrow_seeds: &[&[u8]] = &[b"escrow", extra.mint.as_ref(), &[expected_bump]];
        let extra_escrow_signer_seeds: &[&[&[u8]]] = &[extra_escrow_seeds];

        let extra_spl = SplAccounts {
            mint: UncheckedAccount::try_from(mint),
            token_program: UncheckedAccount::try_from(token_program),
            associated_token_program: ctx.accounts.spl.associated_token_program.clone(),
        };
        transfer_value_out(
            extra.mint,
            extra.amount,
            &user_info,
            user_extra_token_account,
            escrow_vault,
            escrow_ata,
            escrow_vault,
            &payer_info,
            &system_program_info,
            &extra_spl,
            extra_escrow_signer_seeds,
        )?;
    }

    Ok(())
}

#[derive(Accounts)]
pub struct ClaimNativeOperatorExpired<'info> {
    #[account(mut, has_one = user)]
    pub conversion: Account<'info, Conversion>,

    #[account(mut, seeds = [b"pool", conversion.token_mint.as_ref()], bump)]
    pub pool: Account<'info, Pool>,

    /// CHECK: native-SOL vault when `conversion.token_mint == default`, the
    /// SPL escrow authority PDA otherwise.
    #[account(mut, seeds = [b"escrow", conversion.token_mint.as_ref()], bump = pool.escrow_bump)]
    pub escrow_vault: UncheckedAccount<'info>,

    /// CHECK: only touched when `conversion.token_mint != default`.
    #[account(mut)]
    pub escrow_ata: UncheckedAccount<'info>,

    #[account(mut)]
    pub user: SystemAccount<'info>,
    /// CHECK: the user's own token account for `conversion.token_mint` —
    /// only needs to be valid on the SPL path.
    #[account(mut)]
    pub user_token_account: UncheckedAccount<'info>,

    pub spl: SplAccounts<'info>,

    #[account(mut)]
    pub payer: Signer<'info>,
    pub system_program: Program<'info, System>,
}
