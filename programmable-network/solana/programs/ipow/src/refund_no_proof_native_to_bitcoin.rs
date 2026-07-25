use anchor_lang::prelude::*;

use crate::constants::PROOF_BLOCKS_WINDOW;
use crate::errors::IPoWError;
use crate::state::{Conversion, ConversionStatus, GlobalState};
use crate::utils::transfer_from_escrow;

pub fn handler(ctx: Context<RefundNoProofNativeToBitcoin>) -> Result<()> {
    let conversion = &mut ctx.accounts.conversion;
    let global_state = &mut ctx.accounts.global_state;

    require!(
        conversion.is_native_to_bitcoin,
        IPoWError::WrongConversionType
    );
    require!(
        conversion.status == ConversionStatus::Deposited,
        IPoWError::BadState
    );

    let proof_window_end = conversion
        .window_start_height
        .checked_add(PROOF_BLOCKS_WINDOW)
        .unwrap()
        .checked_sub(1)
        .unwrap();

    require!(
        global_state.global_tip_height > proof_window_end,
        IPoWError::DutyNotExpired
    );

    conversion.status = ConversionStatus::Refunded;
    global_state.active_open_conversions =
        global_state.active_open_conversions.checked_sub(1).unwrap();

    let refund_amount = conversion
        .native_amount
        .checked_add(conversion.commit_fee)
        .unwrap();

    global_state.total_locked_deposits = global_state
        .total_locked_deposits
        .checked_sub(conversion.native_amount)
        .unwrap();
    global_state.total_held_commit_fees = global_state
        .total_held_commit_fees
        .checked_sub(conversion.commit_fee)
        .unwrap();

    let escrow_bump = global_state.escrow_bump;
    let seeds = &[b"escrow".as_ref(), &[escrow_bump]];
    let signer_seeds = &[&seeds[..]];

    transfer_from_escrow(
        refund_amount,
        &ctx.accounts.user.to_account_info(),
        &ctx.accounts.escrow_vault.to_account_info(),
        &ctx.accounts.system_program.to_account_info(),
        signer_seeds,
    )?;

    Ok(())
}

#[derive(Accounts)]
pub struct RefundNoProofNativeToBitcoin<'info> {
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
