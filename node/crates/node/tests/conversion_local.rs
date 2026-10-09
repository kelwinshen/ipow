//! The operator taking Conversion's swaps on each local network, against a
//! Bitcoin the test holds and mines (docs/specs/ipow-conversion-app.md).

mod common;

use std::sync::Arc;

use ipow_bitcoin::memory::MemoryBitcoin;
use ipow_bitcoin::view::BitcoinView;
use ipow_bitcoin::wallet::Wallet;
use ipow_bitcoin::{merkle, sha256d, tx};
use ipow_node::operator::Operator;
use ipow_node::supervisor::Worker;
use ipow_node::swaps::Swaps;
use ipow_node::wallet::SharedWallet;
use ipow_protocol_core::conversion::{ConversionApp, SwapState};
use ipow_protocol_core::settings::CoinPrice;
use ipow_testing::{World, mine};

const WIF: &str = "KwDiBf89QgGbjEhKnhXJuH7LrciVrZi3qYjgd9M7rFU73sVHnoWn";
const USER_SCRIPT: [u8; 22] = [0x00, 0x14, 0x22, 0x22, 0x22, 0x22, 0x22, 0x22, 0x22, 0x22, 0x22, 0x22, 0x22, 0x22, 0x22, 0x22, 0x22, 0x22, 0x22, 0x22, 0x22, 0x22];

/// Bitcoin whose block 6 pays the operator's wallet 1 BTC.
fn bitcoin(now: u32, wallet: &Wallet) -> Arc<MemoryBitcoin> {
    let btc = Arc::new(MemoryBitcoin::default());
    let mut prev = [0u8; 32];
    for i in 0..6u32 {
        let h = mine(prev, sha256d(&i.to_le_bytes()), now - 3600 + i * 600);
        btc.add(i, h, &[]);
        prev = sha256d(&h);
    }
    let funding = tx::build(&[([7; 32], 0)], &[tx::Output { value: 100_000_000, script: wallet.script().to_vec() }]);
    btc.add(6, mine(prev, merkle::root_and_proof(&[tx::txid(&funding)], 0).0, now - 60), &[funding]);
    btc
}

/// An operator taking swaps of the network's own coin: it pays at most
/// 0.06 BTC per coin to sellers, and asks at least 0.04 BTC from buyers.
async fn operator(w: &dyn World) -> (Operator, Arc<MemoryBitcoin>, Arc<dyn ConversionApp>) {
    let wallet = Wallet::from_wif(WIF).unwrap();
    let btc = bitcoin(w.latest_time().await, &wallet);
    let app = w.conversion().await;
    let decimals = w.unit().ilog10() as u8;
    let coins = vec![CoinPrice { token: "native".into(), symbol: None, decimals, pay_sats: 6_000_000, ask_sats: 4_000_000 }];
    let o = Operator::new(Arc::new(SharedWallet::new(wallet, btc.clone(), 200.0)), 2 * w.unit())
        .with_swaps(Arc::new(Swaps::new(app.clone(), coins, w.operator().name())));
    w.operator().lock_bond(3 * w.unit()).await.unwrap();
    o.round(w.operator()).await.unwrap();
    btc.mine(w.latest_time().await);
    o.round(w.operator()).await.unwrap();
    assert!(w.operator().chain_head().await.unwrap().is_some());
    (o, btc, app)
}

async fn mine_blocks(w: &dyn World, btc: &MemoryBitcoin, n: usize) {
    for _ in 0..n {
        btc.mine(w.latest_time().await);
    }
}

async fn sells_its_btc_for_the_coin(w: &dyn World) {
    let op = w.operator();
    let (o, btc, app) = operator(w).await;
    w.user_sell(w.unit(), 5_000_000, &USER_SCRIPT).await;
    o.round(op).await.unwrap();
    w.increase_time(61).await;
    o.round(op).await.unwrap();
    // Its tagged transaction pays the user.
    let sent: bitcoin::Transaction = bitcoin::consensus::deserialize(btc.sent().last().unwrap()).unwrap();
    assert!(sent.output.iter().any(|o| o.value.to_sat() == 5_000_000 && o.script_pubkey.as_bytes() == USER_SCRIPT));
    mine_blocks(w, &btc, 6).await;
    o.round(op).await.unwrap();
    let job = op.job(1).await.unwrap();
    assert!(job.txid.is_some());
    // The coin comes once the proof's lock has ended.
    assert_eq!(app.swap(1).await.unwrap().state, SwapState::Open);
    w.increase_time((job.lock_end - op.now().await.unwrap() + 1) as u64).await;
    o.round(op).await.unwrap();
    assert_eq!(app.swap(1).await.unwrap().state, SwapState::Done);
}

async fn gives_the_coin_for_the_users_btc(w: &dyn World) {
    let op = w.operator();
    let (o, btc, app) = operator(w).await;
    w.user_buy(w.unit(), 5_000_000).await;
    o.round(op).await.unwrap();
    w.increase_time(61).await;
    // Anchored and funded: the coin is locked, the address named.
    o.round(op).await.unwrap();
    let swap = app.swap(1).await.unwrap();
    assert_eq!(swap.state, SwapState::Funded);
    // The user pays; the operator's receipt spends the payment.
    let payment = tx::build(&[([8; 32], 0)], &[tx::Output { value: 5_000_000, script: swap.script.clone() }]);
    btc.broadcast(&payment).await.unwrap();
    o.round(op).await.unwrap();
    let receipt: bitcoin::Transaction = bitcoin::consensus::deserialize(btc.sent().last().unwrap()).unwrap();
    assert!(receipt.input.iter().any(|i| bitcoin::hashes::Hash::to_byte_array(i.previous_output.txid) == tx::txid(&payment)));
    mine_blocks(w, &btc, 6).await;
    // Proven in one round; the user receives the coin in the next.
    o.round(op).await.unwrap();
    o.round(op).await.unwrap();
    assert_eq!(app.swap(1).await.unwrap().state, SwapState::Done);
}

async fn takes_the_coin_back_when_the_user_never_pays(w: &dyn World) {
    let op = w.operator();
    let (o, btc, app) = operator(w).await;
    w.user_buy(w.unit(), 5_000_000).await;
    o.round(op).await.unwrap();
    w.increase_time(61).await;
    o.round(op).await.unwrap();
    assert_eq!(app.swap(1).await.unwrap().state, SwapState::Funded);
    let sent = btc.sent().len();
    // No close within the payment blocks.
    mine_blocks(w, &btc, 5).await;
    o.round(op).await.unwrap();
    assert_eq!(btc.sent().len(), sent);
    mine_blocks(w, &btc, 8).await;
    o.round(op).await.unwrap();
    assert_eq!(btc.sent().len(), sent + 1, "the close");
    mine_blocks(w, &btc, 6).await;
    o.round(op).await.unwrap();
    let job = op.job(1).await.unwrap();
    assert!(job.txid.is_some());
    w.increase_time((job.lock_end - op.now().await.unwrap() + 1) as u64).await;
    o.round(op).await.unwrap();
    assert_eq!(app.swap(1).await.unwrap().state, SwapState::Reclaimed);
}

async fn does_not_take_a_swap_below_its_price(w: &dyn World) {
    let op = w.operator();
    let (o, _, _) = operator(w).await;
    // 0.07 BTC for one coin: more than the 0.06 it pays.
    w.user_sell(w.unit(), 7_000_000, &USER_SCRIPT).await;
    o.round(op).await.unwrap();
    assert!(op.job(1).await.unwrap().operator.is_none());
}

async fn refuses_a_sell_it_cannot_pay(w: &dyn World) {
    let op = w.operator();
    let (o, _, _) = operator(w).await;
    // A script Bitcoin nodes do not relay a payment to.
    w.user_sell(w.unit(), 5_000_000, &[0x51]).await;
    // A payment below the dust limit.
    w.user_sell(w.unit() / 1_000_000, 1, &USER_SCRIPT).await;
    o.round(op).await.unwrap();
    assert!(op.job(1).await.unwrap().operator.is_none());
    assert!(op.job(2).await.unwrap().operator.is_none());
}

/// Its anchor is too old to lock the coin on: the swap is never funded, and
/// the operator still sends its transaction (a close), so its duty is done.
async fn closes_a_buy_it_could_not_fund(w: &dyn World) {
    let op = w.operator();
    let (o, btc, app) = operator(w).await;
    w.user_buy(w.unit(), 5_000_000).await;
    o.round(op).await.unwrap();
    w.increase_time(61).await;
    // The operator was away for 40 minutes: Bitcoin's newest blocks, and so
    // its anchor, are too old to lock the coin on, and so is the auction.
    w.increase_time(40 * 60).await;
    o.round(op).await.unwrap();
    assert_eq!(app.swap(1).await.unwrap().state, SwapState::Open);
    let sent = btc.sent().len();
    mine_blocks(w, &btc, 13).await;
    o.round(op).await.unwrap();
    assert_eq!(btc.sent().len(), sent + 1, "the close");
    mine_blocks(w, &btc, 6).await;
    o.round(op).await.unwrap();
    assert!(op.job(1).await.unwrap().txid.is_some(), "proven, not slashed");
}

/// Its job failed and was slashed after it had locked the coin: 36 hours
/// after the deadline it takes the coin back.
async fn takes_the_coin_back_after_its_job_failed(w: &dyn World) {
    let (op, g) = (w.operator(), w.guardian());
    let (o, _, app) = operator(w).await;
    w.user_buy(w.unit(), 5_000_000).await;
    o.round(op).await.unwrap();
    w.increase_time(61).await;
    o.round(op).await.unwrap();
    assert_eq!(app.swap(1).await.unwrap().state, SwapState::Funded);
    // The operator stops; its deadline passes and a guardian reports it.
    let job = op.job(1).await.unwrap();
    w.increase_time((job.deadline - op.now().await.unwrap() + 1) as u64).await;
    let salt = [3u8; 32];
    g.seal_note(1, &ipow_protocol_core::types::Evidence::MissedDuty, &salt).await.unwrap();
    g.report_missed_duty(1, &salt).await.unwrap();
    o.round(op).await.unwrap();
    assert_eq!(app.swap(1).await.unwrap().state, SwapState::Funded);
    w.increase_time(36 * 3600).await;
    o.round(op).await.unwrap();
    assert_eq!(app.swap(1).await.unwrap().state, SwapState::Reclaimed);
}

on_every_network!(
    refuses_a_sell_it_cannot_pay,
    closes_a_buy_it_could_not_fund,
    takes_the_coin_back_after_its_job_failed,
    sells_its_btc_for_the_coin,
    gives_the_coin_for_the_users_btc,
    takes_the_coin_back_when_the_user_never_pays,
    does_not_take_a_swap_below_its_price,
);
