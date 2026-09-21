use anchor_lang::prelude::*;

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, PartialEq, Eq, InitSpace)]
pub enum PartyKind {
    Operator,
    Auditor,
}

/// A registered statement-maker. `party_id` is the cross-chain identity
/// (the same 32 bytes are registered on Ethereum); `anchor_txid_le`/
/// `anchor_vout` is the head of its statement chain as this chain has
/// processed it (DESIGN_V2 §6.2).
#[account]
#[derive(InitSpace)]
pub struct Party {
    pub party_id: [u8; 32],
    pub owner: Pubkey,
    pub kind: PartyKind,
    pub anchor_txid_le: [u8; 32],
    pub anchor_vout: u32,
    /// Number of anchors processed on this chain.
    pub seq: u64,
    /// Lamports of this party's bond held in `bond_escrow`.
    pub bond: u64,
    /// Set by a slash for a false statement about Solana. A dead party's
    /// anchors are still processed (so further lies can be judged) but
    /// never exercised.
    pub dead: bool,
    /// Operator only: set by an auditor's dead-veto; mints are not
    /// exercised until it lapses.
    pub paused_until: i64,
    pub unbond_requested_at: i64,
}
