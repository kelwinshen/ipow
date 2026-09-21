use anchor_lang::prelude::*;
use anchor_spl::token::{self, Burn as TokenBurn, Mint, Token, TokenAccount};

use crate::constants::*;
use crate::errors::FactoryError;
use crate::state::{Burn, FactoryConfig};
use crate::utils::transfer_from_pda;

/// Burn `units` BETA: the SOL half is paid now, locally; the ETH half is
/// recorded as `Burn` for the operator's RELEASE anchor (DESIGN_V2 §6.5).
pub fn handler(ctx: Context<BurnRedeem>, units: u64, to_eth: [u8; 20]) -> Result<()> {
    require!(units > 0, FactoryError::InvalidParams);
    let raw = units.checked_mul(UNIT).ok_or(FactoryError::Overflow)?;
    require!(ctx.accounts.burner_beta.amount >= raw, FactoryError::InsufficientBeta);
    token::burn(
        CpiContext::new(
            ctx.accounts.token_program.key(),
            TokenBurn {
                mint: ctx.accounts.beta_mint.to_account_info(),
                from: ctx.accounts.burner_beta.to_account_info(),
                authority: ctx.accounts.burner.to_account_info(),
            },
        ),
        raw,
    )?;
    let c = &mut ctx.accounts.config;
    let lamports = units.checked_mul(c.params.sol_per_unit).ok_or(FactoryError::Overflow)?;
    c.reserve_lamports = c.reserve_lamports.checked_sub(lamports).ok_or(FactoryError::Overflow)?;
    c.eth_claims_units = c.eth_claims_units.checked_sub(units).ok_or(FactoryError::Overflow)?;
    let burn_id = c.next_burn_id;
    c.next_burn_id += 1;
    let bump = c.vault_bump;
    let seeds: &[&[u8]] = &[b"vault", &[bump]];
    transfer_from_pda(
        lamports,
        &ctx.accounts.vault.to_account_info(),
        &ctx.accounts.burner.to_account_info(),
        &ctx.accounts.system_program.to_account_info(),
        &[seeds],
    )?;
    let b = &mut ctx.accounts.burn;
    b.burn_id = burn_id;
    b.burner = ctx.accounts.burner.key();
    b.units = units;
    b.to_eth = to_eth;
    b.claimed = false;
    b.created_at = Clock::get()?.unix_timestamp;
    Ok(())
}

#[derive(Accounts)]
pub struct BurnRedeem<'info> {
    #[account(mut, seeds = [b"config"], bump)]
    pub config: Account<'info, FactoryConfig>,
    #[account(init, payer = burner, space = 8 + Burn::INIT_SPACE, seeds = [b"burn", config.next_burn_id.to_le_bytes().as_ref()], bump)]
    pub burn: Account<'info, Burn>,
    #[account(mut, seeds = [b"vault"], bump = config.vault_bump)]
    pub vault: SystemAccount<'info>,
    #[account(mut, address = config.beta_mint)]
    pub beta_mint: Account<'info, Mint>,
    #[account(mut, associated_token::mint = beta_mint, associated_token::authority = burner)]
    pub burner_beta: Account<'info, TokenAccount>,
    #[account(mut)]
    pub burner: Signer<'info>,
    pub token_program: Program<'info, Token>,
    pub system_program: Program<'info, System>,
}
