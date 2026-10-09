//! One operator on Ethereum and Solana at once, with one Bitcoin wallet: the
//! two jobs of one transfer carry the same tag, and one Bitcoin transaction
//! carries the message for both (spec sections 3 and 4.3).

use std::sync::Arc;

use ipow_bitcoin::memory::MemoryBitcoin;
use ipow_bitcoin::tx;
use ipow_bitcoin::wallet::Wallet;
use ipow_bitcoin::{merkle, sha256d};
use ipow_network_evm::testing::EvmWorld;
use ipow_network_svm::chain::Chain;
use ipow_network_svm::testing::SvmWorld;
use ipow_node::operator::Operator;
use ipow_node::supervisor::Worker;
use ipow_node::wallet::SharedWallet;
use ipow_protocol_core::types::JobStatus;
use ipow_testing::{World, mine};

const WIF: &str = "KwDiBf89QgGbjEhKnhXJuH7LrciVrZi3qYjgd9M7rFU73sVHnoWn";

/// Both networks, one clock for both (so that Bitcoin's times suit both),
/// and a Bitcoin whose block 6 pays the wallet.
async fn setup() -> (EvmWorld, SvmWorld, Arc<MemoryBitcoin>, Arc<SharedWallet>) {
    let _ = tracing_subscriber::fmt().with_test_writer().with_env_filter("warn").try_init();
    let eth = EvmWorld::new().await;
    let sol = SvmWorld::new().await;
    let now = eth.latest_time().await;
    sol.chain.set_time(now as i64);
    let wallet = Wallet::from_wif(WIF).unwrap();
    let btc = Arc::new(MemoryBitcoin::default());
    let mut prev = [0u8; 32];
    for i in 0..6u32 {
        let h = mine(prev, sha256d(&i.to_le_bytes()), now - 3600 + i * 600);
        btc.add(i, h, &[]);
        prev = sha256d(&h);
    }
    let funding = tx::build(&[([7; 32], 0)], &[tx::Output { value: 1_000_000, script: wallet.script().to_vec() }]);
    btc.add(6, mine(prev, merkle::root_and_proof(&[tx::txid(&funding)], 0).0, now - 60), &[funding]);
    let shared = Arc::new(SharedWallet::new(wallet, btc.clone(), 200.0));
    (eth, sol, btc, shared)
}

#[tokio::test(flavor = "multi_thread")]
async fn carries_one_message_for_two_networks_in_one_transaction() {
    let (eth, sol, btc, shared) = setup().await;
    let tick = async |s: u64| {
        eth.increase_time(s).await;
        sol.increase_time(s).await;
    };
    let on_eth = Operator::new(shared.clone(), 2 * eth.unit());
    let on_sol = Operator::new(shared.clone(), 2 * sol.unit());
    let round = async || {
        on_eth.round(eth.operator()).await.unwrap();
        on_sol.round(sol.operator()).await.unwrap();
    };

    // A chain head on each network.
    eth.operator().lock_bond(3 * eth.unit()).await.unwrap();
    sol.operator().lock_bond(3 * sol.unit()).await.unwrap();
    round().await;
    assert_eq!(btc.sent().len(), 2);
    btc.mine(eth.latest_time().await);
    round().await;

    // The transfer: the same tag on both networks. The operator wins both.
    eth.open_job(0).await;
    sol.open_job(0).await;
    round().await;
    tick(61).await;
    round().await;
    // Both anchored; one transaction for the two.
    let sent = btc.sent();
    assert_eq!(sent.len(), 3, "one tagged transaction for both networks");
    let combined: bitcoin::Transaction = bitcoin::consensus::deserialize(&sent[2]).unwrap();
    assert_eq!(combined.input.len(), 3, "two chain heads and a coin for the fee");
    let txid = tx::txid(&tx::strip_witness(&sent[2]).unwrap());

    // Mined in a block as full as a busy mainnet one: about 3,000
    // transactions, so a Merkle proof of 12 hashes.
    btc.mine_full(eth.latest_time().await, 3000);
    for _ in 0..5 {
        btc.mine(eth.latest_time().await);
    }
    round().await;
    for w in [&eth as &dyn World, &sol as &dyn World] {
        assert_eq!(w.operator().job(1).await.unwrap().status, JobStatus::Proven);
        // Each network's next chain head is the output of its own input.
        let (head_txid, vout) = w.operator().chain_head().await.unwrap().unwrap();
        assert_eq!(head_txid, txid);
        assert!(vout < 2);
    }
    // The Solana proof was too large for one transaction and went through
    // a lookup table; its rent comes back once Solana's cooldown has passed.
    assert_eq!(sol.operator.open_tables().await, 1);
    let slot = sol.chain.clock().await.unwrap().slot;
    sol.chain.wait_past(slot + 600).await.unwrap();
    sol.operator().lock_bond(1).await.unwrap();
    assert_eq!(sol.operator.open_tables().await, 0);

    let vouts: Vec<u32> = [eth.operator(), sol.operator()]
        .iter()
        .map(|n| futures_lite(n.chain_head()).unwrap().unwrap().1)
        .collect();
    assert_ne!(vouts[0], vouts[1]);
}

/// Waits on a future inside a sync closure of the test.
fn futures_lite<F: std::future::Future>(f: F) -> F::Output {
    tokio::task::block_in_place(|| tokio::runtime::Handle::current().block_on(f))
}

/// Solana's auction is still open when Ethereum's job is ready. Ethereum
/// waits one Bitcoin block at most, then sends alone; Solana sends its own
/// once its job is won. No network waits long for another.
#[tokio::test(flavor = "multi_thread")]
async fn does_not_wait_more_than_a_block_for_the_other_network() {
    let (eth, sol, btc, shared) = setup().await;
    let on_eth = Operator::new(shared.clone(), 2 * eth.unit());
    let on_sol = Operator::new(shared.clone(), 2 * sol.unit());
    let round = async || {
        on_eth.round(eth.operator()).await.unwrap();
        on_sol.round(sol.operator()).await.unwrap();
    };
    eth.operator().lock_bond(3 * eth.unit()).await.unwrap();
    sol.operator().lock_bond(3 * sol.unit()).await.unwrap();
    round().await;
    btc.mine(eth.latest_time().await);
    round().await;

    eth.open_job(0).await;
    sol.open_job(0).await;
    round().await;
    // Only Ethereum's auction ends.
    eth.increase_time(61).await;
    round().await;
    assert_eq!(btc.sent().len(), 2, "Ethereum waits for Solana");
    btc.mine_empty(eth.latest_time().await);
    round().await;
    assert_eq!(btc.sent().len(), 3, "a block later Ethereum sends alone");
    // Solana's auction ends; it sends its own.
    sol.increase_time(61).await;
    round().await;
    assert_eq!(btc.sent().len(), 4);
    for _ in 0..6 {
        btc.mine(eth.latest_time().await);
    }
    round().await;
    for w in [&eth as &dyn World, &sol as &dyn World] {
        assert_eq!(w.operator().job(1).await.unwrap().status, JobStatus::Proven);
    }
}
