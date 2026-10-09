use anchor_lang::prelude::*;

use crate::constants::{APPLICATION_SEED, MAX_CHALLENGE_PERIOD, MAX_CLAIM_KINDS, MIN_CHALLENGE_PERIOD};
use crate::errors::ProtocolError;
use crate::state::Application;

/// Registers the signer as an application. Anyone may (D63). The challenge
/// period of each kind of claim is set here and never changes (D17, D28).
/// The account can only be created once, so an application registers once.
pub fn handler(ctx: Context<RegisterApplication>, challenge_periods: Vec<u32>) -> Result<()> {
    require!(challenge_periods.len() <= MAX_CLAIM_KINDS, ProtocolError::TooManyClaimKinds);
    for p in &challenge_periods {
        require!(
            (MIN_CHALLENGE_PERIOD..=MAX_CHALLENGE_PERIOD).contains(p),
            ProtocolError::ChallengePeriodOutOfRange
        );
    }
    let app = &mut ctx.accounts.application;
    app.key = ctx.accounts.key.key();
    app.challenge_periods = challenge_periods;
    app.bump = ctx.bumps.application;
    emit!(crate::ApplicationRegistered { application: app.key });
    Ok(())
}

#[derive(Accounts)]
pub struct RegisterApplication<'info> {
    #[account(
        init,
        payer = funder,
        space = 8 + Application::INIT_SPACE,
        seeds = [APPLICATION_SEED, key.key().as_ref()],
        bump
    )]
    pub application: Account<'info, Application>,
    /// The application's own key. A program registers with one of its PDAs.
    pub key: Signer<'info>,
    #[account(mut)]
    pub funder: Signer<'info>,
    pub system_program: Program<'info, System>,
}
