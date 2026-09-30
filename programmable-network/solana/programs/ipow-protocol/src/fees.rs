//! Window, deadline and fees (D14, D24, D58, D68, D69).

use anchor_lang::prelude::*;

use crate::constants::*;
use crate::errors::ProtocolError;

/// The window of a job: the proof range plus the blocks that only confirm
/// (D15, D68).
pub fn window_of(confirmations: u16) -> Result<u16> {
    require!(confirmations >= DEFAULT_CONFIRMATIONS, ProtocolError::ConfirmationsOutOfRange);
    let window = PROOF_RANGE as u32 + confirmations as u32 - 1;
    require!(window <= MAX_WINDOW as u32, ProtocolError::ConfirmationsOutOfRange);
    Ok(window as u16)
}

/// D69: the time an operator has, from the moment it is locked in.
pub fn duty_time_for(confirmations: u16) -> Result<i64> {
    Ok(window_of(confirmations)? as i64 * DEADLINE_PER_BLOCK)
}

/// D24, D58, D69: amount of work x the network's price x 1.5. On Solana the
/// price is the signature fee, set by the network, not by a person.
pub fn commitment_fee_for(confirmations: u16) -> Result<u64> {
    let work = (window_of(confirmations)? as u64 + FIXED_SIGNATURES)
        .checked_mul(SIGNATURE_FEE)
        .ok_or(ProtocolError::Overflow)?;
    Ok(work * MARGIN_NUMERATOR / MARGIN_DENOMINATOR)
}
