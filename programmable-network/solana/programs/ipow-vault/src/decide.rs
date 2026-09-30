use anchor_lang::prelude::*;

use crate::constants::*;
use crate::errors::VaultError;
use crate::state::{Chain, Claim, Config};
use crate::util::now;

/// Decides a claim whose last statement has stood for 7 days. Anyone may
/// call. A claim of a chain proven false here, or that lost another claim
/// here, is refused (D109, D111). Each winner collects its own share
/// afterwards; what does not divide evenly stays in the vault.
pub fn handler(ctx: Context<Decide>, _claim_id: u64) -> Result<()> {
    let cl = &mut ctx.accounts.claim;
    require!(!cl.decided, VaultError::AlreadyDecided);
    require!(now()? >= cl.last_at + OBJECTION_WINDOW, VaultError::WindowNotOver);
    let c = &mut ctx.accounts.chain;
    let accepted = !cl.held && !c.slashed && !c.refused;
    cl.decided = true;
    cl.accepted = accepted;
    c.open_value -= cl.value;
    c.open_claims -= 1;
    if accepted {
        if cl.peer_bond != 0 {
            c.peer_bond = cl.peer_bond;
        }
    } else {
        c.refused = true;
    }
    let (winners, losers) = if accepted { (cl.answers, cl.objections) } else { (cl.objections, cl.answers) };
    let deposit = ctx.accounts.config.deposit;
    if winners > 0 {
        cl.payout = deposit + (losers as u64 * deposit) / winners as u64;
    }
    Ok(())
}

#[derive(Accounts)]
#[instruction(claim_id: u64)]
pub struct Decide<'info> {
    #[account(mut, seeds = [CLAIM_SEED, &claim_id.to_le_bytes()], bump = claim.bump)]
    pub claim: Account<'info, Claim>,
    #[account(mut, seeds = [CHAIN_SEED, claim.operator.as_ref()], bump = chain.bump)]
    pub chain: Account<'info, Chain>,
    #[account(seeds = [CONFIG_SEED], bump = config.bump)]
    pub config: Account<'info, Config>,
}
