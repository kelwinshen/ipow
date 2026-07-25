use anchor_lang::prelude::*;

use crate::errors::IPoWError;
use crate::state::GlobalState;

pub fn handler(ctx: Context<AddLiquidity>, amount: u64) -> Result<()> {
    require!(amount > 0, IPoWError::ZeroValue);

    anchor_lang::solana_program::program::invoke(
        &anchor_lang::solana_program::system_instruction::transfer(
            &ctx.accounts.operator.key(),
            &ctx.accounts.escrow_vault.key(),
            amount,
        ),
        &[
            ctx.accounts.operator.to_account_info(),
            ctx.accounts.escrow_vault.to_account_info(),
            ctx.accounts.system_program.to_account_info(),
        ],
    )?;

    Ok(())
}

#[derive(Accounts)]
pub struct AddLiquidity<'info> {
    #[account(mut, seeds = [b"global_state"], bump)]
    pub global_state: Account<'info, GlobalState>,

    #[account(mut, seeds = [b"escrow"], bump = global_state.escrow_bump)]
    pub escrow_vault: SystemAccount<'info>,

    #[account(mut, address = global_state.operator @ IPoWError::Unauthorized)]
    pub operator: Signer<'info>,

    pub system_program: Program<'info, System>,
}
