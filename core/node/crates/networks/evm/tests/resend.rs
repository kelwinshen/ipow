//! A transaction that is not mined is sent again with the same nonce and a
//! higher fee, until it is mined: a gas price spike cannot hold the node
//! back.

use std::time::Duration;

use alloy::consensus::Transaction as _;
use alloy::providers::Provider;
use ipow_network_evm::testing::*;
use ipow_protocol_core::network::ProtocolNetwork;

#[tokio::test(flavor = "multi_thread")]
async fn pays_more_until_its_transaction_is_mined() {
    let _ = tracing_subscriber::fmt().with_test_writer().with_env_filter("warn").try_init();
    let mut w = EvmWorld::new().await;
    w.operator.set_receipt_timeout(Duration::from_secs(1));
    let app = &w.apps[0];
    let me = w.operator.address();
    let start_nonce = app.get_transaction_count(me).await.unwrap();
    // Blocks are no longer mined when a transaction arrives.
    let _: serde_json::Value = app.raw_request("evm_setAutomine".into(), (false,)).await.unwrap();

    let operator = &w.operator;
    let (bonded, _) = tokio::join!(operator.lock_bond(ETH), async {
        // Two tries go out, the second paying more; then a block is mined.
        tokio::time::sleep(Duration::from_millis(2500)).await;
        let _: serde_json::Value = app.raw_request("evm_mine".into(), ()).await.unwrap();
    });
    bonded.unwrap();
    assert_eq!(operator.my_bond().await.unwrap(), (ETH, 0));
    // One transaction mined, not two.
    assert_eq!(app.get_transaction_count(me).await.unwrap(), start_nonce + 1);
    let block = app.get_block_by_number(alloy::eips::BlockNumberOrTag::Latest).full().await.unwrap().unwrap();
    let txs: Vec<_> = block.transactions.into_transactions().collect();
    assert_eq!(txs.len(), 1);
    let first_estimate = app.estimate_eip1559_fees().await.unwrap().max_fee_per_gas;
    assert!(txs[0].max_fee_per_gas() > first_estimate, "the transaction mined is a replacement that pays more");
}
