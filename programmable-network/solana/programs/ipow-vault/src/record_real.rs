use anchor_lang::prelude::*;
use ipow_protocol::duty::read_walk;
use ipow_protocol::state::BlockRef;

use crate::constants::*;
use crate::errors::VaultError;
use crate::state::Real;

/// Records a block below a real block as real, with a finished walk of the
/// light client from `high` down to `low` (D108). Anyone may call.
pub fn handler(ctx: Context<RecordReal>, low: BlockRef, high: BlockRef) -> Result<()> {
    read_walk(&ctx.accounts.walk, &high, &low).map_err(|_| error!(VaultError::NotReal))?;
    ctx.accounts.low_real.bump = ctx.bumps.low_real;
    Ok(())
}

#[derive(Accounts)]
#[instruction(low: BlockRef, high: BlockRef)]
pub struct RecordReal<'info> {
    #[account(seeds = [REAL_SEED, high.hash.as_ref(), &high.height.to_le_bytes(), &high.epoch_time.to_le_bytes()], bump = high_real.bump)]
    pub high_real: Account<'info, Real>,
    /// CHECK: a finished walk of the light client; read and checked.
    pub walk: UncheckedAccount<'info>,
    #[account(
        init,
        payer = payer,
        space = 8 + Real::INIT_SPACE,
        seeds = [REAL_SEED, low.hash.as_ref(), &low.height.to_le_bytes(), &low.epoch_time.to_le_bytes()],
        bump
    )]
    pub low_real: Account<'info, Real>,
    #[account(mut)]
    pub payer: Signer<'info>,
    pub system_program: Program<'info, System>,
}
