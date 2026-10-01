use anchor_lang::prelude::*;
use anchor_spl::token::{self, Mint, Token, TokenAccount, Transfer};

use crate::constants::*;
use crate::errors::VaultError;
use crate::state::{Chain, Config, FastLock, LockMark};
use crate::util::{create_pda, now, save, EthLock, Receipt};

/// An attester issues the receipt of a lock on Ethereum at once (section
/// 11.7, D122, D123): new receipts minted to the recipient, backed by the
/// locked asset. The attester is an operator whose chain is not refused,
/// slashed or ended, since it must bring a claim of its own chain carrying
/// the same LOCK record within 7 days, and locks 1.25 times the amount in
/// receipts until then. It states the record, fee, fast fee and time
/// included; it checks the lock on Ethereum first, and loses its receipts
/// if it stated it wrongly. A lock may be attested several times, for 7
/// days from its first (D126); not once its receipt was counted or it was
/// given up.
pub fn handler(ctx: Context<AttestLock>, asset: u32, record: Vec<u8>) -> Result<()> {
    let l = EthLock::read(&record, ctx.accounts.config.peer)?;
    require!(l.asset == asset && l.amount > 0, VaultError::WrongRecord);
    require_keys_eq!(ctx.accounts.to.owner, l.recipient, VaultError::WrongAccount);
    let c = &ctx.accounts.chain;
    require!(!c.refused && !c.slashed && !c.exited, VaultError::NotOperator);
    let collateral = (l.amount as u128 * FAST_COLLATERAL_BPS as u128).div_ceil(BPS as u128);
    let collateral = u64::try_from(collateral).map_err(|_| error!(VaultError::Overflow))?;

    // The lock's mark: made by its first attest.
    let (address, bump) = Pubkey::find_program_address(&[LOCK_SEED, ctx.accounts.config.key().as_ref(), &l.id.to_le_bytes()], &crate::ID);
    let mark_info = ctx.accounts.mark.to_account_info();
    require_keys_eq!(mark_info.key(), address, VaultError::WrongAccount);
    let mut mark = if mark_info.owner == &crate::ID && !mark_info.data_is_empty() {
        let data = mark_info.try_borrow_data()?;
        LockMark::try_deserialize(&mut &data[..])?
    } else {
        create_pda(
            &mark_info,
            &ctx.accounts.attester.to_account_info(),
            &ctx.accounts.system_program.to_account_info(),
            8 + LockMark::INIT_SPACE,
            &[LOCK_SEED, ctx.accounts.config.key().as_ref(), &l.id.to_le_bytes(), &[bump]],
        )?;
        LockMark { lock_id: l.id, issued: false, given_up: false, attests: 0, settled: 0, first_at: now()?, last_attest: 0, bump }
    };
    require!(!mark.issued && !mark.given_up, VaultError::AlreadyDone);
    // Attests for 7 days from the first: no stream of them holds the
    // receipt back (D126).
    require!(now()? < mark.first_at + FAST_OPEN_WINDOW, VaultError::WindowOver);
    let id = ctx.accounts.config.attest_count + 1;
    let prev = mark.last_attest;
    mark.attests = mark.attests.checked_add(1).ok_or(VaultError::Overflow)?;
    mark.last_attest = id;
    save(&mark_info, &mark)?;

    let a = &ctx.accounts;
    token::transfer(
        CpiContext::new(
            a.token_program.key(),
            Transfer { from: a.from.to_account_info(), to: a.holding.to_account_info(), authority: a.attester.to_account_info() },
        ),
        collateral,
    )?;
    ctx.accounts.config.attest_count = id;
    let f = &mut ctx.accounts.fast;
    f.id = id;
    f.lock_id = l.id;
    f.asset = asset;
    f.attester = ctx.accounts.attester.key();
    f.amount = l.amount;
    f.recipient = l.recipient;
    f.fee = l.fee;
    f.fast_fee = l.fast_fee;
    f.locked_at = l.locked_at;
    f.collateral = collateral;
    f.attested_at = now()?;
    f.claim = 0;
    f.prev = prev;
    f.burned = false;
    f.bump = ctx.bumps.fast;
    let a = &ctx.accounts;
    let receipt = Receipt {
        config: &a.config.to_account_info(),
        config_peer: a.config.peer,
        config_bump: a.config.bump,
        mint: &a.mint.to_account_info(),
        holding: &a.holding.to_account_info(),
        token_program: &a.token_program,
    };
    receipt.mint_to(&a.to.to_account_info(), l.amount)
}

#[derive(Accounts)]
#[instruction(asset: u32)]
pub struct AttestLock<'info> {
    #[account(mut, seeds = [CONFIG_SEED, &[config.peer]], bump = config.bump)]
    pub config: Box<Account<'info, Config>>,
    #[account(seeds = [CHAIN_SEED, config.key().as_ref(), attester.key().as_ref()], bump = chain.bump)]
    pub chain: Box<Account<'info, Chain>>,
    /// CHECK: the lock's mark at ["lock", lock_id]; checked, and created by
    /// the first attest, in the handler.
    #[account(mut)]
    pub mark: UncheckedAccount<'info>,
    #[account(
        init,
        payer = attester,
        space = 8 + FastLock::INIT_SPACE,
        seeds = [FAST_SEED, config.key().as_ref(), &(config.attest_count + 1).to_le_bytes()],
        bump
    )]
    pub fast: Box<Account<'info, FastLock>>,
    #[account(mut, seeds = [RECEIPT_SEED, config.key().as_ref(), &asset.to_le_bytes()], bump)]
    pub mint: Box<Account<'info, Mint>>,
    #[account(mut, seeds = [HOLDING_SEED, config.key().as_ref(), &asset.to_le_bytes()], bump)]
    pub holding: Box<Account<'info, TokenAccount>>,
    #[account(mut, token::mint = mint, token::authority = attester)]
    pub from: Box<Account<'info, TokenAccount>>,
    #[account(mut, token::mint = mint)]
    pub to: Box<Account<'info, TokenAccount>>,
    #[account(mut)]
    pub attester: Signer<'info>,
    pub token_program: Program<'info, Token>,
    pub system_program: Program<'info, System>,
}
