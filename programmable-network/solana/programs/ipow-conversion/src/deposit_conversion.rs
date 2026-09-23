use anchor_lang::prelude::*;
use ipow::state::GlobalState as IpowGlobalState;

use crate::constants::DEPOSIT_BLOCKS_WINDOW;
use crate::errors::ConversionError;
use crate::state::{Conversion, ConversionStatus, Pool};
use crate::spl_accounts::{SplAccounts, SplAccountsBumps, __client_accounts_spl_accounts, __cpi_client_accounts_spl_accounts};
use crate::utils::transfer_value_in;

pub fn handler<'info>(ctx: Context<'info, DepositConversion<'info>>) -> Result<()> {
    let conversion = &mut ctx.accounts.conversion;

    require!(
        conversion.is_native_to_bitcoin,
        ConversionError::WrongConversionType
    );
    require!(
        conversion.status == ConversionStatus::Approved,
        ConversionError::BadState
    );

    let tip = ctx.accounts.ipow_global_state.global_tip_height;
    require!(
        tip <= conversion.window_start_height + (DEPOSIT_BLOCKS_WINDOW - 1),
        ConversionError::IncorrectWindow
    );

    let pool = &mut ctx.accounts.pool;
    pool.token_mint = conversion.token_mint;
    pool.escrow_bump = ctx.bumps.escrow_vault;

    // This amount is already load-bearing: the conversion's eventual payout
    // and the operator's Bitcoin-side proof requirement were both sized
    // against `conversion.native_amount` back at commit time. A
    // fee-on-transfer mint silently delivering less than that would
    // under-collateralize a promise that's already been made, so this
    // requires an exact match rather than accepting whatever arrived
    // (mirrors `propose_claim_conversion`'s own self-escrow requirement for
    // the reverse direction).
    let received = transfer_value_in(
        conversion.token_mint,
        conversion.native_amount,
        &ctx.accounts.user.to_account_info(),
        &ctx.accounts.user_token_account.to_account_info(),
        &ctx.accounts.escrow_vault.to_account_info(),
        &ctx.accounts.escrow_ata.to_account_info(),
        &ctx.accounts.escrow_vault.to_account_info(),
        &ctx.accounts.user.to_account_info(),
        &ctx.accounts.system_program.to_account_info(),
        &ctx.accounts.spl,
    )?;
    require!(received == conversion.native_amount, ConversionError::DepositShortfall);

    // Bundle: each extra token comes through `remaining_accounts` as
    // [mint, token_program, escrow_vault, escrow_ata, user_token_account],
    // in the same order as `conversion.extra_tokens` — Anchor's typed
    // `Accounts` struct can't size to a variable list. `escrow_vault`'s
    // PDA is re-derived here (not trusted from the caller) since
    // `remaining_accounts` bypass the `#[account(seeds = ...)]` check a
    // typed field would normally get; a mismatched account is rejected
    // before any transfer happens, not silently misdirected.
    const ACCOUNTS_PER_EXTRA: usize = 5;
    require!(
        ctx.remaining_accounts.len() == conversion.extra_tokens.len() * ACCOUNTS_PER_EXTRA,
        ConversionError::InvalidRemainingAccount
    );
    let user_info = ctx.accounts.user.to_account_info();
    let system_program_info = ctx.accounts.system_program.to_account_info();
    for (i, extra) in conversion.extra_tokens.iter().enumerate() {
        let base = i * ACCOUNTS_PER_EXTRA;
        let mint: &AccountInfo = &ctx.remaining_accounts[base];
        let token_program: &AccountInfo = &ctx.remaining_accounts[base + 1];
        let escrow_vault: &AccountInfo = &ctx.remaining_accounts[base + 2];
        let escrow_ata: &AccountInfo = &ctx.remaining_accounts[base + 3];
        let user_token_account: &AccountInfo = &ctx.remaining_accounts[base + 4];

        require_keys_eq!(mint.key(), extra.mint, ConversionError::InvalidRemainingAccount);
        let (expected_escrow_vault, _) =
            Pubkey::find_program_address(&[b"escrow", extra.mint.as_ref()], ctx.program_id);
        require_keys_eq!(
            escrow_vault.key(),
            expected_escrow_vault,
            ConversionError::InvalidRemainingAccount
        );

        let extra_spl = SplAccounts {
            mint: UncheckedAccount::try_from(mint),
            token_program: UncheckedAccount::try_from(token_program),
            associated_token_program: ctx.accounts.spl.associated_token_program.clone(),
        };
        let received_extra = transfer_value_in(
            extra.mint,
            extra.amount,
            &user_info,
            user_token_account,
            escrow_vault,
            escrow_ata,
            escrow_vault,
            &user_info,
            &system_program_info,
            &extra_spl,
        )?;
        require!(received_extra == extra.amount, ConversionError::DepositShortfall);
    }

    conversion.status = ConversionStatus::Deposited;
    conversion.deposited_at = Clock::get()?.unix_timestamp;
    pool.total_locked_deposits = pool
        .total_locked_deposits
        .checked_add(conversion.native_amount)
        .unwrap();

    Ok(())
}

#[derive(Accounts)]
pub struct DepositConversion<'info> {
    #[account(seeds = [b"global_state"], bump, seeds::program = ipow::ID)]
    pub ipow_global_state: Account<'info, IpowGlobalState>,

    #[account(mut, has_one = user)]
    pub conversion: Account<'info, Conversion>,

    #[account(
        init_if_needed,
        payer = user,
        space = 8 + Pool::INIT_SPACE,
        seeds = [b"pool", conversion.token_mint.as_ref()],
        bump
    )]
    pub pool: Account<'info, Pool>,

    /// CHECK: native-SOL vault when `conversion.token_mint == default`, the
    /// SPL escrow authority PDA otherwise — see `transfer_value_in`.
    #[account(mut, seeds = [b"escrow", conversion.token_mint.as_ref()], bump)]
    pub escrow_vault: UncheckedAccount<'info>,

    /// CHECK: only touched when `conversion.token_mint != default`, created
    /// idempotently by `transfer_value_in`.
    #[account(mut)]
    pub escrow_ata: UncheckedAccount<'info>,

    /// CHECK: the user's own token account for `conversion.token_mint` —
    /// only touched (and only needs to be valid) on the SPL path.
    #[account(mut)]
    pub user_token_account: UncheckedAccount<'info>,

    pub spl: SplAccounts<'info>,

    #[account(mut)]
    pub user: Signer<'info>,
    pub system_program: Program<'info, System>,
}
