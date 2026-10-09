use anchor_lang::prelude::*;

use crate::constants::*;
use crate::errors::VaultError;
use crate::state::{Config, Message};
use crate::util::now;

/// Closes a published batch `MESSAGE_KEPT` after its message was processed,
/// its rent back to whoever paid it: the operator, or a guardian that
/// brought the message. Anyone may close it.
pub fn handler(ctx: Context<CloseMessage>) -> Result<()> {
    require!(now()? >= ctx.accounts.message.processed_at + MESSAGE_KEPT, VaultError::WindowNotOver);
    Ok(())
}

#[derive(Accounts)]
pub struct CloseMessage<'info> {
    #[account(seeds = [CONFIG_SEED, &[config.peer]], bump = config.bump)]
    pub config: Box<Account<'info, Config>>,
    #[account(
        mut,
        close = payer,
        seeds = [MESSAGE_SEED, config.key().as_ref(), message.operator.as_ref(), &message.index.to_le_bytes()],
        bump = message.bump,
    )]
    pub message: Account<'info, Message>,
    /// CHECK: who paid the account's rent, as it records.
    #[account(mut, address = message.payer)]
    pub payer: UncheckedAccount<'info>,
}
