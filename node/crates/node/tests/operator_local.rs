//! The operator on each local network, against a Bitcoin the test holds and
//! mines.

mod common;

use std::sync::Arc;

use ipow_bitcoin::memory::MemoryBitcoin;
use ipow_bitcoin::sha256d;
use ipow_bitcoin::view::BitcoinView;
use ipow_bitcoin::tx;
use ipow_bitcoin::wallet::Wallet;
use ipow_node::operator::Operator;
use ipow_node::supervisor::Worker;
use ipow_node::wallet::SharedWallet;
use ipow_protocol_core::types::{BlockRef, Evidence, JobStatus};
use ipow_testing::{World, mine};

/// A key made for tests only, holding nothing: the secret 1.
const WIF: &str = "KwDiBf89QgGbjEhKnhXJuH7LrciVrZi3qYjgd9M7rFU73sVHnoWn";

/// Bitcoin with 6 blocks, then one that pays the operator's wallet.
fn bitcoin(now: u32, wallet: &Wallet) -> Arc<MemoryBitcoin> {
    let btc = Arc::new(MemoryBitcoin::default());
    let mut prev = [0u8; 32];
    for i in 0..6u32 {
        let h = mine(prev, sha256d(&i.to_le_bytes()), now - 3600 + i * 600);
        btc.add(i, h, &[]);
        prev = sha256d(&h);
    }
    let funding = tx::build(&[([7; 32], 0)], &[tx::Output { value: 1_000_000, script: wallet.script().to_vec() }]);
    let (root, _) = ipow_bitcoin::merkle::root_and_proof(&[tx::txid(&funding)], 0);
    btc.add(6, mine(prev, root, now - 60), &[funding]);
    btc
}

/// An operator with 3 coins of bond and a registered chain head.
async fn registered(w: &dyn World) -> (Operator, Arc<MemoryBitcoin>) {
    let wallet = Wallet::from_wif(WIF).unwrap();
    let btc = bitcoin(w.latest_time().await, &wallet);
    let o = Operator::new(Arc::new(SharedWallet::new(wallet, btc.clone(), 200.0)), 2 * w.unit());
    w.operator().lock_bond(3 * w.unit()).await.unwrap();
    o.round(w.operator()).await.unwrap();
    assert_eq!(btc.sent().len(), 1);
    btc.mine(w.latest_time().await);
    o.round(w.operator()).await.unwrap();
    assert!(w.operator().chain_head().await.unwrap().is_some());
    (o, btc)
}

async fn mine_blocks(w: &dyn World, btc: &MemoryBitcoin, n: usize) {
    for _ in 0..n {
        btc.mine(w.latest_time().await);
    }
}

async fn carries_a_job_from_bid_to_payment(w: &dyn World) {
    let (operator, guardian, u) = (w.operator(), w.guardian(), w.unit());
    let (o, btc) = registered(w).await;
    // Rounds with nothing to do send nothing.
    o.round(operator).await.unwrap();
    assert_eq!(btc.sent().len(), 1);

    // A job: the operator bids and wins.
    w.open_job(0).await;
    o.round(operator).await.unwrap();
    assert_eq!(operator.job(1).await.unwrap().operator.as_deref(), Some(operator.me().as_str()));
    w.increase_time(61).await;

    // Anchored one block below Bitcoin's best block, and the tagged
    // transaction sent.
    o.round(operator).await.unwrap();
    let job = operator.job(1).await.unwrap();
    assert!(job.anchor.is_some());
    assert_eq!(btc.sent().len(), 2);
    // A round before it is mined sends nothing more.
    o.round(operator).await.unwrap();
    assert_eq!(btc.sent().len(), 2);

    // Mined, with 5 blocks on top: proven.
    mine_blocks(w, &btc, 6).await;
    o.round(operator).await.unwrap();
    let job = operator.job(1).await.unwrap();
    assert_eq!(job.status, JobStatus::Proven);
    let sent = tx::strip_witness(&btc.sent()[1]).unwrap();
    assert_eq!(operator.chain_head().await.unwrap(), Some((tx::txid(&sent), 0)));

    // A guardian asks for the anchor's parent; the operator shows it and
    // takes the deposit.
    let salt = [3u8; 32];
    guardian.seal_note(1, &Evidence::Parent(job.anchor.unwrap().hash), &salt).await.unwrap();
    let deposit = guardian.parent_deposit(1).await.unwrap();
    let id = guardian.ask_parent(1, &salt, deposit).await.unwrap();
    o.round(operator).await.unwrap();
    assert!(guardian.challenge(id).await.unwrap().is_none());
    assert_eq!(operator.job(1).await.unwrap().parents_shown, 1);
    assert_eq!(operator.my_credit().await.unwrap(), 0, "withdrawn in the same round");

    // After the lock: settled and paid.
    let wait = job.lock_end - operator.now().await.unwrap() + 1;
    w.increase_time(wait as u64).await;
    o.round(operator).await.unwrap();
    assert_eq!(operator.job(1).await.unwrap().status, JobStatus::Settled);
    assert_eq!(operator.my_bond().await.unwrap(), (3 * u, 0));
    assert_eq!(operator.my_credit().await.unwrap(), 0);
    // It sent exactly two Bitcoin transactions.
    assert_eq!(btc.sent().len(), 2);
}

/// After a restart it finds the transaction it sent and does not send a
/// second one.
async fn finds_its_sent_transaction_after_a_restart(w: &dyn World) {
    let operator = w.operator();
    let (o, btc) = registered(w).await;
    w.open_job(0).await;
    o.round(operator).await.unwrap();
    w.increase_time(61).await;
    o.round(operator).await.unwrap();
    assert_eq!(btc.sent().len(), 2);

    // A new operator with nothing in memory.
    let o = Operator::new(Arc::new(SharedWallet::new(Wallet::from_wif(WIF).unwrap(), btc.clone(), 200.0)), 2 * w.unit());
    o.round(operator).await.unwrap();
    assert_eq!(btc.sent().len(), 2);
    mine_blocks(w, &btc, 6).await;
    o.round(operator).await.unwrap();
    assert_eq!(operator.job(1).await.unwrap().status, JobStatus::Proven);
}

/// Someone shows a made-up branch with more blocks than the operator's.
/// Real Bitcoin keeps growing, the operator adds its blocks to its side, and
/// the challenge fails when the lock ends (D81, D99).
async fn defends_its_proof_against_a_made_up_branch(w: &dyn World) {
    let (operator, guardian) = (w.operator(), w.guardian());
    let (o, btc) = registered(w).await;
    w.open_job(0).await;
    o.round(operator).await.unwrap();
    w.increase_time(61).await;
    o.round(operator).await.unwrap();
    mine_blocks(w, &btc, 6).await;
    o.round(operator).await.unwrap();
    let job = operator.job(1).await.unwrap();
    assert_eq!(job.status, JobStatus::Proven);
    let (proof_block, tip) = (job.proof_block.unwrap(), job.tip.unwrap());

    // Made-up blocks on the parent of the proof's block, 2 more than the
    // operator's branch has from there.
    let parent_hash = operator.stored_block(&proof_block).await.unwrap().unwrap().prev_hash;
    let parent = BlockRef { hash: parent_hash, height: proof_block.height - 1, epoch_time: proof_block.epoch_time };
    let count = tip.height - proof_block.height + 2;
    let mut made_up = vec![];
    let mut prev = parent.hash;
    for i in 0..count {
        let h = mine(prev, [0xbb; 32], w.latest_time().await + i);
        prev = sha256d(&h);
        made_up.push(h);
    }
    guardian.extend(&parent, &made_up).await.unwrap();
    let mut refs = vec![];
    let mut on = parent;
    for h in &made_up {
        on = on.child(h);
        refs.push(on);
    }
    let salt = [4u8; 32];
    guardian.seal_note(1, &Evidence::Fork(refs[0].hash), &salt).await.unwrap();
    let id = guardian.challenge_fork(1, &proof_block, &refs[0], refs.last().unwrap(), 0, &salt, job.commitment_fee).await.unwrap();

    // Bitcoin mines 4 more; the operator shows them and takes the lead.
    mine_blocks(w, &btc, 4).await;
    o.round(operator).await.unwrap();
    let wait = job.lock_end - operator.now().await.unwrap() + 1;
    w.increase_time(wait as u64).await;
    // Resolved in one round; settled in the next, with no challenge open.
    o.round(operator).await.unwrap();
    assert!(guardian.challenge(id).await.unwrap().is_none());
    o.round(operator).await.unwrap();
    assert_eq!(operator.job(1).await.unwrap().status, JobStatus::Settled);
}

/// Its transaction for job 1 is mined after the deadline and job 1 is
/// slashed. That transaction spent the chain head, so the operator moves the
/// chain head past it (N25), and job 2 is still proven.
async fn moves_its_chain_head_past_a_transaction_mined_too_late(w: &dyn World) {
    let (operator, guardian) = (w.operator(), w.guardian());
    let (o, btc) = registered(w).await;

    // Job 1: anchored and sent, then Bitcoin stalls past the deadline.
    w.open_job(0).await;
    o.round(operator).await.unwrap();
    w.increase_time(61).await;
    o.round(operator).await.unwrap();
    assert_eq!(btc.sent().len(), 2);
    let job = operator.job(1).await.unwrap();
    w.increase_time((job.deadline - operator.now().await.unwrap() + 1) as u64).await;
    // Past its deadline the operator sends nothing more for job 1.
    o.round(operator).await.unwrap();
    assert_eq!(btc.sent().len(), 2);
    let salt = [5u8; 32];
    guardian.seal_note(1, &Evidence::MissedDuty, &salt).await.unwrap();
    guardian.report_missed_duty(1, &salt).await.unwrap();
    assert_eq!(operator.job(1).await.unwrap().status, JobStatus::Slashed);
    let late = tx::txid(&tx::strip_witness(&btc.sent()[1]).unwrap());
    btc.mine(w.latest_time().await);

    // Job 2, from a second application: the chain head moves past the late
    // transaction, and job 2's transaction spends the coin it made.
    w.open_job(1).await;
    o.round(operator).await.unwrap();
    assert_eq!(operator.chain_head().await.unwrap(), Some((late, 0)));
    w.increase_time(61).await;
    o.round(operator).await.unwrap();
    assert_eq!(btc.sent().len(), 3);
    mine_blocks(w, &btc, 6).await;
    o.round(operator).await.unwrap();
    assert_eq!(operator.job(2).await.unwrap().status, JobStatus::Proven);
}

/// Bitcoin mined many blocks while the operator had nothing to do: the
/// light client must catch up in steps (the gap is above what one
/// transaction can carry).
async fn catches_up_after_a_long_quiet_time(w: &dyn World) {
    let operator = w.operator();
    let (o, btc) = registered(w).await;
    mine_blocks(w, &btc, 120).await;
    w.open_job(0).await;
    o.round(operator).await.unwrap();
    w.increase_time(61).await;
    o.round(operator).await.unwrap();
    assert!(operator.job(1).await.unwrap().anchor.is_some());
    mine_blocks(w, &btc, 6).await;
    o.round(operator).await.unwrap();
    assert_eq!(operator.job(1).await.unwrap().status, JobStatus::Proven);
}

/// Its tagged transaction waits while blocks are mined without it: the
/// operator replaces it with one that pays more, and the job is proven.
async fn pays_more_for_a_transaction_that_waits(w: &dyn World) {
    let operator = w.operator();
    let (o, btc) = registered(w).await;
    w.open_job(0).await;
    o.round(operator).await.unwrap();
    w.increase_time(61).await;
    o.round(operator).await.unwrap();
    assert_eq!(btc.sent().len(), 2);
    for _ in 0..2 {
        btc.mine_empty(w.latest_time().await);
    }
    o.round(operator).await.unwrap();
    assert_eq!(btc.sent().len(), 3, "a replacement was sent");
    let old = tx::strip_witness(&btc.sent()[1]).unwrap();
    let new = tx::strip_witness(&btc.sent()[2]).unwrap();
    let parse = |raw: &[u8]| -> bitcoin::Transaction { bitcoin::consensus::deserialize(raw).unwrap() };
    // The same coins, less change: a higher fee.
    assert_eq!(parse(&old).input, parse(&new).input);
    assert!(parse(&new).output[2].value < parse(&old).output[2].value);
    mine_blocks(w, &btc, 6).await;
    o.round(operator).await.unwrap();
    assert_eq!(operator.job(1).await.unwrap().status, JobStatus::Proven);
    assert_eq!(operator.chain_head().await.unwrap(), Some((tx::txid(&new), 0)));
}

/// Its first chain head was mined long before the node started working:
/// too far below what the light client holds. It sends a fresh one and
/// registers that.
async fn registers_again_when_its_first_chain_head_is_too_old(w: &dyn World) {
    let operator = w.operator();
    let wallet = Wallet::from_wif(WIF).unwrap();
    let btc = bitcoin(w.latest_time().await, &wallet);
    let o = Operator::new(Arc::new(SharedWallet::new(wallet, btc.clone(), 200.0)), 2 * w.unit());
    // Without a bond it waits.
    o.round(operator).await.unwrap();
    assert_eq!(btc.sent().len(), 0);
    operator.lock_bond(3 * w.unit()).await.unwrap();
    o.round(operator).await.unwrap();
    assert_eq!(btc.sent().len(), 1);
    btc.mine(w.latest_time().await - 3000);
    // 250 blocks later, the node works on this network.
    for i in 0..250 {
        btc.mine_empty(w.latest_time().await - 2900 + i * 10);
    }
    let o = Operator::new(Arc::new(SharedWallet::new(Wallet::from_wif(WIF).unwrap(), btc.clone(), 200.0)), 2 * w.unit());
    o.round(operator).await.unwrap();
    assert_eq!(btc.sent().len(), 2, "a fresh registration");
    btc.mine(w.latest_time().await);
    o.round(operator).await.unwrap();
    let registered = tx::txid(&tx::strip_witness(&btc.sent()[1]).unwrap());
    assert_eq!(operator.chain_head().await.unwrap(), Some((registered, 0)));
}

/// A restart after its transaction waited, was replaced and was mined: the
/// light client then holds only Bitcoin's newest blocks, and the proof needs
/// every block from the anchor up. It stores them and proves.
async fn proves_after_a_restart_between_anchor_and_proof(w: &dyn World) {
    let operator = w.operator();
    let (o, btc) = registered(w).await;
    w.open_job(0).await;
    o.round(operator).await.unwrap();
    w.increase_time(61).await;
    o.round(operator).await.unwrap();
    for _ in 0..3 {
        btc.mine_empty(w.latest_time().await);
    }
    o.round(operator).await.unwrap();
    assert_eq!(btc.sent().len(), 3, "replaced");
    mine_blocks(w, &btc, 6).await;
    let o = Operator::new(Arc::new(SharedWallet::new(Wallet::from_wif(WIF).unwrap(), btc.clone(), 200.0)), 2 * w.unit());
    o.round(operator).await.unwrap();
    assert_eq!(operator.job(1).await.unwrap().status, JobStatus::Proven);
}

/// Bitcoin up to height `blocks`, with the first epoch lasting a little over
/// two weeks so that the test difficulty stays the same after block 2016.
/// Block 6 pays the wallet.
fn long_bitcoin(now: u32, wallet: &Wallet, blocks: u32) -> Arc<MemoryBitcoin> {
    let btc = Arc::new(MemoryBitcoin::default());
    let base = now - 2016 * 601 - 3600;
    let mut prev = [0u8; 32];
    for i in 0..=blocks {
        let (root, txs) = if i == 6 {
            let funding = tx::build(&[([7; 32], 0)], &[tx::Output { value: 1_000_000, script: wallet.script().to_vec() }]);
            (ipow_bitcoin::merkle::root_and_proof(&[tx::txid(&funding)], 0).0, vec![funding])
        } else {
            (sha256d(&i.to_le_bytes()), vec![])
        };
        let h = mine(prev, root, base + i * 601);
        btc.add(i, h, &txs);
        prev = sha256d(&h);
    }
    btc
}

/// A job whose window crosses into a new epoch (block 2016) is proven across
/// it. After a restart the operator jumps into the new epoch, whose first
/// block is stored but whose epoch start was never recorded, and anchors a
/// second job.
async fn works_across_an_epoch_boundary_and_a_restart(w: &dyn World) {
    let operator = w.operator();
    let wallet = Wallet::from_wif(WIF).unwrap();
    let btc = long_bitcoin(w.latest_time().await, &wallet, 2008);
    let o = Operator::new(Arc::new(SharedWallet::new(wallet, btc.clone(), 200.0)), 2 * w.unit());
    operator.lock_bond(3 * w.unit()).await.unwrap();
    o.round(operator).await.unwrap();
    btc.mine(w.latest_time().await);
    o.round(operator).await.unwrap();
    assert!(operator.chain_head().await.unwrap().is_some());

    w.open_job(0).await;
    o.round(operator).await.unwrap();
    w.increase_time(61).await;
    o.round(operator).await.unwrap();
    let anchor = operator.job(1).await.unwrap().anchor.unwrap();
    assert!(anchor.height < 2016);
    // The transaction in block 2014, confirmed by blocks up to 2019.
    for _ in 0..4 {
        btc.mine_empty(w.latest_time().await);
    }
    o.round(operator).await.unwrap();
    while btc.tip_height().await.unwrap() < 2019 {
        btc.mine(w.latest_time().await);
    }
    o.round(operator).await.unwrap();
    let job = operator.job(1).await.unwrap();
    assert_eq!(job.status, JobStatus::Proven);
    assert!(job.tip.unwrap().height >= 2016);
    assert_ne!(job.tip.unwrap().epoch_time, anchor.epoch_time);

    // A restart, then a second job. A jump needs the new epoch's first 6
    // blocks (2016 to 2021).
    let o = Operator::new(Arc::new(SharedWallet::new(Wallet::from_wif(WIF).unwrap(), btc.clone(), 200.0)), 2 * w.unit());
    while btc.tip_height().await.unwrap() < 2022 {
        btc.mine(w.latest_time().await);
    }
    w.open_job(1).await;
    o.round(operator).await.unwrap();
    w.increase_time(61).await;
    o.round(operator).await.unwrap();
    assert!(operator.job(2).await.unwrap().anchor.unwrap().height > 2016);
}

on_every_network!(
    proves_after_a_restart_between_anchor_and_proof,
    works_across_an_epoch_boundary_and_a_restart,
    registers_again_when_its_first_chain_head_is_too_old,
    pays_more_for_a_transaction_that_waits,
    carries_a_job_from_bid_to_payment,
    finds_its_sent_transaction_after_a_restart,
    defends_its_proof_against_a_made_up_branch,
    moves_its_chain_head_past_a_transaction_mined_too_late,
    catches_up_after_a_long_quiet_time,
);
