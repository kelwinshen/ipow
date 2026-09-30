//! The guardian role (spec sections 6.1 to 6.3). It watches every job on
//! one network and earns 20% of the escrow when it proves a failure:
//!
//! - a missed duty: the deadline passed with no proof (D53);
//! - a proof on blocks that are not real Bitcoin (D81): it asks for the
//!   parent of the operator's oldest block while that block is not real,
//!   and shows a branch of real blocks when the operator's branch leaves
//!   the real chain at or above its anchor.
//!
//! Every claim is sealed first and shown a round later (D47).
//!
//! What it remembers lives in memory. After a restart it reads every job
//! and every challenge again: it finds the challenges it opened, and for a
//! competing branch it finds where its real branch starts, which is the
//! same block every time, and keeps growing it from there.

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::sync::Arc;

use async_trait::async_trait;
use ipow_bitcoin::view::BitcoinView;
use ipow_protocol_core::network::ProtocolNetwork;
use ipow_protocol_core::settings::Role;
use ipow_protocol_core::types::{Amount, BlockRef, ChallengeKind, EPOCH_BLOCKS, Evidence, Job, JobStatus};
use tokio::sync::Mutex;
use tracing::{info, warn};

use crate::bitcoin::{STEP, crossing, hex, made_up, stream_real};
use crate::secrets::redact;
use crate::supervisor::Worker;

/// How long anyone has to show a parent that was asked for (the contract's
/// RESPONSE_TIME). No challenge opens this close to the end of a lock (D99).
pub const RESPONSE_TIME: i64 = 12 * 3600;

/// How long a proof's block must stay out of Bitcoin's best chain before the
/// guardian challenges it. An explorer can be a little behind; a guardian
/// that challenges a real block loses its deposit to the operator.
pub const UNKNOWN_FOR: i64 = 20 * 60;

/// The most blocks a branch can be shown with at once: the light client's
/// MAX_WALK.
const MAX_WALK: u32 = 100;

pub struct Guardian {
    btc: Arc<dyn BitcoinView>,
    /// The largest deposit it puts up for one challenge.
    max_deposit: Amount,
    state: Mutex<State>,
}

#[derive(Default)]
struct State {
    /// The last job read.
    cursor: u64,
    /// Jobs that may still fail.
    watched: BTreeSet<u64>,
    /// Sealed notes not yet shown, and their salts.
    notes: HashMap<(u64, Evidence), [u8; 32]>,
    /// When a job's proof block was first seen out of the best chain.
    unknown_since: HashMap<u64, i64>,
    /// The last challenge read.
    challenge_cursor: u64,
    /// Challenges this guardian opened.
    mine: BTreeMap<u64, Mine>,
}

#[derive(Clone, Copy, Debug)]
enum Mine {
    Parent { job: u64 },
    /// The last real block shown on the guardian's side, when known.
    Fork { job: u64, tip: Option<BlockRef> },
}

impl Mine {
    fn job(&self) -> u64 {
        match self {
            Mine::Parent { job } | Mine::Fork { job, .. } => *job,
        }
    }
}

/// What the check of a proof found.
enum Finding {
    /// The proof is on real Bitcoin, or it cannot be told yet.
    Nothing,
    /// The operator's oldest shown block is not real: ask for its parent.
    AskParent(BlockRef),
    /// `operator_block` is not real, and its parent is: real Bitcoin has
    /// another child of that parent.
    Fork { parent: BlockRef, operator_block: BlockRef },
    /// Not real, and no challenge is left (see `find`).
    Stuck,
}

impl Guardian {
    pub fn new(btc: Arc<dyn BitcoinView>, max_deposit: Amount) -> Self {
        Guardian { btc, max_deposit, state: Mutex::new(State::default()) }
    }

    /// The salt of a sealed note for this evidence, or `None` when the note
    /// was sealed just now and can be shown from the next block on.
    async fn salt(&self, net: &dyn ProtocolNetwork, s: &mut State, job: u64, evidence: Evidence) -> anyhow::Result<Option<[u8; 32]>> {
        if let Some(salt) = s.notes.get(&(job, evidence)) {
            return Ok(Some(*salt));
        }
        let mut salt = [0u8; 32];
        getrandom::fill(&mut salt).map_err(|e| anyhow::anyhow!("no randomness: {e}"))?;
        net.seal_note(job, &evidence, &salt).await?;
        s.notes.insert((job, evidence), salt);
        info!(network = net.name(), job, ?evidence, "note sealed");
        Ok(None)
    }

    async fn missed_duty(&self, net: &dyn ProtocolNetwork, s: &mut State, job: &Job) -> anyhow::Result<()> {
        let Some(salt) = self.salt(net, s, job.id, Evidence::MissedDuty).await? else {
            return Ok(());
        };
        net.report_missed_duty(job.id, &salt).await?;
        s.notes.remove(&(job.id, Evidence::MissedDuty));
        info!(network = net.name(), job = job.id, "missed duty reported");
        Ok(())
    }

    /// Where a proven job's branch leaves real Bitcoin, if it does.
    async fn find(&self, net: &dyn ProtocolNetwork, job: &Job) -> anyhow::Result<Finding> {
        let (Some(anchor), Some(proof), Some(deepest)) = (job.anchor, job.proof_block, job.deepest) else {
            anyhow::bail!("a proven job without its blocks");
        };
        let btc = self.btc.as_ref();
        if made_up(btc, &proof).await? != Some(true) {
            return Ok(Finding::Nothing);
        }

        // The operator's branch from the anchor to the proof, oldest first.
        let mut branch = vec![proof];
        while branch.last().unwrap().height > anchor.height {
            let child = *branch.last().unwrap();
            let Some(stored) = net.stored_block(&child).await? else {
                anyhow::bail!("block {} of the proof is not stored", hex(&child.hash));
            };
            let epoch_time = if child.height % EPOCH_BLOCKS == 0 { anchor.epoch_time } else { child.epoch_time };
            branch.push(BlockRef { hash: stored.prev_hash, height: child.height - 1, epoch_time });
        }
        branch.reverse();
        anyhow::ensure!(branch[0] == anchor, "the proof's branch does not reach its anchor");

        // The lowest block that is made up. The proof's block is.
        let mut lowest = branch.len() - 1;
        for (i, b) in branch.iter().enumerate() {
            if made_up(btc, b).await? == Some(true) {
                lowest = i;
                break;
            }
        }
        if lowest > 0 {
            return Ok(Finding::Fork { parent: branch[lowest - 1], operator_block: branch[lowest] });
        }
        // The anchor is made up.
        if made_up(btc, &deepest).await? == Some(true) {
            return Ok(Finding::AskParent(deepest));
        }
        if deepest != anchor && deepest.height + 1 == anchor.height {
            // The parent shown for the anchor is real: real Bitcoin has
            // another child of it.
            return Ok(Finding::Fork { parent: deepest, operator_block: anchor });
        }
        // The blocks shown under the anchor reach real Bitcoin through
        // made-up blocks at the real difficulty. A branch may only part at
        // the anchor or above (D81), so no challenge is left; the forger
        // paid for each of those blocks at the real difficulty.
        Ok(Finding::Stuck)
    }

    /// Checks a proof against real Bitcoin, and challenges it when it is
    /// not real.
    async fn check_proof(&self, net: &dyn ProtocolNetwork, s: &mut State, job: &Job, now: i64) -> anyhow::Result<()> {
        if s.mine.values().any(|m| m.job() == job.id) {
            return Ok(());
        }
        let finding = self.find(net, job).await?;
        if matches!(finding, Finding::Nothing) {
            s.unknown_since.remove(&job.id);
            return Ok(());
        }
        let since = *s.unknown_since.entry(job.id).or_insert(now);
        if now - since < UNKNOWN_FOR {
            return Ok(());
        }
        match finding {
            Finding::Nothing => Ok(()),
            Finding::AskParent(deepest) => self.ask_parent(net, s, job, deepest).await,
            Finding::Fork { parent, operator_block } => self.challenge_fork(net, s, job, parent, operator_block).await,
            Finding::Stuck => {
                warn!(network = net.name(), job = job.id, "the proof is not on real Bitcoin, and no challenge is left for it");
                Ok(())
            }
        }
    }

    async fn ask_parent(&self, net: &dyn ProtocolNetwork, s: &mut State, job: &Job, deepest: BlockRef) -> anyhow::Result<()> {
        let deposit = net.parent_deposit(job.id).await?;
        if deposit > self.max_deposit {
            warn!(network = net.name(), job = job.id, deposit, "the next question costs more than max_deposit");
            return Ok(());
        }
        let evidence = Evidence::Parent(deepest.hash);
        let Some(salt) = self.salt(net, s, job.id, evidence).await? else {
            return Ok(());
        };
        let id = net.ask_parent(job.id, &salt, deposit).await?;
        s.notes.remove(&(job.id, evidence));
        s.mine.insert(id, Mine::Parent { job: job.id });
        info!(network = net.name(), job = job.id, challenge = id, block = %hex(&deepest.hash), "parent asked");
        Ok(())
    }

    /// Shows real blocks from `parent`, a real block of the operator's
    /// branch, against `operator_block`, its child that is not real.
    async fn challenge_fork(&self, net: &dyn ProtocolNetwork, s: &mut State, job: &Job, parent: BlockRef, operator_block: BlockRef) -> anyhow::Result<()> {
        let (Some(anchor), Some(tip)) = (job.anchor, job.tip) else { anyhow::bail!("a proven job without its blocks") };
        let deposit = job.commitment_fee;
        if deposit > self.max_deposit {
            warn!(network = net.name(), job = job.id, deposit, "a challenge costs more than max_deposit");
            return Ok(());
        }
        // One real block more than the operator's branch has from there.
        let count = tip.height - operator_block.height + 2;
        if count > MAX_WALK {
            warn!(network = net.name(), job = job.id, count, "the operator's branch is too long to answer in one challenge");
            return Ok(());
        }
        let Some(real) = stream_real(net, self.btc.as_ref(), &parent, count).await? else {
            return Ok(());
        };
        let (first, last) = (real[0], *real.last().unwrap());
        let evidence = Evidence::Fork(first.hash);
        let Some(salt) = self.salt(net, s, job.id, evidence).await? else {
            return Ok(());
        };
        let prev_epoch_time = crossing(&[anchor, parent, operator_block, tip, first, last]);
        let id = net.challenge_fork(job.id, &operator_block, &first, &last, prev_epoch_time, &salt, deposit).await?;
        s.notes.remove(&(job.id, evidence));
        s.mine.insert(id, Mine::Fork { job: job.id, tip: Some(last) });
        info!(network = net.name(), job = job.id, challenge = id, "real branch shown");
        Ok(())
    }

    /// Challenges it opened before a restart: found by reading every
    /// challenge opened since the last one read.
    async fn recover(&self, net: &dyn ProtocolNetwork, s: &mut State) -> anyhow::Result<()> {
        let count = net.challenge_count().await?;
        let me = net.me();
        for id in s.challenge_cursor + 1..=count {
            if s.mine.contains_key(&id) {
                continue;
            }
            let Some(c) = net.challenge(id).await? else { continue };
            if c.guardian != me {
                continue;
            }
            let mine = match c.kind {
                ChallengeKind::Parent => Mine::Parent { job: c.job_id },
                ChallengeKind::Fork => {
                    // The first block of its branch is where it has always
                    // been: the real child of the parent the operator's
                    // branch left from. The contract keeps a checkpoint there.
                    let job = net.job(c.job_id).await?;
                    let tip = match self.find(net, &job).await? {
                        Finding::Fork { parent, .. } => stream_real(net, self.btc.as_ref(), &parent, 1).await?.map(|r| r[0]),
                        _ => None,
                    };
                    Mine::Fork { job: c.job_id, tip }
                }
            };
            info!(network = net.name(), challenge = id, "found a challenge it opened");
            s.mine.insert(id, mine);
        }
        s.challenge_cursor = count;
        Ok(())
    }

    /// Follows a challenge this guardian opened.
    async fn follow(&self, net: &dyn ProtocolNetwork, s: &mut State, id: u64, mine: Mine, now: i64) -> anyhow::Result<()> {
        let Some(c) = net.challenge(id).await? else {
            // Answered, lost or resolved.
            s.mine.remove(&id);
            return Ok(());
        };
        let job = net.job(c.job_id).await?;
        let ready = match (c.kind, mine) {
            // D88: another challenge slashed the job; the deposit comes back.
            _ if job.status == JobStatus::Slashed => true,
            (ChallengeKind::Parent, _) => now >= c.opened_at + RESPONSE_TIME,
            (ChallengeKind::Fork, Mine::Fork { tip, .. }) => {
                if now >= job.lock_end {
                    true
                } else {
                    // Real Bitcoin keeps growing: show it on our side (D99).
                    if let Some(tip) = tip {
                        self.grow(net, s, id, tip).await?;
                    }
                    false
                }
            }
            (ChallengeKind::Fork, Mine::Parent { .. }) => anyhow::bail!("challenge {id} changed kind"),
        };
        if ready {
            net.resolve_challenge(id).await?;
            s.mine.remove(&id);
            info!(network = net.name(), challenge = id, "challenge resolved");
        }
        Ok(())
    }

    /// Adds Bitcoin's new blocks to its side, in steps the light client can
    /// walk.
    async fn grow(&self, net: &dyn ProtocolNetwork, s: &mut State, id: u64, mut tip: BlockRef) -> anyhow::Result<()> {
        let btc = self.btc.as_ref();
        let Some(real_height) = btc.best_chain_height(&tip.hash).await? else {
            // A reorganisation took the last block shown away. Start again
            // from the branch's first block, which the contract keeps as a
            // checkpoint, in the next round.
            let c = net.challenge(id).await?.ok_or_else(|| anyhow::anyhow!("challenge {id} is closed"))?;
            let job = net.job(c.job_id).await?;
            let first = match self.find(net, &job).await? {
                Finding::Fork { parent, .. } => stream_real(net, btc, &parent, 1).await?.map(|r| r[0]),
                _ => None,
            };
            warn!(network = net.name(), challenge = id, "our last block left Bitcoin's best chain; growing from the first one");
            if let Some(Mine::Fork { tip: t, .. }) = s.mine.get_mut(&id) {
                *t = first;
            }
            return Ok(());
        };
        let mut more = btc.tip_height().await?.saturating_sub(real_height);
        while more > 0 {
            let step = more.min(STEP);
            let Some(real) = stream_real(net, btc, &tip, step).await? else { break };
            let new_tip = *real.last().unwrap();
            net.extend_branch(id, true, &tip, &new_tip, crossing(&[tip, new_tip])).await?;
            tip = new_tip;
            if let Some(Mine::Fork { tip: t, .. }) = s.mine.get_mut(&id) {
                *t = Some(tip);
            }
            more -= step;
        }
        Ok(())
    }
}

#[async_trait]
impl Worker for Guardian {
    fn role(&self) -> Role {
        Role::Guardian
    }

    async fn round(&self, net: &dyn ProtocolNetwork) -> anyhow::Result<()> {
        let mut s = self.state.lock().await;
        let s = &mut *s;
        let now = net.now().await?;

        loop {
            let jobs = net.jobs_after(s.cursor, 50).await?;
            let Some(last) = jobs.last() else { break };
            s.cursor = last.id;
            for j in &jobs {
                if matches!(j.status, JobStatus::Auction | JobStatus::Assigned | JobStatus::Proven) {
                    s.watched.insert(j.id);
                }
            }
        }
        if let Err(e) = self.recover(net, s).await {
            warn!(network = net.name(), error = %redact(&e), "reading challenges failed");
        }

        // One failure is logged and does not stop the rest of the round.
        for (id, mine) in s.mine.clone() {
            if let Err(e) = self.follow(net, s, id, mine, now).await {
                warn!(network = net.name(), challenge = id, error = %redact(&e), "following a challenge failed");
            }
        }

        for id in s.watched.clone() {
            let job = net.job(id).await?;
            let outcome = match job.status {
                JobStatus::Assigned if now > job.deadline => self.missed_duty(net, s, &job).await,
                JobStatus::Proven if now + RESPONSE_TIME <= job.lock_end => self.check_proof(net, s, &job, now).await,
                JobStatus::Auction | JobStatus::Assigned | JobStatus::Proven => Ok(()),
                _ => {
                    s.watched.remove(&id);
                    s.notes.retain(|(j, _), _| *j != id);
                    s.unknown_since.remove(&id);
                    Ok(())
                }
            };
            if let Err(e) = outcome {
                warn!(network = net.name(), job = id, error = %redact(&e), "checking a job failed");
            }
        }

        if net.my_credit().await? > 0 {
            net.withdraw_credit().await?;
        }
        Ok(())
    }
}
