use anchor_lang::prelude::*;
use anchor_lang::system_program::{transfer, Transfer};
use anchor_spl::token::{self, Mint, Token, TokenAccount, Transfer as TokenTransfer};
use anchor_spl::token_interface::{self, Mint as AnyMint, TokenAccount as AnyTokenAccount, TokenInterface, TransferChecked};

use crate::constants::*;
use crate::errors::VaultError;
use crate::state::{Chain, Config, HomeAsset};
use crate::util::{pay_home, Receipt};

/// Adds to the caller's bond in an asset whose home is Solana (D129), in
/// record units: SOL to the settings record, a token to the vault's account
/// of it.
pub fn add_home<'info>(ctx: Context<'info, AddBondHome<'info>>, _asset: u32, amount: u64) -> Result<()> {
    require!(amount > 0, VaultError::ZeroAmount);
    require!(!ctx.accounts.chain.slashed, VaultError::ChainEnded);
    let a = &ctx.accounts.home_asset;
    let native: u64 = (amount as u128 * a.unit as u128).try_into().map_err(|_| error!(VaultError::Overflow))?;
    let s = &ctx.accounts;
    if a.number == 0 {
        transfer(CpiContext::new(s.system_program.key(), Transfer { from: s.operator.to_account_info(), to: s.config.to_account_info() }), native)?;
    } else {
        let (from, tokens, mint, program) = (
            s.from.as_ref().ok_or(VaultError::WrongAccount)?,
            s.tokens.as_ref().ok_or(VaultError::WrongAccount)?,
            s.mint.as_ref().ok_or(VaultError::WrongAccount)?,
            s.token_program.as_ref().ok_or(VaultError::WrongAccount)?,
        );
        require_keys_eq!(mint.key(), a.mint, VaultError::WrongAccount);
        token_interface::transfer_checked(
            CpiContext::new(
                program.key(),
                TransferChecked { from: from.to_account_info(), mint: mint.to_account_info(), to: tokens.to_account_info(), authority: s.operator.to_account_info() },
            ),
            native,
            a.decimals,
        )?;
    }
    let number = a.number;
    let c = &mut ctx.accounts.chain;
    let i = c.slot(SOLANA, number)?;
    c.positions[i].bond = c.positions[i].bond.checked_add(amount).ok_or(VaultError::Overflow)?;
    Ok(())
}

/// Adds receipts of an asset of Ethereum to the caller's bond (D129).
pub fn add_receipt(ctx: Context<AddBondReceipt>, asset: u32, amount: u64) -> Result<()> {
    require!(amount > 0, VaultError::ZeroAmount);
    require!(!ctx.accounts.chain.slashed, VaultError::ChainEnded);
    let s = &ctx.accounts;
    token::transfer(
        CpiContext::new(
            s.token_program.key(),
            TokenTransfer { from: s.from.to_account_info(), to: s.holding.to_account_info(), authority: s.operator.to_account_info() },
        ),
        amount,
    )?;
    let c = &mut ctx.accounts.chain;
    let i = c.slot(ctx.accounts.config.peer, asset)?;
    c.positions[i].bond = c.positions[i].bond.checked_add(amount).ok_or(VaultError::Overflow)?;
    Ok(())
}

/// What of a position's bond may be withdrawn: all of it once the chain has
/// exited and every claim of it here has ended (D112).
fn take_free(c: &mut Chain, home: u8, asset: u32, amount: u64) -> Result<()> {
    require!(amount > 0, VaultError::ZeroAmount);
    let (exited, open) = (c.exited, c.open_claims);
    let i = c.position(home, asset).ok_or(VaultError::BondNotFree)?;
    let p = &mut c.positions[i];
    let free = if exited && open == 0 { p.bond } else { p.bond - p.stated };
    require!(amount <= free, VaultError::BondNotFree);
    p.bond -= amount;
    if p.stated > p.bond {
        p.stated = p.bond;
    }
    Ok(())
}

/// Withdraws free bond in an asset whose home is Solana: lamports to the
/// operator, or tokens to its account `to`.
pub fn withdraw_home<'info>(ctx: Context<'info, WithdrawBondHome<'info>>, asset: u32, amount: u64) -> Result<()> {
    take_free(&mut ctx.accounts.chain, SOLANA, asset, amount)?;
    let s = &ctx.accounts;
    let to = match &s.to {
        Some(t) => t.to_account_info(),
        None => s.operator.to_account_info(),
    };
    pay_home(
        &s.home_asset,
        amount,
        &s.config.to_account_info(),
        s.config.peer,
        s.config.bump,
        &to,
        s.tokens.as_ref().map(|t| t.to_account_info()).as_ref(),
        s.mint.as_ref().map(|m| m.to_account_info()).as_ref(),
        s.token_program.as_ref().map(|p| p.to_account_info()).as_ref(),
    )
}

/// Withdraws free bond in receipts of an asset of Ethereum.
pub fn withdraw_receipt(ctx: Context<WithdrawBondReceipt>, asset: u32, amount: u64) -> Result<()> {
    take_free(&mut ctx.accounts.chain, ctx.accounts.config.peer, asset, amount)?;
    let s = &ctx.accounts;
    let receipt = Receipt {
        config: &s.config.to_account_info(),
        config_peer: s.config.peer,
        config_bump: s.config.bump,
        mint: &s.mint.to_account_info(),
        holding: &s.holding.to_account_info(),
        token_program: &s.token_program,
    };
    receipt.pay_held(&s.to.to_account_info(), amount)
}

#[derive(Accounts)]
#[instruction(asset: u32)]
pub struct AddBondHome<'info> {
    #[account(mut, seeds = [CHAIN_SEED, config.key().as_ref(), operator.key().as_ref()], bump = chain.bump)]
    pub chain: Box<Account<'info, Chain>>,
    #[account(mut, seeds = [CONFIG_SEED, &[config.peer]], bump = config.bump)]
    pub config: Box<Account<'info, Config>>,
    #[account(seeds = [ASSET_SEED, config.key().as_ref(), &asset.to_le_bytes()], bump = home_asset.bump)]
    pub home_asset: Box<Account<'info, HomeAsset>>,
    #[account(mut, token::authority = operator)]
    pub from: Option<Box<InterfaceAccount<'info, AnyTokenAccount>>>,
    #[account(mut, seeds = [HOME_TOKENS_SEED, config.key().as_ref(), home_asset.mint.as_ref()], bump)]
    pub tokens: Option<Box<InterfaceAccount<'info, AnyTokenAccount>>>,
    pub mint: Option<Box<InterfaceAccount<'info, AnyMint>>>,
    pub token_program: Option<Interface<'info, TokenInterface>>,
    #[account(mut)]
    pub operator: Signer<'info>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
#[instruction(asset: u32)]
pub struct AddBondReceipt<'info> {
    #[account(seeds = [CONFIG_SEED, &[config.peer]], bump = config.bump)]
    pub config: Box<Account<'info, Config>>,
    #[account(mut, seeds = [CHAIN_SEED, config.key().as_ref(), operator.key().as_ref()], bump = chain.bump)]
    pub chain: Box<Account<'info, Chain>>,
    #[account(seeds = [RECEIPT_SEED, config.key().as_ref(), &asset.to_le_bytes()], bump)]
    pub mint: Box<Account<'info, Mint>>,
    #[account(mut, token::mint = mint, token::authority = operator)]
    pub from: Box<Account<'info, TokenAccount>>,
    #[account(mut, seeds = [HOLDING_SEED, config.key().as_ref(), &asset.to_le_bytes()], bump)]
    pub holding: Box<Account<'info, TokenAccount>>,
    pub operator: Signer<'info>,
    pub token_program: Program<'info, Token>,
}

#[derive(Accounts)]
#[instruction(asset: u32)]
pub struct WithdrawBondHome<'info> {
    #[account(mut, seeds = [CHAIN_SEED, config.key().as_ref(), operator.key().as_ref()], bump = chain.bump)]
    pub chain: Box<Account<'info, Chain>>,
    #[account(mut, seeds = [CONFIG_SEED, &[config.peer]], bump = config.bump)]
    pub config: Box<Account<'info, Config>>,
    #[account(seeds = [ASSET_SEED, config.key().as_ref(), &asset.to_le_bytes()], bump = home_asset.bump)]
    pub home_asset: Box<Account<'info, HomeAsset>>,
    #[account(mut, token::authority = operator)]
    pub to: Option<Box<InterfaceAccount<'info, AnyTokenAccount>>>,
    #[account(mut, seeds = [HOME_TOKENS_SEED, config.key().as_ref(), home_asset.mint.as_ref()], bump)]
    pub tokens: Option<Box<InterfaceAccount<'info, AnyTokenAccount>>>,
    pub mint: Option<Box<InterfaceAccount<'info, AnyMint>>>,
    pub token_program: Option<Interface<'info, TokenInterface>>,
    #[account(mut)]
    pub operator: Signer<'info>,
}

#[derive(Accounts)]
#[instruction(asset: u32)]
pub struct WithdrawBondReceipt<'info> {
    #[account(mut, seeds = [CHAIN_SEED, config.key().as_ref(), operator.key().as_ref()], bump = chain.bump)]
    pub chain: Box<Account<'info, Chain>>,
    #[account(seeds = [CONFIG_SEED, &[config.peer]], bump = config.bump)]
    pub config: Box<Account<'info, Config>>,
    #[account(seeds = [RECEIPT_SEED, config.key().as_ref(), &asset.to_le_bytes()], bump)]
    pub mint: Box<Account<'info, Mint>>,
    #[account(mut, seeds = [HOLDING_SEED, config.key().as_ref(), &asset.to_le_bytes()], bump)]
    pub holding: Box<Account<'info, TokenAccount>>,
    #[account(mut, token::mint = mint)]
    pub to: Box<Account<'info, TokenAccount>>,
    pub operator: Signer<'info>,
    pub token_program: Program<'info, Token>,
}
