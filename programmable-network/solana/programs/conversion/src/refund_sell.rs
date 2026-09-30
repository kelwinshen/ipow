use anchor_lang::prelude::*;
use anchor_spl::token_interface::{Mint, TokenAccount, TokenInterface};
use ipow_protocol::duty::{deadline, status, Status};
use ipow_protocol::state::Job;

use crate::constants::{CONFIG_SEED, SWAP_SEED};
use crate::errors::ConversionError;
use crate::jobs::{now, require_proven};
use crate::money::{pay, Token};
use crate::payment::pays_at_least;
use crate::compensation::pass_on;
use crate::state::{Config, Side, Swap, SwapState};

/// Sell: gives the user the coin back when the operator did not pay: nobody
/// took the job, the deadline passed with no proof, the proof was shown
/// false, or the proven transaction (`raw_tx`) does not pay the user enough.
/// After a slash, the user also receives this application's share of the
/// escrow. Anyone may call.
pub fn handler(ctx: Context<RefundSell>, raw_tx: Vec<u8>) -> Result<()> {
    let a = &ctx.accounts;
    require!(a.swap.side == Side::Sell && a.swap.state == SwapState::Open, ConversionError::WrongState);
    let job = &a.job;
    let s = status(job, now()?);
    let refundable = match s {
        Status::Expired | Status::Slashed => true,
        Status::Assigned => now()? > deadline(job)?,
        Status::Proven | Status::Settled => {
            require_proven(job, &raw_tx)?;
            !pays_at_least(&raw_tx, &a.swap.script, a.swap.sats)?
        }
        Status::Auction => false,
    };
    require!(refundable, ConversionError::NotRefundable);
    require_keys_eq!(a.user.key(), a.swap.user, ConversionError::WrongAccount);
    let token = Token { mint: &a.mint, escrow: &a.escrow, other: &a.to, token_program: &a.token_program, associated_token_program: &None };
    pay(&a.swap, &a.user.to_account_info(), &token)?;
    ctx.accounts.swap.state = SwapState::Refunded;
    if s == Status::Slashed {
        let a = ctx.accounts;
        let user = a.user.to_account_info();
        pass_on(&mut a.swap, &mut a.config, &a.job, &user, &a.credit, &a.protocol, &a.protocol_vault)?;
    }
    Ok(())
}

#[derive(Accounts)]
pub struct RefundSell<'info> {
    #[account(mut, seeds = [SWAP_SEED, &swap.id.to_le_bytes()], bump = swap.bump)]
    pub swap: Account<'info, Swap>,
    #[account(seeds = [b"job", &swap.job_id.to_le_bytes()], bump = job.bump, seeds::program = ipow_protocol::ID)]
    pub job: Account<'info, Job>,
    /// CHECK: the swap's user; checked in the handler.
    #[account(mut)]
    pub user: UncheckedAccount<'info>,
    #[account(mut, seeds = [CONFIG_SEED], bump = config.bump)]
    pub config: Account<'info, Config>,
    /// CHECK: this application's credit record in the protocol, at its one
    /// address; it may not exist yet. Required, so nobody can leave it out
    /// and mark the swap compensated with nothing paid.
    #[account(mut, seeds = [b"credit", config.key().as_ref()], bump, seeds::program = ipow_protocol::ID)]
    pub credit: UncheckedAccount<'info>,
    /// CHECK: the protocol's record, at its one address.
    #[account(seeds = [b"protocol"], bump, seeds::program = ipow_protocol::ID)]
    pub protocol: UncheckedAccount<'info>,
    /// CHECK: the protocol's vault, at its one address.
    #[account(mut, seeds = [b"vault"], bump, seeds::program = ipow_protocol::ID)]
    pub protocol_vault: UncheckedAccount<'info>,
    /// CHECK: the protocol program, for the withdrawal.
    #[account(address = ipow_protocol::ID)]
    pub protocol_program: UncheckedAccount<'info>,
    pub mint: Option<InterfaceAccount<'info, Mint>>,
    /// CHECK: the swap's associated token account; checked when used.
    #[account(mut)]
    pub escrow: Option<UncheckedAccount<'info>>,
    #[account(mut)]
    pub to: Option<InterfaceAccount<'info, TokenAccount>>,
    pub token_program: Option<Interface<'info, TokenInterface>>,
}
