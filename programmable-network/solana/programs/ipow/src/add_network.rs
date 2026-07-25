use anchor_lang::prelude::*;

use crate::errors::IPoWError;
use crate::state::{GlobalState, SupportedNetwork};

pub fn handler(
    ctx: Context<AddNetwork>,
    network_id: u64,
    min_addr_len: u16,
    max_addr_len: u16,
) -> Result<()> {
    let network_config = &mut ctx.accounts.network_config;
    network_config.network_id = network_id;
    network_config.min_addr_len = min_addr_len;
    network_config.max_addr_len = max_addr_len;
    network_config.is_active = true;
    Ok(())
}

#[derive(Accounts)]
#[instruction(network_id: u64)]
pub struct AddNetwork<'info> {
    #[account(mut, seeds = [b"global_state"], bump)]
    pub global_state: Account<'info, GlobalState>,

    #[account(
        init,
        payer = admin,
        space = 8 + SupportedNetwork::INIT_SPACE,
        seeds = [b"network", network_id.to_le_bytes().as_ref()],
        bump
    )]
    pub network_config: Account<'info, SupportedNetwork>,

    #[account(mut, address = global_state.operator @ IPoWError::Unauthorized)]
    pub admin: Signer<'info>,
    pub system_program: Program<'info, System>,
}
