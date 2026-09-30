use anchor_lang::prelude::*;

use crate::constants::*;
use crate::errors::VaultError;
use crate::state::{Claim, Stake};
use crate::util::credit_to;

/// Credits the caller's deposits on the winning side of a decided claim,
/// each with its share of the losing side's (D111).
pub fn handler(ctx: Context<Collect>, _claim_id: u64) -> Result<()> {
    let cl = &ctx.accounts.claim;
    require!(cl.decided, VaultError::NotDecided);
    let s = &mut ctx.accounts.stake;
    let entries = if cl.accepted {
        std::mem::take(&mut s.answers)
    } else {
        std::mem::take(&mut s.objections)
    };
    require!(entries > 0, VaultError::NothingToCollect);
    let amount = (entries as u64).checked_mul(cl.payout).ok_or(VaultError::Overflow)?;
    let who = ctx.accounts.who.key();
    credit_to(&ctx.accounts.credit, &who, amount, 0, &ctx.accounts.who.to_account_info(), &ctx.accounts.system_program.to_account_info())
}

#[derive(Accounts)]
#[instruction(claim_id: u64)]
pub struct Collect<'info> {
    #[account(seeds = [CLAIM_SEED, &claim_id.to_le_bytes()], bump = claim.bump)]
    pub claim: Account<'info, Claim>,
    #[account(mut, seeds = [STAKE_SEED, &claim_id.to_le_bytes(), who.key().as_ref()], bump = stake.bump)]
    pub stake: Account<'info, Stake>,
    /// CHECK: the caller's credit; checked when used.
    #[account(mut)]
    pub credit: UncheckedAccount<'info>,
    #[account(mut)]
    pub who: Signer<'info>,
    pub system_program: Program<'info, System>,
}
