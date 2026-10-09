use anchor_lang::prelude::*;

use crate::constants::RETENTION;
use crate::errors::LightClientError;
use crate::state::{EpochStart, Node};

/// Closes a block 8 weeks after it was stored. Its rent goes back to whoever
/// stored it (D101). Anyone may call.
pub fn close_node(ctx: Context<CloseNode>) -> Result<()> {
    let now = Clock::get()?.unix_timestamp;
    require!(now >= ctx.accounts.node.stored_at + RETENTION, LightClientError::TooEarlyToClose);
    Ok(())
}

/// Closes an epoch start 8 weeks after it was recorded (D101). Anyone may
/// call. Its entry in the day table stays; by then no comparison reads it.
pub fn close_epoch_start(ctx: Context<CloseEpochStart>) -> Result<()> {
    let now = Clock::get()?.unix_timestamp;
    require!(
        now >= ctx.accounts.epoch_start.recorded_at + RETENTION,
        LightClientError::TooEarlyToClose
    );
    Ok(())
}

#[derive(Accounts)]
pub struct CloseNode<'info> {
    #[account(mut, close = payer, has_one = payer)]
    pub node: Account<'info, Node>,
    /// CHECK: the address that paid the rent, checked by `has_one`.
    #[account(mut)]
    pub payer: UncheckedAccount<'info>,
}

#[derive(Accounts)]
pub struct CloseEpochStart<'info> {
    #[account(mut, close = payer, has_one = payer)]
    pub epoch_start: Account<'info, EpochStart>,
    /// CHECK: the address that paid the rent, checked by `has_one`.
    #[account(mut)]
    pub payer: UncheckedAccount<'info>,
}
