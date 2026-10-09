use anchor_lang::prelude::*;

use crate::constants::{PROTOCOL_SEED, VAULT_SEED};
use crate::state::Protocol;

/// Creates the protocol's record and its vault, once. It takes no settings,
/// so it does not matter who calls it first (D59).
pub fn handler(ctx: Context<InitializeProtocol>) -> Result<()> {
    let protocol = &mut ctx.accounts.protocol;
    protocol.job_count = 0;
    protocol.challenge_count = 0;
    protocol.bump = ctx.bumps.protocol;
    protocol.vault_bump = ctx.bumps.vault;
    Ok(())
}

#[derive(Accounts)]
pub struct InitializeProtocol<'info> {
    #[account(init, payer = payer, space = 8 + Protocol::INIT_SPACE, seeds = [PROTOCOL_SEED], bump)]
    pub protocol: Account<'info, Protocol>,
    /// CHECK: the vault holds lamports only. It is created here, owned by
    /// this program, so only this program can take lamports out of it.
    #[account(init, payer = payer, space = 0, seeds = [VAULT_SEED], bump, owner = crate::ID)]
    pub vault: UncheckedAccount<'info>,
    #[account(mut)]
    pub payer: Signer<'info>,
    pub system_program: Program<'info, System>,
}
