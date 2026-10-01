//! The iPoW protocol on Solana, behind the node's one interface.
//!
//! What differs from Ethereum, and is handled here: a proof, a competing
//! branch and a branch extension read the light client through walks,
//! records the adapter creates and fills in steps before the instruction
//! that uses them, and closes afterwards to recover their rent. A job's
//! status, deadline and next deposit are computed from its record the way
//! the program computes them (see `rules`).

use std::sync::Arc;

use anchor_lang::prelude::Pubkey;
use anchor_lang::solana_program::instruction::{AccountMeta, Instruction};
use anchor_lang::{AccountDeserialize, InstructionData, ToAccountMetas};
use async_trait::async_trait;
use ipow_protocol_core::network::ProtocolNetwork;
use ipow_protocol_core::types::{
    Amount, BlockRef, Challenge, ChallengeKind, EPOCH_BLOCKS, Evidence, Job, JobStatus, Proof, StoredBlock,
};
use sha2::{Digest, Sha256};
use solana_sdk::signature::Keypair;
use solana_sdk::signer::Signer;

use crate::chain::{Chain, PACKET_DATA_SIZE, build, size};
use solana_address_lookup_table_interface::instruction as alt;
use solana_sdk::hash::Hash;
use solana_sdk::message::AddressLookupTableAccount;
use crate::programs::ipow_light_client as lc;
use crate::programs::ipow_protocol as pr;
use crate::rules;

const SYSTEM: Pubkey = anchor_lang::solana_program::system_program::ID;
/// Addresses added to a lookup table per transaction: a legacy transaction
/// of 1,232 bytes holds about 30 with the table's creation; 20 leaves room.
const TABLE_PART: usize = 20;
/// Headers per `extend`: the light client's MAX_EXTEND, what fits in one
/// Solana transaction.
const MAX_EXTEND: usize = 7;
/// Blocks read per walk step.
const WALK_STEP: usize = 20;

fn sha256(parts: &[&[u8]]) -> [u8; 32] {
    let mut h = Sha256::new();
    for p in parts {
        h.update(p);
    }
    h.finalize().into()
}

fn sha256d(data: &[u8]) -> [u8; 32] {
    Sha256::digest(Sha256::digest(data)).into()
}

fn p_ref(r: &BlockRef) -> pr::types::BlockRef {
    pr::types::BlockRef { hash: r.hash, height: r.height, epoch_time: r.epoch_time }
}

fn l_ref(r: &BlockRef) -> lc::types::BlockRef {
    lc::types::BlockRef { hash: r.hash, height: r.height, epoch_time: r.epoch_time }
}

fn from_p(r: &pr::types::BlockRef) -> Option<BlockRef> {
    (r.hash != [0; 32]).then_some(BlockRef { hash: r.hash, height: r.height, epoch_time: r.epoch_time })
}

/// A key from its environment variable: a JSON array of 64 bytes (as the
/// Solana tools write it) or base58.
pub fn parse_key(text: &str) -> anyhow::Result<Keypair> {
    let text = text.trim();
    let bytes: Vec<u8> = if text.starts_with('[') {
        serde_json::from_str(text).map_err(|_| anyhow::anyhow!("the Solana key is not a JSON array of bytes"))?
    } else {
        bs58::decode(text).into_vec().map_err(|_| anyhow::anyhow!("the Solana key is not base58"))?
    };
    Keypair::try_from(bytes.as_slice()).map_err(|_| anyhow::anyhow!("the Solana key is not 64 bytes"))
}

pub struct SvmNetwork {
    name: String,
    chain: Arc<dyn Chain>,
    key: Keypair,
    /// Lookup tables this node made and has not closed yet. `None` until
    /// read from the network, after a start.
    tables: tokio::sync::Mutex<Option<Vec<Pubkey>>>,
    /// The upload buffer is one per key (`["buffer", key]`), shared by every
    /// vault pair on this network: one upload at a time.
    buffer: tokio::sync::Mutex<()>,
}

/// Slots after deactivation before a lookup table is closed. Solana allows
/// it once the slot has left the 512 recent ones, which takes a little
/// longer when slots are skipped.
const TABLE_COOLDOWN: u64 = 600;

impl SvmNetwork {
    /// Held while this key's upload buffer is in use (see `buffer`).
    pub async fn buffer_lock(&self) -> tokio::sync::MutexGuard<'_, ()> {
        self.buffer.lock().await
    }

    /// `light_client` and `protocol` must be the programs this adapter was
    /// built for: their ids are in the programs' own code.
    pub fn connect(name: &str, chain: Arc<dyn Chain>, key: Keypair, light_client: &str, protocol: &str) -> anyhow::Result<Self> {
        anyhow::ensure!(light_client == lc::ID.to_string(), "the light client is {}, this node is built for {}", light_client, lc::ID);
        anyhow::ensure!(protocol == pr::ID.to_string(), "the protocol is {}, this node is built for {}", protocol, pr::ID);
        Ok(SvmNetwork { name: name.to_string(), chain, key, tables: tokio::sync::Mutex::new(None), buffer: tokio::sync::Mutex::new(()) })
    }

    pub fn pubkey(&self) -> Pubkey {
        self.key.pubkey()
    }

    // ------------------------------------------------------------------
    // Addresses
    // ------------------------------------------------------------------

    pub fn lc_pda(seeds: &[&[u8]]) -> Pubkey {
        Pubkey::find_program_address(seeds, &lc::ID).0
    }
    pub fn pr_pda(seeds: &[&[u8]]) -> Pubkey {
        Pubkey::find_program_address(seeds, &pr::ID).0
    }
    pub fn node_pda(r: &BlockRef) -> Pubkey {
        Self::lc_pda(&[b"node", &r.hash, &r.height.to_le_bytes(), &r.epoch_time.to_le_bytes()])
    }
    fn operator_pda(o: &Pubkey) -> Pubkey {
        Self::pr_pda(&[b"operator", o.as_ref()])
    }
    pub fn job_pda(id: u64) -> Pubkey {
        Self::pr_pda(&[b"job", &id.to_le_bytes()])
    }
    fn credit_pda(o: &Pubkey) -> Pubkey {
        Self::pr_pda(&[b"credit", o.as_ref()])
    }
    fn challenge_pda(id: u64) -> Pubkey {
        Self::pr_pda(&[b"challenge", &id.to_le_bytes()])
    }
    fn checkpoint_pda(challenge: u64, guardian_side: bool, r: &BlockRef) -> Pubkey {
        Self::pr_pda(&[
            b"checkpoint",
            &challenge.to_le_bytes(),
            &[guardian_side as u8],
            &r.hash,
            &r.height.to_le_bytes(),
            &r.epoch_time.to_le_bytes(),
        ])
    }
    fn note_pda(note: &[u8; 32]) -> Pubkey {
        Self::pr_pda(&[b"note", note])
    }
    fn used_tx_pda(txid: &[u8; 32]) -> Pubkey {
        Self::pr_pda(&[b"used_tx", txid])
    }
    fn protocol_pda() -> Pubkey {
        Self::pr_pda(&[b"protocol"])
    }
    fn vault_pda() -> Pubkey {
        Self::pr_pda(&[b"vault"])
    }
    fn config_pda() -> Pubkey {
        Self::lc_pda(&[b"config"])
    }

    // ------------------------------------------------------------------
    // Reading and sending
    // ------------------------------------------------------------------

    async fn read<T: AccountDeserialize>(&self, key: &Pubkey) -> anyhow::Result<Option<T>> {
        match self.chain.account(key).await? {
            None => Ok(None),
            Some(data) if data.is_empty() => Ok(None),
            Some(data) => Ok(Some(T::try_deserialize(&mut data.as_slice())?)),
        }
    }

    pub(crate) async fn job_record(&self, id: u64) -> anyhow::Result<pr::accounts::Job> {
        self.read(&Self::job_pda(id)).await?.ok_or_else(|| anyhow::anyhow!("job {id} does not exist"))
    }

    async fn protocol_record(&self) -> anyhow::Result<pr::accounts::Protocol> {
        self.read(&Self::protocol_pda()).await?.ok_or_else(|| anyhow::anyhow!("the protocol is not initialized"))
    }

    async fn node(&self, r: &BlockRef) -> anyhow::Result<Option<lc::accounts::Node>> {
        self.read(&Self::node_pda(r)).await
    }

    fn pr_ix(accounts: impl ToAccountMetas, data: impl InstructionData) -> Instruction {
        Instruction { program_id: pr::ID, accounts: accounts.to_account_metas(None), data: data.data() }
    }

    fn lc_ix(accounts: impl ToAccountMetas, data: impl InstructionData) -> Instruction {
        Instruction { program_id: lc::ID, accounts: accounts.to_account_metas(None), data: data.data() }
    }

    /// Sends one instruction. One too large for a Solana transaction (a
    /// proof of a transaction for several networks, from a full Bitcoin
    /// block) goes through a lookup table that holds its accounts.
    pub(crate) async fn send_ix(&self, ix: Instruction) -> anyhow::Result<()> {
        self.send(ix).await
    }

    pub(crate) async fn owner_and_lamports(&self, key: &Pubkey) -> anyhow::Result<Option<(Pubkey, u64)>> {
        self.chain.owner_and_lamports(key).await
    }

    pub(crate) async fn account_data(&self, key: &Pubkey) -> anyhow::Result<Option<Vec<u8>>> {
        self.chain.account(key).await
    }

    pub(crate) async fn read_account<T: AccountDeserialize>(&self, key: &Pubkey) -> anyhow::Result<Option<T>> {
        self.read(key).await
    }

    /// An account as of Solana's last finalized block.
    pub(crate) async fn read_final<T: AccountDeserialize>(&self, key: &Pubkey) -> anyhow::Result<Option<T>> {
        match self.chain.account_final(key).await? {
            None => Ok(None),
            Some(data) if data.is_empty() => Ok(None),
            Some(data) => Ok(Some(T::try_deserialize(&mut data.as_slice())?)),
        }
    }

    async fn send(&self, ix: Instruction) -> anyhow::Result<()> {
        let plain = build(std::slice::from_ref(&ix), &self.key, Hash::default(), None)?;
        if size(&plain)? <= PACKET_DATA_SIZE {
            let outcome = self.chain.send(&[ix], &self.key, None).await;
            self.tidy_tables().await;
            return outcome;
        }
        let me = self.me_key();
        let (create, table) = alt::create_lookup_table(me, me, self.recent_slot().await?);
        let mut addresses: Vec<Pubkey> = vec![];
        for m in &ix.accounts {
            if !m.is_signer && !addresses.contains(&m.pubkey) {
                addresses.push(m.pubkey);
            }
        }
        // Recorded before it exists: whatever fails below, the table is
        // deactivated and closed later, and its rent comes back.
        self.tables.lock().await.get_or_insert_with(Vec::new).push(table);
        anyhow::ensure!(!addresses.is_empty(), "a transaction too large names no account to put in a table");
        // The table is filled in parts, the first with its creation.
        for (n, part) in addresses.chunks(TABLE_PART).enumerate() {
            let extend = alt::extend_lookup_table(table, me, Some(me), part.to_vec());
            if n == 0 {
                self.chain.send(&[create.clone(), extend], &self.key, None).await?;
            } else {
                self.chain.send(&[extend], &self.key, None).await?;
            }
        }
        self.chain.wait_past(self.chain.clock().await?.slot).await?;
        let account = AddressLookupTableAccount { key: table, addresses };
        let outcome = self.chain.send(std::slice::from_ref(&ix), &self.key, Some(&account)).await;
        // Deactivated now if possible; otherwise by the next tidy.
        let _ = self.chain.send(&[alt::deactivate_lookup_table(table, me)], &self.key, None).await;
        outcome
    }

    /// The newest slot Solana still lists as recent (SlotHashes): what a
    /// new lookup table must name. The current slot is never in that list.
    async fn recent_slot(&self) -> anyhow::Result<u64> {
        let data = self.chain.account(&solana_sdk::sysvar::slot_hashes::ID).await?.ok_or_else(|| anyhow::anyhow!("no recent slots"))?;
        let hashes: solana_sdk::slot_hashes::SlotHashes = bincode::deserialize(&data)?;
        hashes.first().map(|(s, _)| *s).ok_or_else(|| anyhow::anyhow!("no recent slots"))
    }

    /// Lookup tables made and not closed yet.
    pub async fn open_tables(&self) -> usize {
        self.tables.lock().await.as_ref().map_or(0, Vec::len)
    }

    /// Deactivates each lookup table this node made, and closes it once
    /// Solana allows, taking its rent back. Reads each table's state from
    /// the network, so a step that failed, or was lost to a restart, is
    /// done again. A table that no longer exists is forgotten.
    async fn tidy_tables(&self) {
        let mut tables = self.tables.lock().await;
        if tables.is_none() {
            match self.chain.tables_of(&self.me_key()).await {
                Ok(found) => *tables = Some(found),
                Err(_) => return,
            }
        }
        let list = tables.as_mut().unwrap();
        if list.is_empty() {
            return;
        }
        let Ok(clock) = self.chain.clock().await else { return };
        let me = self.me_key();
        let mut kept = vec![];
        for table in list.drain(..) {
            let Ok(data) = self.chain.account(&table).await else {
                kept.push(table);
                continue;
            };
            let Some(data) = data.filter(|d| d.len() >= 12) else { continue };
            let deactivated = u64::from_le_bytes(data[4..12].try_into().unwrap());
            if deactivated == u64::MAX {
                let _ = self.chain.send(&[alt::deactivate_lookup_table(table, me)], &self.key, None).await;
            } else if clock.slot > deactivated + TABLE_COOLDOWN
                && self.chain.send(&[alt::close_lookup_table(table, me, me)], &self.key, None).await.is_ok()
            {
                continue;
            }
            // Kept until it is closed; a close that landed but was reported
            // failed is found by the next read, as the table is gone.
            kept.push(table);
        }
        *list = kept;
    }

    pub(crate) fn me_key(&self) -> Pubkey {
        self.key.pubkey()
    }

    /// The evidence a guardian's note holds (`challenge.rs`, `notes.rs`).
    fn evidence(e: &Evidence) -> [u8; 32] {
        match e {
            Evidence::MissedDuty => [0; 32],
            Evidence::Parent(h) => sha256(&[b"parent", h]),
            Evidence::Fork(h) => sha256(&[b"fork", h]),
        }
    }

    fn note(&self, job_id: u64, evidence: &Evidence, salt: &[u8; 32]) -> [u8; 32] {
        sha256(&[self.me_key().as_ref(), &job_id.to_le_bytes(), &Self::evidence(evidence), salt])
    }

    /// Creates a walk from `descendant` down to `ancestor`, reading every
    /// block between them, and returns its address. Close it after use.
    pub(crate) async fn walk(&self, descendant: &BlockRef, ancestor: &BlockRef) -> anyhow::Result<Pubkey> {
        let prev_epoch_time = if descendant.epoch_time != ancestor.epoch_time { ancestor.epoch_time } else { 0 };
        let mut path = vec![*descendant];
        let mut at = *descendant;
        while at.height > ancestor.height {
            let node = self.node(&at).await?.ok_or_else(|| anyhow::anyhow!("block {} of a walk is not stored", at.height))?;
            let epoch_time = if at.height % EPOCH_BLOCKS == 0 { prev_epoch_time } else { at.epoch_time };
            at = BlockRef { hash: node.prev_hash, height: at.height - 1, epoch_time };
            path.push(at);
        }
        anyhow::ensure!(at == *ancestor, "the two blocks of a walk are not linked");

        let mut id_bytes = [0u8; 8];
        getrandom::fill(&mut id_bytes).map_err(|e| anyhow::anyhow!("no randomness: {e}"))?;
        let walk_id = u64::from_le_bytes(id_bytes);
        let walk = Self::lc_pda(&[b"walk", self.me_key().as_ref(), &walk_id.to_le_bytes()]);
        self.send(Self::lc_ix(
            lc::client::accounts::BeginWalk { walk, payer: self.me_key(), system_program: SYSTEM },
            lc::client::args::BeginWalk { walk_id, descendant: l_ref(descendant), ancestor: l_ref(ancestor), prev_epoch_time },
        ))
        .await?;
        for chunk in path.chunks(WALK_STEP) {
            let mut ix = Self::lc_ix(lc::client::accounts::WalkStep { config: Self::config_pda(), walk }, lc::client::args::WalkStep {});
            ix.accounts.extend(chunk.iter().map(|r| AccountMeta::new_readonly(Self::node_pda(r), false)));
            if let Err(e) = self.send(ix).await {
                // Only its owner can close it: do it now, or its rent is lost.
                self.close_walks(&[walk]).await;
                return Err(e);
            }
        }
        Ok(walk)
    }

    /// Closes walks and takes their rent back. A failure only costs rent.
    pub(crate) async fn close_walks(&self, walks: &[Pubkey]) {
        for walk in walks {
            let ix = Self::lc_ix(lc::client::accounts::CloseWalk { walk: *walk, owner: self.me_key() }, lc::client::args::CloseWalk {});
            if let Err(e) = self.send(ix).await {
                tracing::warn!(network = %self.name, error = %ipow_protocol_core::secrets::redact(&e), "closing a walk failed");
            }
        }
    }

    /// Runs `use_walks` with the walks made for it, then closes them.
    async fn with_walks<F>(&self, pairs: &[(&BlockRef, &BlockRef)], use_walks: F) -> anyhow::Result<()>
    where
        F: AsyncFnOnce(&[Pubkey]) -> anyhow::Result<()>,
    {
        let mut walks = vec![];
        let mut outcome = Ok(());
        for (d, a) in pairs {
            match self.walk(d, a).await {
                Ok(w) => walks.push(w),
                Err(e) => {
                    outcome = Err(e);
                    break;
                }
            }
        }
        if outcome.is_ok() {
            outcome = use_walks(&walks).await;
        }
        self.close_walks(&walks).await;
        outcome
    }
}

#[async_trait]
impl ProtocolNetwork for SvmNetwork {
    fn name(&self) -> &str {
        &self.name
    }

    fn me(&self) -> String {
        self.me_key().to_string()
    }

    async fn now(&self) -> anyhow::Result<i64> {
        Ok(self.chain.clock().await?.unix_timestamp)
    }

    async fn jobs_after(&self, after: u64, limit: usize) -> anyhow::Result<Vec<Job>> {
        let count = self.protocol_record().await?.job_count;
        let mut out = vec![];
        for id in (after + 1)..=count {
            if out.len() >= limit {
                break;
            }
            out.push(self.job(id).await?);
        }
        Ok(out)
    }

    async fn job(&self, id: u64) -> anyhow::Result<Job> {
        let j = self.job_record(id).await?;
        let now = self.now().await?;
        let auction_end = rules::auction_end(&j);
        // `duty::status`.
        let status = if now < auction_end {
            JobStatus::Auction
        } else if !j.has_operator {
            JobStatus::Expired
        } else if j.slashed {
            JobStatus::Slashed
        } else if j.settled {
            JobStatus::Settled
        } else if j.proven_at != 0 {
            JobStatus::Proven
        } else {
            JobStatus::Assigned
        };
        let locked_in = j.has_operator && now >= auction_end;
        Ok(Job {
            id,
            application: j.application.to_string(),
            tag: j.tag,
            escrow: j.escrow as Amount,
            commitment_fee: j.commitment_fee as Amount,
            escrow_fee: j.escrow_fee as Amount,
            bid: j.bid as Amount,
            operator: j.has_operator.then(|| j.operator.to_string()),
            confirmations: j.confirmations,
            claim_kind: j.claim_kind,
            status,
            auction_end,
            deadline: if locked_in { rules::deadline(&j) } else { 0 },
            anchor: if j.anchored_at != 0 { from_p(&j.anchor) } else { None },
            proof_block: if j.proven_at != 0 { from_p(&j.proof_block) } else { None },
            tip: if j.proven_at != 0 { from_p(&j.tip) } else { None },
            txid: (j.proven_at != 0).then_some(j.txid),
            deepest: if j.anchored_at != 0 { from_p(&j.deepest) } else { None },
            parents_shown: j.parents_shown,
            lock_end: j.lock_end,
            attester: j.has_attester.then(|| j.attester.to_string()),
            open_challenges: j.open_challenges,
        })
    }

    async fn my_bond(&self) -> anyhow::Result<(Amount, Amount)> {
        let o: Option<pr::accounts::Operator> = self.read(&Self::operator_pda(&self.me_key())).await?;
        Ok(o.map(|o| (o.bond as Amount, o.locked as Amount)).unwrap_or((0, 0)))
    }

    async fn my_credit(&self) -> anyhow::Result<Amount> {
        let c: Option<pr::accounts::Credit> = self.read(&Self::credit_pda(&self.me_key())).await?;
        Ok(c.map(|c| c.amount as Amount).unwrap_or(0))
    }

    async fn parent_deposit(&self, job_id: u64) -> anyhow::Result<Amount> {
        Ok(rules::parent_deposit(&self.job_record(job_id).await?) as Amount)
    }

    async fn minimum_bid(&self, job_id: u64) -> anyhow::Result<Amount> {
        Ok(rules::minimum_bid(&self.job_record(job_id).await?) as Amount)
    }

    async fn challenge_count(&self) -> anyhow::Result<u64> {
        Ok(self.protocol_record().await?.challenge_count)
    }

    async fn challenge(&self, id: u64) -> anyhow::Result<Option<Challenge>> {
        // A challenge's record is closed when it ends.
        let Some(c) = self.read::<pr::accounts::Challenge>(&Self::challenge_pda(id)).await? else {
            return Ok(None);
        };
        let kind = match c.kind {
            pr::types::ChallengeKind::Parent => ChallengeKind::Parent,
            pr::types::ChallengeKind::Fork => ChallengeKind::Fork,
        };
        Ok(Some(Challenge {
            id,
            kind,
            job_id: c.job_id,
            guardian: c.guardian.to_string(),
            deposit: c.deposit as Amount,
            opened_at: c.opened_at,
            asked: (kind == ChallengeKind::Parent).then(|| from_p(&c.asked)).flatten(),
        }))
    }

    async fn stored_block(&self, at: &BlockRef) -> anyhow::Result<Option<StoredBlock>> {
        Ok(self.node(at).await?.map(|n| StoredBlock {
            prev_hash: n.prev_hash,
            merkle_root: n.merkle_root,
            bits: n.bits,
            time: n.time,
            stored_at: n.stored_at,
        }))
    }

    async fn add_epoch_start(&self, headers: &[[u8; 80]], height: u32) -> anyhow::Result<()> {
        anyhow::ensure!(headers.len() == 6, "an epoch start is 6 headers");
        let epoch_time = u32::from_le_bytes(headers[0][68..72].try_into().unwrap());
        let first = sha256d(&headers[0]);
        let epoch_start = Self::lc_pda(&[b"epoch_start", &first, &height.to_le_bytes(), &epoch_time.to_le_bytes()]);
        let mut ix = Self::lc_ix(
            lc::client::accounts::AddEpochStart {
                config: Self::config_pda(),
                day_table: Self::lc_pda(&[b"days"]),
                epoch_start,
                payer: self.me_key(),
                system_program: SYSTEM,
            },
            lc::client::args::AddEpochStart { headers: headers.concat(), height },
        );
        for (i, h) in headers.iter().enumerate() {
            let r = BlockRef { hash: sha256d(h), height: height + i as u32, epoch_time };
            ix.accounts.push(AccountMeta::new(Self::node_pda(&r), false));
        }
        self.send(ix).await
    }

    async fn jump(&self, header: &[u8; 80], epoch_start: &BlockRef, height: u32) -> anyhow::Result<()> {
        let es = Self::lc_pda(&[b"epoch_start", &epoch_start.hash, &epoch_start.height.to_le_bytes(), &epoch_start.epoch_time.to_le_bytes()]);
        let at = BlockRef { hash: sha256d(header), height, epoch_time: epoch_start.epoch_time };
        self.send(Self::lc_ix(
            lc::client::accounts::Jump {
                config: Self::config_pda(),
                day_table: Self::lc_pda(&[b"days"]),
                epoch_start: es,
                node: Self::node_pda(&at),
                payer: self.me_key(),
                system_program: SYSTEM,
            },
            lc::client::args::Jump { header: *header, height },
        ))
        .await
    }

    async fn extend(&self, parent: &BlockRef, headers: &[[u8; 80]]) -> anyhow::Result<()> {
        let mut on = *parent;
        for chunk in headers.chunks(MAX_EXTEND) {
            let mut ix = Self::lc_ix(
                lc::client::accounts::Extend { config: Self::config_pda(), parent: Self::node_pda(&on), payer: self.me_key(), system_program: SYSTEM },
                lc::client::args::Extend { headers: chunk.concat() },
            );
            // The program skips blocks already stored; a chunk made only of
            // stored blocks is not sent at all.
            let mut new = false;
            for h in chunk {
                on = on.child(h);
                ix.accounts.push(AccountMeta::new(Self::node_pda(&on), false));
                if !new && self.node(&on).await?.is_none() {
                    new = true;
                }
            }
            if !new {
                continue;
            }
            self.send(ix).await?;
        }
        Ok(())
    }

    async fn extend_back(&self, child: &BlockRef, header: &[u8; 80], prev_epoch_time: u32) -> anyhow::Result<()> {
        let epoch_time = if child.height % EPOCH_BLOCKS == 0 { prev_epoch_time } else { child.epoch_time };
        let parent = BlockRef { hash: sha256d(header), height: child.height - 1, epoch_time };
        if self.node(&parent).await?.is_some() {
            return Ok(());
        }
        self.send(Self::lc_ix(
            lc::client::accounts::ExtendBack {
                config: Self::config_pda(),
                child: Self::node_pda(child),
                parent: Self::node_pda(&parent),
                payer: self.me_key(),
                system_program: SYSTEM,
            },
            lc::client::args::ExtendBack { header: *header, prev_epoch_time },
        ))
        .await
    }

    async fn lock_bond(&self, amount: Amount) -> anyhow::Result<()> {
        self.send(Self::pr_ix(
            pr::client::accounts::LockBond {
                protocol: Self::protocol_pda(),
                operator: Self::operator_pda(&self.me_key()),
                vault: Self::vault_pda(),
                owner: self.me_key(),
                system_program: SYSTEM,
            },
            pr::client::args::LockBond { amount: u64::try_from(amount)? },
        ))
        .await
    }

    async fn withdraw_bond(&self, amount: Amount) -> anyhow::Result<()> {
        self.send(Self::pr_ix(
            pr::client::accounts::WithdrawBond {
                protocol: Self::protocol_pda(),
                operator: Self::operator_pda(&self.me_key()),
                vault: Self::vault_pda(),
                owner: self.me_key(),
            },
            pr::client::args::WithdrawBond { amount: u64::try_from(amount)? },
        ))
        .await
    }

    async fn bid(&self, job_id: u64, amount: Amount) -> anyhow::Result<()> {
        let job = self.job_record(job_id).await?;
        let previous = (job.has_operator && job.operator != self.me_key()).then(|| Self::operator_pda(&job.operator));
        self.send(Self::pr_ix(
            pr::client::accounts::Bid { job: Self::job_pda(job_id), operator: Self::operator_pda(&self.me_key()), previous, owner: self.me_key() },
            pr::client::args::Bid { amount: u64::try_from(amount)? },
        ))
        .await
    }

    async fn chain_head_commitment(&self) -> anyhow::Result<[u8; 32]> {
        Ok(sha256(&[b"iPoW chain head", pr::ID.as_ref(), self.me_key().as_ref()]))
    }

    async fn chain_head(&self) -> anyhow::Result<Option<([u8; 32], u32)>> {
        let o: Option<pr::accounts::Operator> = self.read(&Self::operator_pda(&self.me_key())).await?;
        Ok(o.filter(|o| o.chain_head_set).map(|o| (o.chain_head_txid, o.chain_head_vout)))
    }

    async fn tag_payload(&self, tag: &[u8; 32]) -> anyhow::Result<[u8; 32]> {
        Ok(sha256(&[b"iPoW job", tag]))
    }

    async fn register_chain_head(&self, block: &BlockRef, raw_tx: &[u8], siblings: &[[u8; 32]], tx_index: u64, coin_index: u32, tag_index: u32) -> anyhow::Result<()> {
        let txid = sha256d(raw_tx);
        self.send(Self::pr_ix(
            pr::client::accounts::RegisterChainHead {
                operator: Self::operator_pda(&self.me_key()),
                node: Self::node_pda(block),
                used_tx: Self::used_tx_pda(&txid),
                owner: self.me_key(),
                system_program: SYSTEM,
            },
            pr::client::args::RegisterChainHead {
                txid,
                block: p_ref(block),
                raw_tx: raw_tx.to_vec(),
                siblings: siblings.to_vec(),
                tx_index,
                coin_index,
                tag_index,
            },
        ))
        .await
    }

    async fn advance_chain_head(&self, block: &BlockRef, raw_tx: &[u8], siblings: &[[u8; 32]], tx_index: u64, head_index: u32) -> anyhow::Result<()> {
        let txid = sha256d(raw_tx);
        self.send(Self::pr_ix(
            pr::client::accounts::AdvanceChainHead {
                operator: Self::operator_pda(&self.me_key()),
                node: Self::node_pda(block),
                used_tx: Self::used_tx_pda(&txid),
                owner: self.me_key(),
                system_program: SYSTEM,
            },
            pr::client::args::AdvanceChainHead { txid, block: p_ref(block), raw_tx: raw_tx.to_vec(), siblings: siblings.to_vec(), tx_index, head_index },
        ))
        .await
    }

    async fn anchor_job(&self, job_id: u64, anchor: &BlockRef) -> anyhow::Result<()> {
        self.send(Self::pr_ix(
            pr::client::accounts::AnchorJob { job: Self::job_pda(job_id), node: Self::node_pda(anchor), owner: self.me_key() },
            pr::client::args::AnchorJob { anchor: p_ref(anchor) },
        ))
        .await
    }

    async fn prove_job(&self, job_id: u64, proof: &Proof) -> anyhow::Result<()> {
        let job = self.job_record(job_id).await?;
        let anchor = from_p(&job.anchor).ok_or_else(|| anyhow::anyhow!("job {job_id} has no anchor"))?;
        let txid = sha256d(&proof.raw_tx);
        let (pb, tip) = (proof.proof_block, proof.tip);
        self.with_walks(&[(&pb, &anchor), (&tip, &pb)], async |walks: &[Pubkey]| {
            self.send(Self::pr_ix(
                pr::client::accounts::ProveJob {
                    job: Self::job_pda(job_id),
                    application: Self::pr_pda(&[b"application", job.application.as_ref()]),
                    operator: Self::operator_pda(&self.me_key()),
                    proof_node: Self::node_pda(&pb),
                    walk_to_proof: walks[0],
                    walk_to_tip: walks[1],
                    used_tx: Self::used_tx_pda(&txid),
                    owner: self.me_key(),
                    system_program: SYSTEM,
                },
                pr::client::args::ProveJob {
                    proof: pr::types::Proof {
                        proof_block: p_ref(&pb),
                        tip: p_ref(&tip),
                        raw_tx: proof.raw_tx.clone(),
                        txid,
                        siblings: proof.siblings.clone(),
                        tx_index: proof.tx_index,
                        head_index: proof.head_index,
                        tag_index: proof.tag_index,
                    },
                },
            ))
            .await
        })
        .await
    }

    async fn settle(&self, job_id: u64) -> anyhow::Result<()> {
        let job = self.job_record(job_id).await?;
        let other = job.has_attester && job.attester != job.operator;
        self.send(Self::pr_ix(
            pr::client::accounts::Settle {
                job: Self::job_pda(job_id),
                operator: Self::operator_pda(&job.operator),
                operator_credit: Self::credit_pda(&job.operator),
                attester: other.then(|| Self::operator_pda(&job.attester)),
                attester_credit: other.then(|| Self::credit_pda(&job.attester)),
                funder: self.me_key(),
                system_program: SYSTEM,
            },
            pr::client::args::Settle {},
        ))
        .await
    }

    async fn attest(&self, job_id: u64) -> anyhow::Result<()> {
        let job = self.job_record(job_id).await?;
        let me = self.me_key();
        self.send(Self::pr_ix(
            pr::client::accounts::Attest {
                job: Self::job_pda(job_id),
                attester: Self::operator_pda(&me),
                operator: (job.operator != me).then(|| Self::operator_pda(&job.operator)),
                owner: me,
            },
            pr::client::args::Attest {},
        ))
        .await
    }

    async fn seal_note(&self, job_id: u64, evidence: &Evidence, salt: &[u8; 32]) -> anyhow::Result<()> {
        let note = self.note(job_id, evidence, salt);
        self.send(Self::pr_ix(
            pr::client::accounts::SealNote { record: Self::note_pda(&note), guardian: self.me_key(), system_program: SYSTEM },
            pr::client::args::SealNote { note },
        ))
        .await
    }

    async fn report_missed_duty(&self, job_id: u64, salt: &[u8; 32]) -> anyhow::Result<()> {
        let job = self.job_record(job_id).await?;
        let note = self.note(job_id, &Evidence::MissedDuty, salt);
        self.send(Self::pr_ix(
            pr::client::accounts::ReportMissedDuty {
                job: Self::job_pda(job_id),
                operator: Self::operator_pda(&job.operator),
                note_record: Self::note_pda(&note),
                application_credit: Self::credit_pda(&job.application),
                guardian_credit: Self::credit_pda(&self.me_key()),
                payer_credit: Self::credit_pda(&job.payer),
                guardian: self.me_key(),
                system_program: SYSTEM,
            },
            pr::client::args::ReportMissedDuty { note, salt: *salt },
        ))
        .await
    }

    async fn ask_parent(&self, job_id: u64, salt: &[u8; 32], deposit: Amount) -> anyhow::Result<u64> {
        let job = self.job_record(job_id).await?;
        let note = self.note(job_id, &Evidence::Parent(job.deepest.hash), salt);
        let id = self.protocol_record().await?.challenge_count + 1;
        self.send(Self::pr_ix(
            pr::client::accounts::AskParent {
                protocol: Self::protocol_pda(),
                job: Self::job_pda(job_id),
                challenge: Self::challenge_pda(id),
                note_record: Self::note_pda(&note),
                vault: Self::vault_pda(),
                guardian: self.me_key(),
                system_program: SYSTEM,
            },
            pr::client::args::AskParent { note, salt: *salt, paid: u64::try_from(deposit)? },
        ))
        .await?;
        Ok(id)
    }

    async fn show_parent(&self, challenge_id: u64, prev_epoch_time: u32) -> anyhow::Result<()> {
        let c: pr::accounts::Challenge =
            self.read(&Self::challenge_pda(challenge_id)).await?.ok_or_else(|| anyhow::anyhow!("challenge {challenge_id} is not open"))?;
        let job = self.job_record(c.job_id).await?;
        let asked = from_p(&c.asked).ok_or_else(|| anyhow::anyhow!("a question without its block"))?;
        let child = self.node(&asked).await?.ok_or_else(|| anyhow::anyhow!("the asked block is not stored"))?;
        let epoch_time = if asked.height % EPOCH_BLOCKS == 0 { prev_epoch_time } else { asked.epoch_time };
        let parent = BlockRef { hash: child.prev_hash, height: asked.height.saturating_sub(1), epoch_time };
        self.send(Self::pr_ix(
            pr::client::accounts::ShowParent {
                challenge: Self::challenge_pda(challenge_id),
                job: Self::job_pda(c.job_id),
                child: Self::node_pda(&asked),
                parent: Self::node_pda(&parent),
                lc_config: Self::config_pda(),
                operator_credit: Self::credit_pda(&job.operator),
                guardian: c.guardian,
                funder: self.me_key(),
                system_program: SYSTEM,
            },
            pr::client::args::ShowParent { prev_epoch_time },
        ))
        .await
    }

    async fn challenge_fork(
        &self,
        job_id: u64,
        operator_block: &BlockRef,
        guardian_block: &BlockRef,
        guardian_tip: &BlockRef,
        _prev_epoch_time: u32,
        salt: &[u8; 32],
        deposit: Amount,
    ) -> anyhow::Result<u64> {
        let job = self.job_record(job_id).await?;
        let (proof_block, tip) = match (from_p(&job.proof_block), from_p(&job.tip)) {
            (Some(p), Some(t)) if job.proven_at != 0 => (p, t),
            _ => anyhow::bail!("job {job_id} is not proven"),
        };
        let note = self.note(job_id, &Evidence::Fork(guardian_block.hash), salt);
        let id = self.protocol_record().await?.challenge_count + 1;
        let (ob, gb, gt) = (*operator_block, *guardian_block, *guardian_tip);
        self.with_walks(&[(&proof_block, &ob), (&tip, &proof_block), (&gt, &gb)], async |walks: &[Pubkey]| {
            self.send(Self::pr_ix(
                pr::client::accounts::ChallengeFork {
                    protocol: Self::protocol_pda(),
                    job: Self::job_pda(job_id),
                    challenge: Self::challenge_pda(id),
                    note_record: Self::note_pda(&note),
                    operator_node: Self::node_pda(&ob),
                    guardian_node: Self::node_pda(&gb),
                    walk_proof_to_operator: walks[0],
                    walk_tip_to_proof: walks[1],
                    walk_guardian: walks[2],
                    operator_checkpoint: Self::checkpoint_pda(id, false, &tip),
                    guardian_checkpoint: Self::checkpoint_pda(id, true, &gt),
                    proof_checkpoint: Self::checkpoint_pda(id, false, &proof_block),
                    guardian_first_checkpoint: Self::checkpoint_pda(id, true, &gb),
                    vault: Self::vault_pda(),
                    guardian: self.me_key(),
                    system_program: SYSTEM,
                },
                pr::client::args::ChallengeFork {
                    note,
                    salt: *salt,
                    paid: u64::try_from(deposit)?,
                    args: pr::types::ForkArgs { operator_block: p_ref(&ob), guardian_block: p_ref(&gb), guardian_tip: p_ref(&gt) },
                },
            ))
            .await
        })
        .await?;
        Ok(id)
    }

    async fn extend_branch(&self, challenge_id: u64, guardian_side: bool, from: &BlockRef, new_tip: &BlockRef, _prev_epoch_time: u32) -> anyhow::Result<()> {
        let c: pr::accounts::Challenge =
            self.read(&Self::challenge_pda(challenge_id)).await?.ok_or_else(|| anyhow::anyhow!("challenge {challenge_id} is not open"))?;
        let (f, t) = (*from, *new_tip);
        self.with_walks(&[(&t, &f)], async |walks: &[Pubkey]| {
            self.send(Self::pr_ix(
                pr::client::accounts::ExtendBranch {
                    challenge: Self::challenge_pda(challenge_id),
                    job: Self::job_pda(c.job_id),
                    from_checkpoint: Self::checkpoint_pda(challenge_id, guardian_side, &f),
                    new_checkpoint: Self::checkpoint_pda(challenge_id, guardian_side, &t),
                    walk: walks[0],
                    funder: self.me_key(),
                    system_program: SYSTEM,
                },
                pr::client::args::ExtendBranch { guardian_side, from: p_ref(&f), new_tip: p_ref(&t) },
            ))
            .await
        })
        .await
    }

    async fn resolve_challenge(&self, challenge_id: u64) -> anyhow::Result<()> {
        let c: pr::accounts::Challenge =
            self.read(&Self::challenge_pda(challenge_id)).await?.ok_or_else(|| anyhow::anyhow!("challenge {challenge_id} is not open"))?;
        let job = self.job_record(c.job_id).await?;
        let other = job.has_attester && job.attester != job.operator;
        self.send(Self::pr_ix(
            pr::client::accounts::ResolveChallenge {
                challenge: Self::challenge_pda(challenge_id),
                job: Self::job_pda(c.job_id),
                operator: Self::operator_pda(&job.operator),
                attester: other.then(|| Self::operator_pda(&job.attester)),
                operator_credit: Self::credit_pda(&job.operator),
                application_credit: Self::credit_pda(&job.application),
                guardian_credit: Self::credit_pda(&c.guardian),
                payer_credit: Self::credit_pda(&job.payer),
                guardian: c.guardian,
                funder: self.me_key(),
                system_program: SYSTEM,
            },
            pr::client::args::ResolveChallenge {},
        ))
        .await
    }

    async fn withdraw_credit(&self) -> anyhow::Result<()> {
        self.send(Self::pr_ix(
            pr::client::accounts::WithdrawCredit {
                protocol: Self::protocol_pda(),
                credit: Self::credit_pda(&self.me_key()),
                vault: Self::vault_pda(),
                owner: self.me_key(),
            },
            pr::client::args::WithdrawCredit {},
        ))
        .await
    }
}
