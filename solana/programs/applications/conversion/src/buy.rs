use anchor_lang::prelude::*;
use anchor_spl::token_interface::Mint;

use crate::constants::{CLOSE, CONFIG_SEED, MAX_MEMO, MAX_SATS, MEMO_SEED, SWAP_SEED};
use crate::errors::ConversionError;
use crate::jobs::{open_job, Open};
use crate::state::{Config, Memo, Side, Swap, SwapState};

/// The accounts every buy opens with, whichever instruction it came by.
struct Parts<'a, 'info> {
    config: &'a mut Account<'info, Config>,
    swap: &'a mut Account<'info, Swap>,
    user: &'a Signer<'info>,
    protocol: &'a Account<'info, ipow_protocol::state::Protocol>,
    application: &'a UncheckedAccount<'info>,
    job: &'a UncheckedAccount<'info>,
    tag_record: &'a UncheckedAccount<'info>,
    protocol_vault: &'a UncheckedAccount<'info>,
    system_program: &'a Program<'info, System>,
    mint: &'a Option<InterfaceAccount<'info, Mint>>,
}

macro_rules! parts {
    ($a:ident) => {
        Parts {
            config: &mut $a.config,
            swap: &mut $a.swap,
            user: &$a.user,
            protocol: &$a.protocol,
            application: &$a.application,
            job: &$a.job,
            tag_record: &$a.tag_record,
            protocol_vault: &$a.protocol_vault,
            system_program: &$a.system_program,
            mint: &$a.mint,
        }
    };
}
use parts;

/// Buy: asks for `amount` of SOL, or of `mint`'s token, for `sats` paid on
/// Bitcoin. `paid` lamports pay the job's fees.
pub fn handler(ctx: Context<Buy>, amount: u64, sats: u64, confirmations: u16, paid: u64) -> Result<()> {
    let user = ctx.accounts.user.key();
    let bump = ctx.bumps.swap;
    let a = ctx.accounts;
    buy(parts!(a), bump, user, amount, sats, confirmations, paid)
}

/// A buy whose coin goes to `recipient`, the signer paying the fees (T2,
/// D21): an operator opens the buy of a tunnel for the user, who then signs
/// only on the other network.
pub fn for_recipient(ctx: Context<Buy>, recipient: Pubkey, amount: u64, sats: u64, confirmations: u16, paid: u64) -> Result<()> {
    require!(recipient != Pubkey::default(), ConversionError::ZeroRecipient);
    let bump = ctx.bumps.swap;
    let a = ctx.accounts;
    buy(parts!(a), bump, recipient, amount, sats, confirmations, paid)
}

/// A buy that keeps a memo for operators, as a buy on an EVM network does
/// (`buy`'s `memo` there): 1 to 128 bytes, not read by the program. A
/// tunnel's buy names in it the sale that will pay it, so an operator
/// promises that sale with no further word from the user.
pub fn with_memo(ctx: Context<BuyWithMemo>, amount: u64, sats: u64, confirmations: u16, paid: u64, memo: Vec<u8>) -> Result<()> {
    require!(!memo.is_empty() && memo.len() <= MAX_MEMO, ConversionError::InvalidMemo);
    let user = ctx.accounts.user.key();
    let bump = ctx.bumps.swap;
    let memo_bump = ctx.bumps.memo;
    let a = ctx.accounts;
    buy(parts!(a), bump, user, amount, sats, confirmations, paid)?;
    a.memo.swap_id = a.swap.id;
    a.memo.text = memo;
    a.memo.bump = memo_bump;
    Ok(())
}

fn buy(a: Parts, swap_bump: u8, recipient: Pubkey, amount: u64, sats: u64, confirmations: u16, paid: u64) -> Result<()> {
    require!(amount > 0 && sats > 0, ConversionError::InvalidAmount);
    require!(sats <= MAX_SATS, ConversionError::TooLarge);
    a.config.swap_count = a.config.swap_count.checked_add(1).ok_or(ConversionError::Overflow)?;
    let id = a.config.swap_count;
    let job_id = open_job(
        Open {
            protocol: a.protocol,
            application: a.application,
            job: a.job,
            tag_record: a.tag_record,
            protocol_vault: a.protocol_vault,
            config: a.config.to_account_info(),
            config_bump: a.config.bump,
            funder: a.user.to_account_info(),
            system_program: a.system_program.to_account_info(),
        },
        id,
        confirmations,
        CLOSE,
        a.user.key(),
        paid,
    )?;
    let s = a.swap;
    s.id = id;
    s.side = Side::Buy;
    s.state = SwapState::Open;
    s.user = recipient;
    s.mint = a.mint.as_ref().map_or(Pubkey::default(), |m| m.key());
    s.amount = amount;
    s.sats = sats;
    s.job_id = job_id;
    s.bump = swap_bump;
    Ok(())
}

#[derive(Accounts)]
pub struct Buy<'info> {
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
}

#[derive(Accounts)]
pub struct BuyWithMemo<'info> {
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
    /// The memo's own account, beside the swap; the user pays its rent.
    #[account(init, payer = user, space = 8 + Memo::INIT_SPACE, seeds = [MEMO_SEED, &(config.swap_count + 1).to_le_bytes()], bump)]
    pub memo: Account<'info, Memo>,
}
