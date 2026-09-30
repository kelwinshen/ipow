use anchor_lang::prelude::*;
use anchor_spl::token::{Mint, Token, TokenAccount};

use crate::constants::*;
use crate::errors::VaultError;
use crate::state::{Claim, Config, LockMark};
use crate::util::{find_lock, Veth};

/// Issues the vETH of a lock carried by an accepted claim, to its recipient,
/// once per lock, and never for a lock its recipient gave up (section
/// 11.5). Anyone may call.
pub fn handler(ctx: Context<Issue>, _claim_id: u64, lock_id: u64) -> Result<()> {
    let claim = &ctx.accounts.claim;
    require!(claim.accepted, VaultError::NotAccepted);
    let record = find_lock(&claim.records, lock_id).ok_or(VaultError::NotInClaim)?;
    require_keys_eq!(ctx.accounts.to.owner, record.recipient, VaultError::WrongAccount);
    let m = &mut ctx.accounts.mark;
    m.lock_id = lock_id;
    m.issued = true;
    m.bump = ctx.bumps.mark;
    let a = &ctx.accounts;
    let veth = Veth {
        config: &a.config.to_account_info(),
        config_bump: a.config.bump,
        mint: &a.mint.to_account_info(),
        holding: &a.holding.to_account_info(),
        token_program: &a.token_program,
    };
    veth.mint_to(&a.to.to_account_info(), record.amount)
}

#[derive(Accounts)]
#[instruction(claim_id: u64, lock_id: u64)]
pub struct Issue<'info> {
    #[account(seeds = [CLAIM_SEED, &claim_id.to_le_bytes()], bump = claim.bump)]
    pub claim: Account<'info, Claim>,
    #[account(init, payer = payer, space = 8 + LockMark::INIT_SPACE, seeds = [LOCK_SEED, &lock_id.to_le_bytes()], bump)]
    pub mark: Account<'info, LockMark>,
    #[account(seeds = [CONFIG_SEED], bump = config.bump)]
    pub config: Account<'info, Config>,
    #[account(mut, seeds = [MINT_SEED], bump)]
    pub mint: Account<'info, Mint>,
    /// CHECK: the vault's vETH account; only its address is used here.
    #[account(seeds = [HOLDING_SEED], bump)]
    pub holding: UncheckedAccount<'info>,
    #[account(mut, token::mint = mint)]
    pub to: Account<'info, TokenAccount>,
    #[account(mut)]
    pub payer: Signer<'info>,
    pub token_program: Program<'info, Token>,
    pub system_program: Program<'info, System>,
}
