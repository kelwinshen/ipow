//! The node's roles for the protocol's vault (spec, section 11), for the
//! pair Ethereum and Solana:
//!
//! - `VaultOperator` registers this node's pair chain, keeps its bond and
//!   deposit money, gathers records (locks on Ethereum, burns and give-ups
//!   on Solana), writes them in batches on Bitcoin, submits each message to
//!   both vaults, and answers objections to its claims.
//! - `VaultGuardian` checks every claim on each network against the other
//!   network, objects to a false one, and decides and collects.
//!
//! Both work on the two networks at once. The supervisor runs them as
//! ordinary workers; the network it hands them only names the task.

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::Arc;

use async_trait::async_trait;
use ipow_bitcoin::view::{BitcoinView, TxStatus};
use ipow_protocol_core::network::ProtocolNetwork;
use ipow_protocol_core::settings::Role;
use ipow_protocol_core::types::{Amount, BlockRef, JobStatus};
use ipow_protocol_core::settings::FastSettings;
use ipow_protocol_core::vault::{encode, message_payload, Chain, Record, Request, TxProof, VaultApp, ETHEREUM, SOLANA};
use tokio::sync::Mutex;
use tracing::{debug, info, warn};

use crate::light::epoch_time_at;
use crate::supervisor::Worker;
use crate::wallet::{head_spend, merkle_proof, SharedWallet};

/// D110: a chain's open claims are at most 80% of its bond.
const COVER_BPS: u128 = 8000;
/// Section 11.3 (D119) and the Solana vault's limit of 10 LOCK records.
const MAX_BATCH: usize = 2048;
const MAX_HOME_RECORDS: usize = 32;
const MAX_LOCKS: usize = 10;
/// The light client walks at most 100 blocks.
const MAX_WALK: u32 = 100;
/// D111.
const OBJECTION_WINDOW: i64 = 7 * 24 * 3600;
/// How long to wait before opening another checkpoint job on a network.
const CHECKPOINT_EVERY: i64 = 2 * 3600;
/// Wei in a gwei.
const GWEI: u128 = 1_000_000_000;

/// One network of the pair: its vault and its protocol.
#[derive(Clone)]
pub struct Side {
    pub vault: Arc<dyn VaultApp>,
    pub net: Arc<dyn ProtocolNetwork>,
}

/// Blocks a vault may take as real (D108): proof blocks of finished jobs
/// with the least certifying escrow (D118), found by scanning jobs.
#[derive(Default)]
struct Reals {
    job_cursor: u64,
    blocks: Vec<BlockRef>,
    checkpoint_at: i64,
}

async fn block_ref(btc: &dyn BitcoinView, hash: [u8; 32], height: u32) -> anyhow::Result<BlockRef> {
    Ok(BlockRef { hash, height, epoch_time: epoch_time_at(btc, height).await? })
}

/// Records the real blocks of newly finished jobs. Returns whether a job
/// that could certify is still under way: one that may yet finish.
async fn scan_reals(side: &Side, btc: &dyn BitcoinView, reals: &mut Reals) -> anyhow::Result<bool> {
    let min = side.vault.min_certifying_escrow().await?;
    let now = side.net.now().await?;
    loop {
        let jobs = side.net.jobs_after(reals.job_cursor, 50).await?;
        let Some(last) = jobs.last() else { return Ok(false) };
        let last = last.id;
        let mut stop_at = None;
        for j in jobs {
            if j.escrow < min {
                continue;
            }
            let lock_over = now >= j.lock_end;
            let done = j.status == JobStatus::Settled || (j.status == JobStatus::Proven && lock_over && j.open_challenges == 0);
            // Still able to finish: its auction, its duty before the
            // deadline, or its lock. A job past its deadline, or whose lock
            // ended under a challenge, may never finish: it does not hold
            // the scan up.
            let waiting = j.status == JobStatus::Auction
                || (j.status == JobStatus::Assigned && now <= j.deadline)
                || (j.status == JobStatus::Proven && !lock_over);
            if waiting {
                stop_at.get_or_insert(j.id - 1);
                continue;
            }
            if !done || stop_at.is_some() {
                continue;
            }
            let Some(pb) = j.proof_block else { continue };
            // Only a block Bitcoin itself has, at its stated place.
            if btc.best_chain_height(&pb.hash).await? != Some(pb.height) {
                continue;
            }
            if !side.vault.is_real(&pb).await? {
                if let Err(e) = side.vault.record_real_from_job(j.id).await {
                    warn!(job = j.id, error = %crate::secrets::redact(&e), "recording a real block failed");
                    continue;
                }
            }
            if !reals.blocks.contains(&pb) {
                reals.blocks.push(pb);
            }
        }
        reals.job_cursor = stop_at.unwrap_or(last).max(reals.job_cursor);
        if stop_at.is_some() {
            return Ok(true);
        }
    }
}

/// A real block the vault knows, at or above `block` on Bitcoin's best
/// chain and at most `MAX_WALK` above it. When the nearest real block is
/// further up, blocks are recorded real downward from it in steps of at most
/// `MAX_WALK`, storing the blocks between. `None` when no real block is
/// above at all: a checkpoint job is opened when `checkpoint` gives what to
/// pay, no job that could certify is under way, and none was opened in the
/// last `CHECKPOINT_EVERY`.
async fn reach(side: &Side, btc: &dyn BitcoinView, reals: &mut Reals, block: &BlockRef, checkpoint: Option<Amount>) -> anyhow::Result<Option<BlockRef>> {
    if side.vault.is_real(block).await? {
        return Ok(Some(*block));
    }
    let under_way = scan_reals(side, btc, reals).await?;
    // Blocks Bitcoin has since dropped are forgotten.
    let mut kept = vec![];
    for r in reals.blocks.drain(..) {
        if btc.best_chain_height(&r.hash).await? == Some(r.height) {
            kept.push(r);
        }
    }
    reals.blocks = kept;
    let Some(mut real) = reals.blocks.iter().filter(|r| r.height >= block.height).min_by_key(|r| r.height).copied() else {
        if !under_way {
            if let Some(paid) = checkpoint {
                let now = side.net.now().await?;
                if now - reals.checkpoint_at >= CHECKPOINT_EVERY {
                    reals.checkpoint_at = now;
                    let job = side.vault.open_checkpoint(6, paid).await?;
                    info!(network = side.net.name(), job, "opened a checkpoint job to make a real block");
                }
            }
        }
        return Ok(None);
    };
    while real.height - block.height > MAX_WALK {
        let h = real.height - MAX_WALK;
        let mid = block_ref(btc, btc.block_hash(h).await?, h).await?;
        fill_down(side.net.as_ref(), btc, &real, &mid).await?;
        side.vault.record_real(&mid, &real).await?;
        reals.blocks.push(mid);
        real = mid;
    }
    Ok(Some(real))
}

/// Stores in the light client every block from `high`, a stored real
/// block, down to `low`, below it on the same chain: a walk between them
/// needs each. Blocks below the point an operator's feed jumped to are not
/// stored until someone stores them.
async fn fill_down(net: &dyn ProtocolNetwork, btc: &dyn BitcoinView, high: &BlockRef, low: &BlockRef) -> anyhow::Result<()> {
    let mut at = *high;
    while at.height > low.height {
        let node = net.stored_block(&at).await?.ok_or_else(|| anyhow::anyhow!("block {} is not stored", at.height))?;
        let height = at.height - 1;
        let epoch_time = if at.height % ipow_protocol_core::types::EPOCH_BLOCKS == 0 { epoch_time_at(btc, height).await? } else { at.epoch_time };
        let parent = BlockRef { hash: node.prev_hash, height, epoch_time };
        if net.stored_block(&parent).await?.is_none() {
            let header = btc.header(&parent.hash).await?;
            let prev_epoch_time = if at.height % ipow_protocol_core::types::EPOCH_BLOCKS == 0 { epoch_time } else { 0 };
            net.extend_back(&at, &header, prev_epoch_time).await?;
        }
        at = parent;
    }
    anyhow::ensure!(at == *low, "the two blocks are not on one chain");
    Ok(())
}

/// Where a transaction is and its proof against `real`.
async fn tx_proof(btc: &dyn BitcoinView, txid: &[u8; 32], block: &BlockRef, real: BlockRef) -> anyhow::Result<TxProof> {
    let raw = btc.raw_tx(txid).await?.ok_or_else(|| anyhow::anyhow!("the transaction is not known"))?;
    let (siblings, tx_index) = merkle_proof(btc, &block.hash, txid).await?;
    Ok(TxProof { block: *block, raw_tx: raw, siblings, tx_index, real })
}

/// The mined block of a transaction, if it is in Bitcoin's best chain.
async fn mined_in(btc: &dyn BitcoinView, txid: &[u8; 32]) -> anyhow::Result<Option<BlockRef>> {
    match btc.tx_status(txid).await? {
        Some(TxStatus::Confirmed { block, height }) if btc.best_chain_height(&block).await? == Some(height) => Ok(Some(block_ref(btc, block, height).await?)),
        _ => Ok(None),
    }
}

// ----------------------------------------------------------------------
// The journal: the batches this node wrote
// ----------------------------------------------------------------------

/// Every batch this node wrote, by its hash, and its registration. Only a
/// batch's hash is on Bitcoin, so a message a vault has not processed yet
/// can only be submitted with the batch kept here. A batch is written
/// before its message exists, so no crash can lose it; a message replaced
/// to pay more carries the same hash and finds the same batch. A line cut
/// short by a crash is skipped when read.
pub struct Journal {
    path: PathBuf,
}

#[derive(Default)]
struct JournalState {
    registration: Option<[u8; 32]>,
    batches: HashMap<[u8; 32], Vec<u8>>,
}

impl Journal {
    pub fn new(path: PathBuf) -> Self {
        Journal { path }
    }

    fn read(&self) -> anyhow::Result<JournalState> {
        let text = match std::fs::read_to_string(&self.path) {
            Ok(t) => t,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => String::new(),
            Err(e) => return Err(e.into()),
        };
        // A line cut short by a crash is dropped from the file, so the next
        // line appended starts clean.
        if !text.is_empty() && !text.ends_with('\n') {
            let keep = text.rfind('\n').map_or(0, |i| i + 1);
            std::fs::OpenOptions::new().write(true).open(&self.path)?.set_len(keep as u64)?;
            warn!(path = %self.path.display(), "dropped a journal line cut short by a crash");
        }
        let mut j = JournalState::default();
        for line in text.split_inclusive('\n') {
            // A line with no end was cut short by a crash.
            let Some(line) = line.strip_suffix('\n') else { continue };
            let mut parts = line.split(' ');
            match (parts.next(), parts.next().map(hex::decode)) {
                (Some("registration"), Some(Ok(txid))) if txid.len() == 32 => j.registration = Some(txid.try_into().unwrap()),
                (Some("batch"), Some(Ok(batch))) => {
                    j.batches.insert(message_payload(&batch), batch);
                }
                _ => warn!(path = %self.path.display(), "skipped a journal line that does not read"),
            }
        }
        Ok(j)
    }

    fn append(&self, line: &str) -> anyhow::Result<()> {
        use std::io::Write;
        let mut f = std::fs::OpenOptions::new().create(true).append(true).open(&self.path)?;
        f.write_all(format!("{line}\n").as_bytes())?;
        f.sync_all()?;
        Ok(())
    }
}

// ----------------------------------------------------------------------
// The operator
// ----------------------------------------------------------------------

/// What the operator keeps in the vaults, and what it carries.
#[derive(Clone, Debug)]
pub struct VaultOperatorSettings {
    /// ETH to keep bonded in the Ethereum vault, in wei.
    pub eth_bond: Amount,
    /// vETH to keep bonded in the Solana vault, in gwei (0: carry no burn
    /// requests or cancels).
    pub veth_bond: Amount,
    /// Claim deposits to keep in each vault, as a count of deposits: at
    /// least 1.
    pub deposits: u32,
    /// The least fee, in gwei, of a record worth carrying. A record whose
    /// fee another operator already earned, and whose claims were all
    /// refused, is carried anyway, so that its user is not stranded.
    pub min_fee_gwei: u64,
    /// What to pay to open a checkpoint job when no real block is above a
    /// message, on Ethereum and on Solana; `None` never opens one.
    pub checkpoint_paid: [Option<Amount>; 2],
    pub journal: PathBuf,
    /// The fast paths (section 11.7): None attests and pays nothing at once.
    pub fast: Option<FastSettings>,
}

/// Section 11.7: an attester locks 1.25 times a lock's amount, and a claim
/// carrying its record must open within 7 days (D123).
const FAST_COLLATERAL_BPS: u128 = 12_500;
const FAST_OPEN_WINDOW: i64 = 7 * 24 * 3600;

/// What a record makes the acting network mint or pay: the amount and the
/// fast fee, in gwei (section 11.5).
fn value_of(r: &Record) -> u64 {
    match r {
        Record::Lock { amount, fast_fee, .. } | Record::Request { amount, fast_fee, .. } => amount.saturating_add(*fast_fee),
        _ => 0,
    }
}

/// The largest message transaction, without witness data (D119).
const MAX_RAW_TX: usize = 1024;
/// Blocks a message may wait unmined before it is replaced to pay more.
const REPLACE_AFTER: u32 = 3;
/// Locks and requests looked at per round.
const WINDOW: u64 = 100;

/// The claims that carry a record, on the network that acts on it, and
/// whether each is still open or was accepted.
#[derive(Default)]
struct ClaimIndex {
    cursor: u64,
    /// Claim => (operator, decided, accepted).
    status: HashMap<u64, (String, bool, bool)>,
    carrying: HashMap<Record, Vec<u64>>,
    /// Claims whose records could not be read yet (Ethereum's event logs):
    /// tried again each round, without holding anything up.
    unread: Vec<u64>,
}

impl ClaimIndex {
    /// Whether some claim carrying `r` is open or accepted: then it is being
    /// handled, or was.
    fn handled(&self, r: &Record) -> bool {
        self.carrying.get(r).is_some_and(|ids| ids.iter().any(|id| self.status.get(id).is_none_or(|(_, decided, accepted)| !decided || *accepted)))
    }

    /// Whether a claim of `operator` carrying `r` is open or accepted.
    fn handled_by(&self, r: &Record, operator: &str) -> bool {
        self.carrying.get(r).is_some_and(|ids| {
            ids.iter().any(|id| self.status.get(id).is_some_and(|(op, decided, accepted)| op == operator && (!decided || *accepted)))
        })
    }
}

#[derive(Default)]
struct OpState {
    loaded: bool,
    journal: JournalState,
    /// The highest lock and request ids read so far, and those among them
    /// not finished yet: returned, issued, paid, or carried by an accepted
    /// claim. One that never finishes is looked at again each round, and
    /// never hides newer ones.
    lock_cursor: u64,
    request_cursor: u64,
    open_locks: std::collections::BTreeSet<u64>,
    open_requests: std::collections::BTreeSet<u64>,
    claims: [ClaimIndex; 2],
    reals: [Reals; 2],
    /// Messages sent and not mined yet: the Bitcoin height when first seen
    /// waiting, and the fee rate paid.
    waiting: HashMap<[u8; 32], (u32, f64)>,
    /// The attests of locks read so far, and this node's among them not
    /// settled yet, by number, with the record each stated.
    attest_cursor: u64,
    attests: HashMap<u64, Record>,
    /// Burns this node paid at once, until the vault repays it.
    fast_paid: HashMap<u64, Request>,
}

pub struct VaultOperator {
    eth: Side,
    sol: Side,
    btc: Arc<dyn BitcoinView>,
    wallet: Arc<SharedWallet>,
    settings: VaultOperatorSettings,
    journal: Journal,
    state: Mutex<OpState>,
}

/// Whether a chain can open a claim on its network now: a claim of a chain
/// that cannot would earn its home fees and leave its users waiting.
async fn can_open(side: &Side, c: &Chain) -> anyhow::Result<bool> {
    Ok(!c.refused && !c.slashed && !c.exited && c.deposits >= side.vault.deposit().await?)
}

impl VaultOperator {
    pub fn new(eth: Side, sol: Side, btc: Arc<dyn BitcoinView>, wallet: Arc<SharedWallet>, settings: VaultOperatorSettings) -> Self {
        let journal = Journal::new(settings.journal.clone());
        VaultOperator { eth, sol, btc, wallet, settings, journal, state: Mutex::new(OpState::default()) }
    }

    fn sides(&self) -> [&Side; 2] {
        [&self.eth, &self.sol]
    }

    /// Registers the pair chain on each network that does not know it yet.
    /// Returns whether both do.
    async fn register(&self, st: &mut OpState, chains: &[Option<Chain>; 2]) -> anyhow::Result<bool> {
        if chains.iter().all(Option::is_some) {
            return Ok(true);
        }
        let eth_commitment = self.eth.vault.pair_commitment(&self.sol.vault.me_bytes()).await?;
        let sol_commitment = self.sol.vault.pair_commitment(&self.eth.vault.me_bytes()).await?;
        anyhow::ensure!(eth_commitment == sol_commitment, "the two vaults compute different registrations: check their settings");
        let Some(txid) = st.journal.registration else {
            if !self.wallet.can_pay().await? {
                warn!("the Bitcoin wallet cannot pay for the vault's registration");
                return Ok(false);
            }
            // Written after it is sent: a registration lost to a crash is
            // only sent again, at the cost of its fee.
            let sent = self.wallet.send(&[], &eth_commitment).await?;
            self.journal.append(&format!("registration {}", hex::encode(sent.txid)))?;
            st.journal.registration = Some(sent.txid);
            info!(txid = %hex::encode(sent.txid), "sent the vault's registration");
            return Ok(false);
        };
        let Some(block) = mined_in(self.btc.as_ref(), &txid).await? else { return Ok(false) };
        for (i, side) in self.sides().into_iter().enumerate() {
            if chains[i].is_some() {
                continue;
            }
            let Some(real) = reach(side, self.btc.as_ref(), &mut st.reals[i], &block, self.settings.checkpoint_paid[i]).await? else { continue };
            fill_down(side.net.as_ref(), self.btc.as_ref(), &real, &block).await?;
            let other = if i == 0 { &self.sol } else { &self.eth };
            let proof = tx_proof(self.btc.as_ref(), &txid, &block, real).await?;
            side.vault.register_chain(&other.vault.me_bytes(), &proof, 0, 1).await?;
            info!(network = side.net.name(), "registered the vault's pair chain");
        }
        Ok(false)
    }

    /// Keeps the bond and the deposit money at what the settings ask. A
    /// failure is only logged: it must not keep the node from answering
    /// objections.
    async fn top_up(&self, chains: &[Chain; 2]) {
        let mut asks: Vec<(&Side, &str, anyhow::Result<()>)> = vec![];
        if chains[0].bond < self.settings.eth_bond && !chains[0].slashed {
            asks.push((&self.eth, "bond", self.eth.vault.add_bond(self.settings.eth_bond - chains[0].bond).await));
        }
        if chains[1].bond < self.settings.veth_bond && !chains[1].slashed {
            asks.push((&self.sol, "vETH bond", self.sol.vault.add_bond(self.settings.veth_bond - chains[1].bond).await));
        }
        for (i, side) in self.sides().into_iter().enumerate() {
            let outcome = async {
                let want = side.vault.deposit().await? * self.settings.deposits as Amount;
                if chains[i].deposits < want / 2 {
                    side.vault.add_deposits(want - chains[i].deposits).await?;
                }
                Ok(())
            }
            .await;
            asks.push((side, "deposits", outcome));
        }
        for (side, what, outcome) in asks {
            if let Err(e) = outcome {
                warn!(network = side.net.name(), what, error = %crate::secrets::redact(&e), "topping up the vault failed");
            }
        }
    }

    /// Submits the messages a vault has not processed yet, oldest first, and
    /// replaces one that waits too long unmined. Returns whether both
    /// vaults are at the end of the chain, with no message waiting.
    async fn catch_up(&self, st: &mut OpState, chains: &[Chain; 2]) -> anyhow::Result<bool> {
        let mut done = true;
        for (i, side) in self.sides().into_iter().enumerate() {
            let mut coin = chains[i].coin;
            while let Some(txid) = self.btc.spender(&coin.0, coin.1).await? {
                done = false;
                let raw = self.btc.raw_tx(&txid).await?.ok_or_else(|| anyhow::anyhow!("the pair chain's message is not known"))?;
                let payload = SharedWallet::payload_of(&raw).ok_or_else(|| anyhow::anyhow!("the pair chain's message carries no payload"))?;
                let Some(batch) = st.journal.batches.get(&payload).cloned() else {
                    anyhow::bail!("the batch of the pair chain's message {} is not in the journal", hex::encode(txid));
                };
                let Some(block) = mined_in(self.btc.as_ref(), &txid).await? else {
                    self.bump(st, &txid, &payload).await?;
                    break;
                };
                st.waiting.remove(&txid);
                let Some(real) = reach(side, self.btc.as_ref(), &mut st.reals[i], &block, self.settings.checkpoint_paid[i]).await? else { break };
                fill_down(side.net.as_ref(), self.btc.as_ref(), &real, &block).await?;
                let proof = tx_proof(self.btc.as_ref(), &txid, &block, real).await?;
                if let Err(e) = side.vault.submit_message(&side.vault.me(), &proof, 0, 1, &batch).await {
                    // A guardian may have brought the same message first.
                    let moved = side.vault.chain(&side.vault.me()).await?.is_some_and(|c| c.coin == (txid, 0));
                    if !moved {
                        return Err(e);
                    }
                    debug!(network = side.net.name(), txid = %hex::encode(txid), error = %crate::secrets::redact(&e), "a vault message was already brought by someone else");
                } else {
                    info!(network = side.net.name(), txid = %hex::encode(txid), "submitted a vault message");
                }
                coin = (txid, 0);
            }
        }
        Ok(done)
    }

    /// Replaces a message that waited `REPLACE_AFTER` blocks unmined with
    /// one that pays half as much again. It carries the same payload.
    async fn bump(&self, st: &mut OpState, txid: &[u8; 32], payload: &[u8; 32]) -> anyhow::Result<()> {
        let tip = self.btc.tip_height().await?;
        let (since, rate) = *st.waiting.entry(*txid).or_insert((tip, 0.0));
        if tip < since + REPLACE_AFTER {
            return Ok(());
        }
        let floor = if rate > 0.0 { rate } else { self.btc.fee_rate(1).await? };
        // A replacement must pay for what it evicts too (BIP125 rule 3): the
        // waiting transactions of any network may build on this one.
        let others = self.wallet.waiting_rates_except(txid).await?;
        let min_rate = (floor * 1.5).max(floor + others + 1.0);
        let sent = match self.wallet.replace(txid, payload, min_rate, false).await {
            Ok(sent) => sent,
            Err(e) => {
                // The next try pays more, even if this one was refused.
                st.waiting.insert(*txid, (since, min_rate));
                return Err(e);
            }
        };
        st.waiting.remove(txid);
        st.waiting.insert(sent.txid, (tip, sent.rate));
        info!(old = %hex::encode(txid), new = %hex::encode(sent.txid), rate = sent.rate, "replaced a vault message to pay more");
        Ok(())
    }

    /// Reads the claims opened since the last round on both networks, and
    /// the state of those still open, into the index of what they carry.
    async fn index_claims(&self, st: &mut OpState) -> anyhow::Result<()> {
        for (i, side) in self.sides().into_iter().enumerate() {
            let idx = &mut st.claims[i];
            let count = side.vault.claim_count().await?;
            for id in (idx.cursor + 1)..=count {
                let c = side.vault.claim_status(id).await?;
                idx.status.insert(id, (c.operator, c.decided, c.accepted));
                idx.unread.push(id);
                idx.cursor = id;
            }
            let mut unread = vec![];
            for id in std::mem::take(&mut idx.unread) {
                match side.vault.claim(id).await {
                    Ok(c) => {
                        for r in c.records {
                            idx.carrying.entry(r).or_default().push(id);
                        }
                    }
                    Err(e) => {
                        warn!(network = side.net.name(), claim = id, error = %crate::secrets::redact(&e), "reading a claim's records failed");
                        unread.push(id);
                    }
                }
            }
            idx.unread = unread;
            let open: Vec<u64> = idx.status.iter().filter(|(_, (_, d, _))| !d).map(|(id, _)| *id).collect();
            for id in open {
                let c = side.vault.claim_status(id).await?;
                idx.status.insert(id, (c.operator, c.decided, c.accepted));
            }
        }
        Ok(())
    }

    /// Gathers what is worth carrying now: within the limits of section
    /// 11.3, of each chain's counted bond, and only toward a network where
    /// the chain can open a claim.
    async fn gather(&self, st: &mut OpState, chains: &[Chain; 2]) -> anyhow::Result<Vec<Record>> {
        let mut records = vec![];
        let (eth, sol) = (&chains[0], &chains[1]);
        let to_sol = can_open(&self.sol, sol).await?;
        let to_eth = can_open(&self.eth, eth).await?;

        // BOND records, until the other network counts the bond. The same
        // amount again is true: it is carried again when its claim could
        // not open.
        if to_sol && !sol.peer_bond_carried && eth.bond >= GWEI {
            let amount = if eth.stated != 0 { eth.stated } else { eth.bond } / GWEI;
            records.push(Record::Bond { network: ETHEREUM, amount: amount as u64 });
        }
        if to_eth && !eth.peer_bond_carried && sol.bond > 0 {
            let amount = if sol.stated != 0 { sol.stated } else { sol.bond };
            records.push(Record::Bond { network: SOLANA, amount: amount as u64 });
        }

        // Locks: LOCK toward Solana, or CANCEL toward Ethereum once given up.
        // Each fits 80% of the bond counted where it acts.
        let lock_cover = (sol.peer_bond as u128 * COVER_BPS / 10_000) as u64;
        let mut lock_room = lock_cover.saturating_sub(sol.open_value);
        let home_cover = (eth.peer_bond as u128 * COVER_BPS / 10_000) as u64;
        let mut home_room = home_cover.saturating_sub(eth.open_value);
        let (mut locks, mut home) = (0, 0);
        for l in self.eth.vault.final_locks_after(st.lock_cursor, WINDOW as usize).await? {
            st.lock_cursor = l.id;
            st.open_locks.insert(l.id);
        }
        for id in st.open_locks.clone().into_iter().take(WINDOW as usize) {
            let Some(l) = self.eth.vault.lock(id).await? else { continue };
            let r = l.record();
            let accepted = |r: &Record| st.claims[1].carrying.get(r).is_some_and(|ids| ids.iter().any(|c| st.claims[1].status.get(c).is_some_and(|(_, _, a)| *a)));
            // A receipt an attester issued at once still waits for a claim
            // carrying the lock (section 11.7).
            let issued = self.sol.vault.receipt_issued(l.id).await? && self.sol.vault.lock_attest(l.id).await?.is_none();
            if l.returned || accepted(&r) || issued {
                // Finished, or handed to its recipient to issue.
                st.open_locks.remove(&id);
                continue;
            }
            if self.sol.vault.given_up(l.id).await? {
                let c = Record::Cancel { id: l.id };
                let value = value_of(&r);
                if to_eth && home < MAX_HOME_RECORDS && value <= home_room && !st.claims[0].handled(&c) {
                    home_room -= value;
                    home += 1;
                    records.push(c);
                }
                continue;
            }
            // A fee another operator already earned is carried again, for no
            // fee, when no claim is handling the lock: its user must not be
            // stranded. A duplicate true LOCK is never slashed.
            // An attest of this node waits for the claim: carried whatever
            // its fee.
            let attested = st.attests.values().any(|a| *a == r);
            let worth = l.fee_paid || l.fee >= self.settings.min_fee_gwei || attested;
            let value = value_of(&r);
            // A lock this node attested waits for a claim of its own: one of
            // another operator's may be refused after the 7 days (D123).
            let me = self.sol.vault.me();
            let handled = if attested { st.claims[1].handled_by(&r, &me) } else { st.claims[1].handled(&r) };
            if to_sol && worth && locks < MAX_LOCKS && value <= lock_room && !handled {
                lock_room -= value;
                locks += 1;
                records.push(r);
            }
        }

        // Burns: REQUEST toward Ethereum.
        for r in self.sol.vault.requests_after(st.request_cursor, WINDOW as usize).await? {
            st.request_cursor = r.id;
            st.open_requests.insert(r.id);
        }
        for id in st.open_requests.clone().into_iter().take(WINDOW as usize) {
            let Some(r) = self.sol.vault.request(id).await? else { continue };
            let rec = r.record();
            let accepted = st.claims[0].carrying.get(&rec).is_some_and(|ids| ids.iter().any(|c| st.claims[0].status.get(c).is_some_and(|(_, _, a)| *a)));
            if accepted || self.eth.vault.request_paid(r.id).await? {
                st.open_requests.remove(&id);
                continue;
            }
            // A burn this node paid at once is carried whatever its fee: the
            // claim repays it.
            let worth = r.fee_paid || r.fee >= self.settings.min_fee_gwei || st.fast_paid.contains_key(&r.id);
            let value = value_of(&rec);
            if to_eth && worth && home < MAX_HOME_RECORDS && value <= home_room && !st.claims[0].handled(&rec) {
                home_room -= value;
                home += 1;
                records.push(rec);
            }
        }
        anyhow::ensure!(encode(&records).len() <= MAX_BATCH, "a batch within the record limits is at most 2,030 bytes");
        Ok(records)
    }

    /// The fast paths (section 11.7): attests final locks and pays final
    /// burns whose fast fee is worth it, links each of its attests to a
    /// claim carrying its record, settles them, and takes the repayments.
    /// Each on its own: one that fails is only logged.
    async fn fast(&self, st: &mut OpState, sol_chain: &Chain) {
        let outcome: anyhow::Result<()> = async {
            // Its own attests, also after a restart.
            let me = self.sol.vault.me();
            let count = self.sol.vault.attest_count().await?;
            for n in (st.attest_cursor + 1)..=count {
                if let Some(f) = self.sol.vault.fast_lock(n).await? {
                    if f.attester == me {
                        st.attests.insert(n, f.record);
                    }
                }
                st.attest_cursor = n;
            }
            Ok(())
        }
        .await;
        if let Err(e) = outcome {
            warn!(error = %crate::secrets::redact(&e), "reading attests failed");
        }
        for n in st.attests.keys().copied().collect::<Vec<_>>() {
            if let Err(e) = self.tend_attest(st, n).await {
                warn!(attest = n, error = %crate::secrets::redact(&e), "tending an attest failed");
            }
        }
        for id in st.fast_paid.keys().copied().collect::<Vec<_>>() {
            let r = st.fast_paid[&id].clone();
            let outcome: anyhow::Result<()> = async {
                if self.eth.vault.request_paid(id).await? {
                    st.fast_paid.remove(&id);
                    return Ok(());
                }
                let rec = r.record();
                let accepted = st.claims[0].carrying.get(&rec).and_then(|ids| ids.iter().find(|c| st.claims[0].status.get(c).is_some_and(|(_, _, a)| *a)).copied());
                if let Some(claim) = accepted {
                    self.eth.vault.pay_request(claim, id).await?;
                    st.fast_paid.remove(&id);
                    info!(request = id, claim, "the vault repaid a burn paid at once");
                }
                Ok(())
            }
            .await;
            if let Err(e) = outcome {
                warn!(request = id, error = %crate::secrets::redact(&e), "taking a fast repayment failed");
            }
        }
        let Some(fast) = self.settings.fast.clone() else { return };
        if sol_chain.refused || sol_chain.slashed || sol_chain.exited {
            return;
        }
        // Attests: the best-paid first (D122).
        let mut locks = vec![];
        let accepted = |idx: &ClaimIndex, r: &Record| idx.carrying.get(r).is_some_and(|ids| ids.iter().any(|c| idx.status.get(c).is_some_and(|(_, d, a)| *d && *a)));
        for id in st.open_locks.iter().copied().take(WINDOW as usize) {
            match self.eth.vault.lock(id).await {
                // Speed is what the fast fee pays for: none once a claim
                // carrying the lock was accepted.
                // Once only: another attest of its own would count as extra
                // and lose its vETH (D126).
                Ok(Some(l))
                    if l.fast_fee >= fast.min_fee_gwei
                        && l.amount <= fast.max_gwei
                        && !l.returned
                        && !accepted(&st.claims[1], &l.record())
                        && !st.attests.values().any(|a| matches!(a, Record::Lock { id, .. } if *id == l.id)) =>
                {
                    locks.push(l)
                }
                Ok(_) => {}
                Err(e) => warn!(lock = id, error = %crate::secrets::redact(&e), "reading a lock failed"),
            }
        }
        locks.sort_by(|a, b| b.fast_fee.cmp(&a.fast_fee));
        let now = match self.sol.net.now().await {
            Ok(t) => t,
            Err(e) => {
                warn!(error = %crate::secrets::redact(&e), "reading Solana's time failed");
                return;
            }
        };
        for l in locks {
            let outcome: anyhow::Result<()> = async {
                if self.sol.vault.receipt_issued(l.id).await? || self.sol.vault.given_up(l.id).await? {
                    return Ok(());
                }
                if let Some((first_at, open)) = self.sol.vault.lock_attests(l.id).await? {
                    // The lock takes no more attests, or an earlier one states
                    // the true record: this one would count as extra (D126).
                    if now >= first_at + FAST_OPEN_WINDOW || open.iter().any(|f| f.record == l.record()) {
                        return Ok(());
                    }
                }
                // Its vETH credit first: the collateral of settled attests.
                if self.sol.vault.veth_credit().await? > 0 {
                    self.sol.vault.withdraw_credit().await?;
                }
                let n = self.sol.vault.attest_lock(&l).await?;
                st.attests.insert(n, l.record());
                info!(lock = l.id, attest = n, collateral = (l.amount as u128 * FAST_COLLATERAL_BPS).div_ceil(10_000) as u64, "issued a receipt at once");
                Ok(())
            }
            .await;
            if let Err(e) = outcome {
                warn!(lock = l.id, error = %crate::secrets::redact(&e), "attesting a lock failed");
            }
        }
        // Burns paid at once, the best-paid first. Its own from before a
        // restart first: repaid when carried, also once accepted.
        let me = self.eth.vault.me();
        for id in st.open_requests.iter().copied().take(WINDOW as usize) {
            if st.fast_paid.contains_key(&id) {
                continue;
            }
            let outcome: anyhow::Result<()> = async {
                if let Some(r) = self.sol.vault.request(id).await? {
                    if self.eth.vault.fast_paid_by(&r).await?.is_some_and(|by| by.eq_ignore_ascii_case(&me)) {
                        st.fast_paid.insert(id, r);
                    }
                }
                Ok(())
            }
            .await;
            if let Err(e) = outcome {
                warn!(request = id, error = %crate::secrets::redact(&e), "reading a burn failed");
            }
        }
        let mut burns = vec![];
        for id in st.open_requests.iter().copied().take(WINDOW as usize) {
            match self.sol.vault.request(id).await {
                Ok(Some(r)) if r.fast_fee >= fast.min_fee_gwei && r.amount <= fast.max_gwei && !st.fast_paid.contains_key(&id) && !accepted(&st.claims[0], &r.record()) => burns.push(r),
                Ok(_) => {}
                Err(e) => warn!(request = id, error = %crate::secrets::redact(&e), "reading a burn failed"),
            }
        }
        burns.sort_by(|a, b| b.fast_fee.cmp(&a.fast_fee));
        for r in burns {
            let id = r.id;
            let outcome: anyhow::Result<()> = async {
                if self.eth.vault.request_paid(r.id).await? {
                    return Ok(());
                }
                if let Some(by) = self.eth.vault.fast_paid_by(&r).await? {
                    // Its own, from before a restart: repaid when carried.
                    if by.eq_ignore_ascii_case(&self.eth.vault.me()) {
                        st.fast_paid.insert(r.id, r);
                    }
                    return Ok(());
                }
                self.eth.vault.fast_pay(&r).await?;
                info!(request = r.id, amount = r.amount, "paid a burn at once");
                st.fast_paid.insert(r.id, r);
                Ok(())
            }
            .await;
            if let Err(e) = outcome {
                warn!(request = id, error = %crate::secrets::redact(&e), "paying a burn at once failed");
            }
        }
    }

    /// Links one of this node's attests to a claim carrying its record, and
    /// settles it once such a claim is accepted.
    async fn tend_attest(&self, st: &mut OpState, n: u64) -> anyhow::Result<()> {
        let Some(f) = self.sol.vault.fast_lock(n).await? else {
            st.attests.remove(&n);
            return Ok(());
        };
        let idx = &st.claims[1];
        let carrying: Vec<u64> = idx.carrying.get(&f.record).cloned().unwrap_or_default();
        if let Some(claim) = carrying.iter().find(|c| idx.status.get(c).is_some_and(|(_, _, a)| *a)) {
            settle_in_order(self.sol.vault.as_ref(), *claim, n).await?;
            st.attests.remove(&n);
            info!(attest = n, claim, "an attest was settled: its vETH back with the fast fee");
            return Ok(());
        }
        if f.burned {
            return Ok(());
        }
        let linked_refused = f.claim != 0 && idx.status.get(&f.claim).is_some_and(|(_, d, a)| *d && !*a);
        if f.claim == 0 || linked_refused {
            // Only a claim of its own: another operator could let its claim
            // be refused and burn the attest before a relink (D123).
            let me = self.sol.vault.me();
            let open = carrying.iter().find(|c| idx.status.get(c).is_some_and(|(op, d, _)| !*d && *op == me));
            if let Some(claim) = open {
                self.sol.vault.link_fast(*claim, n, (f.claim != 0).then_some(f.claim)).await?;
                info!(attest = n, claim, "linked an attest to its claim");
            }
        }
        Ok(())
    }

    /// Answers objections to this node's claims, and decides and collects.
    /// Each claim on its own: one that fails does not keep the others from
    /// being answered.
    async fn tend_claims(&self, st: &mut OpState) -> anyhow::Result<()> {
        for (i, side) in self.sides().into_iter().enumerate() {
            let me = side.vault.me();
            let now = side.net.now().await?;
            let mine: Vec<u64> = st.claims[i].status.iter().filter(|(_, (op, d, _))| *op == me && !d).map(|(id, _)| *id).collect();
            for id in mine {
                let outcome: anyhow::Result<()> = async {
                    let c = side.vault.claim_status(id).await?;
                    if c.decided {
                        side.vault.collect(id).await?;
                    } else if now >= c.last_at + OBJECTION_WINDOW {
                        side.vault.decide(id).await?;
                        side.vault.collect(id).await?;
                    } else if c.held {
                        // This node carries only what it checked: the
                        // objection is false.
                        side.vault.answer(id).await?;
                        info!(network = side.net.name(), claim = id, "answered an objection to its claim");
                    }
                    Ok(())
                }
                .await;
                if let Err(e) = outcome {
                    warn!(network = side.net.name(), claim = id, error = %crate::secrets::redact(&e), "tending a claim failed");
                }
            }
        }
        Ok(())
    }
}

#[async_trait]
impl Worker for VaultOperator {
    fn role(&self) -> Role {
        Role::Operator
    }

    async fn round(&self, _network: &dyn ProtocolNetwork) -> anyhow::Result<()> {
        let mut st = self.state.lock().await;
        if !st.loaded {
            st.journal = self.journal.read()?;
            st.loaded = true;
        }
        let chains = [self.eth.vault.chain(&self.eth.vault.me()).await?, self.sol.vault.chain(&self.sol.vault.me()).await?];
        if !self.register(&mut st, &chains).await? {
            return Ok(());
        }
        let chains = [chains[0].clone().unwrap(), chains[1].clone().unwrap()];
        // Objections first: nothing else in the round may keep them from
        // being answered within their 7 days.
        self.index_claims(&mut st).await?;
        self.tend_claims(&mut st).await?;
        if chains.iter().any(|c| c.exited) {
            return Ok(());
        }
        self.top_up(&chains).await;
        self.fast(&mut st, &chains[1]).await;
        if !self.catch_up(&mut st, &chains).await? {
            return Ok(());
        }
        // Both vaults are at the end of the chain, and no message waits.
        let chains = [self.eth.vault.chain(&self.eth.vault.me()).await?.unwrap(), self.sol.vault.chain(&self.sol.vault.me()).await?.unwrap()];
        anyhow::ensure!(chains[0].coin == chains[1].coin, "the two vaults follow different coins of the pair chain");
        // A message sent that the explorer does not show spending the coin
        // yet: a new one now would spend the same coin.
        for txid in st.waiting.keys().copied().collect::<Vec<_>>() {
            match self.btc.tx_status(&txid).await? {
                Some(TxStatus::Unconfirmed) => return Ok(()),
                _ => {
                    st.waiting.remove(&txid);
                }
            }
        }
        let records = self.gather(&mut st, &chains).await?;
        if records.is_empty() {
            return Ok(());
        }
        if !self.wallet.can_pay().await? {
            warn!("the Bitcoin wallet cannot pay for a vault message");
            return Ok(());
        }
        let batch = encode(&records);
        let payload = message_payload(&batch);
        // The batch is kept before the message exists: no crash can lose it.
        self.journal.append(&format!("batch {}", hex::encode(&batch)))?;
        st.journal.batches.insert(payload, batch);
        let coin = chains[0].coin;
        let sent = self.wallet.send_within(&[head_spend(coin.0, coin.1)], &payload, MAX_RAW_TX).await?;
        let tip = self.btc.tip_height().await?;
        st.waiting.insert(sent.txid, (tip, sent.rate));
        info!(records = records.len(), txid = %hex::encode(sent.txid), "wrote a vault batch on Bitcoin");
        Ok(())
    }
}

// ----------------------------------------------------------------------
// The guardian
// ----------------------------------------------------------------------

#[derive(Default)]
struct GuardState {
    claim_cursor: [u64; 2],
    open_claims: [HashSet<u64>; 2],
    /// The operators seen in claims, as (Ethereum address, Solana address).
    chains: HashSet<(String, String)>,
    /// Claims whose operator was learned, by (network, claim).
    learned: HashSet<(usize, u64)>,
    /// Attests of locks read so far, and those not settled or burned.
    attest_cursor: u64,
    open_attests: HashSet<u64>,
    /// Locks carried by an accepted claim on Solana: the claim and record.
    accepted_locks: HashMap<u64, (u64, Record)>,
    reals: [Reals; 2],
}

pub struct VaultGuardian {
    eth: Side,
    sol: Side,
    btc: Arc<dyn BitcoinView>,
    /// What to pay to open a checkpoint job when a lie waits for a real
    /// block above it, by network (Ethereum, Solana): the lie's slash pays
    /// 20% of the bond, well more than the job.
    checkpoint_paid: [Option<Amount>; 2],
    state: Mutex<GuardState>,
}

/// Messages brought to a vault per chain and round at most.
const BRING_PER_ROUND: usize = 5;
/// Lagging messages of a chain looked at for a lie per round.
const LOOK_AHEAD: u64 = 20;

impl VaultGuardian {
    pub fn new(eth: Side, sol: Side, btc: Arc<dyn BitcoinView>, checkpoint_paid: [Option<Amount>; 2]) -> Self {
        VaultGuardian { eth, sol, btc, checkpoint_paid, state: Mutex::new(GuardState::default()) }
    }

    /// Learns the chain of a claim's operator: its address on both
    /// networks.
    async fn learn(&self, st: &mut GuardState, acting: u8, operator: &str) -> anyhow::Result<()> {
        let side = if acting == ETHEREUM { &self.eth } else { &self.sol };
        let Some(c) = side.vault.chain(operator).await? else { return Ok(()) };
        let peer = peer_address(&c.peer);
        // Ethereum addresses in one spelling, so a chain is held once.
        let pair = if acting == ETHEREUM { (operator.to_lowercase(), peer) } else { (peer, operator.to_string()) };
        st.chains.insert(pair);
        Ok(())
    }

    /// Whether a message is false on `home`, the network it would be
    /// brought to: a lie that network can prove and slash. The rules of the
    /// batch's shape are `shape_false`'s; here the records are also checked
    /// against what `home` holds. `stated` is the chain's stated bond there,
    /// as it will be once the messages before this one are processed.
    async fn false_on(&self, home: u8, raw_len: usize, batch: &[u8], stated: &mut u128, bond: u128) -> anyhow::Result<bool> {
        let Some(records) = ipow_protocol_core::vault::decode(batch) else { return Ok(true) };
        let Some(next) = shape_false(home, raw_len, batch.len(), &records, *stated, bond) else { return Ok(true) };
        for r in &records {
            let false_here = match (r, home) {
                (Record::Lock { id, .. }, ETHEREUM) => !self.eth.vault.lock(*id).await?.is_some_and(|l| l.record() == *r),
                (Record::Cancel { id }, ETHEREUM) => self.eth.vault.lock(*id).await?.is_none(),
                (Record::Request { id, .. }, SOLANA) => !self.sol.vault.request(*id).await?.is_some_and(|q| q.record() == *r),
                (Record::Cancel { id }, SOLANA) => !self.sol.vault.given_up(*id).await?,
                _ => false,
            };
            if false_here {
                return Ok(true);
            }
        }
        *stated = next;
        Ok(false)
    }

    /// Brings to a vault the messages of a chain that the other vault
    /// processed and it has not, when one of them is false there (D107,
    /// D108, D109): an operator that shows a lie to one network only cannot
    /// hide it from the other. The lie is proven where it is brought and
    /// slashed, 20% to this node. A chain whose next lagging messages are
    /// all true is left to its operator, who brings them itself. The batch
    /// comes from the vault that published it, and must match the hash the
    /// message carries on Bitcoin.
    async fn bring_hidden(&self, st: &mut GuardState) {
        for (eth_op, sol_op) in st.chains.clone() {
            if let Err(e) = self.bring_chain(st, &eth_op, &sol_op).await {
                warn!(ethereum = %eth_op, solana = %sol_op, error = %crate::secrets::redact(&e), "bringing a hidden message failed");
            }
        }
    }

    async fn bring_chain(&self, st: &mut GuardState, eth_op: &str, sol_op: &str) -> anyhow::Result<()> {
        let (Some(e), Some(s)) = (self.eth.vault.chain(eth_op).await?, self.sol.vault.chain(sol_op).await?) else { return Ok(()) };
        if e.messages == s.messages {
            return Ok(());
        }
        let (behind, ahead, behind_op, ahead_op, chain, lead, i, home) = if e.messages < s.messages {
            (&self.eth, &self.sol, eth_op, sol_op, e, s.messages, 0, ETHEREUM)
        } else {
            (&self.sol, &self.eth, sol_op, eth_op, s, e.messages, 1, SOLANA)
        };
        // Already slashed, or the chain ended here. A chain with no bond is
        // still brought to: the slash marks it, which settles the other
        // network's claims that count a bond it never had.
        if chain.slashed || chain.exited {
            return Ok(());
        }
        // The lagging messages up to the first lie, followed on Bitcoin.
        let mut path = vec![];
        let mut coin = chain.coin;
        let mut stated = chain.stated;
        let mut lie = false;
        for index in chain.messages..lead.min(chain.messages + LOOK_AHEAD) {
            let Some(batch) = ahead.vault.message_batch(ahead_op, index).await? else { break };
            let Some(txid) = self.btc.spender(&coin.0, coin.1).await? else { break };
            let raw = self.btc.raw_tx(&txid).await?.ok_or_else(|| anyhow::anyhow!("a message on Bitcoin is not known"))?;
            let input = crate::wallet::input_spending(&raw, coin).ok_or_else(|| anyhow::anyhow!("a message does not spend its chain"))?;
            // The output the vault ahead took as the tag: the one with this
            // batch's hash, wherever the operator put it.
            let tag = crate::wallet::output_carrying(&raw, &message_payload(&batch))
                .ok_or_else(|| anyhow::anyhow!("a published batch does not match its message on Bitcoin"))?;
            lie = self.false_on(home, raw.len(), &batch, &mut stated, chain.bond).await?;
            path.push((index, batch, txid, input, tag));
            coin = (txid, input);
            if lie {
                break;
            }
        }
        if !lie {
            return Ok(());
        }
        for (index, batch, txid, input, tag) in path.into_iter().take(BRING_PER_ROUND) {
            let Some(block) = mined_in(self.btc.as_ref(), &txid).await? else { break };
            let Some(real) = reach(behind, self.btc.as_ref(), &mut st.reals[i], &block, self.checkpoint_paid[i]).await? else { break };
            fill_down(behind.net.as_ref(), self.btc.as_ref(), &real, &block).await?;
            let proof = tx_proof(self.btc.as_ref(), &txid, &block, real).await?;
            behind.vault.submit_message(behind_op, &proof, input, tag, &batch).await?;
            info!(network = behind.net.name(), operator = %behind_op, index, "brought a message the operator had not shown here");
        }
        Ok(())
    }

    /// Checks one claim: collects it once decided, decides it once its 7
    /// days are over, objects to it when a record is false.
    async fn tend_claim(&self, st: &mut GuardState, i: usize, id: u64, now: i64) -> anyhow::Result<()> {
        let (side, id_of) = if i == 0 { (&self.eth, ETHEREUM) } else { (&self.sol, SOLANA) };
        let c = side.vault.claim(id).await?;
        if st.learned.insert((i, id)) {
            if let Err(e) = self.learn(st, id_of, &c.operator).await {
                st.learned.remove(&(i, id));
                warn!(network = side.net.name(), claim = id, error = %crate::secrets::redact(&e), "reading a claim's chain failed");
            }
        }
        if c.decided {
            if c.accepted && id_of == SOLANA {
                for r in &c.records {
                    if let Record::Lock { id: lock, .. } = r {
                        st.accepted_locks.insert(*lock, (id, r.clone()));
                    }
                }
            }
            side.vault.collect(id).await?;
            st.open_claims[i].remove(&id);
            return Ok(());
        }
        if now >= c.last_at + OBJECTION_WINDOW {
            side.vault.decide(id).await?;
            return Ok(());
        }
        if c.held {
            return Ok(());
        }
        for r in &c.records {
            if self.is_true(id_of, &c.operator, r).await? == Some(false) {
                info!(network = side.net.name(), claim = id, record = ?r, "objecting to a false claim");
                side.vault.object(id).await?;
                break;
            }
        }
        Ok(())
    }

    /// The attests of locks on Solana (section 11.7, D123): burns one with
    /// no claim linked within 7 days, or whose linked claim was refused, and
    /// settles one whose lock an accepted claim states otherwise. Either
    /// pays this node the quarter above the amount.
    async fn tend_attests(&self, st: &mut GuardState) -> anyhow::Result<()> {
        let count = self.sol.vault.attest_count().await?;
        for n in (st.attest_cursor + 1)..=count {
            st.open_attests.insert(n);
        }
        st.attest_cursor = count;
        let now = self.sol.net.now().await?;
        // Oldest first: a lock's attests are settled in the order made.
        let mut open: Vec<u64> = st.open_attests.iter().copied().collect();
        open.sort_unstable();
        for n in open {
            let outcome: anyhow::Result<()> = async {
                let Some(f) = self.sol.vault.fast_lock(n).await? else {
                    st.open_attests.remove(&n);
                    return Ok(());
                };
                let Record::Lock { id: lock, .. } = f.record else { return Ok(()) };
                if let Some((claim, record)) = st.accepted_locks.get(&lock).cloned() {
                    // A wrong record: settling it burns the wrong receipt,
                    // or, once burned, issues the true one to its recipient.
                    // As stated: its attester settles it.
                    if record != f.record {
                        settle_in_order(self.sol.vault.as_ref(), claim, n).await?;
                        info!(attest = n, claim, "settled an attest that stated its lock wrongly");
                    }
                    st.open_attests.remove(&n);
                    return Ok(());
                }
                if f.burned {
                    return Ok(());
                }
                let burnable = if f.claim == 0 {
                    now >= f.attested_at + FAST_OPEN_WINDOW
                } else {
                    let c = self.sol.vault.claim_status(f.claim).await?;
                    c.decided && !c.accepted
                };
                if burnable {
                    self.sol.vault.burn_fast(n, (f.claim != 0).then_some(f.claim)).await?;
                    st.open_attests.remove(&n);
                    info!(attest = n, lock, "burned an attest with no claim to back it");
                }
                Ok(())
            }
            .await;
            if let Err(e) = outcome {
                warn!(attest = n, error = %crate::secrets::redact(&e), "tending an attest failed");
            }
        }
        Ok(())
    }

    async fn tend_claims(&self, st: &mut GuardState, i: usize) -> anyhow::Result<()> {
        let side = if i == 0 { &self.eth } else { &self.sol };
        let count = side.vault.claim_count().await?;
        for id in (st.claim_cursor[i] + 1)..=count {
            st.open_claims[i].insert(id);
        }
        st.claim_cursor[i] = count;
        let now = side.net.now().await?;
        for id in st.open_claims[i].clone() {
            // Tried again next round.
            if let Err(e) = self.tend_claim(st, i, id, now).await {
                warn!(network = side.net.name(), claim = id, error = %crate::secrets::redact(&e), "tending a claim failed");
            }
        }
        Ok(())
    }

    /// Whether a record acted on by network `acting` is true on the other
    /// network: `None` when that cannot be told yet.
    async fn is_true(&self, acting: u8, operator: &str, record: &Record) -> anyhow::Result<Option<bool>> {
        Ok(match record {
            Record::Lock { id, .. } => Some(self.eth.vault.lock(*id).await?.is_some_and(|l| l.record() == *record)),
            Record::Request { id, .. } => Some(self.sol.vault.request(*id).await?.is_some_and(|r| r.record() == *record)),
            Record::Cancel { id } => Some(self.sol.vault.given_up(*id).await?),
            Record::Bond { network, amount } => {
                let (acting_side, home) = if acting == ETHEREUM { (&self.eth, &self.sol) } else { (&self.sol, &self.eth) };
                let Some(here) = acting_side.vault.chain(operator).await? else { return Ok(None) };
                let peer = peer_address(&here.peer);
                let Some(there) = home.vault.chain(&peer).await? else { return Ok(Some(false)) };
                let stated = if *network == ETHEREUM { there.stated / GWEI } else { there.stated };
                if stated == *amount as u128 {
                    Some(true)
                } else if there.slashed || there.messages >= here.messages {
                    // The home vault has processed as much of the chain as
                    // this one, and holds another bond.
                    Some(false)
                } else {
                    None
                }
            }
            Record::Exit => Some(true),
        })
    }
}

/// The rules of a message's shape that make it false on `home`, as both
/// vaults judge it (D109, D110, D119): a transaction or batch too large, too
/// many REQUEST and CANCEL records, a REQUEST to address zero, locks whose
/// amounts overflow, a BOND of an unknown network, or a home BOND that is
/// zero, repeated in the batch, or not the chain's bond. `None` when false;
/// otherwise the chain's stated bond after the message, in the home vault's
/// unit. What the records say about locks, requests and give-ups is
/// checked against the vault by the caller.
fn shape_false(home: u8, raw_len: usize, batch_len: usize, records: &[Record], stated: u128, bond: u128) -> Option<u128> {
    if raw_len > MAX_RAW_TX || batch_len > MAX_BATCH {
        return None;
    }
    let mut home_records = 0;
    let mut locked: u64 = 0;
    let mut next = None;
    for r in records {
        match r {
            Record::Request { .. } | Record::Cancel { .. } => {
                home_records += 1;
                if home_records > MAX_HOME_RECORDS {
                    return None;
                }
                if let Record::Request { to, .. } = r {
                    if home == ETHEREUM && *to == [0; 20] {
                        return None;
                    }
                }
            }
            Record::Lock { amount, fast_fee, .. } => {
                // Solana mints the receipt and the fast fee.
                if home == SOLANA {
                    locked = locked.checked_add(*amount)?.checked_add(*fast_fee)?;
                }
            }
            Record::Bond { network, amount } => {
                if *network != ETHEREUM && *network != SOLANA {
                    return None;
                }
                if *network == home {
                    let unit = if home == ETHEREUM { GWEI } else { 1 };
                    let amount = *amount as u128 * unit;
                    if next.is_some() || amount == 0 {
                        return None;
                    }
                    if if stated != 0 { amount != stated } else { amount > bond } {
                        return None;
                    }
                    next = Some(amount);
                }
            }
            Record::Exit => {}
        }
    }
    Some(next.unwrap_or(stated))
}

/// Settles attest `n` with accepted claim `claim`, after the attests of the
/// same lock made before it that are not settled yet, oldest first: they
/// are settled in the order made (D126).
async fn settle_in_order(vault: &dyn VaultApp, claim: u64, n: u64) -> anyhow::Result<()> {
    let mut order = vec![n];
    let mut at = n;
    while let Some(f) = vault.fast_lock(at).await? {
        if f.prev == 0 || vault.fast_lock(f.prev).await?.is_none() {
            break;
        }
        order.push(f.prev);
        at = f.prev;
    }
    for m in order.into_iter().rev() {
        vault.settle_fast(claim, m).await.map_err(|e| e.context(format!("settling attest {m}, of those before {n}")))?;
    }
    Ok(())
}

/// An operator's address as its vault writes it: a Solana chain names its
/// operator on Ethereum in 20 bytes (`0x…`), an Ethereum chain its operator
/// on Solana in 32 (base58).
fn peer_address(bytes: &[u8]) -> String {
    if bytes.len() == 20 {
        format!("0x{}", hex::encode(bytes))
    } else {
        bs58::encode(bytes).into_string()
    }
}

#[async_trait]
impl Worker for VaultGuardian {
    fn role(&self) -> Role {
        Role::Guardian
    }

    async fn round(&self, _network: &dyn ProtocolNetwork) -> anyhow::Result<()> {
        let mut st = self.state.lock().await;
        for i in 0..2 {
            if let Err(e) = self.tend_claims(&mut *st, i).await {
                let side = if i == 0 { &self.eth } else { &self.sol };
                warn!(network = side.net.name(), error = %crate::secrets::redact(&e), "reading claims failed");
            }
        }
        if let Err(e) = self.tend_attests(&mut *st).await {
            warn!(error = %crate::secrets::redact(&e), "reading attests failed");
        }
        self.bring_hidden(&mut *st).await;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_message_is_false_by_its_shape_as_the_vaults_judge_it() {
        let bond = |network, amount| Record::Bond { network, amount };
        let request = |to| Record::Request { id: 1, amount: 1, to, fee: 0, fast_fee: 0, at: 0 };
        let lock = |amount| Record::Lock { id: 1, amount, recipient: [0; 32], fee: 0, fast_fee: 0, at: 0 };
        let e = 5 * GWEI;
        // True: the chain's first stated bond, carried forward.
        assert_eq!(shape_false(ETHEREUM, 300, 10, &[bond(ETHEREUM, 5)], 0, e), Some(e));
        // The same BOND again is true; another one is not.
        assert_eq!(shape_false(ETHEREUM, 300, 10, &[bond(ETHEREUM, 5)], e, 9 * GWEI), Some(e));
        assert_eq!(shape_false(ETHEREUM, 300, 10, &[bond(ETHEREUM, 4)], e, 9 * GWEI), None);
        // More than the bond, zero, twice in a batch, or an unknown network.
        assert_eq!(shape_false(ETHEREUM, 300, 10, &[bond(ETHEREUM, 6)], 0, e), None);
        assert_eq!(shape_false(ETHEREUM, 300, 10, &[bond(ETHEREUM, 0)], 0, e), None);
        assert_eq!(shape_false(ETHEREUM, 300, 20, &[bond(ETHEREUM, 5), bond(ETHEREUM, 5)], 0, e), None);
        assert_eq!(shape_false(SOLANA, 300, 10, &[bond(9, 5)], 0, 5), None);
        // The other network's BOND is not judged here.
        assert_eq!(shape_false(SOLANA, 300, 10, &[bond(ETHEREUM, 999)], 0, 5), Some(0));
        // A REQUEST to address zero is false on Ethereum only.
        assert_eq!(shape_false(ETHEREUM, 300, 45, &[request([0; 20])], 0, e), None);
        assert_eq!(shape_false(SOLANA, 300, 45, &[request([0; 20])], 0, 5), Some(0));
        // Locks that overflow are false on Solana.
        assert_eq!(shape_false(SOLANA, 300, 114, &[lock(u64::MAX), lock(1)], 0, 5), None);
        assert_eq!(shape_false(ETHEREUM, 300, 114, &[lock(u64::MAX), lock(1)], 0, e), Some(0));
        // The fast fee is minted too.
        let fast = Record::Lock { id: 1, amount: u64::MAX, recipient: [0; 32], fee: 0, fast_fee: 1, at: 0 };
        assert_eq!(shape_false(SOLANA, 300, 65, &[fast], 0, 5), None);
        // D119: sizes and the count of home records.
        assert_eq!(shape_false(SOLANA, MAX_RAW_TX + 1, 10, &[], 0, 5), None);
        assert_eq!(shape_false(SOLANA, 300, MAX_BATCH + 1, &[], 0, 5), None);
        let many = vec![Record::Cancel { id: 1 }; MAX_HOME_RECORDS + 1];
        assert_eq!(shape_false(SOLANA, 300, 9 * many.len(), &many, 0, 5), None);
        assert_eq!(shape_false(SOLANA, 300, 9 * 32, &many[..32], 0, 5), Some(0));
    }

    #[test]
    fn a_journal_cut_short_by_a_crash_still_reads() {
        let dir = std::env::temp_dir().join(format!("ipow-journal-{}", std::process::id()));
        let _ = std::fs::remove_file(&dir);
        let j = Journal::new(dir.clone());
        let batch = encode(&[Record::Exit]);
        j.append(&format!("registration {}", hex::encode([7u8; 32]))).unwrap();
        j.append(&format!("batch {}", hex::encode(&batch))).unwrap();
        // A crash in the middle of the next line.
        use std::io::Write;
        std::fs::OpenOptions::new().append(true).open(&dir).unwrap().write_all(b"batch 0a0").unwrap();
        let read = j.read().unwrap();
        assert_eq!(read.registration, Some([7u8; 32]));
        // Found by the hash a message carries, whatever its txid.
        assert_eq!(read.batches.get(&message_payload(&batch)), Some(&batch));
        assert_eq!(read.batches.len(), 1);
        // The cut line was dropped from the file: the next batch appended
        // reads, after another restart too.
        let next = encode(&[Record::Cancel { id: 4 }]);
        j.append(&format!("batch {}", hex::encode(&next))).unwrap();
        let read = j.read().unwrap();
        assert_eq!(read.batches.get(&message_payload(&next)), Some(&next));
        assert_eq!(read.batches.len(), 2);
        let _ = std::fs::remove_file(&dir);
    }
}
