use anchor_lang::prelude::*;

#[account]
#[derive(InitSpace)]
pub struct GlobalState {
    pub operator: Pubkey,
    pub commit_fee_bps: u16,
    pub next_tx_id: u64,
    pub global_tip_height: u64,
    pub min_anchor_height: u64,
    pub active_open_conversions: u64,

    pub total_held_commit_fees: u64,
    pub total_locked_deposits: u64,
    pub total_reserved_native: u64,

    pub escrow_bump: u8,
}
