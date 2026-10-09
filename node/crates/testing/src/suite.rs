//! The adapter tests, written once and run on every network: each network's
//! test file calls these with its own `World`.

use ipow_bitcoin::{sha256d, tx};
use ipow_protocol_core::types::{BlockRef, Evidence, JobStatus, Proof};

use crate::{TestChain, World, mine, proven_job, tagged_tx};

pub async fn reads_jobs_bonds_and_time_and_bids(w: &dyn World) {
    let operator = w.operator();
    let u = w.unit();
    assert!(operator.now().await.unwrap() > 0);
    assert_eq!(operator.jobs_after(0, 10).await.unwrap().len(), 0);

    operator.lock_bond(3 * u).await.unwrap();
    assert_eq!(operator.my_bond().await.unwrap(), (3 * u, 0));

    // An application registers and opens a job.
    w.open_job(0).await;

    let jobs = operator.jobs_after(0, 10).await.unwrap();
    assert_eq!(jobs.len(), 1);
    let job = &jobs[0];
    assert_eq!(job.id, 1);
    assert_eq!(job.status, JobStatus::Auction);
    assert_eq!(job.escrow, u);
    assert_eq!(job.escrow_fee, u / 200);
    assert_eq!(job.confirmations, 6);
    assert!(job.operator.is_none());

    // The operator bids and is locked in after a minute with no better bid.
    operator.bid(1, u).await.unwrap();
    assert_eq!(operator.my_bond().await.unwrap(), (3 * u, u));
    w.increase_time(61).await;
    let job = operator.jobs_after(0, 10).await.unwrap().remove(0);
    assert_eq!(job.status, JobStatus::Assigned);
    assert_eq!(job.operator.as_deref(), Some(operator.me().as_str()));
    // 1 day for a window of 30 blocks, from the moment it was locked in (D60).
    assert_eq!(job.deadline - job.auction_end, 24 * 3600);
    assert!(operator.jobs_after(1, 10).await.unwrap().is_empty());
}

/// The whole life of a job through the adapter: headers streamed, a chain
/// head registered, the job anchored and proven, a guardian's question
/// answered, an attester stepping in, and the job settled and paid.
pub async fn carries_a_job_from_bid_to_payment(w: &dyn World) {
    let (operator, guardian, u) = (w.operator(), w.guardian(), w.unit());
    let mut chain = TestChain::start(operator, w.latest_time().await).await;

    // The operator's first chain head: a coin made by a transaction that
    // names it.
    operator.lock_bond(3 * u).await.unwrap();
    let first = tagged_tx(([9; 32], 0), &operator.chain_head_commitment().await.unwrap());
    let block = chain.add(operator, w.latest_time().await + 1, &[&first]).await;
    let (siblings, index) = chain.proof_of(&block, &first);
    operator.register_chain_head(&block, &first, &siblings, index, 0, 1).await.unwrap();
    assert_eq!(operator.chain_head().await.unwrap(), Some((tx::txid(&first), 0)));

    w.open_job(0).await;
    operator.bid(1, u).await.unwrap();
    w.increase_time(61).await;

    // The duty: a fresh anchor, the tagged transaction in the next block,
    // and 5 blocks on top.
    let anchor = chain.add(operator, w.latest_time().await + 1, &[]).await;
    operator.anchor_job(1, &anchor).await.unwrap();
    assert_eq!(operator.job(1).await.unwrap().anchor, Some(anchor));
    let head = operator.chain_head().await.unwrap().unwrap();
    let raw = tagged_tx(head, &operator.tag_payload(&[1; 32]).await.unwrap());
    let proof_block = chain.add(operator, w.latest_time().await + 1, &[&raw]).await;
    for _ in 0..5 {
        chain.add(operator, w.latest_time().await + 1, &[]).await;
    }
    let (siblings, tx_index) = chain.proof_of(&proof_block, &raw);
    let proof = Proof { proof_block, tip: chain.tip, prev_epoch_time: 0, raw_tx: raw.clone(), siblings, tx_index, head_index: 0, tag_index: 1 };
    operator.prove_job(1, &proof).await.unwrap();
    let job = operator.job(1).await.unwrap();
    assert_eq!(job.status, JobStatus::Proven);
    assert_eq!(job.proof_block, Some(proof_block));
    assert_eq!(operator.chain_head().await.unwrap(), Some((tx::txid(&raw), 0)));
    assert!(operator.stored_block(&proof_block).await.unwrap().is_some());

    // A guardian asks for the parent of the anchor, and the operator shows
    // it: the guardian's deposit goes to the operator (D90).
    let salt = [5u8; 32];
    guardian.seal_note(1, &Evidence::Parent(anchor.hash), &salt).await.unwrap();
    let deposit = guardian.parent_deposit(1).await.unwrap();
    let id = guardian.ask_parent(1, &salt, deposit).await.unwrap();
    let asked = guardian.challenge(id).await.unwrap().unwrap();
    assert_eq!(asked.asked, Some(anchor));
    assert_eq!(operator.job(1).await.unwrap().deepest, Some(anchor));
    assert_eq!(operator.job(1).await.unwrap().open_challenges, 1);
    operator.show_parent(id, 0).await.unwrap();
    assert!(guardian.challenge(id).await.unwrap().is_none());
    assert_eq!(operator.my_credit().await.unwrap(), deposit);
    // The next question is for the block before the anchor.
    let job = operator.job(1).await.unwrap();
    assert_eq!(job.parents_shown, 1);
    assert_eq!(job.deepest.unwrap().height, anchor.height - 1);

    // An attester locks x from its own bond, and frees the operator's (D42).
    guardian.lock_bond(2 * u).await.unwrap();
    guardian.attest(1).await.unwrap();
    assert_eq!(guardian.my_bond().await.unwrap(), (2 * u, u));
    assert_eq!(operator.my_bond().await.unwrap(), (3 * u, 0));
    assert_eq!(operator.job(1).await.unwrap().attester.as_deref(), Some(guardian.me().as_str()));

    // After the lock both are paid, and withdraw.
    let wait = operator.job(1).await.unwrap().lock_end - operator.now().await.unwrap() + 1;
    w.increase_time(wait as u64).await;
    operator.settle(1).await.unwrap();
    assert_eq!(operator.job(1).await.unwrap().status, JobStatus::Settled);
    assert_eq!(guardian.my_bond().await.unwrap(), (2 * u, 0));
    assert!(guardian.my_credit().await.unwrap() > 0);
    assert!(operator.my_credit().await.unwrap() > deposit);
    for n in [operator, guardian] {
        n.withdraw_credit().await.unwrap();
        assert_eq!(n.my_credit().await.unwrap(), 0);
    }
    operator.withdraw_bond(3 * u).await.unwrap();
    assert_eq!(operator.my_bond().await.unwrap(), (0, 0));
}

/// A guardian reports an operator that let its deadline pass (D53).
pub async fn slashes_a_missed_duty(w: &dyn World) {
    let (operator, guardian, u) = (w.operator(), w.guardian(), w.unit());
    operator.lock_bond(3 * u).await.unwrap();
    w.open_job(0).await;
    operator.bid(1, u).await.unwrap();
    w.increase_time(61).await;

    let salt = [6u8; 32];
    guardian.seal_note(1, &Evidence::MissedDuty, &salt).await.unwrap();
    // Too early: the deadline has not passed. The error names the reason.
    let early = guardian.report_missed_duty(1, &salt).await.unwrap_err().to_string();
    assert!(early.contains("DeadlineNotPassed"), "{early}");
    let wait = operator.job(1).await.unwrap().deadline - operator.now().await.unwrap() + 1;
    w.increase_time(wait as u64).await;
    guardian.report_missed_duty(1, &salt).await.unwrap();

    assert_eq!(operator.job(1).await.unwrap().status, JobStatus::Slashed);
    // 20% of the escrow to the guardian.
    assert_eq!(guardian.my_credit().await.unwrap(), u / 5);
    assert_eq!(operator.my_bond().await.unwrap(), (2 * u, 0));
}

/// A node that starts far from the epoch start jumps to a recent block, and
/// stores older blocks one at a time below it.
pub async fn jumps_and_extends_back(w: &dyn World) {
    let node = w.operator();
    let now = w.latest_time().await;
    let chain = TestChain::start(node, now).await;
    let epoch_start = chain.first;

    let parent = mine([3; 32], [4; 32], now - 120);
    let child = mine(sha256d(&parent), [4; 32], now - 60);
    node.jump(&child, &epoch_start, 100).await.unwrap();
    let child_ref = BlockRef { hash: sha256d(&child), height: 100, epoch_time: epoch_start.epoch_time };
    assert!(node.stored_block(&child_ref).await.unwrap().is_some());

    node.extend_back(&child_ref, &parent, 0).await.unwrap();
    let parent_ref = BlockRef { hash: sha256d(&parent), height: 99, ..child_ref };
    let stored = node.stored_block(&parent_ref).await.unwrap().unwrap();
    assert_eq!(stored.prev_hash, [3; 32]);
}

/// A guardian shows a heavier branch from the anchor. Both sides add blocks
/// while the lock runs; the heavier branch when it ends wins (D81).
pub async fn settles_a_fork_challenge_by_work(w: &dyn World) {
    let (operator, guardian, u) = (w.operator(), w.guardian(), w.unit());
    let (mut chain, anchor, proof_block) = proven_job(w).await;
    let operator_tip = chain.tip;

    // 7 blocks on the anchor against the operator's 6.
    chain.tip = anchor;
    let guardian_block = chain.add(guardian, w.latest_time().await + 1, &[]).await;
    for _ in 0..6 {
        chain.add(guardian, w.latest_time().await + 1, &[]).await;
    }
    let guardian_tip = chain.tip;
    let salt = [8u8; 32];
    guardian.seal_note(1, &Evidence::Fork(guardian_block.hash), &salt).await.unwrap();
    let deposit = guardian.job(1).await.unwrap().commitment_fee;
    let id = guardian.challenge_fork(1, &proof_block, &guardian_block, &guardian_tip, 0, &salt, deposit).await.unwrap();
    assert_eq!(guardian.challenge(id).await.unwrap().unwrap().deposit, deposit);

    // The operator's branch grows to 8, then the guardian's to 9.
    chain.tip = operator_tip;
    for _ in 0..2 {
        chain.add(operator, w.latest_time().await + 1, &[]).await;
    }
    operator.extend_branch(id, false, &operator_tip, &chain.tip, 0).await.unwrap();
    chain.tip = guardian_tip;
    for _ in 0..2 {
        chain.add(guardian, w.latest_time().await + 1, &[]).await;
    }
    guardian.extend_branch(id, true, &guardian_tip, &chain.tip, 0).await.unwrap();

    // Not before the lock ends.
    assert!(guardian.resolve_challenge(id).await.is_err());
    let wait = operator.job(1).await.unwrap().lock_end - operator.now().await.unwrap() + 1;
    w.increase_time(wait as u64).await;
    guardian.resolve_challenge(id).await.unwrap();
    assert_eq!(operator.job(1).await.unwrap().status, JobStatus::Slashed);
    // 20% of the escrow, and the deposit back.
    assert_eq!(guardian.my_credit().await.unwrap(), u / 5 + deposit);
}

/// Nobody shows the parent a guardian asked for in 12 hours: the proof is
/// false (D81).
pub async fn slashes_an_unanswered_parent_question(w: &dyn World) {
    let (operator, guardian, u) = (w.operator(), w.guardian(), w.unit());
    let (_, anchor, _) = proven_job(w).await;

    let salt = [9u8; 32];
    guardian.seal_note(1, &Evidence::Parent(anchor.hash), &salt).await.unwrap();
    let deposit = guardian.parent_deposit(1).await.unwrap();
    let id = guardian.ask_parent(1, &salt, deposit).await.unwrap();
    assert!(guardian.resolve_challenge(id).await.is_err());
    w.increase_time(12 * 3600).await;
    // Too late to answer.
    assert!(operator.show_parent(id, 0).await.is_err());
    guardian.resolve_challenge(id).await.unwrap();
    assert_eq!(operator.job(1).await.unwrap().status, JobStatus::Slashed);
    assert_eq!(guardian.my_credit().await.unwrap(), u / 5 + deposit);
}
