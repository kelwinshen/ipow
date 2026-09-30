//! The guardian on each local network, against a Bitcoin the test holds.

mod common;

use std::sync::Arc;

use ipow_bitcoin::memory::MemoryBitcoin;
use ipow_bitcoin::sha256d;
use ipow_node::guardian::{Guardian, UNKNOWN_FOR};
use ipow_node::supervisor::Worker;
use ipow_protocol_core::types::{BlockRef, JobStatus};
use ipow_testing::{World, mine, proven_job};

async fn reports_a_missed_duty_and_takes_its_reward(w: &dyn World) {
    let (operator, guardian, u) = (w.operator(), w.guardian(), w.unit());
    operator.lock_bond(3 * u).await.unwrap();
    w.open_job(0).await;
    operator.bid(1, u).await.unwrap();
    w.increase_time(61).await;

    let g = Guardian::new(Arc::new(MemoryBitcoin::default()), u);
    // Before the deadline there is nothing to do.
    g.round(guardian).await.unwrap();
    assert_eq!(operator.job(1).await.unwrap().status, JobStatus::Assigned);

    let wait = operator.job(1).await.unwrap().deadline - operator.now().await.unwrap() + 1;
    w.increase_time(wait as u64).await;
    // A note first, then the report (D47).
    g.round(guardian).await.unwrap();
    assert_eq!(operator.job(1).await.unwrap().status, JobStatus::Assigned);
    g.round(guardian).await.unwrap();
    assert_eq!(operator.job(1).await.unwrap().status, JobStatus::Slashed);
    // Its 20% was paid and withdrawn in the same round.
    assert_eq!(guardian.my_credit().await.unwrap(), 0);
}

async fn challenges_a_proof_on_blocks_bitcoin_does_not_have(w: &dyn World) {
    let (operator, guardian, u) = (w.operator(), w.guardian(), w.unit());
    // The operator's anchor and every block above it exist only in the light
    // client. Real Bitcoin has the blocks up to the anchor's parent (the
    // block of the chain head registration) and its own blocks after it.
    let (chain, anchor, _) = proven_job(w).await;
    let btc = Arc::new(MemoryBitcoin::default());
    for (height, header, txs) in chain.mined.iter().filter(|(h, ..)| *h < anchor.height) {
        btc.add(*height, *header, txs);
    }
    let mut real_tip = chain.mined.iter().find(|(h, ..)| *h == anchor.height - 1).unwrap().1;
    let mut real_height = anchor.height - 1;
    let mut grow_real = |n: u32, time: u32| {
        for i in 0..n {
            let h = mine(sha256d(&real_tip), [0xaa; 32], time + i);
            real_height += 1;
            btc.add(real_height, h, &[]);
            real_tip = h;
        }
    };
    grow_real(12, w.latest_time().await);

    let g = Guardian::new(btc.clone(), u);
    // The proof's block is unknown to Bitcoin; the guardian gives the
    // explorer time to catch up before it acts.
    g.round(guardian).await.unwrap();
    w.increase_time(UNKNOWN_FOR as u64 + 1).await;
    // The anchor is not real: seal a question for its parent, then ask.
    g.round(guardian).await.unwrap();
    g.round(guardian).await.unwrap();
    let job = operator.job(1).await.unwrap();
    assert_eq!(job.open_challenges, 1);

    // The operator shows the parent: a real block. The question is lost.
    operator.show_parent(1, 0).await.unwrap();
    let job = operator.job(1).await.unwrap();
    assert_eq!(job.open_challenges, 0);
    assert_eq!(job.deepest.unwrap().height, anchor.height - 1);

    // So the anchor names a real parent, and real Bitcoin has another child
    // of it: the guardian shows that branch, sealed first.
    g.round(guardian).await.unwrap();
    g.round(guardian).await.unwrap();
    assert_eq!(operator.job(1).await.unwrap().open_challenges, 1);

    // Bitcoin grows; the guardian adds its new blocks to its side (D99).
    grow_real(3, w.latest_time().await);
    g.round(guardian).await.unwrap();
    let c = guardian.challenge(2).await.unwrap().unwrap();
    assert_eq!(c.job_id, 1);
    let newest = BlockRef { hash: sha256d(&real_tip), height: real_height, epoch_time: anchor.epoch_time };
    assert!(guardian.stored_block(&newest).await.unwrap().is_some());

    // When the lock ends the heavier branch wins.
    let wait = operator.job(1).await.unwrap().lock_end - operator.now().await.unwrap() + 1;
    w.increase_time(wait as u64).await;
    g.round(guardian).await.unwrap();
    assert_eq!(operator.job(1).await.unwrap().status, JobStatus::Slashed);
    assert_eq!(guardian.my_credit().await.unwrap(), 0);
}

async fn leaves_a_proof_on_real_bitcoin_alone(w: &dyn World) {
    let (operator, guardian, u) = (w.operator(), w.guardian(), w.unit());
    let (chain, ..) = proven_job(w).await;
    // Every block the operator used is real.
    let btc = Arc::new(MemoryBitcoin::default());
    for (height, header, txs) in &chain.mined {
        btc.add(*height, *header, txs);
    }
    let g = Guardian::new(btc, u);
    for _ in 0..3 {
        g.round(guardian).await.unwrap();
        w.increase_time(UNKNOWN_FOR as u64 + 1).await;
    }
    let job = operator.job(1).await.unwrap();
    assert_eq!(job.open_challenges, 0);
    assert_eq!(job.status, JobStatus::Proven);
    let wait = job.lock_end - operator.now().await.unwrap() + 1;
    w.increase_time(wait as u64).await;
    operator.settle(1).await.unwrap();
}

/// A guardian that restarts while its branch is shown finds its challenge
/// again, keeps adding Bitcoin's new blocks to it, and wins.
async fn keeps_growing_its_branch_after_a_restart(w: &dyn World) {
    let (operator, guardian, u) = (w.operator(), w.guardian(), w.unit());
    let (chain, anchor, _) = proven_job(w).await;
    let btc = Arc::new(MemoryBitcoin::default());
    for (height, header, txs) in chain.mined.iter().filter(|(h, ..)| *h < anchor.height) {
        btc.add(*height, *header, txs);
    }
    let mut real_tip = chain.mined.iter().find(|(h, ..)| *h == anchor.height - 1).unwrap().1;
    let mut real_height = anchor.height - 1;
    let mut grow_real = |n: u32, time: u32| {
        for i in 0..n {
            let h = mine(sha256d(&real_tip), [0xcc; 32], time + i);
            real_height += 1;
            btc.add(real_height, h, &[]);
            real_tip = h;
        }
    };
    grow_real(12, w.latest_time().await);

    // Asked, answered, then the real branch shown: as in the test above.
    let g = Guardian::new(btc.clone(), u);
    g.round(guardian).await.unwrap();
    w.increase_time(UNKNOWN_FOR as u64 + 1).await;
    g.round(guardian).await.unwrap();
    g.round(guardian).await.unwrap();
    operator.show_parent(1, 0).await.unwrap();
    g.round(guardian).await.unwrap();
    g.round(guardian).await.unwrap();
    assert!(guardian.challenge(2).await.unwrap().is_some());

    // A restart: nothing in memory. Bitcoin grows meanwhile.
    let g = Guardian::new(btc.clone(), u);
    grow_real(5, w.latest_time().await);
    g.round(guardian).await.unwrap();
    let newest = BlockRef { hash: sha256d(&real_tip), height: real_height, epoch_time: anchor.epoch_time };
    assert!(guardian.stored_block(&newest).await.unwrap().is_some());
    // No second challenge was opened for the same job.
    assert_eq!(guardian.challenge_count().await.unwrap(), 2);

    let wait = operator.job(1).await.unwrap().lock_end - operator.now().await.unwrap() + 1;
    w.increase_time(wait as u64).await;
    g.round(guardian).await.unwrap();
    assert_eq!(operator.job(1).await.unwrap().status, JobStatus::Slashed);
}

on_every_network!(
    keeps_growing_its_branch_after_a_restart,
    reports_a_missed_duty_and_takes_its_reward,
    challenges_a_proof_on_blocks_bitcoin_does_not_have,
    leaves_a_proof_on_real_bitcoin_alone,
);
