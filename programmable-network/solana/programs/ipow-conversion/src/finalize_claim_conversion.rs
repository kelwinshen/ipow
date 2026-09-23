use anchor_lang::prelude::*;
use ipow::state::GlobalState as IpowGlobalState;

use crate::constants::{CLAIM_QUIET_PERIOD_SEC, DIFF_PERIOD};
use crate::errors::ConversionError;
use crate::state::{Conversion, ConversionStatus};

/// Permissionless, mirrors `ipow-message-relay`'s `finalize_claim`: once the
/// quiet period passes with no better stake, locks in the current highest
/// staker and starts the duty window (what `approve_conversion` used to do
/// unconditionally, since the "auction" there was really just "the one
/// operator shows up").
///
/// For bitcoin->native, the payout value itself was already self-escrowed by
/// the (now locked-in) claimant back in `propose_claim_conversion` — this
/// just records that amount onto `conversion.reserved_native` for later
/// payout/force-claim to read. No pool/liquidity account or check needed
/// here anymore.
pub fn handler(ctx: Context<FinalizeClaimConversion>) -> Result<()> {
    let conversion = &mut ctx.accounts.conversion;

    // Same two-situation guard `propose_claim_conversion` uses — see its
    // comment.
    require!(
        conversion.status == ConversionStatus::Committed
            || ((conversion.status == ConversionStatus::Approved
                || conversion.status == ConversionStatus::Deposited)
                && conversion.operator_duty_expires_at == 0),
        ConversionError::BadState
    );
    require!(
        conversion.responsible_operator != Pubkey::default(),
        ConversionError::NoClaimsYet
    );

    let now = Clock::get()?.unix_timestamp;
    require!(
        now > conversion.last_claim_at + CLAIM_QUIET_PERIOD_SEC,
        ConversionError::ClaimWindowStillOpen
    );

    // `Committed` -> `Approved` on every finalize (first time or reopened-
    // and-never-deposited); `Deposited`/`Approved` (bitcoin->native reopen)
    // stay as-is — the underlying flow they represent (a real deposit
    // already sitting in escrow, or a real reservation already made) hasn't
    // changed, only who's responsible for finishing it has.
    if conversion.status == ConversionStatus::Committed {
        conversion.status = ConversionStatus::Approved;
    }
    conversion.operator_duty_expires_at = now
        .checked_add(conversion.duty_window_seconds)
        .unwrap();

    // Window/epoch height and (for bitcoin->native) the reservation bookkeeping
    // are set exactly once, at the *first* finalize — reopening must never
    // reset the user's own downstream deadlines
    // (`refund_no_proof_native_to_bitcoin`/force-claim both derive from
    // `window_start_height`) or re-derive a reservation that's already real.
    if !conversion.window_started {
        let tip = ctx.accounts.ipow_global_state.global_tip_height;
        conversion.window_started = true;
        conversion.window_start_height = tip;
        conversion.epoch_start_height = tip - (tip % DIFF_PERIOD);

        if !conversion.is_native_to_bitcoin {
            conversion.reserved_native = conversion.native_amount;
        }
    }

    Ok(())
}

#[derive(Accounts)]
pub struct FinalizeClaimConversion<'info> {
    #[account(mut)]
    pub conversion: Account<'info, Conversion>,

    #[account(seeds = [b"global_state"], bump, seeds::program = ipow::ID)]
    pub ipow_global_state: Account<'info, IpowGlobalState>,
}
