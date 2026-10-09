//! The one interface every network implements, EVM and Solana alike. The
//! roles only talk to a network through it, so each role is written once.
//!
//! The methods say what a role wants done, not how. On Solana a proof needs
//! walk records made first; the Solana adapter makes them itself.

use async_trait::async_trait;

use crate::types::{Amount, BlockRef, Challenge, Evidence, Job, Proof, StoredBlock};

#[async_trait]
pub trait ProtocolNetwork: Send + Sync {
    // ------------------------------------------------------------------
    // Reading
    // ------------------------------------------------------------------

    /// The name given in the settings, for logs.
    fn name(&self) -> &str;

    /// This node's address on the network, as the network writes it.
    fn me(&self) -> String;

    /// The network's time, in Unix seconds, as its contracts see it.
    async fn now(&self) -> anyhow::Result<i64>;

    /// Jobs with an id above `after`, oldest first, at most `limit`.
    async fn jobs_after(&self, after: u64, limit: usize) -> anyhow::Result<Vec<Job>>;

    async fn job(&self, id: u64) -> anyhow::Result<Job>;

    /// This node's bond: everything locked, and the part locked for jobs (D31).
    async fn my_bond(&self) -> anyhow::Result<(Amount, Amount)>;

    /// What this node can withdraw.
    async fn my_credit(&self) -> anyhow::Result<Amount>;

    /// The deposit of the next question for a parent of a job (D91).
    async fn parent_deposit(&self, job_id: u64) -> anyhow::Result<Amount>;

    /// The lowest amount the next bid on a job must lock (D32, D76).
    async fn minimum_bid(&self, job_id: u64) -> anyhow::Result<Amount>;

    /// How many challenges were ever opened; ids run from 1 to this.
    async fn challenge_count(&self) -> anyhow::Result<u64>;

    /// An open challenge, or `None` when it was resolved or never existed.
    async fn challenge(&self, id: u64) -> anyhow::Result<Option<Challenge>>;

    /// A block stored in the light client, or `None`.
    async fn stored_block(&self, at: &BlockRef) -> anyhow::Result<Option<StoredBlock>>;

    // ------------------------------------------------------------------
    // The light client (section 2 of the spec). Anyone may call these.
    // ------------------------------------------------------------------

    /// Records an epoch start: 6 headers of 80 bytes, oldest first.
    async fn add_epoch_start(&self, headers: &[[u8; 80]], height: u32) -> anyhow::Result<()>;

    /// Jumps to an anchor, with the epoch start whose first block is `epoch_start`.
    async fn jump(&self, header: &[u8; 80], epoch_start: &BlockRef, height: u32) -> anyhow::Result<()>;

    /// Streams headers on top of the stored block `parent`.
    async fn extend(&self, parent: &BlockRef, headers: &[[u8; 80]]) -> anyhow::Result<()>;

    /// Stores the parent of the stored block `child`.
    async fn extend_back(&self, child: &BlockRef, header: &[u8; 80], prev_epoch_time: u32) -> anyhow::Result<()>;

    // ------------------------------------------------------------------
    // Operator and attester
    // ------------------------------------------------------------------

    async fn lock_bond(&self, amount: Amount) -> anyhow::Result<()>;
    async fn withdraw_bond(&self, amount: Amount) -> anyhow::Result<()>;
    async fn bid(&self, job_id: u64, amount: Amount) -> anyhow::Result<()>;

    /// What this node's first chain head transaction must carry (D30, N22).
    async fn chain_head_commitment(&self) -> anyhow::Result<[u8; 32]>;
    /// The coin this node's next tagged transaction must spend, if any.
    async fn chain_head(&self) -> anyhow::Result<Option<([u8; 32], u32)>>;
    /// What a job's transaction carries in its OP_RETURN (D80).
    async fn tag_payload(&self, tag: &[u8; 32]) -> anyhow::Result<[u8; 32]>;
    async fn register_chain_head(&self, block: &BlockRef, raw_tx: &[u8], siblings: &[[u8; 32]], tx_index: u64, coin_index: u32, tag_index: u32) -> anyhow::Result<()>;

    /// Moves this node's chain head past a mined transaction that spent it
    /// and can no longer settle a job, such as one mined after its job's
    /// deadline (N25). Output `head_index` is the new chain head (N23).
    async fn advance_chain_head(&self, block: &BlockRef, raw_tx: &[u8], siblings: &[[u8; 32]], tx_index: u64, head_index: u32) -> anyhow::Result<()>;

    async fn anchor_job(&self, job_id: u64, anchor: &BlockRef) -> anyhow::Result<()>;
    async fn prove_job(&self, job_id: u64, proof: &Proof) -> anyhow::Result<()>;
    async fn settle(&self, job_id: u64) -> anyhow::Result<()>;
    async fn attest(&self, job_id: u64) -> anyhow::Result<()>;

    // ------------------------------------------------------------------
    // Guardian
    // ------------------------------------------------------------------

    /// The first of a guardian's two steps (D47): seals a note for this
    /// node, this job, this evidence and a secret salt.
    async fn seal_note(&self, job_id: u64, evidence: &Evidence, salt: &[u8; 32]) -> anyhow::Result<()>;
    async fn report_missed_duty(&self, job_id: u64, salt: &[u8; 32]) -> anyhow::Result<()>;
    /// Asks for the parent of the job's oldest shown block. Returns the
    /// challenge's id.
    async fn ask_parent(&self, job_id: u64, salt: &[u8; 32], deposit: Amount) -> anyhow::Result<u64>;
    async fn show_parent(&self, challenge_id: u64, prev_epoch_time: u32) -> anyhow::Result<()>;
    /// Shows a competing branch. Returns the challenge's id.
    #[allow(clippy::too_many_arguments)]
    async fn challenge_fork(
        &self,
        job_id: u64,
        operator_block: &BlockRef,
        guardian_block: &BlockRef,
        guardian_tip: &BlockRef,
        prev_epoch_time: u32,
        salt: &[u8; 32],
        deposit: Amount,
    ) -> anyhow::Result<u64>;
    async fn extend_branch(&self, challenge_id: u64, guardian_side: bool, from: &BlockRef, new_tip: &BlockRef, prev_epoch_time: u32) -> anyhow::Result<()>;
    async fn resolve_challenge(&self, challenge_id: u64) -> anyhow::Result<()>;
    async fn withdraw_credit(&self) -> anyhow::Result<()>;
}
