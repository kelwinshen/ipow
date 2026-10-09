use anchor_lang::prelude::*;

use crate::constants::*;
use crate::state::Buffer;

/// Closes the caller's buffer and gives its rent back.
pub fn handler(_ctx: Context<CloseBuffer>) -> Result<()> {
    Ok(())
}

#[derive(Accounts)]
pub struct CloseBuffer<'info> {
    #[account(mut, close = owner, seeds = [BUFFER_SEED, owner.key().as_ref()], bump = buffer.bump)]
    pub buffer: Account<'info, Buffer>,
    #[account(mut)]
    pub owner: Signer<'info>,
}
