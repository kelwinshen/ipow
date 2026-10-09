use anchor_lang::prelude::*;
use anchor_spl::token::{Mint, Token, TokenAccount};

use crate::constants::*;
use crate::errors::VaultError;
use crate::genesis_make_receipt::genesis_open;
use crate::state::{Config, LockMark};
use crate::util::{create_pda, save, Receipt};

/// What backs a lock issued at genesis (D145): the lock on Ethereum by its
/// number and asset, the value minted and its recipient.
#[event]
pub struct GenesisIssued {
    pub lock_id: u64,
    pub asset: u32,
    pub value: u64,
    pub recipient: Pubkey,
}

/// G2 (docs/specs/ipow-vault-genesis.md): during genesis the genesis key
/// issues lock `lock_id` on Ethereum, of `asset`, its value (amount and
/// fast fee) to its recipient, as `issue` would after an accepted claim. It
/// sets the same mark at ["lock", lock_id], so the lock is issued once
/// whichever comes first: a later claim of it, an attest or a give-up is
/// refused, and one already issued, given up or attested is refused here
/// (D143).
pub fn handler(ctx: Context<GenesisIssue>, lock_id: u64, asset: u32, value: u64) -> Result<()> {
    let c = &ctx.accounts.config;
    require!(genesis_open(c)? && ctx.accounts.key.key() == c.genesis_key, VaultError::NotGenesis);
    require!(value > 0, VaultError::ZeroAmount);
    let (address, bump) = Pubkey::find_program_address(&[LOCK_SEED, c.key().as_ref(), &lock_id.to_le_bytes()], &crate::ID);
    let mark_info = ctx.accounts.mark.to_account_info();
    require_keys_eq!(mark_info.key(), address, VaultError::WrongAccount);
    let mut mark = if mark_info.owner == &crate::ID && !mark_info.data_is_empty() {
        let m = {
            let data = mark_info.try_borrow_data()?;
            LockMark::try_deserialize(&mut &data[..])?
        };
        require!(!m.issued && !m.given_up && m.attests == 0, VaultError::AlreadyDone);
        m
    } else {
        create_pda(
            &mark_info,
            &ctx.accounts.key.to_account_info(),
            &ctx.accounts.system_program.to_account_info(),
            8 + LockMark::INIT_SPACE,
            &[LOCK_SEED, c.key().as_ref(), &lock_id.to_le_bytes(), &[bump]],
        )?;
        LockMark { lock_id, issued: false, given_up: false, attests: 0, settled: 0, first_at: 0, last_attest: 0, bump }
    };
    mark.issued = true;
    save(&mark_info, &mark)?;
    emit!(GenesisIssued { lock_id, asset, value, recipient: ctx.accounts.to.owner });
    let a = &ctx.accounts;
    let receipt = Receipt {
        config: &a.config.to_account_info(),
        config_peer: a.config.peer,
        config_bump: a.config.bump,
        mint: &a.mint.to_account_info(),
        holding: &a.holding.to_account_info(),
        token_program: &a.token_program,
    };
    receipt.mint_to(&a.to.to_account_info(), value)
}

#[derive(Accounts)]
#[instruction(lock_id: u64, asset: u32)]
pub struct GenesisIssue<'info> {
    /// CHECK: the lock's mark at ["lock", lock_id]; checked, and created if
    /// missing, in the handler.
    #[account(mut)]
    pub mark: UncheckedAccount<'info>,
    #[account(seeds = [CONFIG_SEED, &[config.peer]], bump = config.bump)]
    pub config: Box<Account<'info, Config>>,
    #[account(mut, seeds = [RECEIPT_SEED, config.key().as_ref(), &asset.to_le_bytes()], bump)]
    pub mint: Box<Account<'info, Mint>>,
    /// CHECK: the vault's account of the receipt; only its address is used.
    pub holding: UncheckedAccount<'info>,
    /// The recipient's account of the receipt: its owner is the lock's recipient.
    #[account(mut, token::mint = mint)]
    pub to: Box<Account<'info, TokenAccount>>,
    /// The genesis key, which also pays.
    #[account(mut)]
    pub key: Signer<'info>,
    pub token_program: Program<'info, Token>,
    pub system_program: Program<'info, System>,
}
