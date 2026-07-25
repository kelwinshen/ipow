use anchor_lang::prelude::*;

#[derive(AnchorSerialize, AnchorDeserialize, Clone, PartialEq, Eq)]
pub enum ConversionStatus {
    None,
    Committed,
    Approved,
    Deposited,
    Completed,
    Refunded,
}

#[account]
pub struct Conversion {
    pub tx_id: u64,
    pub user: Pubkey,
    pub is_native_to_bitcoin: bool,

    pub user_program: Vec<u8>,
    pub ipow_receive_program: Vec<u8>,
    pub network_address: Vec<u8>,
    pub network_id: u64,

    pub native_amount: u64,
    pub bitcoin_amount: u64,
    pub commit_fee: u64,
    pub reserved_native: u64,

    pub created_at: i64,
    pub approved_at: i64,
    pub deposited_at: i64,
    pub operator_duty_expires_at: i64,

    pub status: ConversionStatus,

    pub window_started: bool,
    pub window_start_height: u64,
    pub epoch_start_height: u64,

    pub proof_verified: bool,
    pub proof_txid_le: [u8; 32],
    pub proof_block_height: u64,
}

impl Conversion {
    pub const MAX_SPACE: usize = 8
        + 8
        + 32
        + 1
        + (4 + 80)
        + (4 + 80)
        + (4 + 64)
        + 8
        + 8
        + 8
        + 8
        + 8
        + 8
        + 8
        + 8
        + 8
        + 1
        + 1
        + 8
        + 8
        + 1
        + 32;
}
