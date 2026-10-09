use anchor_lang::prelude::*;
use anchor_spl::associated_token::AssociatedToken;
use anchor_spl::token_interface::{Mint, TokenAccount, TokenInterface};
use ipow_protocol::auction::auction_end;
use ipow_protocol::duty::{status, Status};
use ipow_protocol::state::Job;
use sha2::{Digest, Sha256};

use crate::constants::{ANCHOR_AGE, FUNDING_TIME, MAX_SCRIPT_LENGTH, SCRIPT_SEED, SWAP_SEED};
use ipow_protocol::duty::read_node;
use crate::errors::ConversionError;
use crate::jobs::now;
use crate::money::{take, Token};
use crate::state::{ScriptRecord, Side, Swap, SwapState};

pub fn script_key(script: &[u8]) -> [u8; 32] {
    Sha256::digest(script).into()
}

/// Buy: the job's operator locks the coin the user will receive, within 30
/// minutes of the end of the auction, and names the script the user pays.
/// The script must be new: a payment to it then belongs to this swap only.
pub fn handler(mut ctx: Context<Fund>, script: Vec<u8>) -> Result<()> {
    let a = &mut ctx.accounts;
    require!(a.swap.side == Side::Buy && a.swap.state == SwapState::Open, ConversionError::WrongState);
    require!(!script.is_empty() && script.len() <= MAX_SCRIPT_LENGTH, ConversionError::InvalidScript);
    let t = now()?;
    require!(status(&a.job, t) == Status::Assigned, ConversionError::WrongState);
    require_keys_eq!(a.operator.key(), a.job.operator, ConversionError::NotOperator);
    require!(t <= auction_end(&a.job) + FUNDING_TIME, ConversionError::FundingTimeOver);
    // The script is revealed only once the payment blocks are fixed, and
    // while most of them are still ahead.
    require!(a.job.anchored_at != 0, ConversionError::NotAnchored);
    let anchor = read_node(&a.anchor_node, &a.job.anchor).map_err(|_| error!(ConversionError::WrongAccount))?;
    require!(t <= anchor.time as i64 + ANCHOR_AGE, ConversionError::AnchorTooOld);

    let mint = a.swap.mint;
    let token = Token {
        mint: &a.mint,
        escrow: &a.escrow,
        other: &a.from,
        token_program: &a.token_program,
        associated_token_program: &a.associated_token_program,
    };
    let received = take(&a.swap, &mint, a.swap.amount, &a.operator.to_account_info(), &a.system_program.to_account_info(), &token)?;
    // What arrives must be what the user was promised.
    require!(received == a.swap.amount, ConversionError::InvalidAmount);

    a.script_record.swap_id = a.swap.id;
    a.script_record.bump = ctx.bumps.script_record;
    a.swap.script = script;
    a.swap.state = SwapState::Funded;
    Ok(())
}

#[derive(Accounts)]
#[instruction(script: Vec<u8>)]
pub struct Fund<'info> {
    #[account(mut, seeds = [SWAP_SEED, &swap.id.to_le_bytes()], bump = swap.bump)]
    pub swap: Account<'info, Swap>,
    #[account(seeds = [b"job", &swap.job_id.to_le_bytes()], bump = job.bump, seeds::program = ipow_protocol::ID)]
    pub job: Account<'info, Job>,
    /// Fails if the script was named before.
    #[account(init, payer = operator, space = 8 + ScriptRecord::INIT_SPACE, seeds = [SCRIPT_SEED, &script_key(&script)], bump)]
    pub script_record: Account<'info, ScriptRecord>,
    /// CHECK: the job's anchor in the light client; read and checked.
    pub anchor_node: UncheckedAccount<'info>,
    #[account(mut)]
    pub operator: Signer<'info>,
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
