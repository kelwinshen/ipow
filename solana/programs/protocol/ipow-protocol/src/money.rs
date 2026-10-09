//! All money of the protocol sits in one vault account. Records say whose
//! it is. Money is never pushed to an address: it is credited, and the
//! address withdraws it.

use anchor_lang::prelude::*;
use anchor_lang::system_program::{transfer, Transfer};

use crate::errors::ProtocolError;
use crate::state::Credit;

pub fn pay_in<'info>(
    from: &AccountInfo<'info>,
    vault: &AccountInfo<'info>,
    system_program: &AccountInfo<'info>,
    amount: u64,
) -> Result<()> {
    transfer(
        CpiContext::new(system_program.key(), Transfer { from: from.clone(), to: vault.clone() }),
        amount,
    )
}

pub fn pay_out<'info>(vault: &AccountInfo<'info>, to: &AccountInfo<'info>, amount: u64) -> Result<()> {
    vault.sub_lamports(amount)?;
    to.add_lamports(amount)?;
    Ok(())
}

pub fn credit(record: &mut Credit, owner: Pubkey, amount: u64) -> Result<()> {
    record.owner = owner;
    record.amount = record.amount.checked_add(amount).ok_or(ProtocolError::Overflow)?;
    emit!(crate::Credited { to: owner, amount });
    Ok(())
}
