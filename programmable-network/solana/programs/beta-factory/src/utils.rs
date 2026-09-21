use anchor_lang::prelude::*;

use crate::constants::BPS_DENOM;
use crate::errors::FactoryError;
use crate::state::Party;

pub fn transfer_from_pda<'info>(
    amount: u64,
    from: &AccountInfo<'info>,
    to: &AccountInfo<'info>,
    system_program: &AccountInfo<'info>,
    signer_seeds: &[&[&[u8]]],
) -> Result<()> {
    if amount > 0 {
        anchor_lang::solana_program::program::invoke_signed(
            &anchor_lang::solana_program::system_instruction::transfer(from.key, to.key, amount),
            &[from.clone(), to.clone(), system_program.clone()],
            signer_seeds,
        )?;
    }
    Ok(())
}

pub fn transfer_from_signer<'info>(
    amount: u64,
    from: &AccountInfo<'info>,
    to: &AccountInfo<'info>,
    system_program: &AccountInfo<'info>,
) -> Result<()> {
    if amount > 0 {
        anchor_lang::solana_program::program::invoke(
            &anchor_lang::solana_program::system_instruction::transfer(from.key, to.key, amount),
            &[from.clone(), to.clone(), system_program.clone()],
        )?;
    }
    Ok(())
}

/// Takes up to `amount` from the party's bond: `bounty_bps` to the
/// submitter, the rest to `remainder_to`. Returns the amount actually
/// slashed (the bond may be smaller). Marks the party dead.
#[allow(clippy::too_many_arguments)]
pub fn slash<'info>(
    party: &mut Party,
    amount: u64,
    bounty_bps: u16,
    bond_escrow: &AccountInfo<'info>,
    submitter: &AccountInfo<'info>,
    remainder_to: &AccountInfo<'info>,
    system_program: &AccountInfo<'info>,
    escrow_signer_seeds: &[&[&[u8]]],
) -> Result<u64> {
    let slashed = amount.min(party.bond);
    party.bond = party.bond.checked_sub(slashed).ok_or(FactoryError::Overflow)?;
    party.dead = true;
    let bounty = slashed
        .checked_mul(bounty_bps as u64)
        .ok_or(FactoryError::Overflow)?
        / BPS_DENOM;
    let rest = slashed - bounty;
    transfer_from_pda(bounty, bond_escrow, submitter, system_program, escrow_signer_seeds)?;
    transfer_from_pda(rest, bond_escrow, remainder_to, system_program, escrow_signer_seeds)?;
    Ok(slashed)
}

/// Same as `slash`; named for readability where the party being slashed is
/// the statement-maker itself (attester / clearer) rather than an operator.
#[allow(clippy::too_many_arguments)]
pub fn slash_party<'info>(
    party: &mut Party,
    amount: u64,
    bounty_bps: u16,
    bond_escrow: &AccountInfo<'info>,
    submitter: &AccountInfo<'info>,
    remainder_to: &AccountInfo<'info>,
    system_program: &AccountInfo<'info>,
    escrow_signer_seeds: &[&[&[u8]]],
) -> Result<u64> {
    slash(party, amount, bounty_bps, bond_escrow, submitter, remainder_to, system_program, escrow_signer_seeds)
}
