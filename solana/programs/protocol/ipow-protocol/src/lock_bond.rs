use anchor_lang::prelude::*;

use crate::constants::{OPERATOR_SEED, VAULT_SEED, PROTOCOL_SEED};
use crate::errors::ProtocolError;
use crate::money::pay_in;
use crate::state::{Operator, Protocol};

/// Locks a bond. This is all it takes to be an operator (D12). There is no
/// minimum (D43).
pub fn handler(ctx: Context<LockBond>, amount: u64) -> Result<()> {
    require!(amount > 0, ProtocolError::ZeroAmount);
    pay_in(
        &ctx.accounts.owner.to_account_info(),
        &ctx.accounts.vault.to_account_info(),
        &ctx.accounts.system_program.to_account_info(),
        amount,
    )?;
    let operator = &mut ctx.accounts.operator;
    operator.owner = ctx.accounts.owner.key();
    operator.bump = ctx.bumps.operator;
    operator.bond = operator.bond.checked_add(amount).ok_or(ProtocolError::Overflow)?;
    emit!(crate::BondLocked { operator: operator.owner, amount });
    Ok(())
}

#[derive(Accounts)]
pub struct LockBond<'info> {
    #[account(seeds = [PROTOCOL_SEED], bump = protocol.bump)]
    pub protocol: Account<'info, Protocol>,
    #[account(
        init_if_needed,
        payer = owner,
        space = 8 + Operator::INIT_SPACE,
        seeds = [OPERATOR_SEED, owner.key().as_ref()],
        bump
    )]
    pub operator: Account<'info, Operator>,
    /// CHECK: the protocol's vault.
    #[account(mut, seeds = [VAULT_SEED], bump = protocol.vault_bump)]
    pub vault: UncheckedAccount<'info>,
    #[account(mut)]
    pub owner: Signer<'info>,
    pub system_program: Program<'info, System>,
}
