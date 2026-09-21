use anchor_lang::prelude::*;

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, PartialEq, Eq, InitSpace)]
pub enum AnchorStatus {
    /// MINT judged true on the Solana side; waiting for `exercise_mint`.
    Queued,
    /// Acted on (mint executed / veto honored / cancel advanced / release
    /// judged true).
    Exercised,
    /// Judged false; the party was slashed.
    Slashed,
    /// Pointer advanced without a preimage; still auditable later.
    Skipped,
    /// v3: a queued MINT that was still held at the end of its challenge
    /// window — never exercised; its pending slot was un-queued.
    Cancelled,
}

/// One processed anchor, keyed by its Bitcoin txid. Exists forever so an
/// anchor can never be processed twice and so vetoes/audits can refer to
/// it.
#[account]
#[derive(InitSpace)]
pub struct ProcessedAnchor {
    pub party_id: [u8; 32],
    pub txid_le: [u8; 32],
    pub kind: u8,
    pub status: AnchorStatus,
    pub statement_hash: [u8; 32],
    pub block_height: u64,
    pub processed_at: i64,
    /// v3: end of the challenge window (`processed_at + t_challenge_secs`).
    pub challenge_until: i64,
    /// v3: a VETO stands against this anchor (lifted only by a CLEAR).
    pub held: bool,
    /// v3: party whose ATTEST covers this anchor (zero = none) and the
    /// escrow it reserved for it; `settled` once the window was settled.
    pub attested_by: [u8; 32],
    pub escrow: u64,
    pub settled: bool,
    // MINT fields, so `exercise_mint` needs no statement bytes.
    pub eth_lock_id: u64,
    pub sol_user: Pubkey,
    pub nonce: u64,
    pub units: u64,
    pub deadline: i64,
}
