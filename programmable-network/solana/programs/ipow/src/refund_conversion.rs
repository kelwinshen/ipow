use anchor_lang::prelude::*;

use crate::errors::IPoWError;
use crate::state::{Conversion, ConversionStatus, GlobalState};

pub fn handler(ctx: Context<RefundConversion>) -> Result<()> {
    let conversion = &mut ctx.accounts.conversion;
    let global_state = &mut ctx.accounts.global_state;

    require!(
        conversion.status == ConversionStatus::Committed,
        IPoWError::BadState
    );

    let fee_to_refund = conversion.commit_fee;
    conversion.status = ConversionStatus::Refunded;
    conversion.commit_fee = 0;
    global_state.total_held_commit_fees = global_state
        .total_held_commit_fees
        .checked_sub(fee_to_refund)
        .unwrap();

    let bump = global_state.escrow_bump;
    let signer_seeds: &[&[&[u8]]] = &[&[b"escrow", &[bump]]];

    anchor_lang::solana_program::program::invoke_signed(
        &anchor_lang::solana_program::system_instruction::transfer(
            &ctx.accounts.escrow_vault.key(),
            &ctx.accounts.user.key(),
            fee_to_refund,
        ),
        &[
            ctx.accounts.escrow_vault.to_account_info(),
            ctx.accounts.user.to_account_info(),
            ctx.accounts.system_program.to_account_info(),
        ],
        signer_seeds,
    )?;

    Ok(())
}

#[derive(Accounts)]
pub struct RefundConversion<'info> {
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
