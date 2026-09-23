use anchor_lang::prelude::*;

use crate::state::composition::MAX_REMOTE_COMPONENTS;

/// User X's mint request against a registered `Composition` (DESIGN_V2
/// §8.3/§8.10). Every local (network_id == 0) component is verified
/// directly by the transfers this account's creation made — native SOL
/// lamports and/or SPL tokens straight into the vault, the same thing
/// `lock_sol` always did for its one SOL leg, generalized to however
/// many local legs the composition names. Every other component is a
/// remote leg: `remote_lock_id[i]` is what the user approved for it
/// (their on-chain consent, §6.3's `sig_X`); `remote_anchor_txid[i]`
/// caches which MINT anchor Solana's own predicate accepted for it, once
/// `process_anchor` sees one — `[0u8; 32]` until then. `exercise_mint`
/// requires every remote component's cached anchor to be attested (or
/// past its own challenge window) and unheld before it mints anything.
#[account]
#[derive(InitSpace)]
pub struct Pending {
    pub user: Pubkey,
    pub nonce: u64,
    pub composition_id: u64,
    pub units: u64,
    pub deadline: i64,
    pub approved: bool,
    #[max_len(MAX_REMOTE_COMPONENTS)]
    pub remote_lock_id: Vec<u64>,
    #[max_len(MAX_REMOTE_COMPONENTS)]
    pub remote_anchor_txid: Vec<[u8; 32]>,
    /// The operator whose MINT anchor first queued a component of this
    /// mint — every remote component must be anchored by the same
    /// operator (one basket, one carrier). If that party is later
    /// retired, a live operator may re-anchor from scratch and take the
    /// slot over (DESIGN_V2 §6.12), or the user may cancel after the
    /// deadline once nothing is queued.
    pub queued_by: [u8; 32],
    /// v3: acceleration fee the user posted alongside their lock, held in
    /// the `fees` PDA. Paid out whole to whoever posts the first
    /// non-redundant ATTEST of *any* one component (§8's simplification —
    /// rewarding the true bottleneck-clearer would mean checking every
    /// sibling component's readiness from inside a single component's
    /// ATTEST, which isn't built this pass) — see `process_anchor`'s
    /// Attest/KIND_MINT branch; refunded to the user at `exercise_mint`
    /// if nobody ever attested any component.
    pub attest_fee: u64,
    pub created_at: i64,
}

impl Pending {
    pub fn remote_count(&self) -> usize {
        self.remote_lock_id.len()
    }
}
