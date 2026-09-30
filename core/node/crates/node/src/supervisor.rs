//! Runs every (role, network) pair as its own task, and restarts it when it
//! fails. In `core/operator` all networks ran in one `select!`, so one that
//! stopped stopped them all; here they are independent.

use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use ipow_protocol_core::network::ProtocolNetwork;
use ipow_protocol_core::settings::Role;
use tokio::sync::watch;
use tokio::task::JoinHandle;
use tracing::{info, warn};

/// One role, the same on every network. A round looks at the network once
/// and does what the role must do now.
#[async_trait]
pub trait Worker: Send + Sync {
    fn role(&self) -> Role;
    async fn round(&self, network: &dyn ProtocolNetwork) -> anyhow::Result<()>;
}

/// How long to wait between rounds, and after a failure.
#[derive(Clone, Copy, Debug)]
pub struct Timing {
    pub interval: Duration,
    /// The first pause after a failure. Each failure in a row doubles it,
    /// up to `max_backoff`.
    pub backoff: Duration,
    pub max_backoff: Duration,
    /// The longest a round may take. A round that hangs, on a network that
    /// never answers, is stopped so the next one can start.
    pub round_timeout: Duration,
}

impl Timing {
    pub fn from_interval(interval: Duration) -> Self {
        Timing { interval, backoff: interval, max_backoff: interval * 20, round_timeout: Duration::from_secs(15 * 60) }
    }
}

/// Starts one task per pair. Each runs until `stop` is set to true.
pub fn start(
    pairs: Vec<(Arc<dyn Worker>, Arc<dyn ProtocolNetwork>)>,
    timing: Timing,
    stop: watch::Receiver<bool>,
) -> Vec<JoinHandle<()>> {
    pairs
        .into_iter()
        .map(|(worker, network)| tokio::spawn(keep_running(worker, network, timing, stop.clone())))
        .collect()
}

/// Runs rounds of one role on one network. A round that fails, or panics,
/// is followed by a pause that grows with each failure in a row.
async fn keep_running(
    worker: Arc<dyn Worker>,
    network: Arc<dyn ProtocolNetwork>,
    timing: Timing,
    mut stop: watch::Receiver<bool>,
) {
    let label = format!("{:?} on {}", worker.role(), network.name());
    info!(task = %label, "started");
    let mut pause = timing.backoff;
    loop {
        if *stop.borrow() {
            break;
        }
        // Each round runs in a task of its own, so a panic stays inside it.
        let (w, n) = (worker.clone(), network.clone());
        let round = tokio::spawn(async move { w.round(n.as_ref()).await });
        let abort = round.abort_handle();
        let outcome = match tokio::time::timeout(timing.round_timeout, round).await {
            Ok(outcome) => outcome,
            Err(_) => {
                abort.abort();
                Ok(Err(anyhow::anyhow!("the round took longer than {:?}", timing.round_timeout)))
            }
        };
        let wait = match outcome {
            Ok(Ok(())) => {
                pause = timing.backoff;
                timing.interval
            }
            Ok(Err(e)) => {
                warn!(task = %label, error = %crate::secrets::redact(&e), wait = ?pause, "round failed");
                let now = pause;
                pause = (pause * 2).min(timing.max_backoff);
                now
            }
            Err(e) => {
                warn!(task = %label, error = %crate::secrets::redact(&e), wait = ?pause, "round panicked");
                let now = pause;
                pause = (pause * 2).min(timing.max_backoff);
                now
            }
        };
        tokio::select! {
            _ = tokio::time::sleep(wait) => {}
            _ = stop.changed() => {}
        }
    }
    info!(task = %label, "stopped");
}

#[cfg(test)]
mod tests {
    use super::*;
    use ipow_protocol_core::types::{Amount, BlockRef, Challenge, Evidence, Job, Proof, StoredBlock};
    use std::sync::atomic::{AtomicUsize, Ordering};

    struct Net(&'static str);

    /// A network for the supervisor's tests: only its name is used.
    #[async_trait]
    impl ProtocolNetwork for Net {
        fn name(&self) -> &str {
            self.0
        }
        fn me(&self) -> String {
            unimplemented!()
        }
        async fn now(&self) -> anyhow::Result<i64> {
            unimplemented!()
        }
        async fn jobs_after(&self, _: u64, _: usize) -> anyhow::Result<Vec<Job>> {
            unimplemented!()
        }
        async fn job(&self, _: u64) -> anyhow::Result<Job> {
            unimplemented!()
        }
        async fn my_bond(&self) -> anyhow::Result<(Amount, Amount)> {
            unimplemented!()
        }
        async fn my_credit(&self) -> anyhow::Result<Amount> {
            unimplemented!()
        }
        async fn parent_deposit(&self, _: u64) -> anyhow::Result<Amount> {
            unimplemented!()
        }
        async fn advance_chain_head(&self, _: &BlockRef, _: &[u8], _: &[[u8; 32]], _: u64, _: u32) -> anyhow::Result<()> {
            unimplemented!()
        }
        async fn minimum_bid(&self, _: u64) -> anyhow::Result<Amount> {
            unimplemented!()
        }
        async fn challenge_count(&self) -> anyhow::Result<u64> {
            unimplemented!()
        }
        async fn challenge(&self, _: u64) -> anyhow::Result<Option<Challenge>> {
            unimplemented!()
        }
        async fn stored_block(&self, _: &BlockRef) -> anyhow::Result<Option<StoredBlock>> {
            unimplemented!()
        }
        async fn add_epoch_start(&self, _: &[[u8; 80]], _: u32) -> anyhow::Result<()> {
            unimplemented!()
        }
        async fn jump(&self, _: &[u8; 80], _: &BlockRef, _: u32) -> anyhow::Result<()> {
            unimplemented!()
        }
        async fn extend(&self, _: &BlockRef, _: &[[u8; 80]]) -> anyhow::Result<()> {
            unimplemented!()
        }
        async fn extend_back(&self, _: &BlockRef, _: &[u8; 80], _: u32) -> anyhow::Result<()> {
            unimplemented!()
        }
        async fn lock_bond(&self, _: Amount) -> anyhow::Result<()> {
            unimplemented!()
        }
        async fn withdraw_bond(&self, _: Amount) -> anyhow::Result<()> {
            unimplemented!()
        }
        async fn bid(&self, _: u64, _: Amount) -> anyhow::Result<()> {
            unimplemented!()
        }
        async fn chain_head_commitment(&self) -> anyhow::Result<[u8; 32]> {
            unimplemented!()
        }
        async fn chain_head(&self) -> anyhow::Result<Option<([u8; 32], u32)>> {
            unimplemented!()
        }
        async fn tag_payload(&self, _: &[u8; 32]) -> anyhow::Result<[u8; 32]> {
            unimplemented!()
        }
        async fn register_chain_head(&self, _: &BlockRef, _: &[u8], _: &[[u8; 32]], _: u64, _: u32, _: u32) -> anyhow::Result<()> {
            unimplemented!()
        }
        async fn anchor_job(&self, _: u64, _: &BlockRef) -> anyhow::Result<()> {
            unimplemented!()
        }
        async fn prove_job(&self, _: u64, _: &Proof) -> anyhow::Result<()> {
            unimplemented!()
        }
        async fn settle(&self, _: u64) -> anyhow::Result<()> {
            unimplemented!()
        }
        async fn attest(&self, _: u64) -> anyhow::Result<()> {
            unimplemented!()
        }
        async fn seal_note(&self, _: u64, _: &Evidence, _: &[u8; 32]) -> anyhow::Result<()> {
            unimplemented!()
        }
        async fn report_missed_duty(&self, _: u64, _: &[u8; 32]) -> anyhow::Result<()> {
            unimplemented!()
        }
        async fn ask_parent(&self, _: u64, _: &[u8; 32], _: Amount) -> anyhow::Result<u64> {
            unimplemented!()
        }
        async fn show_parent(&self, _: u64, _: u32) -> anyhow::Result<()> {
            unimplemented!()
        }
        async fn challenge_fork(&self, _: u64, _: &BlockRef, _: &BlockRef, _: &BlockRef, _: u32, _: &[u8; 32], _: Amount) -> anyhow::Result<u64> {
            unimplemented!()
        }
        async fn extend_branch(&self, _: u64, _: bool, _: &BlockRef, _: &BlockRef, _: u32) -> anyhow::Result<()> {
            unimplemented!()
        }
        async fn resolve_challenge(&self, _: u64) -> anyhow::Result<()> {
            unimplemented!()
        }
        async fn withdraw_credit(&self) -> anyhow::Result<()> {
            unimplemented!()
        }
    }

    enum Behaviour {
        Works,
        Fails,
        Panics,
    }

    struct Counting {
        rounds: AtomicUsize,
        behaviour: Behaviour,
    }

    #[async_trait]
    impl Worker for Counting {
        fn role(&self) -> Role {
            Role::Guardian
        }
        async fn round(&self, _: &dyn ProtocolNetwork) -> anyhow::Result<()> {
            self.rounds.fetch_add(1, Ordering::SeqCst);
            match self.behaviour {
                Behaviour::Works => Ok(()),
                Behaviour::Fails => anyhow::bail!("the network did not answer"),
                Behaviour::Panics => panic!("a bug"),
            }
        }
    }

    fn fast() -> Timing {
        Timing {
            interval: Duration::from_millis(5),
            backoff: Duration::from_millis(5),
            max_backoff: Duration::from_millis(20),
            round_timeout: Duration::from_secs(5),
        }
    }

    #[tokio::test]
    async fn keeps_a_healthy_task_running_while_another_fails_or_panics() {
        let works = Arc::new(Counting { rounds: AtomicUsize::new(0), behaviour: Behaviour::Works });
        let fails = Arc::new(Counting { rounds: AtomicUsize::new(0), behaviour: Behaviour::Fails });
        let panics = Arc::new(Counting { rounds: AtomicUsize::new(0), behaviour: Behaviour::Panics });
        let (tx, rx) = watch::channel(false);
        let handles = start(
            vec![
                (works.clone(), Arc::new(Net("a"))),
                (fails.clone(), Arc::new(Net("b"))),
                (panics.clone(), Arc::new(Net("c"))),
            ],
            fast(),
            rx,
        );
        tokio::time::sleep(Duration::from_millis(200)).await;
        tx.send(true).unwrap();
        for h in handles {
            h.await.unwrap();
        }
        // The healthy task ran many rounds; the failing ones were retried.
        assert!(works.rounds.load(Ordering::SeqCst) > 10);
        assert!(fails.rounds.load(Ordering::SeqCst) >= 3);
        assert!(panics.rounds.load(Ordering::SeqCst) >= 3);
        // Failures back off: fewer rounds than the healthy task.
        assert!(fails.rounds.load(Ordering::SeqCst) < works.rounds.load(Ordering::SeqCst));
    }

    #[tokio::test]
    async fn stops_every_task_when_asked() {
        let works = Arc::new(Counting { rounds: AtomicUsize::new(0), behaviour: Behaviour::Works });
        let (tx, rx) = watch::channel(false);
        let handles = start(vec![(works.clone(), Arc::new(Net("a")))], Timing::from_interval(Duration::from_secs(3600)), rx);
        tokio::time::sleep(Duration::from_millis(20)).await;
        tx.send(true).unwrap();
        for h in handles {
            tokio::time::timeout(Duration::from_secs(1), h).await.unwrap().unwrap();
        }
        assert_eq!(works.rounds.load(Ordering::SeqCst), 1);
    }
}
