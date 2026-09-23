use anchor_lang::prelude::*;
use ipow::state::GlobalState as IpowGlobalState;

use crate::constants::PROOF_BLOCKS_WINDOW;
use crate::errors::ConversionError;
use crate::state::{Conversion, ConversionStatus, Pool};
use crate::spl_accounts::{SplAccounts, SplAccountsBumps, __client_accounts_spl_accounts, __cpi_client_accounts_spl_accounts};
use crate::utils::{transfer_native_from_escrow, transfer_value_out};

/// Permissionless — unchanged trigger/condition from the original design.
/// The operator's forfeited stake is handled separately by `reclaim_expired_
/// conversion` (which anyone can call once `operator_duty_expires_at`
/// passes); this instruction only ever returns the *user's* own deposit.
/// `native_amount` refunds native or SPL depending on `conversion.token_
/// mint`; `commit_fee` always refunds native SOL from the separate,
/// always-native `fee_pool`.
pub fn handler<'info>(
    ctx: Context<'info, RefundNoProofNativeToBitcoin<'info>>,
) -> Result<()> {
    let conversion = &mut ctx.accounts.conversion;

    require!(
        conversion.is_native_to_bitcoin,
        ConversionError::WrongConversionType
    );
    require!(
        conversion.status == ConversionStatus::Deposited,
        ConversionError::BadState
    );

    let proof_window_end = conversion
        .window_start_height
        .checked_add(PROOF_BLOCKS_WINDOW)
        .unwrap()
        .checked_sub(1)
        .unwrap();
    let tip = ctx.accounts.ipow_global_state.global_tip_height;
    require!(tip > proof_window_end, ConversionError::DutyNotExpired);

    conversion.status = ConversionStatus::Refunded;

    let token_mint = conversion.token_mint;
    let native_amount = conversion.native_amount;
    let commit_fee = conversion.commit_fee;

    ctx.accounts.pool.total_locked_deposits =
        ctx.accounts.pool.total_locked_deposits.checked_sub(native_amount).unwrap();
    ctx.accounts.fee_pool.total_held_commit_fees =
        ctx.accounts.fee_pool.total_held_commit_fees.checked_sub(commit_fee).unwrap();

    let escrow_bump = ctx.accounts.pool.escrow_bump;
    let escrow_seeds = &[b"escrow".as_ref(), token_mint.as_ref(), &[escrow_bump]];
    let escrow_signer_seeds = &[&escrow_seeds[..]];

    let user_info = ctx.accounts.user.to_account_info();
    let payer_info = ctx.accounts.payer.to_account_info();
    let system_program_info = ctx.accounts.system_program.to_account_info();

    transfer_value_out(
        token_mint,
        native_amount,
        &user_info,
        &ctx.accounts.user_token_account.to_account_info(),
        &ctx.accounts.escrow_vault.to_account_info(),
        &ctx.accounts.escrow_ata.to_account_info(),
        &ctx.accounts.escrow_vault.to_account_info(),
        &payer_info,
        &system_program_info,
        &ctx.accounts.spl,
        escrow_signer_seeds,
    )?;

    // Bundle: each extra token refunds via `remaining_accounts`, same
    // shape and same re-derived-PDA validation as `deposit_conversion`'s
    // own loop — see its comment.
    const ACCOUNTS_PER_EXTRA: usize = 5;
    require!(
        ctx.remaining_accounts.len() == conversion.extra_tokens.len() * ACCOUNTS_PER_EXTRA,
        ConversionError::InvalidRemainingAccount
    );
    for (i, extra) in conversion.extra_tokens.iter().enumerate() {
        let base = i * ACCOUNTS_PER_EXTRA;
        let mint: &AccountInfo = &ctx.remaining_accounts[base];
        let token_program: &AccountInfo = &ctx.remaining_accounts[base + 1];
        let escrow_vault: &AccountInfo = &ctx.remaining_accounts[base + 2];
        let escrow_ata: &AccountInfo = &ctx.remaining_accounts[base + 3];
        let user_token_account: &AccountInfo = &ctx.remaining_accounts[base + 4];

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
            user_token_account,
            escrow_vault,
            escrow_ata,
            escrow_vault,
            &payer_info,
            &system_program_info,
            &extra_spl,
            extra_escrow_signer_seeds,
        )?;
    }

    let fee_bump = ctx.accounts.fee_pool.escrow_bump;
    let mint_key = Pubkey::default();
    let fee_seeds = &[b"escrow".as_ref(), mint_key.as_ref(), &[fee_bump]];
    let fee_signer_seeds = &[&fee_seeds[..]];
    transfer_native_from_escrow(
        commit_fee,
        &user_info,
        &ctx.accounts.fee_escrow.to_account_info(),
        &system_program_info,
        fee_signer_seeds,
    )?;

    Ok(())
}

#[derive(Accounts)]
pub struct RefundNoProofNativeToBitcoin<'info> {
    #[account(seeds = [b"global_state"], bump, seeds::program = ipow::ID)]
    pub ipow_global_state: Account<'info, IpowGlobalState>,

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

    /// `dup`: for a native conversion this is the same PDA as `pool` above
    /// — see `submit_proof_cache.rs`'s identical note.
    #[account(mut, seeds = [b"pool", Pubkey::default().as_ref()], bump, dup)]
    pub fee_pool: Account<'info, Pool>,

    #[account(mut, seeds = [b"escrow", Pubkey::default().as_ref()], bump = fee_pool.escrow_bump, dup)]
    pub fee_escrow: SystemAccount<'info>,

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
