use anchor_lang::prelude::*;

use crate::constants::DEPOSIT_BLOCKS_WINDOW;
use crate::errors::IPoWError;
use crate::state::{Conversion, ConversionStatus, GlobalState};

pub fn handler(ctx: Context<CloseNoBitcoin>) -> Result<()> {
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

    let deposit_window_end = conversion
        .window_start_height
        .checked_add(DEPOSIT_BLOCKS_WINDOW)
        .unwrap()
        .checked_sub(1)
        .unwrap();

    require!(
        global_state.global_tip_height > deposit_window_end,
        IPoWError::DutyNotExpired
    );

    conversion.status = ConversionStatus::Refunded;
    global_state.active_open_conversions =
        global_state.active_open_conversions.checked_sub(1).unwrap();

    global_state.total_reserved_native = global_state
        .total_reserved_native
        .checked_sub(conversion.reserved_native)
        .unwrap();

    Ok(())
}

#[derive(Accounts)]
pub struct CloseNoBitcoin<'info> {
    #[account(mut)]
    pub global_state: Account<'info, GlobalState>,
    #[account(mut)]
    pub conversion: Account<'info, Conversion>,
    #[account(mut, address = global_state.operator @ IPoWError::Unauthorized)]
    pub operator: Signer<'info>,
}
