use anchor_lang::prelude::*;

use crate::errors::FactoryError;
use crate::state::Party;

pub fn handler(ctx: Context<RequestUnbond>) -> Result<()> {
    let party = &mut ctx.accounts.party;
    require!(!party.dead, FactoryError::PartyDead);
    party.unbond_requested_at = Clock::get()?.unix_timestamp;
    Ok(())
}

#[derive(Accounts)]
pub struct RequestUnbond<'info> {
    #[account(mut, has_one = owner @ FactoryError::Unauthorized)]
    pub party: Account<'info, Party>,
    pub owner: Signer<'info>,
}
