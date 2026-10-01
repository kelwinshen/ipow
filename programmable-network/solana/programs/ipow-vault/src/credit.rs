use anchor_lang::prelude::*;
use anchor_spl::token::{Mint, Token, TokenAccount};
use anchor_spl::token_interface::{Mint as AnyMint, TokenAccount as AnyTokenAccount, TokenInterface};

use crate::constants::*;
use crate::errors::VaultError;
use crate::state::{Claim, Config, Credit, HomeAsset, Stake};
use crate::util::{credit_to, pay_home, Receipt};

/// Credits the caller's deposits on the winning side of a decided claim,
/// each with its share of the losing side's (D111), in SOL.
pub fn collect(ctx: Context<Collect>, _claim_id: u64) -> Result<()> {
    let cl = &ctx.accounts.claim;
    require!(cl.decided, VaultError::NotDecided);
    let s = &mut ctx.accounts.stake;
    let entries = if cl.accepted {
        std::mem::take(&mut s.answers)
    } else {
        std::mem::take(&mut s.objections)
    };
    require!(entries > 0, VaultError::NothingToCollect);
    let amount = (entries as u64).checked_mul(cl.payout).ok_or(VaultError::Overflow)?;
    let who = ctx.accounts.who.key();
    credit_to(&ctx.accounts.credit, &who, SOLANA, 0, amount, &ctx.accounts.who.to_account_info(), &ctx.accounts.system_program.to_account_info())
}

/// Withdraws the caller's credit in an asset whose home is Solana: lamports
/// to itself, or tokens to its account `to`.
pub fn withdraw_home<'info>(ctx: Context<'info, WithdrawCreditHome<'info>>, _asset: u32) -> Result<()> {
    let amount = std::mem::take(&mut ctx.accounts.credit.amount);
    require!(amount > 0, VaultError::ZeroAmount);
    let s = &ctx.accounts;
    let to = match &s.to {
        Some(t) => t.to_account_info(),
        None => s.owner.to_account_info(),
    };
    // Credits are in native units: pay them at a unit of one.
    let mut a: HomeAsset = (**s.home_asset).clone();
    a.unit = 1;
    pay_home(
        &a,
        amount,
        &s.config.to_account_info(),
        s.config.bump,
        &to,
        s.tokens.as_ref().map(|t| t.to_account_info()).as_ref(),
        s.mint.as_ref().map(|m| m.to_account_info()).as_ref(),
        s.token_program.as_ref().map(|p| p.to_account_info()).as_ref(),
    )
}

/// Withdraws the caller's credit in receipts of an asset of Ethereum.
pub fn withdraw_receipt(ctx: Context<WithdrawCreditReceipt>, _asset: u32) -> Result<()> {
    let amount = std::mem::take(&mut ctx.accounts.credit.amount);
    require!(amount > 0, VaultError::ZeroAmount);
    let s = &ctx.accounts;
    let receipt = Receipt {
        config: &s.config.to_account_info(),
        config_bump: s.config.bump,
        mint: &s.mint.to_account_info(),
        holding: &s.holding.to_account_info(),
        token_program: &s.token_program,
    };
    receipt.pay_held(&s.to.to_account_info(), amount)
}

#[derive(Accounts)]
#[instruction(claim_id: u64)]
pub struct Collect<'info> {
    #[account(seeds = [CLAIM_SEED, &claim_id.to_le_bytes()], bump = claim.bump)]
    pub claim: Box<Account<'info, Claim>>,
    #[account(mut, seeds = [STAKE_SEED, &claim_id.to_le_bytes(), who.key().as_ref()], bump = stake.bump)]
    pub stake: Box<Account<'info, Stake>>,
    /// CHECK: the caller's SOL credit; checked when used.
    #[account(mut)]
    pub credit: UncheckedAccount<'info>,
    #[account(mut)]
    pub who: Signer<'info>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
#[instruction(asset: u32)]
pub struct WithdrawCreditHome<'info> {
    #[account(mut, seeds = [CREDIT_SEED, owner.key().as_ref(), &[SOLANA], &asset.to_le_bytes()], bump = credit.bump)]
    pub credit: Box<Account<'info, Credit>>,
    #[account(mut, seeds = [CONFIG_SEED], bump = config.bump)]
    pub config: Box<Account<'info, Config>>,
    #[account(seeds = [ASSET_SEED, &asset.to_le_bytes()], bump = home_asset.bump)]
    pub home_asset: Box<Account<'info, HomeAsset>>,
    #[account(mut, token::authority = owner)]
    pub to: Option<Box<InterfaceAccount<'info, AnyTokenAccount>>>,
    #[account(mut, seeds = [HOME_TOKENS_SEED, home_asset.mint.as_ref()], bump)]
    pub tokens: Option<Box<InterfaceAccount<'info, AnyTokenAccount>>>,
    pub mint: Option<Box<InterfaceAccount<'info, AnyMint>>>,
    pub token_program: Option<Interface<'info, TokenInterface>>,
    #[account(mut)]
    pub owner: Signer<'info>,
}

#[derive(Accounts)]
#[instruction(asset: u32)]
pub struct WithdrawCreditReceipt<'info> {
    #[account(mut, seeds = [CREDIT_SEED, owner.key().as_ref(), &[ETHEREUM], &asset.to_le_bytes()], bump = credit.bump)]
    pub credit: Box<Account<'info, Credit>>,
    #[account(seeds = [CONFIG_SEED], bump = config.bump)]
    pub config: Box<Account<'info, Config>>,
    #[account(seeds = [RECEIPT_SEED, &asset.to_le_bytes()], bump)]
    pub mint: Box<Account<'info, Mint>>,
    #[account(mut, seeds = [HOLDING_SEED, &asset.to_le_bytes()], bump)]
    pub holding: Box<Account<'info, TokenAccount>>,
    #[account(mut, token::mint = mint)]
    pub to: Box<Account<'info, TokenAccount>>,
    pub owner: Signer<'info>,
    pub token_program: Program<'info, Token>,
}
