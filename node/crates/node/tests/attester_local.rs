//! The attester on each local network, against a Bitcoin the test holds.

mod common;

use std::sync::Arc;

use ipow_bitcoin::memory::MemoryBitcoin;
use ipow_node::attester::Attester;
use ipow_node::supervisor::Worker;
use ipow_protocol_core::types::{Evidence, JobStatus};
use ipow_testing::{TestChain, World, proven_job};

/// Bitcoin that holds every block the test mined: the proof is real.
fn real(chain: &TestChain) -> Arc<MemoryBitcoin> {
    let btc = Arc::new(MemoryBitcoin::default());
    for (height, header, txs) in &chain.mined {
        btc.add(*height, *header, txs);
    }
    btc
}

async fn attests_a_real_proof_and_is_paid(w: &dyn World) {
    let (operator, attester, u) = (w.operator(), w.guardian(), w.unit());
    let (chain, ..) = proven_job(w).await;
    let a = Attester::new(real(&chain), u);
    attester.lock_bond(2 * u).await.unwrap();

    a.round(attester).await.unwrap();
    let job = operator.job(1).await.unwrap();
    assert_eq!(job.attester.as_deref(), Some(attester.me().as_str()));
    assert_eq!(attester.my_bond().await.unwrap(), (2 * u, u));
    // The operator's bond is free for other jobs (D42).
    assert_eq!(operator.my_bond().await.unwrap(), (3 * u, 0));

    let wait = job.lock_end - operator.now().await.unwrap() + 1;
    w.increase_time(wait as u64).await;
    a.round(attester).await.unwrap();
    assert_eq!(operator.job(1).await.unwrap().status, JobStatus::Settled);
    assert_eq!(attester.my_bond().await.unwrap(), (2 * u, 0));
    assert_eq!(attester.my_credit().await.unwrap(), 0, "withdrawn in the same round");
}

async fn does_not_attest_a_proof_bitcoin_does_not_have(w: &dyn World) {
    let (operator, attester, u) = (w.operator(), w.guardian(), w.unit());
    let (chain, anchor, _) = proven_job(w).await;
    // Real Bitcoin stops below the anchor.
    let btc = Arc::new(MemoryBitcoin::default());
    for (height, header, txs) in chain.mined.iter().filter(|(h, ..)| *h < anchor.height) {
        btc.add(*height, *header, txs);
    }
    let a = Attester::new(btc, u);
    attester.lock_bond(2 * u).await.unwrap();
    a.round(attester).await.unwrap();
    assert!(operator.job(1).await.unwrap().attester.is_none());
    assert_eq!(attester.my_bond().await.unwrap(), (2 * u, 0));
}

async fn answers_a_question_about_a_proof_it_attested(w: &dyn World) {
    let (operator, attester, u) = (w.operator(), w.guardian(), w.unit());
    let (chain, anchor, _) = proven_job(w).await;
    let a = Attester::new(real(&chain), u);
    attester.lock_bond(2 * u).await.unwrap();
    a.round(attester).await.unwrap();

    // Someone asks for the anchor's parent. The operator does not answer;
    // the attester does.
    let salt = [2u8; 32];
    operator.seal_note(1, &Evidence::Parent(anchor.hash), &salt).await.unwrap();
    let deposit = operator.parent_deposit(1).await.unwrap();
    let id = operator.ask_parent(1, &salt, deposit).await.unwrap();
    a.round(attester).await.unwrap();
    assert!(attester.challenge(id).await.unwrap().is_none());
    assert_eq!(operator.job(1).await.unwrap().parents_shown, 1);
}

on_every_network!(
    attests_a_real_proof_and_is_paid,
    does_not_attest_a_proof_bitcoin_does_not_have,
    answers_a_question_about_a_proof_it_attested,
);
