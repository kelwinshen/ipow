use anchor_lang::prelude::*;

use crate::constants::*;
use crate::errors::VaultError;
use crate::state::{Claim, Config, LockMark};
use crate::util::EthLock;

/// The recipient of a lock on Ethereum gives it up: no receipt will ever be
/// issued for it, and a CANCEL record returns it to its owner there. The
/// lock must be known here from a LOCK record naming the caller, in an
/// accepted claim: a claim not yet decided may carry a false LOCK record,
/// which could otherwise give up any lock (section 11.5). A lock whose
/// receipt was issued, or that was attested, has its mark already and
/// cannot be given up.
pub fn handler(ctx: Context<GiveUp>, _claim_id: u64, lock_id: u64, record: Vec<u8>) -> Result<()> {
    let l = EthLock::read(&record, ctx.accounts.config.peer)?;
    require!(l.id == lock_id, VaultError::WrongRecord);
    let claim = &ctx.accounts.claim;
    require!(claim.accepted && claim.carries(&record), VaultError::NotAccepted);
    require_keys_eq!(l.recipient, ctx.accounts.recipient.key(), VaultError::WrongAccount);
    let m = &mut ctx.accounts.mark;
    m.lock_id = lock_id;
    m.given_up = true;
    m.issued = false;
    m.attests = 0;
    m.settled = 0;
    m.first_at = 0;
    m.last_attest = 0;
    m.bump = ctx.bumps.mark;
    Ok(())
}

#[derive(Accounts)]
#[instruction(claim_id: u64, lock_id: u64)]
pub struct GiveUp<'info> {
    #[account(seeds = [CONFIG_SEED, &[config.peer]], bump = config.bump)]
    pub config: Box<Account<'info, Config>>,
    #[account(seeds = [CLAIM_SEED, config.key().as_ref(), &claim_id.to_le_bytes()], bump = claim.bump)]
    pub claim: Box<Account<'info, Claim>>,
    #[account(init, payer = recipient, space = 8 + LockMark::INIT_SPACE, seeds = [LOCK_SEED, config.key().as_ref(), &lock_id.to_le_bytes()], bump)]
    pub mark: Box<Account<'info, LockMark>>,
    #[account(mut)]
    pub recipient: Signer<'info>,
    pub system_program: Program<'info, System>,
}
