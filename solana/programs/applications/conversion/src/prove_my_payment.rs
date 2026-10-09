use anchor_lang::prelude::*;
use anchor_spl::token_interface::{Mint, TokenAccount, TokenInterface};
use ipow_light_client::utils::tx_in_block;
use ipow_protocol::duty::{deadline, read_node, read_walk, status, Status};
use ipow_protocol::state::{BlockRef, Job};

use crate::constants::{PAY_BLOCKS, SWAP_SEED};
use crate::errors::ConversionError;
use crate::jobs::now;
use crate::money::{pay, Token};
use crate::payment::output_pays;
use crate::state::{Side, Swap, SwapState};

/// Buy: the user proves their payment themselves (D10). Anyone may call;
/// the coin goes to the user. The payment is in one of the payment blocks,
/// and:
/// - when the operator proved a close mined after the payment blocks, in a
///   block below that close, on the chain the operator proved, which the
///   protocol's guardians watch: `walk_low` goes from the payment's block
///   down to the anchor, `walk_high` from the close's block down to the
///   payment's block, and `top` is not used;
/// - otherwise, once the operator can no longer prove (its deadline passed,
///   it was slashed, or its close came within the payment blocks), in a
///   block on top of the anchor's parent, so a reorganisation that took the
///   anchor away does not strand it: `walk_low` goes down to that parent,
///   `walk_high` from `top` down to the payment's block, and `top` gives
///   the job's confirmations. `parent_epoch_time` is the parent's epoch time
///   when the anchor is the first block of its epoch, unused otherwise.
#[allow(clippy::too_many_arguments)]
pub fn handler(
    ctx: Context<ProveMyPayment>,
    payment_raw: Vec<u8>,
    vout: u32,
    pay_block: BlockRef,
    siblings: Vec<[u8; 32]>,
    tx_index: u64,
    top: BlockRef,
    parent_epoch_time: u32,
) -> Result<()> {
    let a = &ctx.accounts;
    require!(a.swap.side == Side::Buy && a.swap.state == SwapState::Funded, ConversionError::WrongState);
    let job = &a.job;
    require!(job.anchored_at != 0, ConversionError::NotLinked);
    let anchor = job.anchor;
    require!(
        pay_block.height > anchor.height && pay_block.height <= anchor.height + PAY_BLOCKS,
        ConversionError::OutsidePaymentBlocks
    );
    let t = now()?;
    let s = status(job, t);
    let proven = s == Status::Proven || s == Status::Settled;
    let linked = if proven && !job.slashed && job.proof_block.height > anchor.height + PAY_BLOCKS {
        // Below the operator's close, on its proven chain.
        read_walk(&a.walk_low, &pay_block, &anchor).is_ok() && read_walk(&a.walk_high, &job.proof_block, &pay_block).is_ok()
    } else {
        let failed = s == Status::Slashed || proven || (s == Status::Assigned && t > deadline(job)?);
        require!(failed, ConversionError::NotLinked);
        require!(
            top.height >= pay_block.height && (top.height - pay_block.height) as u64 + 1 >= job.confirmations as u64,
            ConversionError::TooFewConfirmations
        );
        let anchor_node = read_node(&a.anchor_node, &anchor).map_err(|_| error!(ConversionError::WrongAccount))?;
        let parent = BlockRef {
            hash: anchor_node.prev_hash,
            height: anchor.height - 1,
            epoch_time: if anchor.height % 2016 == 0 { parent_epoch_time } else { anchor.epoch_time },
        };
        read_walk(&a.walk_low, &pay_block, &parent).is_ok() && read_walk(&a.walk_high, &top, &pay_block).is_ok()
    };
    require!(linked, ConversionError::NotLinked);
    let node = read_node(&a.pay_node, &pay_block).map_err(|_| error!(ConversionError::NotInBlock))?;
    require!(tx_in_block(&node, &payment_raw, &siblings, tx_index)?, ConversionError::NotInBlock);
    require!(output_pays(&payment_raw, vout, &a.swap.script, a.swap.sats)?, ConversionError::NotPaid);
    require_keys_eq!(a.user.key(), a.swap.user, ConversionError::WrongAccount);
    let token = Token { mint: &a.mint, escrow: &a.escrow, other: &a.to, token_program: &a.token_program, associated_token_program: &None };
    pay(&a.swap, &a.user.to_account_info(), &token)?;
    ctx.accounts.swap.state = SwapState::Done;
    Ok(())
}

#[derive(Accounts)]
pub struct ProveMyPayment<'info> {
    #[account(mut, seeds = [SWAP_SEED, &swap.id.to_le_bytes()], bump = swap.bump)]
    pub swap: Account<'info, Swap>,
    #[account(seeds = [b"job", &swap.job_id.to_le_bytes()], bump = job.bump, seeds::program = ipow_protocol::ID)]
    pub job: Account<'info, Job>,
    /// CHECK: the job's anchor in the light client; read and checked.
    pub anchor_node: UncheckedAccount<'info>,
    /// CHECK: the payment's block in the light client; read and checked.
    pub pay_node: UncheckedAccount<'info>,
    /// CHECK: a finished walk from the payment's block down; read and checked.
    pub walk_low: UncheckedAccount<'info>,
    /// CHECK: a finished walk down to the payment's block; read and checked.
    pub walk_high: UncheckedAccount<'info>,
    /// CHECK: the swap's user; checked in the handler.
    #[account(mut)]
    pub user: UncheckedAccount<'info>,
    pub mint: Option<InterfaceAccount<'info, Mint>>,
    /// CHECK: the swap's associated token account; checked when used.
    #[account(mut)]
    pub escrow: Option<UncheckedAccount<'info>>,
    #[account(mut)]
    pub to: Option<InterfaceAccount<'info, TokenAccount>>,
    pub token_program: Option<Interface<'info, TokenInterface>>,
}
