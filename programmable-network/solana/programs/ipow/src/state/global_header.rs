use anchor_lang::prelude::*;

#[account]
#[derive(InitSpace)]
pub struct GlobalHeader {
    pub height: u64,
    pub hash_le: [u8; 32],
    pub prev_hash_le: [u8; 32],
    pub merkle_root_le: [u8; 32],
    pub n_bits: u32,
    pub timestamp: u32,
    pub arrival_time: i64,
}
