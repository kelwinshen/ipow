use anchor_lang::prelude::*;
use anchor_spl::token_2022::spl_token_2022::extension::{BaseStateWithExtensions, ExtensionType, StateWithExtensions};
use anchor_spl::token_2022::spl_token_2022::state::Mint as SplMint;
use anchor_spl::token_interface::{Mint, TokenAccount, TokenInterface};

use crate::constants::*;
use crate::errors::VaultError;
use crate::state::{Config, HomeAsset};

/// Registers a token whose home is Solana: it gets the next number (D128).
/// Anyone may call, once per token; nobody approves it. Its receipt on
/// Ethereum is made by an accepted claim carrying its ASSET record. A token
/// that takes a fee on transfer, has a transfer hook, confidential
/// transfers, cannot be moved, or has an extension not known yet, is
/// refused: the vault could not count what it holds. Its issuer's powers,
/// such as freezing, are its holders' risk.
pub fn handler(ctx: Context<RegisterAsset>) -> Result<()> {
    let decimals = check_mint(&ctx.accounts.mint.to_account_info())?;
    // A unit of 10^(decimals - 9) must fit 64 bits.
    require!(decimals <= MAX_DECIMALS + 19, VaultError::BadMint);
    let n = ctx.accounts.config.asset_count;
    ctx.accounts.config.asset_count = n.checked_add(1).ok_or(VaultError::Overflow)?;
    let record_decimals = decimals.min(MAX_DECIMALS);
    let a = &mut ctx.accounts.asset;
    a.number = n;
    a.mint = ctx.accounts.mint.key();
    a.token_program = ctx.accounts.token_program.key();
    a.decimals = decimals;
    a.record_decimals = record_decimals;
    a.unit = 10u64.pow((decimals - record_decimals) as u32);
    a.reserve = 0;
    a.bump = ctx.bumps.asset;
    Ok(())
}

/// The token's decimals, or an error when the vault cannot hold it.
fn check_mint(mint: &AccountInfo) -> Result<u8> {
    let program = *mint.owner;
    require!(program == anchor_spl::token::ID || program == anchor_spl::token_2022::ID, VaultError::BadMint);
    let data = mint.try_borrow_data()?;
    let m = StateWithExtensions::<SplMint>::unpack(&data).map_err(|_| error!(VaultError::BadMint))?;
    for ext in m.get_extension_types().map_err(|_| error!(VaultError::BadMint))? {
        match ext {
            // The issuer's powers, its holders' risk.
            ExtensionType::PermanentDelegate
            | ExtensionType::Pausable
            | ExtensionType::MintCloseAuthority
            | ExtensionType::InterestBearingConfig
            | ExtensionType::ScaledUiAmount
            | ExtensionType::DefaultAccountState
            // What changes how a token is described, not how it moves.
            | ExtensionType::MetadataPointer
            | ExtensionType::TokenMetadata
            | ExtensionType::GroupPointer
            | ExtensionType::TokenGroup
            | ExtensionType::GroupMemberPointer
            | ExtensionType::TokenGroupMember => {}
            _ => return err!(VaultError::BadMint),
        }
    }
    Ok(m.base.decimals)
}

#[derive(Accounts)]
pub struct RegisterAsset<'info> {
    #[account(mut, seeds = [CONFIG_SEED, &[config.peer]], bump = config.bump)]
    pub config: Box<Account<'info, Config>>,
    #[account(
        init,
        payer = payer,
        space = 8 + HomeAsset::INIT_SPACE,
        seeds = [ASSET_SEED, config.key().as_ref(), &config.asset_count.to_le_bytes()],
        bump
    )]
    pub asset: Box<Account<'info, HomeAsset>>,
    #[account(mint::token_program = token_program)]
    pub mint: Box<InterfaceAccount<'info, Mint>>,
    /// The vault's account of the token; one per token, so it is registered
    /// once.
    #[account(
        init,
        payer = payer,
        seeds = [HOME_TOKENS_SEED, config.key().as_ref(), mint.key().as_ref()],
        bump,
        token::mint = mint,
        token::authority = config,
        token::token_program = token_program
    )]
    pub tokens: Box<InterfaceAccount<'info, TokenAccount>>,
    #[account(mut)]
    pub payer: Signer<'info>,
    pub token_program: Interface<'info, TokenInterface>,
    pub system_program: Program<'info, System>,
}
