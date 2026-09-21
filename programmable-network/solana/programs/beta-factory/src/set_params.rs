use anchor_lang::prelude::*;

use crate::errors::FactoryError;
use crate::initialize::validate_params;
use crate::state::{FactoryConfig, FactoryParams};

pub fn handler(ctx: Context<SetParams>, params: FactoryParams, paused: bool) -> Result<()> {
    validate_params(&params)?;
    let c = &mut ctx.accounts.config;
    c.params = params;
    c.paused = paused;
    Ok(())
}

#[derive(Accounts)]
pub struct SetParams<'info> {
    #[account(mut, seeds = [b"config"], bump)]
    pub config: Account<'info, FactoryConfig>,
    #[account(address = config.governance @ FactoryError::Unauthorized)]
    pub governance: Signer<'info>,
}
