use anchor_lang::prelude::*;
use anchor_spl::token::{Mint, Token, TokenAccount};

use crate::constants::*;
use crate::errors::VaultError;
use crate::state::{Chain, Config, HomeAsset};
use crate::util::{credit_to, Receipt};

/// Settles a slashed chain's bond in one asset (D109, D129): its 80% backs
/// the asset's receipts, its 20% is credited to whoever submitted the false
/// message. For an asset whose home is Solana the backing joins its
/// reserve; a receipt bond's backing is burned: fewer receipts for the same
/// asset. Anyone may call.
pub fn handler<'info>(ctx: Context<'info, SettleSlash<'info>>, home: u8, asset: u32) -> Result<()> {
    let c = &mut ctx.accounts.chain;
    require!(c.slashed, VaultError::NothingToCollect);
    let i = c.position(home, asset).ok_or(VaultError::NothingToCollect)?;
    let backing = std::mem::take(&mut c.positions[i].slash_backing);
    let share = std::mem::take(&mut c.positions[i].slash_share);
    require!(backing > 0 || share > 0, VaultError::NothingToCollect);
    let slasher = c.slasher;
    let s = &ctx.accounts;
    // Credits are in native units.
    let mut credited = share;
    if home == SOLANA {
        let a = s.home_asset.as_ref().ok_or(VaultError::WrongAccount)?;
        require!(a.number == asset, VaultError::WrongAccount);
        credited = share.checked_mul(a.unit).ok_or(VaultError::Overflow)?;
    } else {
        let (mint, holding) = (s.mint.as_ref().ok_or(VaultError::WrongAccount)?, s.holding.as_ref().ok_or(VaultError::WrongAccount)?);
        let a = asset.to_le_bytes();
        require_keys_eq!(mint.key(), Pubkey::find_program_address(&[RECEIPT_SEED, &a], &crate::ID).0, VaultError::WrongAccount);
        require_keys_eq!(holding.key(), Pubkey::find_program_address(&[HOLDING_SEED, &a], &crate::ID).0, VaultError::WrongAccount);
        let receipt = Receipt {
            config: &s.config.to_account_info(),
            config_bump: s.config.bump,
            mint: &mint.to_account_info(),
            holding: &holding.to_account_info(),
            token_program: s.token_program.as_ref().ok_or(VaultError::WrongAccount)?,
        };
        receipt.burn_held(backing)?;
    }
    credit_to(&s.slasher_credit, &slasher, home, asset, credited, &s.payer.to_account_info(), &s.system_program.to_account_info())?;
    if home == SOLANA {
        let a = ctx.accounts.home_asset.as_mut().ok_or(VaultError::WrongAccount)?;
        a.reserve = a.reserve.checked_add(backing).ok_or(VaultError::Overflow)?;
    }
    Ok(())
}

#[derive(Accounts)]
pub struct SettleSlash<'info> {
    #[account(mut, seeds = [CHAIN_SEED, chain.operator.as_ref()], bump = chain.bump)]
    pub chain: Box<Account<'info, Chain>>,
    #[account(seeds = [CONFIG_SEED], bump = config.bump)]
    pub config: Box<Account<'info, Config>>,
    /// For an asset whose home is Solana.
    #[account(mut, seeds = [ASSET_SEED, &home_asset.number.to_le_bytes()], bump = home_asset.bump)]
    pub home_asset: Option<Box<Account<'info, HomeAsset>>>,
    /// For a receipt: its mint and the vault's account of it.
    #[account(mut)]
    pub mint: Option<Box<Account<'info, Mint>>>,
    #[account(mut)]
    pub holding: Option<Box<Account<'info, TokenAccount>>>,
    /// CHECK: the slasher's credit in the asset; checked when used.
    #[account(mut)]
    pub slasher_credit: UncheckedAccount<'info>,
    #[account(mut)]
    pub payer: Signer<'info>,
    pub token_program: Option<Program<'info, Token>>,
    pub system_program: Program<'info, System>,
}
