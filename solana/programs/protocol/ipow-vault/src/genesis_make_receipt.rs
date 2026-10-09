use anchor_lang::prelude::*;
use anchor_spl::token::{Mint, Token, TokenAccount};

use crate::constants::*;
use crate::errors::VaultError;
use crate::state::Config;
use crate::util::now;

/// What backs a receipt made at genesis (D145).
#[event]
pub struct GenesisReceipt {
    pub asset: u32,
    pub token: [u8; 20],
    pub decimals: u8,
}

/// Whether the pair is in genesis (D142, D144): a key, not finalized, before its end.
pub fn genesis_open(c: &Config) -> Result<bool> {
    Ok(c.genesis_key != Pubkey::default() && !c.genesis_done && now()? < c.genesis_end)
}

/// G1 (docs/specs/ipow-vault-genesis.md): during genesis the genesis key
/// makes the receipt here of an asset whose home is Ethereum, as an
/// accepted ASSET record would: its token there and its decimals, which
/// must be the asset's record decimals there. A later ASSET claim finds it
/// made, since its mint already exists.
pub fn handler(ctx: Context<GenesisMakeReceipt>, asset: u32, token: [u8; 20], decimals: u8) -> Result<()> {
    let c = &ctx.accounts.config;
    require!(genesis_open(c)? && ctx.accounts.key.key() == c.genesis_key, VaultError::NotGenesis);
    require!(decimals <= MAX_DECIMALS, VaultError::WrongRecord);
    emit!(GenesisReceipt { asset, token, decimals });
    Ok(())
}

#[derive(Accounts)]
#[instruction(asset: u32, token: [u8; 20], decimals: u8)]
pub struct GenesisMakeReceipt<'info> {
    #[account(seeds = [CONFIG_SEED, &[config.peer]], bump = config.bump)]
    pub config: Box<Account<'info, Config>>,
    #[account(init, payer = key, seeds = [RECEIPT_SEED, config.key().as_ref(), &asset.to_le_bytes()], bump, mint::decimals = decimals, mint::authority = config)]
    pub mint: Box<Account<'info, Mint>>,
    #[account(init, payer = key, seeds = [HOLDING_SEED, config.key().as_ref(), &asset.to_le_bytes()], bump, token::mint = mint, token::authority = config)]
    pub holding: Box<Account<'info, TokenAccount>>,
    /// The genesis key, which also pays.
    #[account(mut)]
    pub key: Signer<'info>,
    pub token_program: Program<'info, Token>,
    pub system_program: Program<'info, System>,
}
