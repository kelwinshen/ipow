use anchor_lang::prelude::*;

use crate::constants::MAX_CLAIM_KINDS;

/// The single record of the protocol. It holds no key: nothing can change
/// after deployment (D59).
#[account]
#[derive(InitSpace)]
pub struct Protocol {
    pub job_count: u64,
    pub challenge_count: u64,
    pub bump: u8,
    pub vault_bump: u8,
}

/// Everything an operator has locked (D31), and the part locked for jobs.
#[account]
#[derive(InitSpace)]
pub struct Operator {
    pub owner: Pubkey,
    pub bond: u64,
    pub locked: u64,
    /// D30, D34: the coin its next tagged transaction must spend.
    pub chain_head_txid: [u8; 32],
    pub chain_head_vout: u32,
    pub chain_head_set: bool,
    pub bump: u8,
}

/// A registered application (D63), with the challenge period of each kind
/// of claim, set once (D17).
#[account]
#[derive(InitSpace)]
pub struct Application {
    pub key: Pubkey,
    #[max_len(MAX_CLAIM_KINDS)]
    pub challenge_periods: Vec<u32>,
    pub bump: u8,
}

/// An application uses a tag once (D80).
#[account]
#[derive(InitSpace)]
pub struct TagRecord {
    pub job_id: u64,
    pub bump: u8,
}

/// Money an address can withdraw.
#[account]
#[derive(InitSpace)]
pub struct Credit {
    pub owner: Pubkey,
    pub amount: u64,
    pub bump: u8,
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, Default, PartialEq, Eq, Debug, InitSpace)]
pub struct BlockRef {
    pub hash: [u8; 32],
    pub height: u32,
    pub epoch_time: u32,
}

#[account]
#[derive(InitSpace)]
pub struct Job {
    pub id: u64,
    pub application: Pubkey,
    /// Who paid the fees and receives them back (D62, D65).
    pub payer: Pubkey,
    pub tag: [u8; 32],
    /// x (D40).
    pub escrow: u64,
    /// The fee at the price of the moment, and what was sent above the fees
    /// (D24, D79).
    pub commitment_fee: u64,
    pub escrow_fee: u64,
    pub bid: u64,
    pub operator: Pubkey,
    pub has_operator: bool,
    pub confirmations: u16,
    /// 0 for a settlement, otherwise the kind of claim, counted from 1.
    pub claim_kind: u16,
    pub opened_at: i64,
    pub last_bid_at: i64,
    pub fees_returned: bool,
    pub bump: u8,

    // The duty, from the anchor on.
    pub anchor: BlockRef,
    pub anchored_at: i64,
    pub proof_block: BlockRef,
    /// The block on top of the proof that was shown with it.
    pub tip: BlockRef,
    /// The oldest block of the operator's branch asked for and shown (D81).
    pub deepest: BlockRef,
    /// How many parents were shown; the next question is this + 1 (D91).
    pub parents_shown: u32,
    pub txid: [u8; 32],
    pub proven_at: i64,
    pub lock_end: i64,
    pub slashed: bool,
    pub settled: bool,
    /// D42: who locked x in the operator's place, if anyone.
    pub attester: Pubkey,
    pub has_attester: bool,
    pub attested_at: i64,
    /// How many challenges of the job are open.
    pub open_challenges: u32,
}

/// A Bitcoin transaction settles one job on this network (D80), and a
/// transaction used as a chain head can never settle one.
#[account]
#[derive(InitSpace)]
pub struct UsedTx {
    pub bump: u8,
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, PartialEq, Eq, Debug, InitSpace)]
pub enum ChallengeKind {
    /// A guardian asked for the parent of the oldest block.
    Parent,
    /// A guardian showed a competing branch.
    Fork,
}

/// An open challenge of a proof (D49, D81). A job can have several (D88).
#[account]
#[derive(InitSpace)]
pub struct Challenge {
    pub id: u64,
    pub kind: ChallengeKind,
    pub job_id: u64,
    pub guardian: Pubkey,
    /// D81, D91: the commitment fee, times the number of the question.
    pub deposit: u64,
    pub opened_at: i64,
    /// Parent only: the block whose parent is asked for.
    pub asked: BlockRef,
    /// Fork only: the most mining work shown on each side, counted from the
    /// block where the branches part. 256-bit numbers, big-endian.
    pub operator_work: [u8; 32],
    pub guardian_work: [u8; 32],
    pub bump: u8,
}

/// A block shown on one side of a competing-branch challenge, and the work
/// of that side up to and with it. Any of them can be built on, so nobody
/// can freeze a side by showing a block that goes nowhere.
#[account]
#[derive(InitSpace)]
pub struct Checkpoint {
    pub work: [u8; 32],
    pub bump: u8,
}

/// D47: a guardian's sealed note, and the slot it was sealed in.
#[account]
#[derive(InitSpace)]
pub struct Note {
    pub slot: u64,
    pub bump: u8,
}
