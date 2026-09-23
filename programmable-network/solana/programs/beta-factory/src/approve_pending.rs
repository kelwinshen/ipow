use anchor_lang::prelude::*;

use crate::errors::FactoryError;
use crate::state::Pending;

/// X's consent (DESIGN_V2 §6.3 `sig_X`, generalized to §8's multiple
/// remote components): after depositing on every remote chain the
/// composition names and learning each lock id, X binds this pending
/// mint to all of them at once, in composition order (excluding the
/// local Solana leg, which needs no lock id). A MINT for any lock id
/// that doesn't match, or before approval, is a false statement about
/// Solana.
pub fn handler(ctx: Context<ApprovePending>, _nonce: u64, remote_lock_id: Vec<u64>) -> Result<()> {
    let p = &mut ctx.accounts.pending;
    require!(!p.approved, FactoryError::AlreadyApproved);
    require!(remote_lock_id.len() == p.remote_count(), FactoryError::InvalidParams);
    p.remote_lock_id = remote_lock_id;
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
