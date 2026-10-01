use anchor_lang::prelude::*;
use anchor_lang::system_program::{transfer, Transfer};
use anchor_spl::token_interface::{self, Mint, TokenAccount, TokenInterface, TransferChecked};

use crate::constants::*;
use crate::errors::VaultError;
use crate::state::{Config, HomeAsset, HomeLock};
use crate::util::{is_eth_address, now};

/// Locks an asset whose home is Solana for its receipt on Ethereum: lock
/// number `lock_count + 1` (section 11.9). `amount`, `fee` and `fast_fee`
/// are in record units; SOL goes to the settings record, a token to the
/// vault's account of it. The fee goes to the first operator whose message
/// carrying the lock is judged true here (D113); the fast fee is minted on
/// Ethereum to the attester, or the recipient (D122). `recipient` is an
/// Ethereum address in 32 bytes.
pub fn handler<'info>(
    ctx: Context<'info, LockHome<'info>>,
    _asset: u32,
    recipient: [u8; 32],
    amount: u64,
    fee: u64,
    fast_fee: u64,
) -> Result<()> {
    require!(amount > 0, VaultError::ZeroAmount);
    require!(is_eth_address(&recipient), VaultError::ZeroAddress);
    let total = amount.checked_add(fee).and_then(|t| t.checked_add(fast_fee)).ok_or(VaultError::Overflow)?;
    let a = &ctx.accounts.home_asset;
    let native: u64 = (total as u128 * a.unit as u128).try_into().map_err(|_| error!(VaultError::Overflow))?;
    if a.number == 0 {
        transfer(
            CpiContext::new(
                ctx.accounts.system_program.key(),
                Transfer { from: ctx.accounts.user.to_account_info(), to: ctx.accounts.config.to_account_info() },
            ),
            native,
        )?;
    } else {
        let (from, tokens, mint, program) = (
            ctx.accounts.from.as_ref().ok_or(VaultError::WrongAccount)?,
            ctx.accounts.tokens.as_ref().ok_or(VaultError::WrongAccount)?,
            ctx.accounts.mint.as_ref().ok_or(VaultError::WrongAccount)?,
            ctx.accounts.token_program.as_ref().ok_or(VaultError::WrongAccount)?,
        );
        require_keys_eq!(mint.key(), a.mint, VaultError::WrongAccount);
        token_interface::transfer_checked(
            CpiContext::new(
                program.key(),
                TransferChecked {
                    from: from.to_account_info(),
                    mint: mint.to_account_info(),
                    to: tokens.to_account_info(),
                    authority: ctx.accounts.user.to_account_info(),
                },
            ),
            native,
            a.decimals,
        )?;
    }
    let id = ctx.accounts.config.lock_count + 1;
    ctx.accounts.config.lock_count = id;
    let backing = amount.checked_add(fast_fee).ok_or(VaultError::Overflow)?;
    let asset = &mut ctx.accounts.home_asset;
    asset.reserve = asset.reserve.checked_add(backing).ok_or(VaultError::Overflow)?;
    let number = asset.number;
    let l = &mut ctx.accounts.lock;
    l.id = id;
    l.asset = number;
    l.owner = ctx.accounts.user.key();
    l.amount = amount;
    l.recipient = recipient;
    l.fee = fee;
    l.fast_fee = fast_fee;
    l.locked_at = now()?;
    l.fee_to = Pubkey::default();
    l.fee_paid = false;
    l.fee_taken = false;
    l.returned = false;
    l.bump = ctx.bumps.lock;
    Ok(())
}

#[derive(Accounts)]
#[instruction(asset: u32)]
pub struct LockHome<'info> {
    #[account(mut, seeds = [CONFIG_SEED, &[config.peer]], bump = config.bump)]
    pub config: Box<Account<'info, Config>>,
    #[account(mut, seeds = [ASSET_SEED, config.key().as_ref(), &asset.to_le_bytes()], bump = home_asset.bump)]
    pub home_asset: Box<Account<'info, HomeAsset>>,
    #[account(
        init,
        payer = user,
        space = 8 + HomeLock::INIT_SPACE,
        seeds = [HOME_LOCK_SEED, config.key().as_ref(), &(config.lock_count + 1).to_le_bytes()],
        bump
    )]
    pub lock: Box<Account<'info, HomeLock>>,
    /// For a token: the user's account, the vault's, the mint and its
    /// program.
    #[account(mut, token::authority = user)]
    pub from: Option<Box<InterfaceAccount<'info, TokenAccount>>>,
    #[account(mut, seeds = [HOME_TOKENS_SEED, config.key().as_ref(), home_asset.mint.as_ref()], bump)]
    pub tokens: Option<Box<InterfaceAccount<'info, TokenAccount>>>,
    pub mint: Option<Box<InterfaceAccount<'info, Mint>>>,
    pub token_program: Option<Interface<'info, TokenInterface>>,
    #[account(mut)]
    pub user: Signer<'info>,
    pub system_program: Program<'info, System>,
}
