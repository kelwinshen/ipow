use anchor_lang::prelude::*;

use crate::errors::LightClientError;
use crate::state::Node;
use crate::utils::tx_in_block;

/// Fails unless the transaction is in the block. For tests and tools; a
/// program that needs this reads the block's account and calls
/// `utils::tx_in_block` itself.
pub fn handler(ctx: Context<CheckTx>, raw_tx: Vec<u8>, siblings: Vec<[u8; 32]>, index: u64) -> Result<()> {
    require!(tx_in_block(&ctx.accounts.node, &raw_tx, &siblings, index)?, LightClientError::NotInBlock);
    Ok(())
}

#[derive(Accounts)]
pub struct CheckTx<'info> {
    pub node: Account<'info, Node>,
}
