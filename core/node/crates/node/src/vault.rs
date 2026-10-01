//! The node's roles for the protocol's vault (spec, section 11), for the
//! pair Ethereum and Solana:
//!
//! - `VaultOperator` registers this node's pair chain, keeps its bonds in
//!   each asset and its deposit money, gathers records in both directions
//!   (locks, burns, give-ups, bonds and assets of each network, section
//!   11.9), writes them in batches on Bitcoin, submits each message to both
//!   vaults, answers objections to its claims, and takes the fast paths.
//! - `VaultGuardian` checks every claim on each network against the other
//!   network, objects to a false one, decides and collects, burns or
//!   settles attests, brings a message hidden from one network, and settles
//!   the bonds of a slashed chain.
//!
//! Both work on the two networks at once. The supervisor runs them as
//! ordinary workers; the network it hands them only names the task.

use std::collections::{BTreeSet, HashMap, HashSet};
use std::path::PathBuf;
use std::sync::Arc;

use async_trait::async_trait;
use ipow_bitcoin::view::{BitcoinView, TxStatus};
use ipow_protocol_core::network::ProtocolNetwork;
use ipow_protocol_core::settings::{FastSettings, Role};
use ipow_protocol_core::types::{Amount, BlockRef, JobStatus};
use ipow_protocol_core::vault::{
    decode, encode, message_payload, record_hash, AssetInfo, Chain, Position, Record, TxProof, VaultApp, MAX_ASSETS, MAX_BATCH,
    MAX_LOCKS_ON_SOLANA, MAX_RAW_TX, MAX_RECORDS, SOLANA,
};
#[cfg(test)]
use ipow_protocol_core::vault::{eth_address32, ETHEREUM};
use tokio::sync::Mutex;
use tracing::{debug, info, warn};

use crate::light::epoch_time_at;
use crate::supervisor::Worker;
use crate::wallet::{head_spend, merkle_proof, SharedWallet};

/// D110, D129: a chain's open claims in an asset are at most 80% of its
/// bond in it.
const COVER_BPS: u128 = 8000;
/// The light client walks at most 100 blocks.
const MAX_WALK: u32 = 100;
/// D111.
const OBJECTION_WINDOW: i64 = 7 * 24 * 3600;
/// How long to wait before opening another checkpoint job on a network.
const CHECKPOINT_EVERY: i64 = 2 * 3600;

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
            net.extend_back(&at, &header, prev_epoch_time)
                .await
                .map_err(|e| e.context(format!("storing block {} below {} (epoch time {}, previous epoch {})", height, at.height, at.epoch_time, prev_epoch_time)))?;
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
    /// The assets it carries, with its bonds, fees and fast paths in each
    /// (section 11.9). An asset not listed is never carried.
    pub assets: Vec<CarriedAsset>,
    /// Claim deposits to keep in each vault, as a count of deposits: at
    /// least 1.
    pub deposits: u32,
    /// What to pay to open a checkpoint job when no real block is above a
    /// message, on each network of the pair; `None` never opens one.
    pub checkpoint_paid: [Option<Amount>; 2],
    pub journal: PathBuf,
}

impl VaultOperatorSettings {
    /// The settings of asset (`home`, `asset`), if it is carried.
    fn asset(&self, home: u8, asset: u32) -> Option<&CarriedAsset> {
        self.assets.iter().find(|a| a.home == home && a.asset == asset)
    }
}

/// An asset the operator carries, its home by network number (D133), and
/// what it keeps and asks in it, in record units (settings: `VaultAsset`).
#[derive(Clone, Debug)]
pub struct CarriedAsset {
    pub home: u8,
    pub asset: u32,
    pub bond_home: u64,
    pub bond_receipt: u64,
    pub min_fee: u64,
    pub fast: Option<FastSettings>,
}

/// Whether two vaults name each other as their pair's other vault: a
/// vault's peer number alone does not tell a pair from another of the same
/// two networks (a redeployed vault, a wrong address in the settings).
pub async fn check_pair(a: &Side, b: &Side) -> anyhow::Result<()> {
    anyhow::ensure!(
        a.vault.peer_vault().await? == b.vault.vault_id() && b.vault.peer_vault().await? == a.vault.vault_id(),
        "the vaults of networks {} and {} do not name each other",
        a.vault.network_id(),
        b.vault.network_id()
    );
    Ok(())
}

/// The pair of two vaults: each must name the other's network as its peer.
fn pair_of(a: &Side, b: &Side) -> anyhow::Result<Nets> {
    let (x, y) = (a.vault.network_id(), b.vault.network_id());
    anyhow::ensure!(
        a.vault.peer_id() == y && b.vault.peer_id() == x,
        "the two vaults are not one pair: networks {x} and {y}, peers {} and {}",
        a.vault.peer_id(),
        b.vault.peer_id()
    );
    Ok(Nets([x, y]))
}

/// A pair's two networks, as a role keeps them: its sides in this order
/// (D132).
#[derive(Clone, Copy, Debug)]
struct Nets([u8; 2]);

impl Nets {
    /// Where a network's part sits in the pair.
    fn ix(&self, net: u8) -> usize {
        if net == self.0[0] { 0 } else { 1 }
    }
    /// The pair's other network.
    fn other(&self, net: u8) -> u8 {
        if net == self.0[0] { self.0[1] } else { self.0[0] }
    }
}

/// Section 11.7: an attester locks 1.25 times a lock's amount, and a claim
/// carrying its record must open within 7 days (D123).
const FAST_COLLATERAL_BPS: u128 = 12_500;
const FAST_OPEN_WINDOW: i64 = 7 * 24 * 3600;

/// What a record makes the acting network issue or pay: the amount and the
/// fast fee, in record units (section 11.5).
fn value_of(r: &Record) -> u64 {
    match r {
        Record::Lock { amount, fast_fee, .. } | Record::Request { amount, fast_fee, .. } => amount.saturating_add(*fast_fee),
        _ => 0,
    }
}

/// Blocks a message may wait unmined before it is replaced to pay more.
const REPLACE_AFTER: u32 = 3;
/// Locks and requests looked at per network and round.
const WINDOW: usize = 100;

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

    /// An accepted claim carrying `r`, if any.
    fn accepted(&self, r: &Record) -> Option<u64> {
        self.carrying.get(r)?.iter().copied().find(|id| self.status.get(id).is_some_and(|(_, decided, accepted)| *decided && *accepted))
    }
}

/// The chain's place in each asset on one network, read from the vault:
/// Ethereum lists only the assets a chain is bonded in.
type Positions = HashMap<(u8, u32), Position>;

/// Up to `WINDOW` of `set`, from `from` on and then from its start. A set
/// larger than the window is looked at in turns, so ids nobody ever
/// finishes (a lock whose fee is too low, which anyone can make) cannot hide
/// newer ones.
fn window(set: &BTreeSet<u64>, from: u64) -> Vec<u64> {
    set.range(from..).chain(set.range(..from)).copied().take(WINDOW).collect()
}

/// Where the next turn over `set` starts after looking at `seen`.
fn next_turn(set: &BTreeSet<u64>, seen: &[u64]) -> u64 {
    match seen.last() {
        Some(last) if set.len() > WINDOW => last + 1,
        _ => 0,
    }
}

/// What batches on their way to network `at`, from its message `messages`
/// on, will move there in each asset of network `home`: cover the vault
/// does not count yet.
fn in_flight_value(in_flight: &[(u64, Moved)], at: usize, home: u8, messages: u64) -> HashMap<u32, u64> {
    let mut out: HashMap<u32, u64> = HashMap::new();
    for (index, moved) in in_flight {
        if *index >= messages {
            for ((h, asset), value) in &moved[at] {
                if *h == home {
                    *out.entry(*asset).or_default() += value;
                }
            }
        }
    }
    out
}

/// Whether `value` more, after `pending` not counted yet, keeps a chain's
/// open claims in an asset within 80% of its bond there (D110, D129).
fn fits_cover(p: &Position, pending: u64, value: u64) -> bool {
    (p.open_value as u128 + pending as u128 + value as u128) * 10_000 <= p.peer_bond as u128 * COVER_BPS
}

/// A batch being gathered, within D119 and the limits of section 11.9.
struct Gathered<'a> {
    /// The chain on each network, as each vault sees it, and its places.
    chains: &'a [Chain; 2],
    positions: [Positions; 2],
    nets: Nets,
    records: Vec<Record>,
    len: usize,
    counted: usize,
    locks_on_solana: usize,
    /// What the batch moves on each acting network, by asset.
    moved: Moved,
}

/// What a batch moves on each acting network (Ethereum, Solana), by asset.
type Moved = [Vec<((u8, u32), u64)>; 2];

impl<'a> Gathered<'a> {
    fn new(chains: &'a [Chain; 2], positions: [Positions; 2], nets: Nets) -> Self {
        Gathered { chains, positions, nets, records: vec![], len: 0, counted: 0, locks_on_solana: 0, moved: [vec![], vec![]] }
    }

    fn position(&self, net: u8, k: (u8, u32)) -> Position {
        self.positions[self.nets.ix(net)].get(&k).cloned().unwrap_or(Position { home: k.0, asset: k.1, ..Default::default() })
    }

    /// Adds `r`, acted on by network `acting`, moving `value` there in asset
    /// `key`, when it fits: the batch's size, its 32 counted records, the 10
    /// LOCK records Solana acts on, a claim's 8 assets (on Solana with the
    /// chain's own), and 80% of the chain's bond in the asset as `acting`
    /// counts it. Returns whether it was added.
    fn add(&mut self, r: Record, acting: u8, key: Option<(u8, u32)>, value: u64) -> bool {
        let n = r.bytes().len();
        if self.len + n > MAX_BATCH || (r.counted() && self.counted >= MAX_RECORDS) {
            return false;
        }
        let lock_on_solana = acting == SOLANA && matches!(r, Record::Lock { .. });
        if lock_on_solana && self.locks_on_solana >= MAX_LOCKS_ON_SOLANA {
            return false;
        }
        let i = self.nets.ix(acting);
        if let Some(k) = key {
            let chain = &self.chains[i];
            let moved = &self.moved[i];
            let before = moved.iter().find(|(m, _)| *m == k).map(|(_, v)| *v);
            if before.is_none() {
                if moved.len() >= MAX_ASSETS {
                    return false;
                }
                if acting == SOLANA {
                    let held = |m: &(u8, u32)| chain.positions.iter().any(|p| (p.home, p.asset) == *m);
                    let new = moved.iter().filter(|(m, _)| !held(m)).count() + usize::from(!held(&k));
                    if chain.positions.len() + new > MAX_ASSETS {
                        return false;
                    }
                }
            }
            if value > 0 && !fits_cover(&self.position(acting, k), before.unwrap_or(0), value) {
                return false;
            }
            match self.moved[i].iter_mut().find(|(m, _)| *m == k) {
                Some((_, v)) => *v += value,
                None => self.moved[i].push((k, value)),
            }
        }
        self.len += n;
        if r.counted() {
            self.counted += 1;
        }
        if lock_on_solana {
            self.locks_on_solana += 1;
        }
        self.records.push(r);
        true
    }
}

#[derive(Default)]
struct OpState {
    loaded: bool,
    journal: JournalState,
    /// By home network: the highest lock id read so far, and those among
    /// them of a carried asset not finished yet: returned, issued, or carried
    /// by an accepted claim. One that never finishes is looked at again each
    /// round, and never hides newer ones.
    lock_cursor: [u64; 2],
    open_locks: [BTreeSet<u64>; 2],
    /// Where the next look at each open set starts (`window`).
    lock_turn: [u64; 2],
    request_turn: [u64; 2],
    /// By network of the burn: the same for burns.
    request_cursor: [u64; 2],
    open_requests: [BTreeSet<u64>; 2],
    /// By acting network.
    claims: [ClaimIndex; 2],
    reals: [Reals; 2],
    /// Messages sent and not mined yet: the Bitcoin height when first seen
    /// waiting, and the fee rate paid.
    waiting: HashMap<[u8; 32], (u32, f64)>,
    /// By network of the attests: those read so far, and this node's among
    /// them not settled yet, by number, with the lock each is of.
    attest_cursor: [u64; 2],
    attests: [HashMap<u64, u64>; 2],
    /// By paying network: burns this node paid at once, until the vault
    /// repays it.
    fast_paid: [HashSet<Record>; 2],
    /// Batches this node wrote that a vault may not have processed yet, by
    /// their message number on the chain, with what each moves: their claims
    /// take cover the vaults do not count yet.
    in_flight: Vec<(u64, Moved)>,
    /// The ASSET records of carried assets, by (home, asset): they never
    /// change once registered.
    asset_records: HashMap<(u8, u32), Record>,
}

pub struct VaultOperator {
    /// The pair's two sides (D132).
    a: Side,
    b: Side,
    nets: Nets,
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

/// Withdraws this node's credits on a network: in its coin (deposits won)
/// and in each of `assets` (fees earned, burns paid at once repaid, the
/// collateral of settled attests, a slasher's share).
async fn withdraw_credits(side: &Side, assets: &[(u8, u32)]) {
    let net = side.vault.network_id();
    let mut keys = vec![(net, 0)];
    keys.extend_from_slice(assets);
    keys.sort_unstable();
    keys.dedup();
    for (home, asset) in keys {
        let outcome: anyhow::Result<()> = async {
            if side.vault.credit(home, asset).await? > 0 {
                side.vault.withdraw_credit(home, asset).await?;
            }
            Ok(())
        }
        .await;
        if let Err(e) = outcome {
            warn!(network = side.net.name(), home, asset, error = %crate::secrets::redact(&e), "withdrawing a credit failed");
        }
    }
}

impl VaultOperator {
    /// The operator of the pair whose sides are `a` and `b`: two vaults of
    /// one pair (D132), each knowing its network and the other's.
    pub fn new(a: Side, b: Side, btc: Arc<dyn BitcoinView>, wallet: Arc<SharedWallet>, settings: VaultOperatorSettings) -> anyhow::Result<Self> {
        let nets = pair_of(&a, &b)?;
        let journal = Journal::new(settings.journal.clone());
        Ok(VaultOperator { a, b, nets, btc, wallet, settings, journal, state: Mutex::new(OpState::default()) })
    }

    fn ix(&self, net: u8) -> usize {
        self.nets.ix(net)
    }

    fn other(&self, net: u8) -> u8 {
        self.nets.other(net)
    }

    /// Forgets all it learned, as after a restart: the next round reads its
    /// journal and both vaults again.
    pub async fn restart(&self) {
        *self.state.lock().await = OpState::default();
    }

    fn sides(&self) -> [&Side; 2] {
        [&self.a, &self.b]
    }

    fn side(&self, net: u8) -> &Side {
        if net == self.nets.0[0] { &self.a } else { &self.b }
    }

    /// Registers the pair chain on each network that does not know it yet.
    /// Returns whether both do.
    async fn register(&self, st: &mut OpState, chains: &[Option<Chain>; 2]) -> anyhow::Result<bool> {
        if chains.iter().all(Option::is_some) {
            return Ok(true);
        }
        let eth_commitment = self.a.vault.pair_commitment(&self.b.vault.me_bytes()).await?;
        let sol_commitment = self.b.vault.pair_commitment(&self.a.vault.me_bytes()).await?;
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
            let other = if i == 0 { &self.b } else { &self.a };
            let proof = tx_proof(self.btc.as_ref(), &txid, &block, real).await?;
            side.vault.register_chain(&other.vault.me_bytes(), &proof, 0, 1).await?;
            info!(network = side.net.name(), "registered the vault's pair chain");
        }
        Ok(false)
    }

    /// Keeps the bonds and the deposit money at what the settings ask, and
    /// withdraws its credits: deposits won, fees, repayments. A failure is only logged: it must not
    /// keep the node from answering objections.
    async fn top_up(&self, chains: &[Chain; 2]) {
        let mut asks: Vec<(&Side, &str, anyhow::Result<()>)> = vec![];
        for s in &self.settings.assets {
            let home = s.home;
            // In the asset on its home, in its receipt on the other network.
            for (net, want) in [(home, s.bond_home), (self.other(home), s.bond_receipt)] {
                let c = &chains[self.ix(net)];
                let have = c.position(home, s.asset).bond;
                if want > have && !c.slashed {
                    let side = self.side(net);
                    // A receipt bond waits until this node holds the
                    // receipts: from a lock of its own, the slow way.
                    let outcome = async {
                        if net != home && side.vault.receipt_balance(s.asset).await? < want - have {
                            return Ok(());
                        }
                        side.vault.add_bond(home, s.asset, want - have).await
                    }
                    .await;
                    asks.push((side, "bond", outcome));
                }
            }
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
            let assets: Vec<(u8, u32)> = self.settings.assets.iter().map(|a| (a.home, a.asset)).collect();
            withdraw_credits(side, &assets).await;
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
                        return Err(e.context(format!("submitting message {} to {}", chains[i].messages, side.net.name())));
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

    /// The ASSET record of carried asset (`home`, `asset`), if its home vault
    /// registered it.
    async fn asset_record(&self, st: &mut OpState, home: u8, asset: u32) -> anyhow::Result<Option<Record>> {
        if let Some(r) = st.asset_records.get(&(home, asset)) {
            return Ok(Some(r.clone()));
        }
        for a in self.side(home).vault.assets().await? {
            if self.settings.asset(home, a.number).is_some() {
                st.asset_records.insert((home, a.number), a.record(home));
            }
        }
        Ok(st.asset_records.get(&(home, asset)).cloned())
    }

    /// Makes the receipt of each carried asset whose ASSET record an
    /// accepted claim carries: until then no lock of it can be issued.
    async fn make_receipts(&self, st: &mut OpState) {
        for s in self.settings.assets.clone() {
            let (home, acting) = (s.home, self.other(s.home));
            let outcome: anyhow::Result<()> = async {
                let Some(r) = self.asset_record(st, home, s.asset).await? else { return Ok(()) };
                let Some(claim) = st.claims[self.ix(acting)].accepted(&r) else { return Ok(()) };
                let there = self.side(acting);
                if !there.vault.has_receipt(s.asset).await? {
                    there.vault.make_receipt(claim, &r).await?;
                    info!(network = there.net.name(), asset = s.asset, claim, "made the receipt of an asset");
                }
                Ok(())
            }
            .await;
            if let Err(e) = outcome {
                warn!(asset = s.asset, error = %crate::secrets::redact(&e), "making a receipt failed");
            }
        }
    }

    /// Reads the locks and burns of carried assets made since the last
    /// round, once final, into those open. Every round, also while a message
    /// is on its way: the fast paths act on them at once.
    async fn discover(&self, st: &mut OpState) -> anyhow::Result<()> {
        for net in self.nets.0 {
            let (i, side) = (self.ix(net), self.side(net));
            for l in side.vault.final_locks_after(st.lock_cursor[i], WINDOW).await? {
                st.lock_cursor[i] = l.id;
                if self.settings.asset(net, l.asset).is_some() {
                    st.open_locks[i].insert(l.id);
                }
            }
            // A burn here is of a receipt of the other network's asset.
            for q in side.vault.requests_after(st.request_cursor[i], WINDOW).await? {
                st.request_cursor[i] = q.id;
                if self.settings.asset(self.other(net), q.asset).is_some() {
                    st.open_requests[i].insert(q.id);
                }
            }
        }
        Ok(())
    }

    /// Gathers what is worth carrying now, in both directions: within the
    /// limits of section 11.9, of each chain's counted bond in each asset,
    /// and only toward a network where the chain can open a claim.
    async fn gather(&self, st: &mut OpState, chains: &[Chain; 2]) -> anyhow::Result<(Vec<Record>, Moved)> {
        let open = [can_open(&self.a, &chains[0]).await?, can_open(&self.b, &chains[1]).await?];
        // Its place in every asset it carries or is bonded in, on both.
        let mut keys: Vec<(u8, u32)> = self.settings.assets.iter().map(|a| (a.home, a.asset)).collect();
        keys.extend(chains.iter().flat_map(|c| c.positions.iter().map(|p| (p.home, p.asset))));
        keys.sort_unstable();
        keys.dedup();
        let mut positions: [Positions; 2] = Default::default();
        for (i, side) in self.sides().into_iter().enumerate() {
            let me = side.vault.me();
            for &k in &keys {
                positions[i].insert(k, side.vault.position(&me, k.0, k.1).await?);
            }
        }
        let mut g = Gathered::new(chains, positions, self.nets);

        // BOND records, until the other network counts the bond. The same
        // amount again is true: it is carried again when its claim could
        // not open.
        for fact in self.nets.0 {
            let acting = self.other(fact);
            if !open[self.ix(acting)] {
                continue;
            }
            for p in &chains[self.ix(fact)].positions {
                if p.bond == 0 || g.position(acting, (p.home, p.asset)).peer_bond_carried {
                    continue;
                }
                let amount = if p.stated != 0 { p.stated } else { p.bond };
                g.add(Record::Bond { net: fact, home: p.home, asset: p.asset, amount }, acting, Some((p.home, p.asset)), 0);
            }
        }

        // ASSET records: the receipt of each carried asset, once.
        for s in self.settings.assets.clone() {
            let (home, acting) = (s.home, self.other(s.home));
            if !open[self.ix(acting)] {
                continue;
            }
            let Some(r) = self.asset_record(st, home, s.asset).await? else { continue };
            if st.claims[self.ix(acting)].handled(&r) || self.side(acting).vault.has_receipt(s.asset).await? {
                continue;
            }
            g.add(r, acting, None, 0);
        }

        // Locks: LOCK toward the other network, or CANCEL home once given up
        // there. Each fits 80% of the bond in its asset where it acts.
        for home in self.nets.0 {
            let acting = self.other(home);
            let (h, a) = (self.ix(home), self.ix(acting));
            let (hs, ts) = (self.side(home), self.side(acting));
            let me = ts.vault.me();
            // Its own attested locks first: each needs a claim within 7 days
            // (D123), ahead of other locks for the same cover.
            let mine: BTreeSet<u64> = st.attests[a].values().copied().filter(|id| st.open_locks[h].contains(id)).collect();
            let rest: Vec<u64> = window(&st.open_locks[h], st.lock_turn[h]).into_iter().filter(|id| !mine.contains(id)).collect();
            st.lock_turn[h] = next_turn(&st.open_locks[h], &rest);
            let ids: Vec<u64> = mine.iter().copied().chain(rest).take(WINDOW).collect();
            for id in ids {
                let Some(l) = hs.vault.lock(id).await? else { continue };
                let Some(s) = self.settings.asset(home, l.asset) else {
                    st.open_locks[h].remove(&id);
                    continue;
                };
                let r = l.record(home);
                // Its fee, if this node's message earned it (on Solana it is
                // taken with a call of its own).
                if l.fee_paid && l.fee > 0 {
                    if let Err(e) = hs.vault.take_fee(&r).await {
                        warn!(network = hs.net.name(), lock = id, error = %crate::secrets::redact(&e), "taking a fee failed");
                    }
                }
                // A receipt an attester issued at once still waits for a claim
                // carrying the lock (section 11.7).
                let attested = ts.vault.lock_attests(id).await?.is_some_and(|(_, open)| !open.is_empty());
                let issued = !attested && ts.vault.receipt_issued(id).await?;
                if l.returned || st.claims[a].accepted(&r).is_some() || issued {
                    // Finished, or handed to its recipient to issue.
                    st.open_locks[h].remove(&id);
                    continue;
                }
                if ts.vault.given_up(id).await? {
                    let c = Record::Cancel { net: acting, id };
                    if open[h] && !st.claims[h].handled(&c) {
                        g.add(c, home, Some((home, l.asset)), value_of(&r));
                    }
                    continue;
                }
                // A fee another operator already earned is carried again, for
                // no fee, when no claim is handling the lock: its user must
                // not be stranded. A duplicate true LOCK is never slashed. An
                // attest of this node waits for the claim: carried whatever
                // its fee, and only in a claim of its own, since another
                // operator's may be refused after the 7 days (D123).
                let mine = st.attests[a].values().any(|lock| *lock == id);
                let worth = l.fee_paid || l.fee >= s.min_fee || mine;
                let handled = if mine { st.claims[a].handled_by(&r, &me) } else { st.claims[a].handled(&r) };
                if open[a] && worth && !handled {
                    let value = value_of(&r);
                    g.add(r, acting, Some((home, l.asset)), value);
                }
            }
        }

        // Burns: REQUEST toward the asset's home.
        for net in self.nets.0 {
            let home = self.other(net);
            let (n, a) = (self.ix(net), self.ix(home));
            let (bs, hs) = (self.side(net), self.side(home));
            let ids = window(&st.open_requests[n], st.request_turn[n]);
            st.request_turn[n] = next_turn(&st.open_requests[n], &ids);
            for id in ids {
                let Some(q) = bs.vault.request(id).await? else { continue };
                let Some(s) = self.settings.asset(home, q.asset) else {
                    st.open_requests[n].remove(&id);
                    continue;
                };
                let r = q.record(net);
                if q.fee_paid && q.fee > 0 {
                    if let Err(e) = bs.vault.take_fee(&r).await {
                        warn!(network = bs.net.name(), request = id, error = %crate::secrets::redact(&e), "taking a fee failed");
                    }
                }
                let accepted = st.claims[a].accepted(&r).is_some();
                let paid = hs.vault.request_paid(id).await?;
                if accepted || paid {
                    // One it paid at once, found after a restart: the claim
                    // repays it.
                    if accepted && !paid && !st.fast_paid[a].contains(&r) {
                        match hs.vault.fast_paid_by(&r).await {
                            Ok(by) => {
                                if by.is_some_and(|by| by.eq_ignore_ascii_case(&hs.vault.me())) {
                                    st.fast_paid[a].insert(r);
                                }
                            }
                            Err(e) => {
                                // Looked at again next round.
                                warn!(network = hs.net.name(), request = id, error = %crate::secrets::redact(&e), "reading who paid a burn failed");
                                continue;
                            }
                        }
                    }
                    st.open_requests[n].remove(&id);
                    continue;
                }
                // A burn this node paid at once is carried whatever its fee:
                // the claim repays it.
                let worth = q.fee_paid || q.fee >= s.min_fee || st.fast_paid[a].contains(&r);
                if open[a] && worth && !st.claims[a].handled(&r) {
                    let value = value_of(&r);
                    g.add(r, home, Some((home, q.asset)), value);
                }
            }
        }
        Ok((g.records, g.moved))
    }

    /// The fast paths (section 11.7), acted on each network: attests there
    /// of locks on the other network, and burns on the other network paid
    /// there at once. Each on its own: one that fails is only logged.
    async fn fast(&self, st: &mut OpState, chains: &[Chain; 2]) {
        for at in self.nets.0 {
            if let Err(e) = self.fast_on(st, at, &chains[self.ix(at)]).await {
                warn!(network = self.side(at).net.name(), error = %crate::secrets::redact(&e), "the fast paths failed");
            }
        }
    }

    async fn fast_on(&self, st: &mut OpState, at: u8, chain: &Chain) -> anyhow::Result<()> {
        // The home of the locks attested here, and the network of the burns
        // paid here.
        let home = self.other(at);
        let (i, h) = (self.ix(at), self.ix(home));
        let (here, there) = (self.side(at), self.side(home));
        let me = here.vault.me();

        // Its own attests, also after a restart.
        let count = here.vault.attest_count().await?;
        for n in (st.attest_cursor[i] + 1)..=count {
            if let Some(f) = here.vault.fast_lock(n).await? {
                if f.attester.eq_ignore_ascii_case(&me) {
                    st.attests[i].insert(n, f.lock_id);
                }
            }
            st.attest_cursor[i] = n;
        }
        for n in st.attests[i].keys().copied().collect::<Vec<_>>() {
            if let Err(e) = self.tend_attest(st, at, n).await {
                warn!(network = here.net.name(), attest = n, error = %crate::secrets::redact(&e), "tending an attest failed");
            }
        }

        // Burns it paid at once: repaid once a claim carrying them is
        // accepted.
        for r in st.fast_paid[i].iter().cloned().collect::<Vec<_>>() {
            let Record::Request { id, .. } = r else { continue };
            let outcome: anyhow::Result<()> = async {
                if here.vault.request_paid(id).await? {
                    st.fast_paid[i].remove(&r);
                    return Ok(());
                }
                if let Some(claim) = st.claims[i].accepted(&r) {
                    here.vault.pay_request(claim, &r).await?;
                    st.fast_paid[i].remove(&r);
                    info!(network = here.net.name(), request = id, claim, "the vault repaid a burn paid at once");
                }
                Ok(())
            }
            .await;
            if let Err(e) = outcome {
                warn!(network = here.net.name(), request = id, error = %crate::secrets::redact(&e), "taking a fast repayment failed");
            }
        }

        // Attests, the best-paid first (D122), by a chain that may attest and
        // open the claim each needs within 7 days (D123): it attests a lock
        // only when the claims of its attests waiting for one still fit 80%
        // of its bond in the asset here, as counted now.
        // A batch on its way here takes cover the vault does not count yet:
        // counted with the attests waiting for a claim. A message this node
        // keeps no count of (after a restart) stops attests until it is
        // processed.
        let unknown = match self.btc.spender(&chain.coin.0, chain.coin.1).await {
            Ok(spender) => spender.is_some() && !st.in_flight.iter().any(|(index, _)| *index == chain.messages),
            Err(e) => {
                warn!(error = %crate::secrets::redact(&e), "reading the pair chain on Bitcoin failed: no attests this round");
                true
            }
        };
        if unknown {
            debug!(network = here.net.name(), "no attests: a message this node keeps no count of is on its way here");
        }
        let waiting = if !unknown && can_open(here, chain).await? { self.waiting_attests(st, at).await } else { None };
        if let Some(mut waiting) = waiting {
            for (asset, value) in in_flight_value(&st.in_flight, self.ix(at), home, chain.messages) {
                *waiting.entry(asset).or_default() += value;
            }
            let mut locks = vec![];
            for id in window(&st.open_locks[h], st.lock_turn[h]) {
                let l = match there.vault.lock(id).await {
                    Ok(Some(l)) => l,
                    Ok(None) => continue,
                    Err(e) => {
                        warn!(network = there.net.name(), lock = id, error = %crate::secrets::redact(&e), "reading a lock failed");
                        continue;
                    }
                };
                let Some(fast) = self.settings.asset(home, l.asset).and_then(|s| s.fast.as_ref()) else { continue };
                // Speed is what the fast fee pays for: none once a claim
                // carrying the lock was accepted. Once only: another attest
                // of its own would count as extra and lose its receipts
                // (D126).
                if l.fast_fee >= fast.min_fee
                    && l.amount <= fast.max
                    && !l.returned
                    && st.claims[i].accepted(&l.record(home)).is_none()
                    && !st.attests[i].values().any(|lock| *lock == id)
                {
                    locks.push(l);
                }
            }
            locks.sort_by(|a, b| b.fast_fee.cmp(&a.fast_fee));
            let now = here.net.now().await?;
            for l in locks {
                let r = l.record(home);
                let outcome: anyhow::Result<()> = async {
                    if here.vault.receipt_issued(l.id).await? || here.vault.given_up(l.id).await? {
                        return Ok(());
                    }
                    if let Some((first_at, open)) = here.vault.lock_attests(l.id).await? {
                        // The lock takes no more attests, or an earlier one
                        // states the true record: this one would count as
                        // extra (D126).
                        if now >= first_at + FAST_OPEN_WINDOW || open.iter().any(|f| f.stated == record_hash(&r)) {
                            return Ok(());
                        }
                    }
                    let p = here.vault.position(&me, home, l.asset).await?;
                    if !fits_cover(&p, waiting.get(&l.asset).copied().unwrap_or(0), value_of(&r)) {
                        return Ok(());
                    }
                    // Its receipt credit first: the collateral of settled
                    // attests.
                    if here.vault.credit(home, l.asset).await? > 0 {
                        here.vault.withdraw_credit(home, l.asset).await?;
                    }
                    let n = here.vault.attest_lock(&r).await?;
                    st.attests[i].insert(n, l.id);
                    *waiting.entry(l.asset).or_default() += value_of(&r);
                    let collateral = (l.amount as u128 * FAST_COLLATERAL_BPS).div_ceil(10_000) as u64;
                    info!(network = here.net.name(), lock = l.id, attest = n, collateral, "issued a receipt at once");
                    Ok(())
                }
                .await;
                if let Err(e) = outcome {
                    // The attest may have landed although the call failed:
                    // its cover is not counted, so none more this round.
                    warn!(network = here.net.name(), lock = l.id, error = %crate::secrets::redact(&e), "attesting a lock failed: no more attests this round");
                    break;
                }
            }
        }

        // Burns paid at once, the best-paid first. Anyone may: it risks only
        // its own money. Its own from before a restart are found first.
        let mut burns = vec![];
        for id in window(&st.open_requests[h], st.request_turn[h]) {
            let outcome: anyhow::Result<()> = async {
                let Some(q) = there.vault.request(id).await? else { return Ok(()) };
                let r = q.record(home);
                if st.fast_paid[i].contains(&r) {
                    return Ok(());
                }
                // Its own first, also once accepted: repaid by the claim.
                if here.vault.fast_paid_by(&r).await?.is_some_and(|by| by.eq_ignore_ascii_case(&me)) {
                    st.fast_paid[i].insert(r);
                    return Ok(());
                }
                if st.claims[i].accepted(&r).is_some() {
                    return Ok(());
                }
                let Some(fast) = self.settings.asset(at, q.asset).and_then(|s| s.fast.as_ref()) else { return Ok(()) };
                if q.fast_fee >= fast.min_fee && q.amount <= fast.max {
                    burns.push((q.fast_fee, r));
                }
                Ok(())
            }
            .await;
            if let Err(e) = outcome {
                warn!(network = there.net.name(), request = id, error = %crate::secrets::redact(&e), "reading a burn failed");
            }
        }
        burns.sort_by(|a, b| b.0.cmp(&a.0));
        for (_, r) in burns {
            let Record::Request { id, amount, .. } = r else { continue };
            let outcome: anyhow::Result<()> = async {
                if here.vault.request_paid(id).await? || here.vault.fast_paid_by(&r).await?.is_some() {
                    return Ok(());
                }
                here.vault.fast_pay(&r).await?;
                info!(network = here.net.name(), request = id, amount, "paid a burn at once");
                st.fast_paid[i].insert(r);
                Ok(())
            }
            .await;
            if let Err(e) = outcome {
                warn!(network = here.net.name(), request = id, error = %crate::secrets::redact(&e), "paying a burn at once failed");
            }
        }
        Ok(())
    }

    /// What this node's attests on network `at` that no claim of its own
    /// carries yet, and not burned, will move, by asset: their claims need
    /// that cover. `None`
    /// when a lock cannot be read: then nothing is attested this round.
    async fn waiting_attests(&self, st: &OpState, at: u8) -> Option<HashMap<u32, u64>> {
        let (i, home) = (self.ix(at), self.other(at));
        let me = self.side(at).vault.me();
        let mut waiting: HashMap<u32, u64> = HashMap::new();
        for (&n, &lock) in &st.attests[i] {
            // A burned attest waits for no claim: its collateral is gone.
            match self.side(at).vault.fast_lock(n).await {
                Ok(Some(f)) if !f.burned => {}
                Ok(_) => continue,
                Err(e) => {
                    warn!(network = self.side(at).net.name(), attest = n, error = %crate::secrets::redact(&e), "reading an attest failed");
                    return None;
                }
            }
            match self.side(home).vault.lock(lock).await {
                Ok(Some(l)) => {
                    let r = l.record(home);
                    if !st.claims[i].handled_by(&r, &me) {
                        *waiting.entry(l.asset).or_default() += value_of(&r);
                    }
                }
                Ok(None) => {}
                Err(e) => {
                    warn!(network = self.side(home).net.name(), lock, error = %crate::secrets::redact(&e), "reading an attested lock failed");
                    return None;
                }
            }
        }
        Some(waiting)
    }

    /// Links one of this node's attests on network `at` to a claim carrying
    /// its lock's record, and settles it once such a claim is accepted.
    async fn tend_attest(&self, st: &mut OpState, at: u8, n: u64) -> anyhow::Result<()> {
        let (here, there) = (self.side(at), self.side(self.other(at)));
        let i = self.ix(at);
        let Some(f) = here.vault.fast_lock(n).await? else {
            st.attests[i].remove(&n);
            return Ok(());
        };
        // It attests only a lock it read: the record is the lock's own.
        let Some(l) = there.vault.lock(f.lock_id).await? else { return Ok(()) };
        let r = l.record(self.other(at));
        let idx = &st.claims[i];
        if let Some(claim) = idx.accepted(&r) {
            settle_in_order(here.vault.as_ref(), claim, n).await?;
            st.attests[i].remove(&n);
            info!(network = here.net.name(), attest = n, claim, "an attest was settled: its receipts back with the fast fee");
            return Ok(());
        }
        if f.burned {
            return Ok(());
        }
        let linked_refused = f.claim != 0 && idx.status.get(&f.claim).is_some_and(|(_, d, a)| *d && !*a);
        if f.claim == 0 || linked_refused {
            // Only a claim of its own: another operator could let its claim
            // be refused and burn the attest before a relink (D123).
            let me = here.vault.me();
            let carrying = idx.carrying.get(&r).cloned().unwrap_or_default();
            let open = carrying.iter().find(|c| idx.status.get(c).is_some_and(|(op, d, _)| !*d && *op == me));
            if let Some(claim) = open {
                here.vault.link_fast(*claim, n, (f.claim != 0).then_some(f.claim)).await?;
                info!(network = here.net.name(), attest = n, claim, "linked an attest to its claim");
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
        let chains = [self.a.vault.chain(&self.a.vault.me()).await?, self.b.vault.chain(&self.b.vault.me()).await?];
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
        self.make_receipts(&mut st).await;
        // Batches both vaults processed are counted in their claims.
        let processed = chains[0].messages.min(chains[1].messages);
        st.in_flight.retain(|(index, _)| *index >= processed);
        if let Err(e) = self.discover(&mut st).await {
            warn!(error = %crate::secrets::redact(&e), "reading new locks and burns failed");
        }
        self.fast(&mut st, &chains).await;
        if !self.catch_up(&mut st, &chains).await? {
            return Ok(());
        }
        // Both vaults are at the end of the chain, and no message waits.
        let chains = [self.a.vault.chain(&self.a.vault.me()).await?.unwrap(), self.b.vault.chain(&self.b.vault.me()).await?.unwrap()];
        anyhow::ensure!(chains[0].coin == chains[1].coin, "the two vaults follow different coins of the pair chain");
        anyhow::ensure!(chains[0].messages == chains[1].messages, "the two vaults count different messages of the pair chain");
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
        let (records, moved) = self.gather(&mut st, &chains).await?;
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
        // Both vaults are at the end of the chain: this is message number
        // `messages` on each.
        st.in_flight.push((chains[0].messages, moved));
        info!(records = records.len(), txid = %hex::encode(sent.txid), "wrote a vault batch on Bitcoin");
        debug!(batch = ?records, "the batch written");
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
    /// By network of the attests: those read so far, and those not settled
    /// or burned.
    attest_cursor: [u64; 2],
    open_attests: [HashSet<u64>; 2],
    /// By acting network: locks carried by an accepted claim there, with the
    /// claim and the record.
    accepted_locks: [HashMap<u64, (u64, Record)>; 2],
    /// By network: the assets this node was paid attest rewards in, as
    /// (home, asset), withdrawn each round.
    rewards: [HashSet<(u8, u32)>; 2],
    /// Slashed chains, by (network, operator), whose bonds are all settled.
    settled: HashSet<(usize, String)>,
    reals: [Reals; 2],
}

pub struct VaultGuardian {
    /// The pair's two sides (D132).
    a: Side,
    b: Side,
    nets: Nets,
    btc: Arc<dyn BitcoinView>,
    /// What to pay to open a checkpoint job when a lie waits for a real
    /// block above it, by network (Ethereum, Solana): the lie's slash pays
    /// 20% of the bonds, well more than the job.
    checkpoint_paid: [Option<Amount>; 2],
    state: Mutex<GuardState>,
}

/// Messages brought to a vault per chain and round at most.
const BRING_PER_ROUND: usize = 5;
/// Lagging messages of a chain looked at for a lie per round.
const LOOK_AHEAD: u64 = 20;

/// A chain's stated bond and bond in each asset on one network.
type Bonds = HashMap<(u8, u32), u64>;

impl VaultGuardian {
    pub fn new(a: Side, b: Side, btc: Arc<dyn BitcoinView>, checkpoint_paid: [Option<Amount>; 2]) -> anyhow::Result<Self> {
        let nets = pair_of(&a, &b)?;
        Ok(VaultGuardian { a, b, nets, btc, checkpoint_paid, state: Mutex::new(GuardState::default()) })
    }

    fn side(&self, net: u8) -> &Side {
        if net == self.nets.0[0] { &self.a } else { &self.b }
    }

    fn ix(&self, net: u8) -> usize {
        self.nets.ix(net)
    }

    fn other(&self, net: u8) -> u8 {
        self.nets.other(net)
    }

    /// Learns the chain of a claim's operator: its address on both
    /// networks.
    async fn learn(&self, st: &mut GuardState, acting: u8, operator: &str) -> anyhow::Result<()> {
        let Some(c) = self.side(acting).vault.chain(operator).await? else { return Ok(()) };
        let peer = peer_address(&c.peer);
        // EVM addresses in one spelling, so a chain is held once; the pair's
        // first side first.
        let norm = |net: u8, a: String| if net == SOLANA { a } else { a.to_lowercase() };
        let (mine, theirs) = (norm(acting, operator.to_string()), norm(self.other(acting), peer));
        let pair = if self.ix(acting) == 0 { (mine, theirs) } else { (theirs, mine) };
        st.chains.insert(pair);
        Ok(())
    }

    /// Whether a message is false on network `j`, the one it would be
    /// brought to: a lie that network can prove and slash. The rules of the
    /// batch's shape are `shape_false`'s; here the records are also checked
    /// against what `j` holds. `stated` is the chain's stated bonds there,
    /// as they will be once the messages before this one are processed.
    async fn false_on(&self, j: u8, raw_len: usize, batch: &[u8], stated: &mut Bonds, bonds: &Bonds) -> anyhow::Result<bool> {
        let Some(records) = decode(batch) else { return Ok(true) };
        let Some(next) = shape_false(j, self.other(j), raw_len, batch.len(), &records, stated, bonds) else { return Ok(true) };
        let v = &self.side(j).vault;
        let mut assets: Option<Vec<AssetInfo>> = None;
        for r in &records {
            let false_here = match r {
                // Facts of `j`.
                Record::Lock { home, id, .. } if *home == j => !v.lock(*id).await?.is_some_and(|l| l.record(j) == *r),
                Record::Request { net, id, .. } if *net == j => !v.request(*id).await?.is_some_and(|q| q.record(j) == *r),
                Record::Cancel { net, id } if *net == j => !v.given_up(*id).await?,
                Record::Asset { home, .. } if *home == j => {
                    let known = match &assets {
                        Some(a) => a,
                        None => assets.insert(v.assets().await?),
                    };
                    !known.iter().any(|a| a.record(j) == *r)
                }
                // Acted on by `j`: the other network gives up only a lock
                // that exists here, and burns only receipts of an asset
                // registered here.
                Record::Cancel { id, .. } => v.lock(*id).await?.is_none(),
                Record::Request { asset, .. } => {
                    let known = match &assets {
                        Some(a) => a,
                        None => assets.insert(v.assets().await?),
                    };
                    !known.iter().any(|a| a.number == *asset)
                }
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
        for (first_op, second_op) in st.chains.clone() {
            if let Err(e) = self.bring_chain(st, &first_op, &second_op).await {
                warn!(first = %first_op, second = %second_op, error = %crate::secrets::redact(&e), "bringing a hidden message failed");
            }
        }
    }

    async fn bring_chain(&self, st: &mut GuardState, first_op: &str, second_op: &str) -> anyhow::Result<()> {
        let (Some(e), Some(s)) = (self.a.vault.chain(first_op).await?, self.b.vault.chain(second_op).await?) else { return Ok(()) };
        if e.messages == s.messages {
            return Ok(());
        }
        let (j, behind_op, ahead_op, chain, lead) =
            if e.messages < s.messages { (self.nets.0[0], first_op, second_op, e, s.messages) } else { (self.nets.0[1], second_op, first_op, s, e.messages) };
        let (behind, ahead) = (self.side(j), self.side(self.other(j)));
        // Already slashed, or the chain ended here. A chain with no bond is
        // still brought to: the slash marks it, which settles the other
        // network's claims that count a bond it never had.
        if chain.slashed || chain.exited {
            return Ok(());
        }
        // The lagging messages up to the first lie, followed on Bitcoin.
        let mut path = vec![];
        let mut coin = chain.coin;
        let mut stated: Bonds = chain.positions.iter().map(|p| ((p.home, p.asset), p.stated)).collect();
        let bonds: Bonds = chain.positions.iter().map(|p| ((p.home, p.asset), p.bond)).collect();
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
            lie = self.false_on(j, raw.len(), &batch, &mut stated, &bonds).await?;
            path.push((index, batch, txid, input, tag));
            coin = (txid, input);
            if lie {
                break;
            }
        }
        if !lie {
            return Ok(());
        }
        let i = self.ix(j);
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

    /// Settles the bonds of slashed chains, asset by asset (D129): anyone
    /// may, and on Solana it credits the slasher's 20%. Then withdraws this
    /// node's share, if it was the slasher.
    async fn settle_slashes(&self, st: &mut GuardState) {
        for (first_op, second_op) in st.chains.clone() {
            for (net, op) in [(self.nets.0[0], first_op), (self.nets.0[1], second_op)] {
                let i = self.ix(net);
                if st.settled.contains(&(i, op.clone())) {
                    continue;
                }
                let side = self.side(net);
                let outcome: anyhow::Result<()> = async {
                    let Some(c) = side.vault.chain(&op).await? else { return Ok(()) };
                    if !c.slashed {
                        return Ok(());
                    }
                    let mut pending = false;
                    for p in &c.positions {
                        if side.vault.slash_pending(&op, p.home, p.asset).await? > 0 {
                            if let Err(e) = side.vault.settle_slash(&op, p.home, p.asset).await {
                                // A token that will not move keeps only its
                                // own asset unsettled.
                                pending = true;
                                warn!(network = side.net.name(), operator = %op, asset = p.asset, error = %crate::secrets::redact(&e), "settling a slashed bond failed");
                                continue;
                            }
                            info!(network = side.net.name(), operator = %op, home = p.home, asset = p.asset, "settled a slashed bond");
                        }
                        if side.vault.credit(p.home, p.asset).await? > 0 {
                            side.vault.withdraw_credit(p.home, p.asset).await?;
                        }
                    }
                    if !pending {
                        st.settled.insert((i, op.clone()));
                    }
                    Ok(())
                }
                .await;
                if let Err(e) = outcome {
                    warn!(network = side.net.name(), operator = %op, error = %crate::secrets::redact(&e), "settling a slash failed");
                }
            }
        }
    }

    /// Checks one claim: collects it once decided, decides it once its 7
    /// days are over, objects to it when a record is false.
    async fn tend_claim(&self, st: &mut GuardState, acting: u8, id: u64, now: i64) -> anyhow::Result<()> {
        let i = self.ix(acting);
        let side = self.side(acting);
        let c = side.vault.claim(id).await?;
        if st.learned.insert((i, id)) {
            if let Err(e) = self.learn(st, acting, &c.operator).await {
                st.learned.remove(&(i, id));
                warn!(network = side.net.name(), claim = id, error = %crate::secrets::redact(&e), "reading a claim's chain failed");
            }
        }
        if c.decided {
            if c.accepted {
                for r in &c.records {
                    if let Record::Lock { id: lock, .. } = r {
                        st.accepted_locks[i].insert(*lock, (id, r.clone()));
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
            if self.is_true(acting, &c.operator, r).await? == Some(false) {
                info!(network = side.net.name(), claim = id, record = ?r, "objecting to a false claim");
                side.vault.object(id).await?;
                break;
            }
        }
        Ok(())
    }

    /// The attests on network `at` of locks on the other (section 11.7,
    /// D123): burns one with no claim linked within 7 days, or whose linked
    /// claim was refused, and settles one whose lock an accepted claim
    /// states otherwise. Either pays this node the quarter above the
    /// amount.
    async fn tend_attests(&self, st: &mut GuardState, at: u8) -> anyhow::Result<()> {
        let i = self.ix(at);
        let here = self.side(at);
        let count = here.vault.attest_count().await?;
        for n in (st.attest_cursor[i] + 1)..=count {
            st.open_attests[i].insert(n);
        }
        st.attest_cursor[i] = count;
        let now = here.net.now().await?;
        // Oldest first: a lock's attests are settled in the order made.
        let mut open: Vec<u64> = st.open_attests[i].iter().copied().collect();
        open.sort_unstable();
        for n in open {
            let outcome: anyhow::Result<()> = async {
                let Some(f) = here.vault.fast_lock(n).await? else {
                    st.open_attests[i].remove(&n);
                    return Ok(());
                };
                if let Some((claim, record)) = st.accepted_locks[i].get(&f.lock_id).cloned() {
                    // A wrong record: settling it burns the wrong receipt,
                    // or, once burned, issues the true one to its recipient.
                    // As stated: its attester settles it.
                    if record_hash(&record) != f.stated {
                        settle_in_order(here.vault.as_ref(), claim, n).await?;
                        st.rewards[i].insert((self.other(at), f.asset));
                        info!(network = here.net.name(), attest = n, claim, "settled an attest that stated its lock wrongly");
                    }
                    st.open_attests[i].remove(&n);
                    return Ok(());
                }
                if f.burned {
                    return Ok(());
                }
                let burnable = if f.claim == 0 {
                    now >= f.attested_at + FAST_OPEN_WINDOW
                } else {
                    let c = here.vault.claim_status(f.claim).await?;
                    c.decided && !c.accepted
                };
                if burnable {
                    here.vault.burn_fast(n, (f.claim != 0).then_some(f.claim)).await?;
                    st.rewards[i].insert((self.other(at), f.asset));
                    st.open_attests[i].remove(&n);
                    info!(network = here.net.name(), attest = n, lock = f.lock_id, "burned an attest with no claim to back it");
                }
                Ok(())
            }
            .await;
            if let Err(e) = outcome {
                warn!(network = here.net.name(), attest = n, error = %crate::secrets::redact(&e), "tending an attest failed");
            }
        }
        Ok(())
    }

    async fn tend_claims(&self, st: &mut GuardState, acting: u8) -> anyhow::Result<()> {
        let i = self.ix(acting);
        let side = self.side(acting);
        let count = side.vault.claim_count().await?;
        for id in (st.claim_cursor[i] + 1)..=count {
            st.open_claims[i].insert(id);
        }
        st.claim_cursor[i] = count;
        let now = side.net.now().await?;
        for id in st.open_claims[i].clone() {
            // Tried again next round.
            if let Err(e) = self.tend_claim(st, acting, id, now).await {
                warn!(network = side.net.name(), claim = id, error = %crate::secrets::redact(&e), "tending a claim failed");
            }
        }
        Ok(())
    }

    /// Whether a record acted on by network `acting` is true on the other
    /// network, where its fact lives: `None` when that cannot be told yet.
    async fn is_true(&self, acting: u8, operator: &str, record: &Record) -> anyhow::Result<Option<bool>> {
        let fact = self.other(acting);
        let v = &self.side(fact).vault;
        Ok(match record {
            Record::Lock { id, .. } => Some(v.lock(*id).await?.is_some_and(|l| l.record(fact) == *record)),
            Record::Request { id, .. } => Some(v.request(*id).await?.is_some_and(|q| q.record(fact) == *record)),
            Record::Cancel { id, .. } => Some(v.given_up(*id).await?),
            Record::Asset { .. } => Some(v.assets().await?.iter().any(|a| a.record(fact) == *record)),
            Record::Bond { home, asset, amount, .. } => {
                let Some(here) = self.side(acting).vault.chain(operator).await? else { return Ok(None) };
                let peer = peer_address(&here.peer);
                let Some(there) = v.chain(&peer).await? else { return Ok(Some(false)) };
                if there.position(*home, *asset).stated == *amount {
                    Some(true)
                } else if there.slashed || there.messages >= here.messages {
                    // The vault holding it has processed as much of the
                    // chain as this one, and holds another bond.
                    Some(false)
                } else {
                    None
                }
            }
            Record::Exit => Some(true),
        })
    }
}

/// The network byte of a record: the one that holds its fact, or for a
/// BOND the one holding it.
fn net_byte(r: &Record) -> Option<u8> {
    match r {
        Record::Lock { home, .. } | Record::Asset { home, .. } => Some(*home),
        Record::Request { net, .. } | Record::Cancel { net, .. } | Record::Bond { net, .. } => Some(*net),
        Record::Exit => None,
    }
}

/// The rules of a message's shape that make it false on network `j`, as
/// both vaults judge it (D109, D110, D119, D131), in the pair of `j` and
/// `peer` (D132): a transaction or batch too large, a record of a network
/// outside the pair, more than 32 LOCK, REQUEST, CANCEL and ASSET records,
/// a BOND of `j` that is zero, repeated in the batch, or not the chain's
/// bond, a BOND of an asset of a network outside the pair, a REQUEST paid on `j` to no address, or on Solana a lock or
/// burn whose amount and fast fee overflow. `None` when false; otherwise
/// the chain's stated bonds on `j` after the message. What the records say
/// about locks, burns, give-ups and assets is checked against the vault by
/// the caller.
fn shape_false(j: u8, peer: u8, raw_len: usize, batch_len: usize, records: &[Record], stated: &Bonds, bonds: &Bonds) -> Option<Bonds> {
    if raw_len > MAX_RAW_TX || batch_len > MAX_BATCH {
        return None;
    }
    let mut counted = 0;
    let mut next = stated.clone();
    let mut seen: Vec<(u8, u32)> = vec![];
    for r in records {
        let Some(net) = net_byte(r) else { continue };
        if net != j && net != peer {
            return None;
        }
        if r.counted() {
            counted += 1;
            if counted > MAX_RECORDS {
                return None;
            }
        }
        match r {
            Record::Bond { home, asset, amount, .. } => {
                if *home != j && *home != peer {
                    return None;
                }
                if net == j {
                    let k = (*home, *asset);
                    let s = stated.get(&k).copied().unwrap_or(0);
                    let bond = bonds.get(&k).copied().unwrap_or(0);
                    if *amount == 0 || seen.contains(&k) || seen.len() == MAX_ASSETS || if s != 0 { *amount != s } else { *amount > bond } {
                        return None;
                    }
                    seen.push(k);
                    next.insert(k, *amount);
                }
            }
            // Solana issues the receipt and the fast fee.
            Record::Lock { amount, fast_fee, .. } if net != j && j == SOLANA => {
                amount.checked_add(*fast_fee)?;
            }
            Record::Request { to, amount, fast_fee, .. } if net != j => {
                if j == SOLANA {
                    if *to == [0u8; 32] {
                        return None;
                    }
                    amount.checked_add(*fast_fee)?;
                } else if to[..12] != [0u8; 12] || to[12..] == [0u8; 20] {
                    return None;
                }
            }
            _ => {}
        }
    }
    Some(next)
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
        for net in self.nets.0 {
            if let Err(e) = self.tend_claims(&mut st, net).await {
                warn!(network = self.side(net).net.name(), error = %crate::secrets::redact(&e), "reading claims failed");
            }
            if let Err(e) = self.tend_attests(&mut st, net).await {
                warn!(network = self.side(net).net.name(), error = %crate::secrets::redact(&e), "reading attests failed");
            }
        }
        self.bring_hidden(&mut st).await;
        self.settle_slashes(&mut st).await;
        for (i, side) in [&self.a, &self.b].into_iter().enumerate() {
            let rewards: Vec<(u8, u32)> = st.rewards[i].iter().copied().collect();
            withdraw_credits(side, &rewards).await;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bonds(entries: &[((u8, u32), u64)]) -> Bonds {
        entries.iter().copied().collect()
    }

    #[test]
    fn a_message_is_false_by_its_shape_as_the_vaults_judge_it() {
        let bond = |net, home, amount| Record::Bond { net, home, asset: 0, amount };
        let request = |net, to| Record::Request { net, asset: 0, id: 1, amount: 1, to, fee: 0, fast_fee: 0, at: 0 };
        let lock = |home, amount, fast_fee| Record::Lock { home, asset: 0, id: 1, amount, recipient: [0; 32], fee: 0, fast_fee, at: 0 };
        let eth = (ETHEREUM, 0);
        let none = Bonds::new();
        let held = bonds(&[(eth, 5)]);
        // True: the chain's first stated bond, carried forward.
        assert_eq!(shape_false(ETHEREUM, SOLANA, 300, 15, &[bond(ETHEREUM, ETHEREUM, 5)], &none, &held), Some(bonds(&[(eth, 5)])));
        // The same BOND again is true; another one is not.
        let more = bonds(&[(eth, 9)]);
        assert_eq!(shape_false(ETHEREUM, SOLANA, 300, 15, &[bond(ETHEREUM, ETHEREUM, 5)], &held, &more), Some(held.clone()));
        assert_eq!(shape_false(ETHEREUM, SOLANA, 300, 15, &[bond(ETHEREUM, ETHEREUM, 4)], &held, &more), None);
        // More than the bond, zero, twice in a batch, of an unknown network
        // or of an asset of one.
        assert_eq!(shape_false(ETHEREUM, SOLANA, 300, 15, &[bond(ETHEREUM, ETHEREUM, 6)], &none, &held), None);
        assert_eq!(shape_false(ETHEREUM, SOLANA, 300, 15, &[bond(ETHEREUM, ETHEREUM, 0)], &none, &held), None);
        assert_eq!(shape_false(ETHEREUM, SOLANA, 300, 30, &[bond(ETHEREUM, ETHEREUM, 5), bond(ETHEREUM, ETHEREUM, 5)], &none, &held), None);
        assert_eq!(shape_false(SOLANA, ETHEREUM, 300, 15, &[bond(9, ETHEREUM, 5)], &none, &held), None);
        assert_eq!(shape_false(SOLANA, ETHEREUM, 300, 15, &[bond(ETHEREUM, 9, 5)], &none, &held), None);
        // The other network's BOND is not judged here.
        assert_eq!(shape_false(SOLANA, ETHEREUM, 300, 15, &[bond(ETHEREUM, ETHEREUM, 999)], &none, &none), Some(none.clone()));
        // A REQUEST paid to no address, as each network reads one.
        assert_eq!(shape_false(ETHEREUM, SOLANA, 300, 78, &[request(SOLANA, [0; 32])], &none, &none), None);
        let mut high = eth_address32(&[1; 20]);
        high[0] = 1;
        assert_eq!(shape_false(ETHEREUM, SOLANA, 300, 78, &[request(SOLANA, high)], &none, &none), None);
        assert_eq!(shape_false(ETHEREUM, SOLANA, 300, 78, &[request(SOLANA, eth_address32(&[1; 20]))], &none, &none), Some(none.clone()));
        assert_eq!(shape_false(SOLANA, ETHEREUM, 300, 78, &[request(ETHEREUM, [0; 32])], &none, &none), None);
        assert_eq!(shape_false(SOLANA, ETHEREUM, 300, 78, &[request(ETHEREUM, high)], &none, &none), Some(none.clone()));
        // A burn made on `j` is judged against the vault, not here.
        assert_eq!(shape_false(SOLANA, ETHEREUM, 300, 78, &[request(SOLANA, [0; 32])], &none, &none), Some(none.clone()));
        // A lock whose amount and fast fee overflow is false on Solana only.
        assert_eq!(shape_false(SOLANA, ETHEREUM, 300, 78, &[lock(ETHEREUM, u64::MAX, 1)], &none, &none), None);
        assert_eq!(shape_false(ETHEREUM, SOLANA, 300, 78, &[lock(SOLANA, u64::MAX, 1)], &none, &none), Some(none.clone()));
        // D119: sizes, and the count of LOCK, REQUEST, CANCEL and ASSET
        // records on both networks together; BOND is not counted.
        assert_eq!(shape_false(SOLANA, ETHEREUM, MAX_RAW_TX + 1, 10, &[], &none, &none), None);
        assert_eq!(shape_false(SOLANA, ETHEREUM, 300, MAX_BATCH + 1, &[], &none, &none), None);
        let mut many = vec![Record::Cancel { net: ETHEREUM, id: 1 }; 16];
        many.extend(vec![Record::Cancel { net: SOLANA, id: 1 }; 16]);
        assert_eq!(shape_false(SOLANA, ETHEREUM, 300, 320, &many, &none, &none), Some(none.clone()));
        many.push(Record::Asset { home: ETHEREUM, asset: 0, token: [0; 32], decimals: 9 });
        assert_eq!(shape_false(SOLANA, ETHEREUM, 300, 359, &many, &none, &none), None);
        many.pop();
        many.push(bond(ETHEREUM, SOLANA, 1));
        assert_eq!(shape_false(SOLANA, ETHEREUM, 300, 335, &many, &none, &none), Some(none.clone()));
    }

    #[test]
    fn judges_a_pair_of_two_evm_networks_by_the_same_shape() {
        use ipow_protocol_core::vault::BASE;
        let none = Bonds::new();
        let request = |net, to| Record::Request { net, asset: 0, id: 1, amount: 1, to, fee: 0, fast_fee: 0, at: 0 };
        // In the pair Ethereum and Base, a record of Solana is false.
        assert_eq!(shape_false(ETHEREUM, BASE, 300, 10, &[Record::Cancel { net: SOLANA, id: 1 }], &none, &none), None);
        assert_eq!(shape_false(ETHEREUM, BASE, 300, 10, &[Record::Cancel { net: BASE, id: 1 }], &none, &none), Some(none.clone()));
        // A burn on Base paid on Ethereum, to an address there; the same
        // rule the other way.
        assert_eq!(shape_false(ETHEREUM, BASE, 300, 78, &[request(BASE, eth_address32(&[1; 20]))], &none, &none), Some(none.clone()));
        assert_eq!(shape_false(BASE, ETHEREUM, 300, 78, &[request(ETHEREUM, [7; 32])], &none, &none), None);
        // Solana's limits do not apply: a lock whose amount and fast fee
        // overflow opens no claim on an EVM network, and is not false.
        let lock = Record::Lock { home: BASE, asset: 0, id: 1, amount: u64::MAX, recipient: [0; 32], fee: 0, fast_fee: 1, at: 0 };
        assert_eq!(shape_false(ETHEREUM, BASE, 300, 78, &[lock], &none, &none), Some(none.clone()));
    }

    #[test]
    fn a_batch_keeps_to_the_limits_of_section_11_9() {
        let eth0 = Position { home: ETHEREUM, asset: 0, peer_bond: 1_000, ..Default::default() };
        let sol_chain = Chain { positions: vec![eth0.clone()], ..Default::default() };
        let chains = [Chain::default(), sol_chain];
        let positions = || [Positions::new(), Positions::from([((ETHEREUM, 0), eth0.clone())])];
        let mut g = Gathered::new(&chains, positions(), Nets([ETHEREUM, SOLANA]));
        let lock = |id, amount| Record::Lock { home: ETHEREUM, asset: 0, id, amount, recipient: [1; 32], fee: 0, fast_fee: 0, at: 0 };
        // 80% of the bond counted on Solana.
        assert!(g.add(lock(1, 500), SOLANA, Some((ETHEREUM, 0)), 500));
        assert!(!g.add(lock(2, 301), SOLANA, Some((ETHEREUM, 0)), 301));
        assert!(g.add(lock(2, 300), SOLANA, Some((ETHEREUM, 0)), 300));
        // An asset with no bond counted moves nothing.
        assert!(!g.add(lock(3, 1), SOLANA, Some((ETHEREUM, 1)), 1));
        // At most 10 LOCK records act on Solana.
        let mut g = Gathered::new(&chains, positions(), Nets([ETHEREUM, SOLANA]));
        for id in 0..MAX_LOCKS_ON_SOLANA as u64 {
            assert!(g.add(lock(id, 0), SOLANA, Some((ETHEREUM, 0)), 0));
        }
        assert!(!g.add(lock(99, 0), SOLANA, Some((ETHEREUM, 0)), 0));
        // On Solana a claim's assets count with the chain's own: 8 at most.
        let mut g = Gathered::new(&chains, positions(), Nets([ETHEREUM, SOLANA]));
        for asset in 1..MAX_ASSETS as u32 {
            assert!(g.add(Record::Bond { net: ETHEREUM, home: ETHEREUM, asset, amount: 1 }, SOLANA, Some((ETHEREUM, asset)), 0));
        }
        assert!(!g.add(Record::Bond { net: ETHEREUM, home: ETHEREUM, asset: 99, amount: 1 }, SOLANA, Some((ETHEREUM, 99)), 0));
        // 32 counted records in all, whichever network acts.
        let mut g = Gathered::new(&chains, positions(), Nets([ETHEREUM, SOLANA]));
        for id in 0..MAX_RECORDS as u64 {
            assert!(g.add(Record::Cancel { net: SOLANA, id }, ETHEREUM, None, 0));
        }
        assert!(!g.add(Record::Cancel { net: ETHEREUM, id: 99 }, SOLANA, None, 0));
        assert!(g.add(Record::Exit, SOLANA, None, 0));
    }

    #[test]
    fn an_attest_or_a_record_fits_80_percent_of_the_bond() {
        let p = Position { peer_bond: 1_000, open_value: 300, ..Default::default() };
        assert!(fits_cover(&p, 200, 300));
        // What attests waiting for a claim will move counts too.
        assert!(!fits_cover(&p, 201, 300));
        assert!(!fits_cover(&Position::default(), 0, 1));
        assert!(fits_cover(&Position::default(), 0, 0));
    }

    #[test]
    fn counts_what_batches_on_their_way_will_move() {
        let moved = |eth: Vec<((u8, u32), u64)>, sol: Vec<((u8, u32), u64)>| -> Moved { [eth, sol] };
        let in_flight = vec![
            // Message 3: processed on Solana already.
            (3, moved(vec![], vec![((ETHEREUM, 0), 100)])),
            // Message 4: on its way; a lock of ETH and of a token, and a
            // burn paid on Ethereum.
            (4, moved(vec![((ETHEREUM, 0), 7)], vec![((ETHEREUM, 0), 50), ((ETHEREUM, 2), 9), ((SOLANA, 0), 1)])),
            (5, moved(vec![], vec![((ETHEREUM, 0), 5)])),
        ];
        let on_solana = in_flight_value(&in_flight, 1, ETHEREUM, 4);
        assert_eq!(on_solana, HashMap::from([(0, 55), (2, 9)]));
        assert_eq!(in_flight_value(&in_flight, 1, ETHEREUM, 6), HashMap::new());
        assert_eq!(in_flight_value(&in_flight, 0, ETHEREUM, 0), HashMap::from([(0, 7)]));
    }

    #[test]
    fn looks_at_a_large_open_set_in_turns() {
        let set: BTreeSet<u64> = (1..=250).collect();
        let first = window(&set, 0);
        assert_eq!((first.len(), first[0], *first.last().unwrap()), (WINDOW, 1, 100));
        let turn = next_turn(&set, &first);
        let second = window(&set, turn);
        assert_eq!((second[0], *second.last().unwrap()), (101, 200));
        // The last turn wraps to the start.
        let third = window(&set, next_turn(&set, &second));
        assert_eq!((third[0], third[49], third[50]), (201, 250, 1));
        // A set within the window is read whole, from its start.
        let small: BTreeSet<u64> = (1..=10).collect();
        assert_eq!(next_turn(&small, &window(&small, 0)), 0);
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
        let next = encode(&[Record::Cancel { net: SOLANA, id: 4 }]);
        j.append(&format!("batch {}", hex::encode(&next))).unwrap();
        let read = j.read().unwrap();
        assert_eq!(read.batches.get(&message_payload(&next)), Some(&next));
        assert_eq!(read.batches.len(), 2);
        let _ = std::fs::remove_file(&dir);
    }
}
