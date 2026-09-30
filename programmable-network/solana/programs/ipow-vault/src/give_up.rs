use anchor_lang::prelude::*;

use crate::constants::*;
use crate::errors::VaultError;
use crate::state::{Claim, LockMark};
use crate::util::find_lock;

/// The recipient of a lock on Ethereum gives it up: no receipt will ever be
/// issued for it, and a CANCEL record returns it to its owner there. The
/// lock must be known here from a LOCK record naming the caller, in an
/// accepted claim: a claim not yet decided may carry a false LOCK record,
/// which could otherwise give up any lock (section 11.5).
pub fn handler(ctx: Context<GiveUp>, _claim_id: u64, lock_id: u64) -> Result<()> {
    require!(ctx.accounts.claim.accepted, VaultError::NotAccepted);
    let record = find_lock(&ctx.accounts.claim.records, lock_id).ok_or(VaultError::NotInClaim)?;
    require_keys_eq!(record.recipient, ctx.accounts.recipient.key(), VaultError::WrongAccount);
    let m = &mut ctx.accounts.mark;
    m.lock_id = lock_id;
    m.given_up = true;
    m.bump = ctx.bumps.mark;
    Ok(())
}

#[derive(Accounts)]
#[instruction(claim_id: u64, lock_id: u64)]
pub struct GiveUp<'info> {
    #[account(seeds = [CLAIM_SEED, &claim_id.to_le_bytes()], bump = claim.bump)]
    pub claim: Account<'info, Claim>,
    #[account(init, payer = recipient, space = 8 + LockMark::INIT_SPACE, seeds = [LOCK_SEED, &lock_id.to_le_bytes()], bump)]
    pub mark: Account<'info, LockMark>,
    #[account(mut)]
    pub recipient: Signer<'info>,
    pub system_program: Program<'info, System>,
}
