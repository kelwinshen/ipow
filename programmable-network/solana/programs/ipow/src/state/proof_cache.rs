use anchor_lang::prelude::*;

#[account]
pub struct ProofCache {
    pub tx_id: u64,
    pub is_set: bool,
    pub is_verified: bool,
    pub is_invalid: bool,
    pub attempts: u8,
    pub txid_le: [u8; 32],
    pub proof_block_height: u64,
    pub branch_le: Vec<[u8; 32]>,
    pub merkle_index: u64,
    pub out_value_sats: u64,
    pub out_program: Vec<u8>,
    pub out_set: bool,
}

impl ProofCache {
    pub const MAX_SIBLINGS: usize = 30;
    pub const MAX_SPACE: usize =
        8 + 1 + 1 + 1 + 1 + 32 + 8 + (4 + Self::MAX_SIBLINGS * 32) + 8 + 8 + (4 + 80) + 1;
}
