use anchor_lang::prelude::*;

use crate::constants::*;
use crate::errors::VaultError;
use crate::object::put_down;
use crate::state::{Claim, Config, Stake};
use crate::util::now;

/// Answers the objection that holds a claim: the hold is lifted and the 7
/// days restart (D111).
pub fn handler(ctx: Context<Answer>, _claim_id: u64) -> Result<()> {
    let a = &ctx.accounts;
    require!(a.claim.held, VaultError::NotHeld);
    put_down(&a.claim, &a.who.to_account_info(), &a.config, &a.system_program.to_account_info())?;
    let t = now()?;
    let cl = &mut ctx.accounts.claim;
    cl.held = false;
    cl.last_at = t;
    cl.answers += 1;
    let s = &mut ctx.accounts.stake;
    s.answers += 1;
    s.bump = ctx.bumps.stake;
    Ok(())
}

#[derive(Accounts)]
#[instruction(claim_id: u64)]
pub struct Answer<'info> {
    #[account(mut, seeds = [CLAIM_SEED, config.key().as_ref(), &claim_id.to_le_bytes()], bump = claim.bump)]
    pub claim: Account<'info, Claim>,
    #[account(
        init_if_needed,
        payer = who,
        space = 8 + Stake::INIT_SPACE,
        seeds = [STAKE_SEED, config.key().as_ref(), &claim_id.to_le_bytes(), who.key().as_ref()],
        bump
    )]
    pub stake: Account<'info, Stake>,
    #[account(mut, seeds = [CONFIG_SEED, &[config.peer]], bump = config.bump)]
    pub config: Account<'info, Config>,
    #[account(mut)]
    pub who: Signer<'info>,
    pub system_program: Program<'info, System>,
}
