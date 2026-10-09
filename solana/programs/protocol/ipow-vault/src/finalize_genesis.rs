use anchor_lang::prelude::*;

use crate::constants::*;
use crate::errors::VaultError;
use crate::genesis_make_receipt::genesis_open;
use crate::state::Config;
use crate::util::now;

#[event]
pub struct GenesisFinalized {
    pub at: i64,
}

/// Ends the pair's genesis for good (D144): the genesis key's last act.
pub fn handler(ctx: Context<FinalizeGenesis>) -> Result<()> {
    let c = &mut ctx.accounts.config;
    require!(genesis_open(c)? && ctx.accounts.key.key() == c.genesis_key, VaultError::NotGenesis);
    c.genesis_done = true;
    emit!(GenesisFinalized { at: now()? });
    Ok(())
}

#[derive(Accounts)]
pub struct FinalizeGenesis<'info> {
    #[account(mut, seeds = [CONFIG_SEED, &[config.peer]], bump = config.bump)]
    pub config: Box<Account<'info, Config>>,
    pub key: Signer<'info>,
}
