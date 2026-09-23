use anchor_lang::prelude::*;

#[account]
#[derive(InitSpace)]
pub struct SupportedNetwork {
    pub network_id: u64,
    pub min_addr_len: u16,
    pub max_addr_len: u16,
    pub is_active: bool,
}
