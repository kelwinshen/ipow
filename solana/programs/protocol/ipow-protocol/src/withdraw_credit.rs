use anchor_lang::prelude::*;

use crate::constants::{CREDIT_SEED, PROTOCOL_SEED, VAULT_SEED};
use crate::errors::ProtocolError;
use crate::money::pay_out;
use crate::state::{Credit, Protocol};

pub fn handler(ctx: Context<WithdrawCredit>) -> Result<()> {
    let record = &mut ctx.accounts.credit;
    let amount = record.amount;
    require!(amount > 0, ProtocolError::NothingToWithdraw);
    record.amount = 0;
    pay_out(&ctx.accounts.vault.to_account_info(), &ctx.accounts.owner.to_account_info(), amount)?;
    emit!(crate::CreditWithdrawn { to: record.owner, amount });
    Ok(())
}

#[derive(Accounts)]
pub struct WithdrawCredit<'info> {
    #[account(seeds = [PROTOCOL_SEED], bump = protocol.bump)]
    pub protocol: Account<'info, Protocol>,
    #[account(mut, seeds = [CREDIT_SEED, owner.key().as_ref()], bump = credit.bump)]
    pub credit: Account<'info, Credit>,
    /// CHECK: the protocol's vault.
    #[account(mut, seeds = [VAULT_SEED], bump = protocol.vault_bump)]
    pub vault: UncheckedAccount<'info>,
    #[account(mut)]
    pub owner: Signer<'info>,
}
