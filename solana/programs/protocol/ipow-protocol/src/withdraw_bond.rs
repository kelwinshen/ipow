use anchor_lang::prelude::*;

use crate::constants::{OPERATOR_SEED, PROTOCOL_SEED, VAULT_SEED};
use crate::errors::ProtocolError;
use crate::money::pay_out;
use crate::state::{Operator, Protocol};

/// Withdraws free bond. Bond that is locked for a job stays (D43).
pub fn handler(ctx: Context<WithdrawBond>, amount: u64) -> Result<()> {
    require!(amount > 0, ProtocolError::ZeroAmount);
    let operator = &mut ctx.accounts.operator;
    require!(amount <= operator.bond - operator.locked, ProtocolError::BondNotFree);
    operator.bond -= amount;
    pay_out(&ctx.accounts.vault.to_account_info(), &ctx.accounts.owner.to_account_info(), amount)?;
    emit!(crate::BondWithdrawn { operator: operator.owner, amount });
    Ok(())
}

#[derive(Accounts)]
pub struct WithdrawBond<'info> {
    #[account(seeds = [PROTOCOL_SEED], bump = protocol.bump)]
    pub protocol: Account<'info, Protocol>,
    #[account(mut, seeds = [OPERATOR_SEED, owner.key().as_ref()], bump = operator.bump)]
    pub operator: Account<'info, Operator>,
    /// CHECK: the protocol's vault.
    #[account(mut, seeds = [VAULT_SEED], bump = protocol.vault_bump)]
    pub vault: UncheckedAccount<'info>,
    #[account(mut)]
    pub owner: Signer<'info>,
}
