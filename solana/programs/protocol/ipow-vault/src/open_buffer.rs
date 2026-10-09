use anchor_lang::prelude::*;

use crate::constants::*;
use crate::state::Buffer;

/// Opens the caller's buffer, to upload a message's transaction and batch in
/// pieces (section 11.3). One per caller at a time.
pub fn handler(ctx: Context<OpenBuffer>) -> Result<()> {
    let b = &mut ctx.accounts.buffer;
    b.owner = ctx.accounts.owner.key();
    b.bump = ctx.bumps.buffer;
    Ok(())
}

#[derive(Accounts)]
pub struct OpenBuffer<'info> {
    #[account(init, payer = owner, space = 8 + Buffer::INIT_SPACE, seeds = [BUFFER_SEED, owner.key().as_ref()], bump)]
    pub buffer: Account<'info, Buffer>,
    #[account(mut)]
    pub owner: Signer<'info>,
    pub system_program: Program<'info, System>,
}
