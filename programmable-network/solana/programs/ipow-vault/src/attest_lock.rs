use anchor_lang::prelude::*;
use anchor_spl::token::{self, Mint, Token, TokenAccount, Transfer};

use crate::constants::*;
use crate::errors::VaultError;
use crate::state::{Chain, Config, FastLock, LockMark};
use crate::util::{create_pda, now, save, Veth};

/// An attester issues the receipt of a lock on Ethereum at once (section
/// 11.7, D122, D123): new vETH minted to the recipient, backed by the locked
/// ETH. The attester is an operator whose chain is not refused, slashed or
/// ended, since it must bring a claim of its own chain carrying the same
/// LOCK record within 7 days, and locks 1.25 times the amount in vETH until
/// then. It states the record, fee, fast fee and time included; it checks
/// the lock on Ethereum first, and loses its vETH if it stated it wrongly.
/// A lock may be attested several times (D126): a wrong attest made first
/// blocks nothing. Not once its receipt was counted or it was given up.
pub fn handler(
    ctx: Context<AttestLock>,
    lock_id: u64,
    amount: u64,
    recipient: Pubkey,
    fee: u64,
    fast_fee: u64,
    locked_at: i64,
) -> Result<()> {
    require!(amount > 0, VaultError::ZeroAmount);
    require_keys_eq!(ctx.accounts.to.owner, recipient, VaultError::WrongAccount);
    let c = &ctx.accounts.chain;
    require!(!c.refused && !c.slashed && !c.exited, VaultError::NotOperator);
    let collateral = (amount as u128 * FAST_COLLATERAL_BPS as u128).div_ceil(BPS as u128);
    let collateral = u64::try_from(collateral).map_err(|_| error!(VaultError::Overflow))?;

    // The lock's mark: made by its first attest.
    let (address, bump) = Pubkey::find_program_address(&[LOCK_SEED, &lock_id.to_le_bytes()], &crate::ID);
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
            &[LOCK_SEED, &lock_id.to_le_bytes(), &[bump]],
        )?;
        LockMark { lock_id, issued: false, given_up: false, attests: 0, settled: 0, first_at: now()?, last_attest: 0, bump }
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
    f.lock_id = lock_id;
    f.attester = ctx.accounts.attester.key();
    f.amount = amount;
    f.recipient = recipient;
    f.fee = fee;
    f.fast_fee = fast_fee;
    f.locked_at = locked_at;
    f.collateral = collateral;
    f.attested_at = now()?;
    f.claim = 0;
    f.prev = prev;
    f.burned = false;
    f.bump = ctx.bumps.fast;
    let a = &ctx.accounts;
    let veth = Veth {
        config: &a.config.to_account_info(),
        config_bump: a.config.bump,
        mint: &a.mint.to_account_info(),
        holding: &a.holding.to_account_info(),
        token_program: &a.token_program,
    };
    veth.mint_to(&a.to.to_account_info(), amount)
}

#[derive(Accounts)]
pub struct AttestLock<'info> {
    #[account(mut, seeds = [CONFIG_SEED], bump = config.bump)]
    pub config: Box<Account<'info, Config>>,
    #[account(seeds = [CHAIN_SEED, attester.key().as_ref()], bump = chain.bump)]
    pub chain: Box<Account<'info, Chain>>,
    /// CHECK: the lock's mark at ["lock", lock_id]; checked, and created by
    /// the first attest, in the handler.
    #[account(mut)]
    pub mark: UncheckedAccount<'info>,
    #[account(
        init,
        payer = attester,
        space = 8 + FastLock::INIT_SPACE,
        seeds = [FAST_SEED, &(config.attest_count + 1).to_le_bytes()],
        bump
    )]
    pub fast: Box<Account<'info, FastLock>>,
    #[account(mut, seeds = [MINT_SEED], bump)]
    pub mint: Box<Account<'info, Mint>>,
    #[account(mut, seeds = [HOLDING_SEED], bump)]
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
