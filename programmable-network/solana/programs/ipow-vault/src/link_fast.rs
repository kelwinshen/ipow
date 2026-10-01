use anchor_lang::prelude::*;

use crate::constants::*;
use crate::errors::VaultError;
use crate::state::{Claim, FastLock};
use crate::util::find_lock;

/// Links an attest to a claim of the attester's own chain carrying the same
/// LOCK record, opened within 7 days of the attest: the attest then waits
/// for that claim, however long objections hold it (section 11.7, D123).
/// Another claim may be linked once the linked one is refused. Only the
/// attester may call (D125): another operator could link its own claim,
/// let it be refused, and burn the attest before a relink.
pub fn handler(ctx: Context<LinkFast>, _claim_id: u64, _attest: u64) -> Result<()> {
    let f = &ctx.accounts.fast;
    let claim = &ctx.accounts.claim;
    require!(!f.burned, VaultError::AlreadyDone);
    require_keys_eq!(claim.operator, f.attester, VaultError::WrongAccount);
    let record = find_lock(&claim.records, f.lock_id).ok_or(VaultError::NotInClaim)?;
    require!(record.stated_by(f), VaultError::WrongRecord);
    require!(claim.opened_at <= f.attested_at + FAST_OPEN_WINDOW, VaultError::WindowOver);
    require!(!claim.decided || claim.accepted, VaultError::NotAccepted);
    if f.claim != 0 {
        let linked = ctx.accounts.linked.as_ref().ok_or(VaultError::WrongAccount)?;
        require!(linked.id == f.claim, VaultError::WrongAccount);
        require!(linked.decided && !linked.accepted, VaultError::NotRefused);
    }
    ctx.accounts.fast.claim = claim.id;
    Ok(())
}

#[derive(Accounts)]
#[instruction(claim_id: u64, attest: u64)]
pub struct LinkFast<'info> {
    #[account(mut, seeds = [FAST_SEED, &attest.to_le_bytes()], bump = fast.bump)]
    pub fast: Account<'info, FastLock>,
    #[account(seeds = [CLAIM_SEED, &claim_id.to_le_bytes()], bump = claim.bump)]
    pub claim: Account<'info, Claim>,
    /// The claim linked before, when there is one: it must be refused.
    pub linked: Option<Account<'info, Claim>>,
    #[account(address = fast.attester)]
    pub attester: Signer<'info>,
}
