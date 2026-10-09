//! Values the roles work with, the same on every network.

use serde::{Deserialize, Serialize};

/// Where a block is in a light client: its hash in header byte order, its
/// stated number and the time of the first block of its epoch (D48).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
pub struct BlockRef {
    pub hash: [u8; 32],
    pub height: u32,
    pub epoch_time: u32,
}

/// Bitcoin's epoch: 2,016 blocks.
pub const EPOCH_BLOCKS: u32 = 2016;

impl BlockRef {
    /// Where `header`, a child of this block, is in the light client. The
    /// first block of an epoch gives the epoch its time (D72).
    pub fn child(&self, header: &[u8; 80]) -> BlockRef {
        let height = self.height + 1;
        let epoch_time = if height % EPOCH_BLOCKS == 0 {
            u32::from_le_bytes(header[68..72].try_into().unwrap())
        } else {
            self.epoch_time
        };
        BlockRef { hash: double_sha256(header), height, epoch_time }
    }
}

/// What a job's Bitcoin transaction carries: `sha256("iPoW job", tag)`,
/// the same on every network (D80, N30).
pub fn tag_payload(tag: &[u8; 32]) -> [u8; 32] {
    use sha2::{Digest, Sha256};
    let mut h = Sha256::new();
    h.update(b"iPoW job");
    h.update(tag);
    h.finalize().into()
}

fn double_sha256(data: &[u8]) -> [u8; 32] {
    use sha2::{Digest, Sha256};
    Sha256::digest(Sha256::digest(data)).into()
}

/// An amount in the network's own coin, in its smallest unit (D84): wei on
/// EVM networks, lamports on Solana.
pub type Amount = u128;

/// The state of a job, as every network reports it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum JobStatus {
    /// Bidding is open.
    Auction,
    /// Nobody bid (D61).
    Expired,
    /// An operator won and its duty runs.
    Assigned,
    /// The proof was accepted; the lock runs.
    Proven,
    Slashed,
    Settled,
}

/// A job, as the roles see it.
#[derive(Clone, Debug)]
pub struct Job {
    pub id: u64,
    pub application: String,
    pub tag: [u8; 32],
    /// x (D40).
    pub escrow: Amount,
    pub commitment_fee: Amount,
    pub escrow_fee: Amount,
    pub bid: Amount,
    pub operator: Option<String>,
    pub confirmations: u16,
    /// 0 for a settlement.
    pub claim_kind: u16,
    pub status: JobStatus,
    /// Unix seconds. Zero when not reached yet.
    pub auction_end: i64,
    pub deadline: i64,
    pub anchor: Option<BlockRef>,
    pub proof_block: Option<BlockRef>,
    /// The block on top of the proof that was shown with it.
    pub tip: Option<BlockRef>,
    /// The transaction the proof carried.
    pub txid: Option<[u8; 32]>,
    /// The oldest block of the operator's branch that was asked for and
    /// shown: the anchor until a parent is shown. A guardian's next
    /// question is for its parent (D81).
    pub deepest: Option<BlockRef>,
    /// How many parents were shown; the next question is number
    /// `parents_shown + 1` (D91).
    pub parents_shown: u32,
    pub lock_end: i64,
    pub attester: Option<String>,
    pub open_challenges: u32,
}

/// A block as the light client stores it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct StoredBlock {
    pub prev_hash: [u8; 32],
    pub merkle_root: [u8; 32],
    pub bits: u32,
    pub time: u32,
    pub stored_at: i64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ChallengeKind {
    Parent,
    Fork,
}

/// An open challenge of a proof.
#[derive(Clone, Debug)]
pub struct Challenge {
    pub id: u64,
    pub kind: ChallengeKind,
    pub job_id: u64,
    pub guardian: String,
    pub deposit: Amount,
    pub opened_at: i64,
    /// Parent only: the block whose parent is asked for.
    pub asked: Option<BlockRef>,
}

/// What a guardian's sealed note covers (D47).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Evidence {
    MissedDuty,
    /// A question for the parent of this block.
    Parent([u8; 32]),
    /// A competing branch that starts with this block.
    Fork([u8; 32]),
}

/// Everything a proof of a job's tagged transaction needs.
#[derive(Clone, Debug)]
pub struct Proof {
    pub proof_block: BlockRef,
    pub tip: BlockRef,
    pub prev_epoch_time: u32,
    /// The transaction without witness data.
    pub raw_tx: Vec<u8>,
    pub siblings: Vec<[u8; 32]>,
    pub tx_index: u64,
    pub head_index: u32,
    pub tag_index: u32,
}
