use anchor_lang::prelude::*;
use anchor_spl::associated_token::AssociatedToken;
use anchor_spl::token_interface::{Mint, TokenAccount, TokenInterface};

use crate::constants::{CONFIG_SEED, MAX_SCRIPT_LENGTH, SWAP_SEED};
use crate::errors::ConversionError;
use crate::jobs::{open_job, Open};
use crate::money::{take, Token};
use crate::state::{Config, Side, Swap, SwapState};

/// Sell: locks `amount` of SOL, or of `mint`'s token, for at least `sats`
/// paid to `script`. `paid` lamports pay the job's fees.
pub fn handler(mut ctx: Context<Sell>, amount: u64, sats: u64, script: Vec<u8>, confirmations: u16, paid: u64) -> Result<()> {
    require!(amount > 0 && sats > 0, ConversionError::InvalidAmount);
    require!(!script.is_empty() && script.len() <= MAX_SCRIPT_LENGTH, ConversionError::InvalidScript);
    let a = &mut ctx.accounts;
    a.config.swap_count = a.config.swap_count.checked_add(1).ok_or(ConversionError::Overflow)?;
    let id = a.config.swap_count;
    let mint = a.mint.as_ref().map_or(Pubkey::default(), |m| m.key());

    let token = Token {
        mint: &a.mint,
        escrow: &a.escrow,
        other: &a.from,
        token_program: &a.token_program,
        associated_token_program: &a.associated_token_program,
    };
    let received = take(&a.swap, &mint, amount, &a.user.to_account_info(), &a.system_program.to_account_info(), &token)?;
    require!(received > 0, ConversionError::InvalidAmount);

    let job_id = open_job(
        Open {
            protocol: &a.protocol,
            application: &a.application,
            job: &a.job,
            tag_record: &a.tag_record,
            protocol_vault: &a.protocol_vault,
            config: a.config.to_account_info(),
            config_bump: a.config.bump,
            funder: a.user.to_account_info(),
            system_program: a.system_program.to_account_info(),
        },
        id,
        confirmations,
        0,
        a.user.key(),
        paid,
    )?;

    let s = &mut a.swap;
    s.id = id;
    s.side = Side::Sell;
    s.state = SwapState::Open;
    s.user = a.user.key();
    s.mint = mint;
    s.amount = received;
    s.sats = sats;
    s.job_id = job_id;
    s.script = script;
    s.bump = ctx.bumps.swap;
    Ok(())
}

#[derive(Accounts)]
pub struct Sell<'info> {
    #[account(mut, seeds = [CONFIG_SEED], bump = config.bump)]
    pub config: Account<'info, Config>,
    #[account(init, payer = user, space = 8 + Swap::INIT_SPACE, seeds = [SWAP_SEED, &(config.swap_count + 1).to_le_bytes()], bump)]
    pub swap: Account<'info, Swap>,
    #[account(mut)]
    pub user: Signer<'info>,
    #[account(mut, seeds = [b"protocol"], bump = protocol.bump, seeds::program = ipow_protocol::ID)]
    pub protocol: Account<'info, ipow_protocol::state::Protocol>,
    /// CHECK: this application's record in the protocol; the protocol checks it.
    pub application: UncheckedAccount<'info>,
    /// CHECK: the job, created by the protocol.
    #[account(mut)]
    pub job: UncheckedAccount<'info>,
    /// CHECK: the job's tag record, created by the protocol.
    #[account(mut)]
    pub tag_record: UncheckedAccount<'info>,
    /// CHECK: the protocol's vault; the protocol checks it.
    #[account(mut)]
    pub protocol_vault: UncheckedAccount<'info>,
    /// CHECK: the protocol program.
    #[account(address = ipow_protocol::ID)]
    pub protocol_program: UncheckedAccount<'info>,
    pub system_program: Program<'info, System>,
    pub mint: Option<InterfaceAccount<'info, Mint>>,
    /// CHECK: the swap's associated token account; checked when used.
    #[account(mut)]
    pub escrow: Option<UncheckedAccount<'info>>,
    #[account(mut)]
    pub from: Option<InterfaceAccount<'info, TokenAccount>>,
    pub token_program: Option<Interface<'info, TokenInterface>>,
    pub associated_token_program: Option<Program<'info, AssociatedToken>>,
}
