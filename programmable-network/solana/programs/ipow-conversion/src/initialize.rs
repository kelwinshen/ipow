use anchor_lang::prelude::*;

use crate::errors::ConversionError;
use crate::state::ConversionGlobalState;

pub fn handler(ctx: Context<Initialize>, governance: Pubkey, commit_fee_bps: u16) -> Result<()> {
    require!(commit_fee_bps <= 10_000, ConversionError::BadState);

    let gs = &mut ctx.accounts.conversion_global_state;
    gs.governance = governance;
    gs.commit_fee_bps = commit_fee_bps;
    gs.next_tx_id = 1;

    Ok(())
}

#[derive(Accounts)]
pub struct Initialize<'info> {
    #[account(
        init,
        payer = admin,
        space = 8 + ConversionGlobalState::INIT_SPACE,
        seeds = [b"conversion_global_state"],
        bump
    )]
    pub conversion_global_state: Account<'info, ConversionGlobalState>,

    #[account(mut)]
    pub admin: Signer<'info>,
    pub system_program: Program<'info, System>,
}
