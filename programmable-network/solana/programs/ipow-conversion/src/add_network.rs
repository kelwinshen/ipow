use anchor_lang::prelude::*;

use crate::errors::ConversionError;
use crate::state::{ConversionGlobalState, SupportedNetwork};

/// Same validation `iPoW.sol`'s own `addNetwork` already enforces
/// (mirrored into `iPoWConversion.sol` too): `network_id == 0`,
/// `min_addr_len == 0`, or `min_addr_len > max_addr_len` all revert
/// `InvalidNetworkConfig`. Re-registering an already-active `network_id`
/// is separately guarded by `init` on `network_config` below — Anchor
/// refuses to re-initialize an already-existing PDA, so there's no
/// explicit "already enabled" check needed here the way the EVM mapping-
/// based registry requires.
pub fn handler(
    ctx: Context<AddNetwork>,
    network_id: u64,
    min_addr_len: u16,
    max_addr_len: u16,
) -> Result<()> {
    require!(network_id != 0, ConversionError::InvalidNetworkConfig);
    require!(min_addr_len != 0, ConversionError::InvalidNetworkConfig);
    require!(min_addr_len <= max_addr_len, ConversionError::InvalidNetworkConfig);

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
    #[account(seeds = [b"conversion_global_state"], bump)]
    pub conversion_global_state: Account<'info, ConversionGlobalState>,

    #[account(
        init,
        payer = admin,
        space = 8 + SupportedNetwork::INIT_SPACE,
        seeds = [b"network", network_id.to_le_bytes().as_ref()],
        bump
    )]
    pub network_config: Account<'info, SupportedNetwork>,

    #[account(mut, address = conversion_global_state.governance @ ConversionError::Unauthorized)]
    pub admin: Signer<'info>,
    pub system_program: Program<'info, System>,
}
