use anchor_lang::prelude::*;

/// A redemption: BETA burned, SOL paid, ETH half awaiting the operator's
/// RELEASE anchor. `claimed` is set when a RELEASE naming this burn is
/// judged true here; a second RELEASE for it is a lie.
#[account]
#[derive(InitSpace)]
pub struct Burn {
    pub burn_id: u64,
    pub burner: Pubkey,
    pub units: u64,
    pub to_eth: [u8; 20],
    pub claimed: bool,
    pub created_at: i64,
}
