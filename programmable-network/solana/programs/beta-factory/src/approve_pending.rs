use anchor_lang::prelude::*;

use crate::errors::FactoryError;
use crate::state::Pending;

/// X's consent (DESIGN_V2 §6.3 `sig_X`): after depositing on Ethereum and
/// learning lock id N, X binds this pending lock to N. A MINT for any
/// other N, or before approval, is a false statement about Solana.
pub fn handler(ctx: Context<ApprovePending>, _nonce: u64, eth_lock_id: u64) -> Result<()> {
    let p = &mut ctx.accounts.pending;
    require!(!p.approved, FactoryError::AlreadyApproved);
    p.eth_lock_id = eth_lock_id;
    p.approved = true;
    Ok(())
}

#[derive(Accounts)]
#[instruction(nonce: u64)]
pub struct ApprovePending<'info> {
    #[account(mut, seeds = [b"pending", user.key().as_ref(), nonce.to_le_bytes().as_ref()], bump, has_one = user @ FactoryError::Unauthorized)]
    pub pending: Account<'info, Pending>,
    pub user: Signer<'info>,
}
