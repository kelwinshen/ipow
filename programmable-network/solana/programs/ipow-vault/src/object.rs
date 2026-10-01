use anchor_lang::prelude::*;
use anchor_lang::system_program::{transfer, Transfer};

use crate::constants::*;
use crate::errors::VaultError;
use crate::state::{Claim, Config, Stake};
use crate::util::now;

/// Puts down the flat deposit on one side of an open claim (D111).
pub fn put_down<'info>(claim: &Claim, who: &AccountInfo<'info>, config: &Account<'info, Config>, system: &AccountInfo<'info>) -> Result<()> {
    require!(!claim.decided, VaultError::NotOpen);
    require!(now()? < claim.last_at + OBJECTION_WINDOW, VaultError::WindowOver);
    transfer(CpiContext::new(system.key(), Transfer { from: who.clone(), to: config.to_account_info() }), config.deposit)
}

/// Objects to a claim: it is held.
pub fn handler(ctx: Context<Object>, _claim_id: u64) -> Result<()> {
    let a = &ctx.accounts;
    require!(!a.claim.held, VaultError::AlreadyHeld);
    put_down(&a.claim, &a.who.to_account_info(), &a.config, &a.system_program.to_account_info())?;
    let t = now()?;
    let cl = &mut ctx.accounts.claim;
    cl.held = true;
    cl.last_at = t;
    cl.objections += 1;
    let s = &mut ctx.accounts.stake;
    s.objections += 1;
    s.bump = ctx.bumps.stake;
    Ok(())
}

#[derive(Accounts)]
#[instruction(claim_id: u64)]
pub struct Object<'info> {
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
