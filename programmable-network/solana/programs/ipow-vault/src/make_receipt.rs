use anchor_lang::prelude::*;
use anchor_spl::token::{Mint, Token, TokenAccount};

use crate::constants::*;
use crate::errors::VaultError;
use crate::state::{Claim, Config};
use crate::util::u32_at;

/// Makes the receipt here of an asset whose home is Ethereum, from an
/// accepted claim carrying its ASSET record, with the decimals it states
/// (section 11.9). Anyone may call, once per asset.
pub fn handler(ctx: Context<MakeReceipt>, _claim_id: u64, asset: u32, record: Vec<u8>) -> Result<()> {
    require!(record.len() == ASSET_LEN && record[0] == ASSET && record[1] == ctx.accounts.config.peer, VaultError::WrongRecord);
    require!(u32_at(&record, 2) == asset && record[38] <= MAX_DECIMALS, VaultError::WrongRecord);
    let claim = &ctx.accounts.claim;
    require!(claim.accepted && claim.carries(&record), VaultError::NotAccepted);
    Ok(())
}

#[derive(Accounts)]
#[instruction(claim_id: u64, asset: u32, record: Vec<u8>)]
pub struct MakeReceipt<'info> {
    #[account(seeds = [CONFIG_SEED, &[config.peer]], bump = config.bump)]
    pub config: Box<Account<'info, Config>>,
    #[account(seeds = [CLAIM_SEED, config.key().as_ref(), &claim_id.to_le_bytes()], bump = claim.bump)]
    pub claim: Box<Account<'info, Claim>>,
    #[account(
        init,
        payer = payer,
        seeds = [RECEIPT_SEED, config.key().as_ref(), &asset.to_le_bytes()],
        bump,
        mint::decimals = *record.get(38).unwrap_or(&0),
        mint::authority = config
    )]
    pub mint: Box<Account<'info, Mint>>,
    #[account(init, payer = payer, seeds = [HOLDING_SEED, config.key().as_ref(), &asset.to_le_bytes()], bump, token::mint = mint, token::authority = config)]
    pub holding: Box<Account<'info, TokenAccount>>,
    #[account(mut)]
    pub payer: Signer<'info>,
    pub token_program: Program<'info, Token>,
    pub system_program: Program<'info, System>,
}
