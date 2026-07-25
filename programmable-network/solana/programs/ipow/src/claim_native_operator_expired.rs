use anchor_lang::prelude::*;

use crate::errors::IPoWError;
use crate::state::{Conversion, ConversionStatus, GlobalState};
use crate::utils::transfer_from_escrow;

pub fn handler(ctx: Context<ClaimNativeOperatorExpired>) -> Result<()> {
    let conversion = &mut ctx.accounts.conversion;
    let global_state = &mut ctx.accounts.global_state;

    require!(
        !conversion.is_native_to_bitcoin,
        IPoWError::WrongConversionType
    );
    require!(
        conversion.status == ConversionStatus::Approved,
        IPoWError::BadState
    );

    let now = Clock::get()?.unix_timestamp;
    require!(
        conversion.operator_duty_expires_at > 0 && now > conversion.operator_duty_expires_at,
        IPoWError::DutyNotExpired
    );

    conversion.status = ConversionStatus::Completed;
    global_state.active_open_conversions =
        global_state.active_open_conversions.checked_sub(1).unwrap();

    let amount_to_claim = conversion.reserved_native;

    global_state.total_reserved_native = global_state
        .total_reserved_native
        .checked_sub(amount_to_claim)
        .unwrap();

    let escrow_bump = global_state.escrow_bump;
    let seeds = &[b"escrow".as_ref(), &[escrow_bump]];
    let signer_seeds = &[&seeds[..]];

    transfer_from_escrow(
        amount_to_claim,
        &ctx.accounts.user.to_account_info(),
        &ctx.accounts.escrow_vault.to_account_info(),
        &ctx.accounts.system_program.to_account_info(),
        signer_seeds,
    )?;

    Ok(())
}

#[derive(Accounts)]
pub struct ClaimNativeOperatorExpired<'info> {
    #[account(mut)]
    pub global_state: Account<'info, GlobalState>,
    #[account(mut, seeds = [b"escrow"], bump = global_state.escrow_bump)]
    pub escrow_vault: SystemAccount<'info>,
    #[account(mut, has_one = user)]
    pub conversion: Account<'info, Conversion>,
    #[account(mut)]
    pub user: SystemAccount<'info>,
    #[account(mut)]
    pub payer: Signer<'info>,
    pub system_program: Program<'info, System>,
}
