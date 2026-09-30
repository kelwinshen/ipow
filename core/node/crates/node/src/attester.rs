//! The attester role (spec section 7.3). It locks x in the operator's place
//! for a proven job and earns up to 40% of the escrow fee, less the later
//! it steps in (D42, D96). If the proof is shown false its x is slashed, so
//! it attests only a proof whose blocks are in real Bitcoin's best chain,
//! and defends that proof like the operator (D99).

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use async_trait::async_trait;
use ipow_bitcoin::view::BitcoinView;
use ipow_protocol_core::network::ProtocolNetwork;
use ipow_protocol_core::settings::Role;
use ipow_protocol_core::types::{Amount, BlockRef, Job, JobStatus};
use tokio::sync::Mutex;
use tracing::{info, warn};

use crate::bitcoin::at_real_places;
use crate::defence::defend;
use crate::secrets::redact;
use crate::supervisor::Worker;

pub struct Attester {
    btc: Arc<dyn BitcoinView>,
    /// The largest x it locks for one job.
    max_lock: Amount,
    state: Mutex<State>,
}

#[derive(Default)]
struct State {
    cursor: u64,
    /// Jobs it may attest or has attested.
    jobs: BTreeSet<u64>,
    challenge_cursor: u64,
    /// Open challenges of jobs it attested.
    challenges: BTreeMap<u64, Option<BlockRef>>,
}

impl Attester {
    pub fn new(btc: Arc<dyn BitcoinView>, max_lock: Amount) -> Self {
        Attester { btc, max_lock, state: Mutex::new(State::default()) }
    }

    async fn consider(&self, net: &dyn ProtocolNetwork, job: &Job, now: i64) -> anyhow::Result<()> {
        if job.attester.is_some() || now >= job.lock_end || job.open_challenges > 0 || job.escrow > self.max_lock {
            return Ok(());
        }
        // The tip is on top of the proof, and the proof on top of the anchor:
        // a real tip means every block of the proof is real. Real blocks are
        // not enough: stated at another height, a real block can make a
        // question about its parent impossible to answer (an epoch that
        // does not start there), and the attester would be slashed. The
        // blocks between them are linked one height apart, so checking the
        // two ends checks them all.
        let (Some(anchor), Some(tip)) = (job.anchor, job.tip) else {
            anyhow::bail!("a proven job without its blocks");
        };
        if !at_real_places(self.btc.as_ref(), &[anchor, tip]).await? {
            return Ok(());
        }
        let (bond, locked) = net.my_bond().await?;
        if bond - locked < job.escrow {
            warn!(network = net.name(), job = job.id, escrow = job.escrow, free = bond - locked, "not enough free bond to attest");
            return Ok(());
        }
        net.attest(job.id).await?;
        info!(network = net.name(), job = job.id, escrow = job.escrow, "attested");
        Ok(())
    }
}

#[async_trait]
impl Worker for Attester {
    fn role(&self) -> Role {
        Role::Attester
    }

    async fn round(&self, net: &dyn ProtocolNetwork) -> anyhow::Result<()> {
        let mut s = self.state.lock().await;
        let s = &mut *s;
        let now = net.now().await?;
        let me = net.me();

        loop {
            let jobs = net.jobs_after(s.cursor, 50).await?;
            let Some(last) = jobs.last() else { break };
            s.cursor = last.id;
            s.jobs.extend(jobs.iter().filter(|j| matches!(j.status, JobStatus::Auction | JobStatus::Assigned | JobStatus::Proven)).map(|j| j.id));
        }

        let count = net.challenge_count().await?;
        for id in s.challenge_cursor + 1..=count {
            if let Some(c) = net.challenge(id).await?
                && net.job(c.job_id).await?.attester.as_deref() == Some(me.as_str())
            {
                s.challenges.insert(id, None);
            }
        }
        s.challenge_cursor = count;
        for id in s.challenges.keys().copied().collect::<Vec<_>>() {
            let shown = s.challenges.get_mut(&id).unwrap();
            match defend(net, self.btc.as_ref(), id, shown, now).await {
                Ok(true) => {
                    s.challenges.remove(&id);
                }
                Ok(false) => {}
                Err(e) => warn!(network = net.name(), challenge = id, error = %redact(&e), "defending an attested proof failed"),
            }
        }

        for id in s.jobs.clone() {
            let job = net.job(id).await?;
            let outcome = match job.status {
                JobStatus::Auction | JobStatus::Assigned => Ok(()),
                JobStatus::Proven if job.attester.as_deref() == Some(me.as_str()) => {
                    // Its x stays locked until someone settles; it does.
                    if now >= job.lock_end && job.open_challenges == 0 {
                        net.settle(id).await.inspect(|_| info!(network = net.name(), job = id, "settled"))
                    } else {
                        Ok(())
                    }
                }
                JobStatus::Proven if job.attester.is_none() && now < job.lock_end => self.consider(net, &job, now).await,
                _ => {
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
