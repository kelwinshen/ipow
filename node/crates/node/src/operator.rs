//! The operator role (spec sections 3 and 4). On one network it:
//!
//! - registers its chain head once (D30, D34);
//! - bids on jobs, from its free bond, up to `max_bid` (D32, D76);
//! - for each job it won: anchors one block below Bitcoin's best block,
//!   sends the tagged transaction, pays more when it waits too long, and
//!   proves it once it has its confirmations (D9, D15);
//! - moves its chain head past a transaction no job can use (N25);
//! - answers every question for a parent, and keeps adding real blocks to
//!   its side of every competing-branch challenge until the lock ends
//!   (D81, D90, D99);
//! - settles its jobs, and withdraws what it earned.
//!
//! What it remembers lives in memory. After a restart it finds what it had
//! sent on Bitcoin by following its chain head, so it never sends a job's
//! transaction twice.

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::sync::Arc;

use async_trait::async_trait;
use ipow_bitcoin::view::TxStatus;
use ipow_bitcoin::wallet::CHAIN_HEAD_VALUE;
use ipow_protocol_core::network::ProtocolNetwork;
use ipow_protocol_core::settings::Role;
use ipow_protocol_core::types::{Amount, BlockRef, Job, JobStatus, Proof, tag_payload};
use tokio::sync::Mutex;
use tracing::{info, warn};

use crate::bitcoin::{crossing, hex};
use crate::defence::defend;
use crate::light::{Feed, MAX_REACH_DOWN};
use crate::secrets::redact;
use crate::supervisor::Worker;
use crate::swaps::{BUY_ANCHOR_AGE, Plan, SELL_MARGIN, Swaps};
use ipow_protocol_core::conversion::Side;
use crate::wallet::{Sent, SharedWallet, head_spend, input_spending, merkle_proof, tag_index};

/// The proof range: the transaction must be in blocks 1 to 25 after the
/// anchor (D15, D68).
const PROOF_RANGE: u32 = 25;

/// Blocks a tagged transaction may wait before it is replaced by one that
/// pays more.
const BUMP_AFTER: u32 = 2;
/// How much more a replacement pays.
const BUMP_FACTOR: f64 = 1.5;

/// The oldest block, in seconds, the operator anchors below Bitcoin's best
/// block. An anchor may be at most 2 hours old (D39); an older block below
/// the best one is passed over for the best one.
const ANCHOR_BELOW_BEST_FOR: i64 = 90 * 60;

pub struct Operator {
    wallet: Arc<SharedWallet>,
    max_bid: Amount,
    /// The Conversion application on this network, when it takes swaps.
    swaps: Option<Arc<Swaps>>,
    state: Mutex<State>,
}

/// A tagged transaction waiting to be mined.
struct Waiting {
    sent: Sent,
    /// Bitcoin's height when it was sent or last replaced.
    since: u32,
}

#[derive(Default)]
struct State {
    cursor: u64,
    /// Jobs it may bid on or works on.
    jobs: BTreeSet<u64>,
    feed: Feed,
    /// The transaction of its first chain head, sent and not yet registered.
    registration: Option<[u8; 32]>,
    challenge_cursor: u64,
    /// Open challenges of its jobs, with the last block it showed on its side.
    challenges: BTreeMap<u64, Option<BlockRef>>,
    /// Tagged transactions not mined yet, by the payload they carry.
    waiting: HashMap<[u8; 32], Waiting>,
}

impl Operator {
    pub fn new(wallet: Arc<SharedWallet>, max_bid: Amount) -> Self {
        Operator { wallet, max_bid, swaps: None, state: Mutex::new(State::default()) }
    }

    /// Takes part in Conversion's swaps on this network.
    pub fn with_swaps(mut self, swaps: Arc<Swaps>) -> Self {
        self.swaps = Some(swaps);
        self
    }

    /// The swaps module, when the job is a swap of Conversion.
    fn swaps_for(&self, job: &Job) -> Option<&Arc<Swaps>> {
        self.swaps.as_ref().filter(|s| s.is_ours(job))
    }

    /// The newest transaction of the wallet that carries `commitment`: a
    /// registration sent before a restart. Waiting ones are the newest.
    async fn find_registration(&self, commitment: &[u8; 32]) -> anyhow::Result<Option<[u8; 32]>> {
        let btc = self.wallet.btc();
        let mut best: Option<(u32, [u8; 32])> = None;
        for c in btc.coins(&self.wallet.address()).await? {
            if c.value != CHAIN_HEAD_VALUE || c.vout != 0 {
                continue;
            }
            let raw = btc.raw_tx(&c.txid).await?.unwrap_or_default();
            if SharedWallet::payload_of(&raw) != Some(*commitment) {
                continue;
            }
            let height = match btc.tx_status(&c.txid).await? {
                Some(TxStatus::Confirmed { height, .. }) => height,
                _ => u32::MAX,
            };
            if best.is_none_or(|(h, _)| height > h) {
                best = Some((height, c.txid));
            }
        }
        Ok(best.map(|(_, t)| t))
    }

    /// Sends the first chain head, then registers it once it is mined.
    async fn register(&self, net: &dyn ProtocolNetwork, s: &mut State) -> anyhow::Result<()> {
        let btc = self.wallet.btc();
        // No job without a bond; and on Solana the operator's record, which
        // holds the chain head, is made when it first locks one.
        if net.my_bond().await?.0 == 0 {
            warn!(network = net.name(), "lock a bond on this network to start as an operator");
            return Ok(());
        }
        let commitment = net.chain_head_commitment().await?;
        if s.registration.is_none() {
            s.registration = self.find_registration(&commitment).await?;
        }
        let Some(txid) = s.registration else {
            let sent = self.wallet.send(&[], &commitment).await?;
            info!(network = net.name(), txid = %hex(&sent.txid), "first chain head sent");
            s.registration = Some(sent.txid);
            return Ok(());
        };
        let Some(TxStatus::Confirmed { block, height }) = btc.tx_status(&txid).await? else {
            return Ok(());
        };
        // Mined too far below what the light client holds to reach it: a
        // fresh registration is cheaper than storing every block between.
        if s.feed.lowest().is_none() && s.feed.tip(net, btc).await?.is_none() {
            return Ok(());
        }
        if s.feed.lowest().is_some_and(|low| height + MAX_REACH_DOWN < low) {
            let sent = self.wallet.send(&[], &commitment).await?;
            warn!(network = net.name(), old = %hex(&txid), new = %hex(&sent.txid), "the chain head registration was too old; sent a new one");
            s.registration = Some(sent.txid);
            return Ok(());
        }
        let Some(at) = s.feed.at(net, btc, height).await? else { return Ok(()) };
        anyhow::ensure!(at.hash == block, "Bitcoin moved while the chain head was registered");
        let raw = btc.raw_tx(&txid).await?.ok_or_else(|| anyhow::anyhow!("the chain head transaction is unknown"))?;
        let (siblings, index) = merkle_proof(btc, &block, &txid).await?;
        net.register_chain_head(&at, &raw, &siblings, index, 0, 1).await?;
        s.registration = None;
        info!(network = net.name(), txid = %hex(&txid), "chain head registered");
        Ok(())
    }

    async fn bid(&self, net: &dyn ProtocolNetwork, job: &Job, now: i64) -> anyhow::Result<()> {
        if now >= job.auction_end || job.operator.as_deref() == Some(net.me().as_str()) {
            return Ok(());
        }
        let amount = net.minimum_bid(job.id).await?;
        if amount > self.max_bid {
            return Ok(());
        }
        // A swap: only at a price it accepts, with what its side needs.
        if let Some(swaps) = self.swaps_for(job) {
            let Some(swap) = swaps.swap_of(job.id).await? else { return Ok(()) };
            if !swaps.worth_it(job.id, &swap, &self.wallet).await? {
                return Ok(());
            }
        }
        // A job it cannot pay the Bitcoin fee for would end in a missed duty.
        if !self.wallet.can_pay().await? {
            warn!(network = net.name(), job = job.id, address = %self.wallet.address(), "the Bitcoin wallet cannot pay a fee; not bidding");
            return Ok(());
        }
        let (bond, locked) = net.my_bond().await?;
        if bond - locked < amount {
            warn!(network = net.name(), job = job.id, amount, free = bond - locked, "not enough free bond to bid");
            return Ok(());
        }
        net.bid(job.id, amount).await?;
        if let Some(swaps) = self.swaps_for(job)
            && let Some(swap) = swaps.swap_of(job.id).await?
        {
            swaps.reserve(job.id, &swap).await;
        }
        // From the bid on, another network's task holding the same message
        // waits for this one before sending.
        self.wallet.interested(&tag_payload(&job.tag), net.name()).await;
        info!(network = net.name(), job = job.id, amount, "bid");
        Ok(())
    }

    /// Follows the chain head from the registered one, through the
    /// transactions that spent it. Returns the one that carries `payload`
    /// with the input that spends this network's head, if it was sent, and
    /// the coin at the end of the chain. The next chain head is the output
    /// with the number of the input that spent the head (N23): a
    /// transaction for several networks spends one head per network.
    async fn follow_head(&self, net: &dyn ProtocolNetwork, payload: &[u8; 32]) -> anyhow::Result<(Option<([u8; 32], u32)>, ([u8; 32], u32))> {
        let btc = self.wallet.btc();
        let mut head = net.chain_head().await?.ok_or_else(|| anyhow::anyhow!("no chain head"))?;
        while let Some(next) = btc.spender(&head.0, head.1).await? {
            let raw = btc.raw_tx(&next).await?.ok_or_else(|| anyhow::anyhow!("a spender that is not known"))?;
            let index = input_spending(&raw, head).ok_or_else(|| anyhow::anyhow!("a spender that does not spend the head"))?;
            if SharedWallet::payload_of(&raw) == Some(*payload) {
                return Ok((Some((next, index)), head));
            }
            head = (next, index);
        }
        Ok((None, head))
    }

    /// Where to anchor: one block below Bitcoin's best block, so that a
    /// reorganisation of one block does not take the anchor away, unless
    /// that block is too old to anchor at.
    async fn anchor_at(&self, net: &dyn ProtocolNetwork, s: &mut State, now: i64, max_age: i64) -> anyhow::Result<Option<BlockRef>> {
        let btc = self.wallet.btc();
        let Some(best) = s.feed.tip(net, btc).await? else { return Ok(None) };
        if let Some(below) = s.feed.at(net, btc, best.height - 1).await?
            && let Some(stored) = net.stored_block(&below).await?
            && now - (stored.time as i64) <= max_age
        {
            return Ok(Some(below));
        }
        Ok(Some(best))
    }

    /// Moves the chain head past a mined transaction that spent it when none
    /// of its jobs can still prove that transaction (N25). Otherwise every
    /// later proof would be refused: each must spend the registered head.
    async fn clear_head(&self, net: &dyn ProtocolNetwork, s: &mut State, now: i64) -> anyhow::Result<()> {
        let btc = self.wallet.btc();
        let Some(head) = net.chain_head().await? else { return Ok(()) };
        let Some(next) = btc.spender(&head.0, head.1).await? else { return Ok(()) };
        let Some(TxStatus::Confirmed { block, height }) = btc.tx_status(&next).await? else {
            return Ok(());
        };
        let raw = btc.raw_tx(&next).await?.ok_or_else(|| anyhow::anyhow!("a spender that is not known"))?;
        let payload = SharedWallet::payload_of(&raw);
        let head_index = input_spending(&raw, head).ok_or_else(|| anyhow::anyhow!("a spender that does not spend the head"))?;
        let me = net.me();
        for id in &s.jobs {
            let job = net.job(*id).await?;
            let usable = job.status == JobStatus::Assigned
                && job.operator.as_deref() == Some(me.as_str())
                && now <= job.deadline
                && job.anchor.is_none_or(|a| height > a.height && height <= a.height + PROOF_RANGE);
            if usable && payload == Some(tag_payload(&job.tag)) {
                return Ok(());
            }
        }
        let Some(at) = s.feed.at(net, btc, height).await? else { return Ok(()) };
        anyhow::ensure!(at.hash == block, "Bitcoin moved while the chain head was moved");
        let (siblings, index) = merkle_proof(btc, &block, &next).await?;
        net.advance_chain_head(&at, &raw, &siblings, index, head_index).await?;
        warn!(network = net.name(), txid = %hex(&next), "chain head moved past a transaction no job can use");
        Ok(())
    }

    async fn duty(&self, net: &dyn ProtocolNetwork, s: &mut State, job: &Job, now: i64) -> anyhow::Result<()> {
        let payload = tag_payload(&job.tag);
        // Past the deadline no proof is accepted (D27): nothing to send.
        if now > job.deadline {
            self.wallet.forget(&payload, net.name()).await;
            return Ok(());
        }
        let btc = self.wallet.btc();
        // The swap, when the job is one of Conversion's.
        let swap = match self.swaps_for(job) {
            Some(swaps) => swaps.swap_of(job.id).await?,
            None => None,
        };
        let anchor = match job.anchor {
            Some(a) => a,
            None => {
                // A buy's anchor must be young enough to lock the coin on.
                let max_age = if swap.as_ref().is_some_and(|w| w.side == Side::Buy) { BUY_ANCHOR_AGE } else { ANCHOR_BELOW_BEST_FOR };
                let Some(at) = self.anchor_at(net, s, now, max_age).await? else { return Ok(()) };
                net.anchor_job(job.id, &at).await?;
                info!(network = net.name(), job = job.id, height = at.height, "anchored");
                at
            }
        };
        let best = btc.tip_height().await?;
        // A buy's key is known before its receipt is looked for or replaced,
        // after a restart too.
        if let (Some(swaps), Some(w)) = (self.swaps_for(job), &swap)
            && w.side == Side::Buy
        {
            swaps.key(&self.wallet, w.id)?;
        }
        let (txid, head_index) = match self.follow_head(net, &payload).await? {
            (Some(found), _) => {
                // Out already, perhaps sent by another network's task with
                // this network's head in it: nothing to wait for.
                self.wallet.forget(&payload, net.name()).await;
                found
            }
            (None, head) => {
                // A transaction mined from here would be past the proof range.
                if best >= anchor.height + PROOF_RANGE {
                    self.wallet.forget(&payload, net.name()).await;
                    warn!(network = net.name(), job = job.id, "too late to send: the proof range has passed");
                    return Ok(());
                }
                let (sent, index) = if let Some(swaps) = self.swaps_for(job) {
                    // A swap's transaction carries what the swap needs, and
                    // goes alone: its tag is its own.
                    let swap = swap.as_ref().ok_or_else(|| anyhow::anyhow!("job {} has no swap", job.id))?;
                    match swaps.plan(swap, &anchor, best, &self.wallet).await? {
                        Plan::Wait => return Ok(()),
                        Plan::Send { inputs, outputs } => {
                            (self.wallet.send_with(&[head_spend(head.0, head.1)], &inputs, &payload, &outputs).await?, 0)
                        }
                    }
                } else {
                    // Sent now, with the other networks that carry the same
                    // message, or in a later round once they are ready.
                    let urgent = best + 2 >= anchor.height + PROOF_RANGE;
                    let Some(ready) = self.wallet.ready(&payload, net.name(), head_spend(head.0, head.1), best, urgent).await? else {
                        return Ok(());
                    };
                    ready
                };
                info!(network = net.name(), job = job.id, txid = %hex(&sent.txid), rate = sent.rate, "tagged transaction sent");
                s.waiting.insert(payload, Waiting { sent, since: best });
                (sent.txid, index)
            }
        };
        let Some(TxStatus::Confirmed { block, height }) = btc.tx_status(&txid).await? else {
            let sell = swap.as_ref().is_some_and(|w| w.side == Side::Sell);
            return self.bump_if_stuck(net, s, job, &payload, txid, anchor, best, sell).await;
        };
        s.waiting.remove(&payload);
        if height <= anchor.height || height > anchor.height + PROOF_RANGE {
            warn!(network = net.name(), job = job.id, height, anchor = anchor.height, "the transaction is outside the proof range");
            return Ok(());
        }
        let top = height + job.confirmations as u32 - 1;
        if best < top {
            return Ok(());
        }
        // Both walks of the proof need every block from the anchor up. After
        // a restart, or a jump over a long gap, the light client may hold
        // only the newest ones: bring it up to date first, then fill it down
        // to the anchor, so no later jump leaves a hole between them.
        if s.feed.tip(net, btc).await?.is_none() {
            return Ok(());
        }
        let Some(stored_anchor) = s.feed.at(net, btc, anchor.height).await? else { return Ok(()) };
        anyhow::ensure!(stored_anchor == anchor, "the anchor is not the real block at its height");
        let (Some(proof_block), Some(tip)) = (s.feed.at(net, btc, height).await?, s.feed.at(net, btc, top).await?) else {
            return Ok(());
        };
        anyhow::ensure!(proof_block.hash == block, "Bitcoin moved while the proof was built");
        let raw_tx = btc.raw_tx(&txid).await?.ok_or_else(|| anyhow::anyhow!("the tagged transaction is unknown"))?;
        let (siblings, tx_index) = merkle_proof(btc, &block, &txid).await?;
        let prev_epoch_time = crossing(&[anchor, tip]);
        let tag_index = tag_index(&raw_tx).ok_or_else(|| anyhow::anyhow!("the tagged transaction carries no payload"))?;
        let proof = Proof { proof_block, tip, prev_epoch_time, raw_tx, siblings, tx_index, head_index, tag_index };
        net.prove_job(job.id, &proof).await?;
        info!(network = net.name(), job = job.id, txid = %hex(&txid), "proven");
        Ok(())
    }

    /// Replaces a tagged transaction that has waited `BUMP_AFTER` blocks
    /// with one that pays more, while it can still land in the proof range.
    #[allow(clippy::too_many_arguments)]
    /// Follows a swap after its duty; once it is over, forgets the job.
    async fn follow_swap(&self, net: &dyn ProtocolNetwork, s: &mut State, job: &Job, now: i64) {
        let Some(swaps) = self.swaps_for(job) else { return };
        match swaps.follow_up(job, now, self.wallet.btc()).await {
            Ok(true) => {
                swaps.release(job.id).await;
                if !matches!(job.status, JobStatus::Assigned | JobStatus::Proven) {
                    s.jobs.remove(&job.id);
                }
            }
            Ok(false) => {}
            Err(e) => warn!(network = net.name(), job = job.id, error = %redact(&e), "following the swap failed"),
        }
    }

    /// A sell's transaction still waiting when its payment would come too
    /// late (`sell`) is replaced by one without the payment.
    async fn bump_if_stuck(&self, net: &dyn ProtocolNetwork, s: &mut State, job: &Job, payload: &[u8; 32], txid: [u8; 32], anchor: BlockRef, best: u32, sell: bool) -> anyhow::Result<()> {
        // Sent before a restart, or by the task of another network for a
        // message they share, or replaced by it: count from now.
        if s.waiting.get(payload).is_none_or(|w| w.sent.txid != txid) {
            let rate = self.wallet.btc().fee_rate(2).await?;
            s.waiting.insert(*payload, Waiting { sent: Sent { txid, rate }, since: best });
        }
        let w = &s.waiting[payload];
        let late = best + SELL_MARGIN >= anchor.height + PROOF_RANGE;
        if sell && late {
            if best + 1 >= anchor.height + PROOF_RANGE + 1 {
                return Ok(());
            }
            let min_rate = w.sent.rate * BUMP_FACTOR;
            let sent = self.wallet.replace(&txid, payload, min_rate, false).await?;
            warn!(network = net.name(), job = job.id, new = %hex(&sent.txid), "a sell's payment would come too late; replaced by a transaction without it");
            s.waiting.insert(*payload, Waiting { sent, since: best });
            return Ok(());
        }
        if best < w.since + BUMP_AFTER || best + 1 >= anchor.height + PROOF_RANGE {
            return Ok(());
        }
        // A replacement must pay for what it evicts too (BIP125 rule 3): the
        // waiting transactions of later jobs, on any network, may build on
        // this one. Counting all waiting ones may pay a little more than
        // needed, never less.
        let others = self.wallet.waiting_rates_except(&txid).await?;
        let min_rate = (w.sent.rate * BUMP_FACTOR).max(w.sent.rate + others + 1.0);
        let sent = match self.wallet.replace(&txid, payload, min_rate, true).await {
            Ok(sent) => sent,
            Err(e) => {
                // The next try pays more, even if this one was refused.
                if let Some(w) = s.waiting.get_mut(payload) {
                    w.sent.rate = min_rate;
                }
                return Err(e);
            }
        };
        info!(network = net.name(), job = job.id, old = %hex(&txid), new = %hex(&sent.txid), rate = sent.rate, "replaced a waiting transaction to pay more");
        s.waiting.insert(*payload, Waiting { sent, since: best });
        Ok(())
    }
}

#[async_trait]
impl Worker for Operator {
    fn role(&self) -> Role {
        Role::Operator
    }

    async fn round(&self, net: &dyn ProtocolNetwork) -> anyhow::Result<()> {
        let mut s = self.state.lock().await;
        let s = &mut *s;
        let now = net.now().await?;
        let me = net.me();

        let registered = net.chain_head().await?.is_some();
        if !registered && let Err(e) = self.register(net, s).await {
            warn!(network = net.name(), error = %redact(&e), "registering the chain head failed");
        }

        loop {
            let jobs = net.jobs_after(s.cursor, 50).await?;
            let Some(last) = jobs.last() else { break };
            s.cursor = last.id;
            // Jobs open for bids, and after a restart the jobs it already won.
            s.jobs.extend(
                jobs.iter()
                    .filter(|j| match j.status {
                        JobStatus::Auction => true,
                        JobStatus::Assigned | JobStatus::Proven => j.operator.as_deref() == Some(me.as_str()),
                        // A swap's coin may still be to collect.
                        // A swap may still have its coin to collect or take back.
                        JobStatus::Settled | JobStatus::Slashed => j.operator.as_deref() == Some(me.as_str()) && self.swaps_for(j).is_some(),
                        _ => false,
                    })
                    .map(|j| j.id),
            );
        }

        // Questions first: an unanswered one slashes a proven job (D90).
        let count = net.challenge_count().await?;
        for id in s.challenge_cursor + 1..=count {
            if let Some(c) = net.challenge(id).await?
                && net.job(c.job_id).await?.operator.as_deref() == Some(me.as_str())
            {
                s.challenges.insert(id, None);
            }
        }
        s.challenge_cursor = count;
        for id in s.challenges.keys().copied().collect::<Vec<_>>() {
            let shown = s.challenges.get_mut(&id).unwrap();
            match defend(net, self.wallet.btc(), id, shown, now).await {
                Ok(true) => {
                    s.challenges.remove(&id);
                }
                Ok(false) => {}
                Err(e) => warn!(network = net.name(), challenge = id, error = %redact(&e), "answering a challenge failed"),
            }
        }

        if registered && let Err(e) = self.clear_head(net, s, now).await {
            warn!(network = net.name(), error = %redact(&e), "moving the chain head failed");
        }

        // In id order: a job's transaction spends the one of the job before.
        for id in s.jobs.clone() {
            let job = net.job(id).await?;
            let mine = job.operator.as_deref() == Some(me.as_str());
            // Tell the wallet which messages this network may soon carry, so
            // that a message for two networks goes out once.
            let payload = tag_payload(&job.tag);
            // A won job's duty says when it is ready, and forgets the message
            // once its transaction is out.
            match job.status {
                JobStatus::Auction if mine => self.wallet.interested(&payload, net.name()).await,
                JobStatus::Assigned if mine => {}
                _ => self.wallet.forget(&payload, net.name()).await,
            }
            let swap_job = mine && self.swaps_for(&job).is_some();
            let outcome = match job.status {
                JobStatus::Auction if registered => self.bid(net, &job, now).await,
                JobStatus::Auction => Ok(()),
                JobStatus::Assigned if mine => {
                    let duty = self.duty(net, s, &job, now).await;
                    // Past the deadline a swap still has its coin to follow.
                    if swap_job && now > job.deadline {
                        self.follow_swap(net, s, &job, now).await;
                    }
                    duty
                }
                JobStatus::Proven if mine => {
                    // A swap first: a buy's user gets the coin at once, the
                    // rest waits for the lock like the settlement.
                    if swap_job {
                        self.follow_swap(net, s, &job, now).await;
                    }
                    if now >= job.lock_end && job.open_challenges == 0 {
                        net.settle(id).await.inspect(|_| info!(network = net.name(), job = id, "settled"))
                    } else {
                        Ok(())
                    }
                }
                // Settled, slashed or expired: a swap may still have its coin
                // to collect or take back.
                _ if swap_job => {
                    self.follow_swap(net, s, &job, now).await;
                    Ok(())
                }
                _ => {
                    if let Some(swaps) = self.swaps_for(&job) {
                        swaps.release(id).await;
                    }
                    s.jobs.remove(&id);
                    Ok(())
                }
            };
            if let Err(e) = outcome {
                warn!(network = net.name(), job = id, error = %redact(&e), "working on a job failed");
            }
        }

        if net.my_credit().await? > 0 {
            net.withdraw_credit().await?;
        }
        Ok(())
    }
}
