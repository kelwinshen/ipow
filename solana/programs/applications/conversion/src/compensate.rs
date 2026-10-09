use anchor_lang::prelude::*;
use ipow_protocol::duty::{status, Status};
use ipow_protocol::state::Job;

use crate::compensation::pass_on;
use crate::constants::{CONFIG_SEED, SWAP_SEED};
use crate::errors::ConversionError;
use crate::jobs::now;
use crate::state::{Config, Swap};

/// Passes on to the swap's user this application's share of its job's
/// escrow, once the job is slashed, in either direction. Once per swap.
/// Anyone may call, whenever the slash happened.
pub fn handler(ctx: Context<Compensate>) -> Result<()> {
    let a = ctx.accounts;
    require!(status(&a.job, now()?) == Status::Slashed, ConversionError::NotSlashed);
    let user = a.user.to_account_info();
    pass_on(&mut a.swap, &mut a.config, &a.job, &user, &a.credit, &a.protocol, &a.protocol_vault)
}

#[derive(Accounts)]
pub struct Compensate<'info> {
    #[account(mut, seeds = [SWAP_SEED, &swap.id.to_le_bytes()], bump = swap.bump)]
    pub swap: Account<'info, Swap>,
    #[account(seeds = [b"job", &swap.job_id.to_le_bytes()], bump = job.bump, seeds::program = ipow_protocol::ID)]
    pub job: Account<'info, Job>,
    /// CHECK: the swap's user; checked against the swap.
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
}
