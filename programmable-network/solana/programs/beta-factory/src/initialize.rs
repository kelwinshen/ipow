use anchor_lang::prelude::*;
use anchor_spl::token::{Mint, Token};

use crate::errors::FactoryError;
use crate::state::{FactoryConfig, FactoryParams};

pub fn validate_params(p: &FactoryParams) -> Result<()> {
    require!(p.sol_per_unit > 0, FactoryError::InvalidParams);
    require!(p.eth_gwei_per_unit > 0, FactoryError::InvalidParams);
    require!(p.window_blocks > 0, FactoryError::InvalidParams);
    require!(p.t_skip_secs > 0 && p.t_challenge_secs > 0, FactoryError::InvalidParams);
    require!(p.unbond_delay_secs > 0, FactoryError::InvalidParams);
    require!(p.bounty_bps as u64 <= crate::constants::BPS_DENOM, FactoryError::InvalidParams);
    Ok(())
}

pub fn handler(ctx: Context<Initialize>, governance: Pubkey, params: FactoryParams) -> Result<()> {
    validate_params(&params)?;
    let c = &mut ctx.accounts.config;
    c.governance = governance;
    c.beta_mint = ctx.accounts.beta_mint.key();
    c.vault_bump = ctx.bumps.vault;
    c.mint_authority_bump = ctx.bumps.mint_authority;
    c.bond_escrow_bump = ctx.bumps.bond_escrow;
    c.insurance_bump = ctx.bumps.insurance;
    c.params = params;
    c.paused = false;
    c.window_start_height = 0;
    c.window_units = 0;
    c.eth_claims_units = 0;
    c.reserve_lamports = 0;
    c.pending_lamports = 0;
    c.next_burn_id = 0;
    Ok(())
}

#[derive(Accounts)]
pub struct Initialize<'info> {
    #[account(init, payer = admin, space = 8 + FactoryConfig::INIT_SPACE, seeds = [b"config"], bump)]
    pub config: Account<'info, FactoryConfig>,

    /// The SOL reserve: pending locks and the backing of outstanding BETA.
    #[account(seeds = [b"vault"], bump)]
    pub vault: SystemAccount<'info>,

    /// CHECK: PDA signer for MintTo only.
    #[account(seeds = [b"mint_authority"], bump)]
    pub mint_authority: UncheckedAccount<'info>,

    /// Party bonds (operators and auditors), exact per-party accounting in `Party.bond`.
    #[account(seeds = [b"bond_escrow"], bump)]
    pub bond_escrow: SystemAccount<'info>,

    /// Slashed bonds not owed to a specific victim accumulate here.
    #[account(seeds = [b"insurance"], bump)]
    pub insurance: SystemAccount<'info>,

    #[account(init, payer = admin, mint::decimals = 9, mint::authority = mint_authority)]
    pub beta_mint: Account<'info, Mint>,

    #[account(mut)]
    pub admin: Signer<'info>,
    pub token_program: Program<'info, Token>,
    pub system_program: Program<'info, System>,
}
