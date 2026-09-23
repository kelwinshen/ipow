use anchor_lang::prelude::*;

use crate::errors::FactoryError;
use crate::state::{Component, Composition, FactoryConfig, MAX_COMPONENTS, MAX_LOCAL_COMPONENTS};

/// Governance-only (DESIGN_V2 §8.2/§8.12, §8.9 step 1). Registers a
/// fixed, immutable composition recipe against one shared budget of at
/// most `MAX_COMPONENTS` legs, spent however the recipe likes — many
/// networks at one token each, one network at several tokens, or any mix
/// — except for local (Solana) legs, capped separately and more tightly
/// at `MAX_LOCAL_COMPONENTS` since each one costs a real CPI transfer in
/// `lock_sol`/`expire_pending` rather than remote legs' cheap account
/// read in `exercise_mint`. At least one local component is still
/// required — the hub always verifies its own leg(s) directly. A
/// composition is never edited once registered; a new recipe is a new
/// `id`, so mints already in flight against an old one are never moved.
pub fn handler(ctx: Context<RegisterComposition>, id: u64, components: Vec<Component>) -> Result<()> {
    require!(!components.is_empty() && components.len() <= MAX_COMPONENTS, FactoryError::InvalidParams);
    for c in &components {
        require!(c.amount_per_unit > 0, FactoryError::InvalidParams);
    }
    // No two components may name the same (network, token) — each leg is
    // a distinct asset.
    for i in 0..components.len() {
        for j in (i + 1)..components.len() {
            require!(
                components[i].network_id != components[j].network_id || components[i].token_id != components[j].token_id,
                FactoryError::InvalidParams
            );
        }
    }
    let local_count = components.iter().filter(|c| c.network_id == 0).count();
    require!((1..=MAX_LOCAL_COMPONENTS).contains(&local_count), FactoryError::InvalidParams);
    let comp = &mut ctx.accounts.composition;
    comp.id = id;
    comp.components = components;
    Ok(())
}

#[derive(Accounts)]
#[instruction(id: u64)]
pub struct RegisterComposition<'info> {
    #[account(seeds = [b"config"], bump)]
    pub config: Account<'info, FactoryConfig>,
    #[account(init, payer = governance, space = 8 + Composition::INIT_SPACE, seeds = [b"composition", id.to_le_bytes().as_ref()], bump)]
    pub composition: Account<'info, Composition>,
    #[account(mut, address = config.governance @ FactoryError::Unauthorized)]
    pub governance: Signer<'info>,
    pub system_program: Program<'info, System>,
}
