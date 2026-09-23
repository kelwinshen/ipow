use anchor_lang::prelude::*;

use crate::errors::ConversionError;
use crate::state::{ConversionGlobalState, SupportedNetwork};

/// Mirrors `iPoWV1.sol`/`iPoWV1Conversion.sol`'s own `removeNetwork` —
/// `add_network.rs` had no counterpart before this. Closing the PDA
/// (rather than merely flipping `is_active = false`) is the Solana-native
/// equivalent of the EVM side's `delete networkConfigs[networkId]`: the
/// account stops existing at all, its rent refunds to `admin`, and a
/// later `add_network` for the same `network_id` can `init` it fresh
/// rather than needing an `init_if_needed`/reactivate path.
pub fn handler(_ctx: Context<RemoveNetwork>, _network_id: u64) -> Result<()> {
    Ok(())
}

#[derive(Accounts)]
#[instruction(network_id: u64)]
pub struct RemoveNetwork<'info> {
    #[account(seeds = [b"conversion_global_state"], bump)]
    pub conversion_global_state: Account<'info, ConversionGlobalState>,

    #[account(
        mut,
        close = admin,
        seeds = [b"network", network_id.to_le_bytes().as_ref()],
        bump
    )]
    pub network_config: Account<'info, SupportedNetwork>,

    #[account(mut, address = conversion_global_state.governance @ ConversionError::Unauthorized)]
    pub admin: Signer<'info>,
    pub system_program: Program<'info, System>,
}
