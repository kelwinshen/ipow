use anchor_lang::prelude::*;

/// User X's SOL half of a mint, locked in the vault, waiting for the
/// operator's MINT anchor. `approved`/`eth_lock_id` is the user's on-chain
/// consent (DESIGN_V2 §6.3's `sig_X`, done as a signed instruction instead
/// of an ed25519 signature inside the statement).
#[account]
#[derive(InitSpace)]
pub struct Pending {
    pub user: Pubkey,
    pub nonce: u64,
    pub units: u64,
    pub deadline: i64,
    pub eth_lock_id: u64,
    pub approved: bool,
    /// A MINT anchor referencing this lock has been processed and queued;
    /// the lock can no longer expire — it will be minted when capacity
    /// allows (§6.7).
    pub queued: bool,
    /// Party whose MINT anchor queued this lock. If that party is later
    /// retired (`dead`), a live operator may re-anchor the same MINT and
    /// take the slot over, or the user may cancel after the deadline
    /// (DESIGN_V2 §6.12, "stranded queued mint").
    pub queued_by: [u8; 32],
    pub created_at: i64,
}
