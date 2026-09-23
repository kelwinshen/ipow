use anchor_lang::prelude::*;

/// DESIGN_V2 §8 (§8.12's flat-budget revision): a composition spends a
/// single shared budget of at most `MAX_COMPONENTS` legs however it
/// likes across networks — 8 networks at 1 token each, one network at
/// several tokens and the rest skipped, any mix — rather than a rigid
/// per-network grid. The one asymmetry that's real, not arbitrary: local
/// (Solana) legs cost a CPI transfer each in `lock_sol`/`expire_pending`
/// while remote legs cost only a cheap account read in `exercise_mint`,
/// so local legs get their own smaller sub-budget, `MAX_LOCAL_COMPONENTS`
/// (native SOL plus up to 3 SPL tokens — `lock_sol`'s `local_spl_0/1/2`).
/// A composition still needs at least one local leg (§8.2: the hub always
/// verifies its own leg directly) but may now spend the rest of its
/// budget on any number of distinct remote networks, one token each or
/// several — nothing beyond the shared total caps how many networks.
pub const MAX_COMPONENTS: usize = 8;
pub const MAX_LOCAL_COMPONENTS: usize = 4;
/// Worst case for `Pending`'s and `exercise_mint`'s remote-tracking:
/// every component but the one mandatory local leg is remote.
pub const MAX_REMOTE_COMPONENTS: usize = MAX_COMPONENTS - 1;

/// One (network, token, amount) leg of a composition — DESIGN_V2 §8.2.
/// Same generic shape as Conversion's own `TokenAmount` bundle entries,
/// widened by `network_id` since a composition spans chains where a
/// Conversion bundle never leaves one.
#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, InitSpace)]
pub struct Component {
    /// 0 = Solana (the hub's own local lock, no cross-chain judging
    /// needed — exactly what `lock_sol` verifies directly). Any other
    /// value names a reserve chain whose own `BetaVault`-shaped contract
    /// judges this component, exactly like §7's single ETH leg.
    pub network_id: u64,
    /// 0 = that chain's native asset; else a Pubkey (Solana) or a
    /// zero-padded EVM address, as bytes.
    pub token_id: [u8; 32],
    pub amount_per_unit: u64,
}

/// Governance-registered recipe for one BETA composition. `id` is the
/// value users pass to `lock_sol`. Immutable once registered — a new
/// composition (not an edit) is how the recipe changes, so pending mints
/// already in flight are never affected out from under them.
#[account]
#[derive(InitSpace)]
pub struct Composition {
    pub id: u64,
    #[max_len(MAX_COMPONENTS)]
    pub components: Vec<Component>,
}
