use anchor_lang::prelude::*;

use crate::constants::{CLOSE_PERIOD, CONFIG_SEED};
use crate::state::Config;

/// Creates the settings record and registers the application with the
/// protocol, with one kind of claim: a close, challenged for 36 hours.
/// Anyone may call, once; nothing in it can be chosen.
pub fn handler(ctx: Context<Initialize>) -> Result<()> {
    ctx.accounts.config.bump = ctx.bumps.config;
    let seeds: &[&[u8]] = &[CONFIG_SEED, &[ctx.bumps.config]];
    ipow_protocol::cpi::register_application(
        CpiContext::new_with_signer(
            ipow_protocol::ID,
            ipow_protocol::cpi::accounts::RegisterApplication {
                application: ctx.accounts.application.to_account_info(),
                key: ctx.accounts.config.to_account_info(),
                funder: ctx.accounts.payer.to_account_info(),
                system_program: ctx.accounts.system_program.to_account_info(),
            },
            &[seeds],
        ),
        vec![CLOSE_PERIOD],
    )
}

#[derive(Accounts)]
pub struct Initialize<'info> {
    #[account(init, payer = payer, space = 8 + Config::INIT_SPACE, seeds = [CONFIG_SEED], bump)]
    pub config: Account<'info, Config>,
    /// CHECK: the application's record in the protocol, created by it.
    #[account(mut)]
    pub application: UncheckedAccount<'info>,
    #[account(mut)]
    pub payer: Signer<'info>,
    /// CHECK: the protocol program.
    #[account(address = ipow_protocol::ID)]
    pub protocol_program: UncheckedAccount<'info>,
    pub system_program: Program<'info, System>,
}
