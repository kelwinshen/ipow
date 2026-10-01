use anchor_lang::prelude::*;
use anchor_spl::token::{Mint, Token, TokenAccount};

use crate::constants::*;
use crate::errors::VaultError;
use crate::state::{Claim, Config, FastLock, LockMark};
use crate::util::{credit_to, fast_share, find_lock, now, Veth};

/// Settles an attest with an accepted claim carrying a LOCK record for its
/// lock (section 11.7, D123, D124, D126). Anyone may call; the attest is
/// then closed, its rent back to the attester.
///
/// The attest counts when it stated the true record and no attest of the
/// lock counted before; any other is wrong or extra.
///
/// | The attest | Not burned | Burned |
/// |---|---|---|
/// | Counts | Its vETH back and its share of the fast fee | The receipt and its share of the fast fee, since it was only late |
/// | Wrong or extra | Its amount burned, the rest to the caller | Nothing |
///
/// Attests of a lock are settled in the order made, so the earliest that
/// stated the true record counts, whoever settles (D126). The rest of the
/// fast fee goes to the recipient. With no attest counting, the last
/// settled issues the true receipt to its recipient once the lock takes no
/// more attests; settled earlier, the recipient issues it after that. The
/// receipt is never minted twice.
pub fn handler(ctx: Context<SettleFast>, _claim_id: u64, _attest: u64) -> Result<()> {
    let claim = &ctx.accounts.claim;
    require!(claim.accepted, VaultError::NotAccepted);
    let f = &ctx.accounts.fast;
    let record = find_lock(&claim.records, f.lock_id).ok_or(VaultError::NotInClaim)?;
    require!(ctx.accounts.mark.lock_id == f.lock_id, VaultError::WrongAccount);
    if f.prev != 0 {
        // The attest before it is settled: its account is closed.
        let prev = ctx.accounts.prev.as_ref().ok_or(VaultError::NotInOrder)?;
        let (address, _) = Pubkey::find_program_address(&[FAST_SEED, &f.prev.to_le_bytes()], &crate::ID);
        require_keys_eq!(prev.key(), address, VaultError::WrongAccount);
        require!(prev.data_is_empty() || prev.owner != &crate::ID, VaultError::NotInOrder);
    }
    let counts = record.stated_by(f) && !ctx.accounts.mark.issued;
    let mark = &mut ctx.accounts.mark;
    mark.settled = mark.settled.checked_add(1).ok_or(VaultError::Overflow)?;
    let last = mark.settled == mark.attests && now()? >= mark.first_at + FAST_OPEN_WINDOW;
    let a = &ctx.accounts;
    let payer = a.caller.to_account_info();
    let system = a.system_program.to_account_info();
    let veth = Veth {
        config: &a.config.to_account_info(),
        config_bump: a.config.bump,
        mint: &a.mint.to_account_info(),
        holding: &a.holding.to_account_info(),
        token_program: &a.token_program,
    };
    let attester = f.attester;
    // What the attester and the caller are credited, what is minted to the
    // vault for them, and what is minted to the recipient.
    let share = fast_share(record.fast_fee, record.locked_at, f.attested_at);
    let (to_attester, minted, to_caller, to_recipient) = match (counts, f.burned) {
        (true, false) => (f.collateral.checked_add(share).ok_or(VaultError::Overflow)?, share, 0, record.fast_fee - share),
        (true, true) => {
            let v = record.amount.checked_add(share).ok_or(VaultError::Overflow)?;
            (v, v, 0, record.fast_fee - share)
        }
        // The collateral is 1.25 times the amount, rounded up: never less.
        (false, false) => (0, 0, f.collateral - f.amount, if !a.mark.issued && last { record.value()? } else { 0 }),
        (false, true) => (0, 0, 0, if !a.mark.issued && last { record.value()? } else { 0 }),
    };
    if !counts && !f.burned {
        veth.burn_held(f.amount)?;
    }
    if to_recipient > 0 {
        let to = a.to.as_ref().ok_or(VaultError::WrongAccount)?;
        require_keys_eq!(to.owner, record.recipient, VaultError::WrongAccount);
        require_keys_eq!(to.mint, a.mint.key(), VaultError::WrongAccount);
        veth.mint_to(&to.to_account_info(), to_recipient)?;
    }
    if minted > 0 {
        veth.mint_to(&a.holding.to_account_info(), minted)?;
    }
    if to_attester > 0 {
        credit_to(&a.attester_credit, &attester, 0, to_attester, &payer, &system)?;
    }
    if to_caller > 0 {
        let caller = a.caller.key();
        credit_to(&a.caller_credit, &caller, 0, to_caller, &payer, &system)?;
    }
    // The receipt is counted: by this attest, or issued to the recipient.
    if counts || to_recipient >= record.amount {
        ctx.accounts.mark.issued = true;
    }
    Ok(())
}

#[derive(Accounts)]
#[instruction(claim_id: u64, attest: u64)]
pub struct SettleFast<'info> {
    #[account(seeds = [CONFIG_SEED], bump = config.bump)]
    pub config: Box<Account<'info, Config>>,
    #[account(seeds = [CLAIM_SEED, &claim_id.to_le_bytes()], bump = claim.bump)]
    pub claim: Box<Account<'info, Claim>>,
    #[account(mut, close = attester, seeds = [FAST_SEED, &attest.to_le_bytes()], bump = fast.bump)]
    pub fast: Box<Account<'info, FastLock>>,
    #[account(mut, seeds = [LOCK_SEED, &fast.lock_id.to_le_bytes()], bump = mark.bump)]
    pub mark: Box<Account<'info, LockMark>>,
    /// CHECK: the attester, as the attest records; its rent goes back to it.
    #[account(mut, address = fast.attester)]
    pub attester: UncheckedAccount<'info>,
    #[account(mut, seeds = [MINT_SEED], bump)]
    pub mint: Box<Account<'info, Mint>>,
    #[account(mut, seeds = [HOLDING_SEED], bump)]
    pub holding: Box<Account<'info, TokenAccount>>,
    /// CHECK: the attest of the same lock made before this one, when there
    /// is one: it must be settled. Checked in the handler.
    pub prev: Option<UncheckedAccount<'info>>,
    /// The true record's recipient: for the rest of the fast fee, or its
    /// receipt when no attest delivered it.
    #[account(mut)]
    pub to: Option<Box<Account<'info, TokenAccount>>>,
    /// CHECK: the attester's credit; checked when used.
    #[account(mut)]
    pub attester_credit: UncheckedAccount<'info>,
    /// CHECK: the caller's credit; checked when used.
    #[account(mut)]
    pub caller_credit: UncheckedAccount<'info>,
    #[account(mut)]
    pub caller: Signer<'info>,
    pub token_program: Program<'info, Token>,
    pub system_program: Program<'info, System>,
}
