use anchor_lang::prelude::*;
use anchor_spl::token::{Mint, Token, TokenAccount};

use crate::constants::*;
use crate::errors::VaultError;
use crate::state::{Claim, Config, FastLock};
use crate::util::{credit_to, now, Veth};

/// Burns an attester's vETH when no claim carrying its LOCK record was
/// linked within 7 days of the attest, or the linked claim was refused
/// (section 11.7, D123): the amount, in place of the receipt minted, and
/// the rest to the caller. If the true record arrives later, the receipt
/// goes to the attester. Anyone may call.
pub fn handler(ctx: Context<BurnFast>, _attest: u64) -> Result<()> {
    let f = &ctx.accounts.fast;
    require!(!f.burned, VaultError::AlreadyDone);
    if f.claim == 0 {
        require!(now()? >= f.attested_at + FAST_OPEN_WINDOW, VaultError::WindowNotOver);
    } else {
        let linked = ctx.accounts.linked.as_ref().ok_or(VaultError::WrongAccount)?;
        require!(linked.id == f.claim, VaultError::WrongAccount);
        require!(linked.decided && !linked.accepted, VaultError::NotRefused);
    }
    // The collateral is 1.25 times the amount, rounded up: never less.
    let (amount, rest) = (f.amount, f.collateral - f.amount);
    ctx.accounts.fast.burned = true;
    let a = &ctx.accounts;
    let caller = a.caller.key();
    credit_to(&a.caller_credit, &caller, 0, rest, &a.caller.to_account_info(), &a.system_program.to_account_info())?;
    let veth = Veth {
        config: &a.config.to_account_info(),
        config_bump: a.config.bump,
        mint: &a.mint.to_account_info(),
        holding: &a.holding.to_account_info(),
        token_program: &a.token_program,
    };
    veth.burn_held(amount)
}

#[derive(Accounts)]
#[instruction(attest: u64)]
pub struct BurnFast<'info> {
    #[account(seeds = [CONFIG_SEED], bump = config.bump)]
    pub config: Box<Account<'info, Config>>,
    #[account(mut, seeds = [FAST_SEED, &attest.to_le_bytes()], bump = fast.bump)]
    pub fast: Box<Account<'info, FastLock>>,
    /// The claim linked to the attest, when there is one: it must be refused.
    pub linked: Option<Box<Account<'info, Claim>>>,
    #[account(mut, seeds = [MINT_SEED], bump)]
    pub mint: Box<Account<'info, Mint>>,
    #[account(mut, seeds = [HOLDING_SEED], bump)]
    pub holding: Box<Account<'info, TokenAccount>>,
    /// CHECK: the caller's credit; checked when used.
    #[account(mut)]
    pub caller_credit: UncheckedAccount<'info>,
    #[account(mut)]
    pub caller: Signer<'info>,
    pub token_program: Program<'info, Token>,
    pub system_program: Program<'info, System>,
}
