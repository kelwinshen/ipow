use anchor_lang::prelude::*;

use crate::constants::*;
use crate::errors::VaultError;
use crate::state::Buffer;

/// Appends `data` to the transaction (`raw` true) or the batch in the
/// caller's buffer, up to the limits of section 11.3.
pub fn handler(ctx: Context<WriteBuffer>, raw: bool, data: Vec<u8>) -> Result<()> {
    let b = &mut ctx.accounts.buffer;
    let (into, max) = if raw { (&mut b.raw_tx, MAX_RAW_TX) } else { (&mut b.batch, MAX_BATCH) };
    require!(into.len() + data.len() <= max, VaultError::TooLarge);
    into.extend_from_slice(&data);
    Ok(())
}

#[derive(Accounts)]
pub struct WriteBuffer<'info> {
    #[account(mut, seeds = [BUFFER_SEED, owner.key().as_ref()], bump = buffer.bump)]
    pub buffer: Account<'info, Buffer>,
    pub owner: Signer<'info>,
}
