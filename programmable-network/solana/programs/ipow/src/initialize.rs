use anchor_lang::prelude::*;

use crate::errors::IPoWError;
use crate::state::GlobalState;

pub fn handler(ctx: Context<Initialize>, operator: Pubkey, commit_fee_bps: u16) -> Result<()> {
    let global_state = &mut ctx.accounts.global_state;
    require!(commit_fee_bps <= 10000, IPoWError::InvalidFeeConfig);

    global_state.operator = operator;
    global_state.commit_fee_bps = commit_fee_bps;
    global_state.next_tx_id = 1;
    global_state.global_tip_height = 0;
    global_state.min_anchor_height = 0;
    global_state.active_open_conversions = 0;

    global_state.total_held_commit_fees = 0;
    global_state.total_locked_deposits = 0;
    global_state.total_reserved_native = 0;

    global_state.escrow_bump = ctx.bumps.escrow_vault;

    Ok(())
}

#[derive(Accounts)]
pub struct Initialize<'info> {
    #[account(init, payer = admin, space = 8 + GlobalState::INIT_SPACE, seeds = [b"global_state"], bump)]
    pub global_state: Account<'info, GlobalState>,

    #[account(seeds = [b"escrow"], bump)]
    pub escrow_vault: SystemAccount<'info>,

    #[account(mut)]
    pub admin: Signer<'info>,
    pub system_program: Program<'info, System>,
}
