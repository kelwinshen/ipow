use anchor_lang::prelude::*;

use crate::errors::IPoWError;
use crate::state::GlobalState;
use crate::utils::transfer_from_escrow;

pub fn handler(ctx: Context<RemoveLiquidity>, amount: u64) -> Result<()> {
    require!(amount > 0, IPoWError::ZeroValue);
    let global_state = &ctx.accounts.global_state;

    let current_balance = ctx.accounts.escrow_vault.lamports();
    let locked_funds = global_state
        .total_locked_deposits
        .checked_add(global_state.total_reserved_native)
        .unwrap()
        .checked_add(global_state.total_held_commit_fees)
        .unwrap();

    let rent_minimum = Rent::get()?.minimum_balance(0);
    let total_unavailable = locked_funds.checked_add(rent_minimum).unwrap();

    let available_liquidity = current_balance.saturating_sub(total_unavailable);
    require!(
        available_liquidity >= amount,
        IPoWError::InsufficientLiquidity
    );

    let bump = global_state.escrow_bump;
    let signer_seeds: &[&[&[u8]]] = &[&[b"escrow", &[bump]]];

    transfer_from_escrow(
        amount,
        &ctx.accounts.operator.to_account_info(),
        &ctx.accounts.escrow_vault.to_account_info(),
        &ctx.accounts.system_program.to_account_info(),
        signer_seeds,
    )?;

    Ok(())
}

#[derive(Accounts)]
pub struct RemoveLiquidity<'info> {
    #[account(mut, seeds = [b"global_state"], bump)]
    pub global_state: Account<'info, GlobalState>,

    /// CHECK: address is fully constrained by the `seeds`/`bump` PDA derivation
    /// above; only used as the source of a System Program lamport transfer via
    /// `invoke_signed` with the escrow's own PDA seeds, which the runtime itself
    /// validates.
    #[account(mut, seeds = [b"escrow"], bump = global_state.escrow_bump)]
    pub escrow_vault: UncheckedAccount<'info>,

    #[account(mut, address = global_state.operator @ IPoWError::Unauthorized)]
    pub operator: Signer<'info>,

    pub system_program: Program<'info, System>,
}
