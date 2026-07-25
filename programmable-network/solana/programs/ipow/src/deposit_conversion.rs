use anchor_lang::prelude::*;

use crate::constants::DEPOSIT_BLOCKS_WINDOW;
use crate::errors::IPoWError;
use crate::state::{Conversion, ConversionStatus, GlobalState};

pub fn handler(ctx: Context<DepositConversion>) -> Result<()> {
    let conversion = &mut ctx.accounts.conversion;
    let global_state = &mut ctx.accounts.global_state;

    require!(
        conversion.is_native_to_bitcoin,
        IPoWError::WrongConversionType
    );
    require!(
        conversion.status == ConversionStatus::Approved,
        IPoWError::BadState
    );

    require!(
        global_state.global_tip_height
            <= conversion.window_start_height + (DEPOSIT_BLOCKS_WINDOW - 1),
        IPoWError::IncorrectWindow
    );

    anchor_lang::solana_program::program::invoke(
        &anchor_lang::solana_program::system_instruction::transfer(
            &ctx.accounts.user.key(),
            &ctx.accounts.escrow_vault.key(),
            conversion.native_amount,
        ),
        &[
            ctx.accounts.user.to_account_info(),
            ctx.accounts.escrow_vault.to_account_info(),
            ctx.accounts.system_program.to_account_info(),
        ],
    )?;

    conversion.status = ConversionStatus::Deposited;
    conversion.deposited_at = Clock::get()?.unix_timestamp;
    global_state.total_locked_deposits = global_state
        .total_locked_deposits
        .checked_add(conversion.native_amount)
        .unwrap();

    Ok(())
}

#[derive(Accounts)]
pub struct DepositConversion<'info> {
    #[account(mut)]
    pub global_state: Account<'info, GlobalState>,
    #[account(mut, seeds = [b"escrow"], bump = global_state.escrow_bump)]
    pub escrow_vault: SystemAccount<'info>,
    #[account(mut, has_one = user)]
    pub conversion: Account<'info, Conversion>,
    #[account(mut)]
    pub user: Signer<'info>,
    pub system_program: Program<'info, System>,
}
