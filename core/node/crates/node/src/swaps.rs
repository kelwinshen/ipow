//! The operator's side of the Conversion application
//! (docs/drafts/ipow-conversion-app.md). A job Conversion opened is a swap
//! the operator takes part in: for a sell it pays the user BTC in its tagged
//! transaction; for a buy it locks the coin, names a new Bitcoin address,
//! and its tagged transaction spends the user's payment (a receipt) or, when
//! none came, carries nothing more (a close).
//!
//! Whatever happens to a swap, the operator's tagged transaction still goes
//! out, so its duty is done: a swap it cannot serve is closed or carries no
//! payment, and the user takes their coin back through Conversion.

use std::collections::HashMap;
use std::sync::Arc;

use ipow_bitcoin::view::BitcoinView;
use ipow_bitcoin::wallet::{Spend, Wallet};
use ipow_protocol_core::conversion::{ConversionApp, Side, Swap, SwapState};
use ipow_protocol_core::settings::CoinPrice;
use ipow_protocol_core::types::{Amount, BlockRef, Job, JobStatus};
use tokio::sync::Mutex;
use tracing::{info, warn};

use crate::secrets::redact;
use crate::wallet::SharedWallet;

/// Buy: the user pays in one of the 12 blocks after the anchor; a close
/// counts only when mined after them (Conversion's PAY_BLOCKS).
pub const PAY_BLOCKS: u32 = 12;
/// Buy: the operator may lock the coin only while its anchor is at most 30
/// minutes old (Conversion's ANCHOR_AGE). It anchors a buy at a block at
/// most this old, to leave time to lock the coin.
pub const BUY_ANCHOR_AGE: i64 = 20 * 60;
/// Sell: no payment is sent in the last blocks of the proof range; a
/// payment mined after the range pays the user BTC the operator can never
/// collect the coin for.
pub const SELL_MARGIN: u32 = 6;
/// Buy: 36 hours after the job's deadline the operator takes the coin back
/// (Conversion's CLOSE_PERIOD).
pub const CLOSE_PERIOD: i64 = 36 * 3600;
const PROOF_RANGE: u32 = 25;

/// What the operator's tagged transaction must carry for a swap.
pub enum Plan {
    /// Not yet: a buy's payment blocks are not over and no payment was seen.
    Wait,
    /// Send now, with these coins spent and these outputs paid.
    Send { inputs: Vec<(Spend, Arc<Wallet>)>, outputs: Vec<(u64, Vec<u8>)> },
}

/// What a won swap holds back until it is over: BTC to pay for a sell, the
/// coin to lock for a buy.
#[derive(Clone)]
struct Reserve {
    sats: u64,
    token: Option<String>,
    amount: Amount,
}

pub struct Swaps {
    app: Arc<dyn ConversionApp>,
    coins: Vec<CoinPrice>,
    /// Keeps the per-swap keys of each network and each Conversion apart.
    key_index: u32,
    state: Mutex<State>,
}

#[derive(Default)]
struct State {
    cursor: u64,
    by_job: HashMap<u64, u64>,
    reserved: HashMap<u64, Reserve>,
}

/// Two addresses as the network writes them: EVM addresses in any case,
/// others exactly.
fn same_address(a: &str, b: &str) -> bool {
    if a.starts_with("0x") { a.eq_ignore_ascii_case(b) } else { a == b }
}

/// The dust limit of a standard output script, or `None` for a script
/// Bitcoin nodes would not relay a payment to.
pub fn standard_dust(script: &[u8]) -> Option<u64> {
    match script {
        // Pay to public key hash.
        [0x76, 0xa9, 0x14, .., 0x88, 0xac] if script.len() == 25 => Some(546),
        // Pay to script hash.
        [0xa9, 0x14, .., 0x87] if script.len() == 23 => Some(540),
        // Witness version 0: key hash, script hash.
        [0x00, 0x14, ..] if script.len() == 22 => Some(294),
        [0x00, 0x20, ..] if script.len() == 34 => Some(330),
        // Taproot.
        [0x51, 0x20, ..] if script.len() == 34 => Some(330),
        _ => None,
    }
}

/// Whether a swap's price suits the operator: `sats` for `amount` of a coin
/// with `decimals` places. A sell may pay at most `pay_sats` per whole
/// coin; a buy must bring at least `ask_sats` per whole coin. Numbers too
/// large to compare are refused.
pub fn price_ok(side: Side, sats: u64, amount: Amount, price: &CoinPrice) -> bool {
    let Some(unit) = 10u128.checked_pow(price.decimals as u32) else { return false };
    let Some(have) = (sats as u128).checked_mul(unit) else { return false };
    match side {
        Side::Sell => (price.pay_sats as u128).checked_mul(amount).is_some_and(|limit| have <= limit),
        Side::Buy => (price.ask_sats as u128).checked_mul(amount).is_some_and(|floor| have >= floor),
    }
}

impl Swaps {
    /// `network` is the network's name in the settings.
    pub fn new(app: Arc<dyn ConversionApp>, coins: Vec<CoinPrice>, network: &str) -> Self {
        let h = ipow_bitcoin::sha256d(format!("{network}/{}", app.application()).as_bytes());
        let key_index = u32::from_le_bytes(h[..4].try_into().unwrap()) & 0x7fff_ffff;
        Swaps { app, coins, key_index, state: Mutex::new(State::default()) }
    }

    pub fn is_ours(&self, job: &Job) -> bool {
        same_address(&job.application, &self.app.application())
    }

    /// The swap of a job Conversion opened.
    pub async fn swap_of(&self, job_id: u64) -> anyhow::Result<Option<Swap>> {
        let mut s = self.state.lock().await;
        if !s.by_job.contains_key(&job_id) {
            loop {
                let swaps = self.app.swaps_after(s.cursor, 50).await?;
                let Some(last) = swaps.last() else { break };
                s.cursor = last.id;
                for w in &swaps {
                    s.by_job.insert(w.job_id, w.id);
                }
            }
        }
        match s.by_job.get(&job_id) {
            Some(id) => Ok(Some(self.app.swap(*id).await?)),
            None => Ok(None),
        }
    }

    fn price(&self, swap: &Swap) -> Option<&CoinPrice> {
        self.coins.iter().find(|c| match &swap.token {
            None => c.token == "native",
            Some(t) => same_address(&c.token, t),
        })
    }

    /// Whether to take a swap: its price suits this operator, a sell pays a
    /// script Bitcoin relays at least its dust, and it can take its side on
    /// top of the swaps it already holds (`job_id`'s own excepted).
    pub async fn worth_it(&self, job_id: u64, swap: &Swap, wallet: &SharedWallet) -> anyhow::Result<bool> {
        let Some(price) = self.price(swap) else { return Ok(false) };
        if !price_ok(swap.side, swap.sats, swap.amount, price) {
            return Ok(false);
        }
        let reserved: Vec<Reserve> = self.state.lock().await.reserved.iter().filter(|(j, _)| **j != job_id).map(|(_, r)| r.clone()).collect();
        Ok(match swap.side {
            Side::Sell => {
                let Some(dust) = standard_dust(&swap.script) else { return Ok(false) };
                // Paying its own address would read as its change.
                if swap.sats < dust || swap.script == wallet.script() {
                    return Ok(false);
                }
                let held: u64 = reserved.iter().map(|r| r.sats).sum();
                wallet.can_pay_sats(swap.sats.saturating_add(held)).await?
            }
            Side::Buy => {
                let held: Amount = reserved.iter().filter(|r| r.token == swap.token).map(|r| r.amount).sum();
                // A twentieth more, for the network's fees and rent.
                let need = swap.amount.saturating_add(held).saturating_add(swap.amount / 20);
                self.app.my_balance(swap.token.as_deref()).await? >= need
            }
        })
    }

    /// Holds back what a won swap needs until it is over.
    pub async fn reserve(&self, job_id: u64, swap: &Swap) {
        let r = match swap.side {
            Side::Sell => Reserve { sats: swap.sats, token: None, amount: 0 },
            Side::Buy => Reserve { sats: 0, token: swap.token.clone(), amount: swap.amount },
        };
        self.state.lock().await.reserved.insert(job_id, r);
    }

    pub async fn release(&self, job_id: u64) {
        self.state.lock().await.reserved.remove(&job_id);
    }

    /// The key of a buy's address: derived for this Conversion, network and
    /// swap, so it can always be found again from the wallet's key, and is
    /// never given out twice.
    pub fn key(&self, wallet: &SharedWallet, swap_id: u64) -> anyhow::Result<Arc<Wallet>> {
        wallet.derive(&[0x4950, self.key_index, (swap_id >> 31) as u32, (swap_id & 0x7fff_ffff) as u32])
    }

    /// What the tagged transaction of a swap's job carries, now.
    pub async fn plan(&self, swap: &Swap, anchor: &BlockRef, best: u32, wallet: &SharedWallet) -> anyhow::Result<Plan> {
        let close_or_wait = || if best > anchor.height + PAY_BLOCKS { Plan::Send { inputs: vec![], outputs: vec![] } } else { Plan::Wait };
        match swap.side {
            Side::Sell => {
                // Too late to pay: the transaction goes out without the
                // payment, so the duty is done and the user takes the coin
                // back.
                if best + SELL_MARGIN >= anchor.height + PROOF_RANGE {
                    warn!(swap = swap.id, "too late to pay the user; sending without the payment");
                    return Ok(Plan::Send { inputs: vec![], outputs: vec![] });
                }
                Ok(Plan::Send { inputs: vec![], outputs: vec![(swap.sats, swap.script.clone())] })
            }
            Side::Buy => {
                let key = self.key(wallet, swap.id)?;
                if swap.state == SwapState::Open {
                    // Lock the coin and name the address, now that the anchor
                    // is fixed. When it cannot be done (the anchor or the
                    // funding time too old, the coin short), the swap is
                    // never funded and the job ends with a close.
                    match self.app.fund(swap.id, key.script()).await {
                        Ok(()) => {
                            info!(swap = swap.id, address = %key.address(), "swap funded; waiting for the user's payment");
                            return Ok(Plan::Wait);
                        }
                        Err(e) => {
                            warn!(swap = swap.id, error = %redact(&e), "could not lock the coin; the swap will be closed");
                            return Ok(close_or_wait());
                        }
                    }
                }
                // A payment in the mempool is spent too: the receipt cannot be
                // mined before it.
                let paid = wallet.btc().coins(&key.address()).await?.into_iter().find(|c| c.value >= swap.sats);
                if let Some(c) = paid {
                    return Ok(Plan::Send { inputs: vec![(Spend { txid: c.txid, vout: c.vout, value: c.value }, key)], outputs: vec![] });
                }
                Ok(close_or_wait())
            }
        }
    }

    /// Follows a swap once its job is past bidding and its duty: collects a
    /// sell's coin once the lock has ended, gives a buy's user the coin at
    /// once (receipt), and takes a buy's coin back after a close or, in any
    /// other ending, 36 hours after the deadline. Returns whether the swap
    /// is over for this operator.
    pub async fn follow_up(&self, job: &Job, now: i64, btc: &dyn BitcoinView) -> anyhow::Result<bool> {
        let Some(swap) = self.swap_of(job.id).await? else { return Ok(true) };
        let proven = matches!(job.status, JobStatus::Proven | JobStatus::Settled);
        let lock_ended = now >= job.lock_end && job.open_challenges == 0;
        match (swap.side, swap.state) {
            (Side::Sell, SwapState::Open) => {
                if !proven {
                    // Slashed, expired, or never proven: the user takes the
                    // coin back.
                    return Ok(!matches!(job.status, JobStatus::Assigned | JobStatus::Auction));
                }
                if !lock_ended {
                    return Ok(false);
                }
                let txid = job.txid.ok_or_else(|| anyhow::anyhow!("a proven job without its transaction"))?;
                let raw = btc.raw_tx(&txid).await?.ok_or_else(|| anyhow::anyhow!("the sell's transaction is unknown"))?;
                let t: bitcoin::Transaction = bitcoin::consensus::deserialize(&raw)?;
                let paid = t.output.iter().any(|o| o.script_pubkey.as_bytes() == swap.script && o.value.to_sat() >= swap.sats);
                if paid {
                    self.app.complete_sell(swap.id, &raw).await?;
                    info!(swap = swap.id, "sell completed: coin received");
                }
                Ok(true)
            }
            (Side::Buy, SwapState::Funded) => {
                if proven && let Some(txid) = job.txid {
                    let raw = btc.raw_tx(&txid).await?.ok_or_else(|| anyhow::anyhow!("the buy's transaction is unknown"))?;
                    let t: bitcoin::Transaction = bitcoin::consensus::deserialize(&raw)?;
                    // A receipt spends a coin paid to the swap's script.
                    for input in &t.input {
                        let prev_txid = bitcoin::hashes::Hash::to_byte_array(input.previous_output.txid);
                        let Some(prev) = btc.raw_tx(&prev_txid).await? else { continue };
                        let p: bitcoin::Transaction = bitcoin::consensus::deserialize(&prev)?;
                        if p.output.get(input.previous_output.vout as usize).is_some_and(|o| o.script_pubkey.as_bytes() == swap.script) {
                            self.app.complete_buy(swap.id, &raw, &prev, input.previous_output.vout).await?;
                            info!(swap = swap.id, "buy completed: coin given to the user");
                            return Ok(true);
                        }
                    }
                    // A close: the coin comes back once its lock has ended.
                    let closed = job.proof_block.is_some_and(|b| job.anchor.is_some_and(|a| b.height > a.height + PAY_BLOCKS));
                    if closed && lock_ended {
                        self.app.reclaim(swap.id).await?;
                        info!(swap = swap.id, "buy closed: coin taken back");
                        return Ok(true);
                    }
                }
                // Any other ending: 36 hours after the deadline.
                if job.deadline != 0 && now >= job.deadline + CLOSE_PERIOD {
                    self.app.reclaim(swap.id).await?;
                    info!(swap = swap.id, "buy ended without a close: coin taken back");
                    return Ok(true);
                }
                Ok(false)
            }
            _ => Ok(true),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn eth() -> CoinPrice {
        CoinPrice { token: "native".into(), decimals: 18, pay_sats: 3_000_000, ask_sats: 3_100_000 }
    }

    #[test]
    fn prices_whole_coins() {
        let one = 10u128.pow(18);
        // Selling 1 ETH for 0.03 BTC: the most it pays.
        assert!(price_ok(Side::Sell, 3_000_000, one, &eth()));
        assert!(!price_ok(Side::Sell, 3_000_001, one, &eth()));
        // Half an ETH for 0.0155 BTC: exactly its ask.
        assert!(price_ok(Side::Buy, 1_550_000, one / 2, &eth()));
        assert!(!price_ok(Side::Buy, 1_549_999, one / 2, &eth()));
    }

    #[test]
    fn refuses_numbers_too_large_to_compare() {
        assert!(!price_ok(Side::Buy, u64::MAX, u128::MAX, &eth()));
        assert!(!price_ok(Side::Sell, 1, 1, &CoinPrice { decimals: 40, ..eth() }));
    }

    #[test]
    fn knows_the_scripts_bitcoin_relays_and_their_dust() {
        let mut p2wpkh = vec![0x00, 0x14];
        p2wpkh.extend([1; 20]);
        assert_eq!(standard_dust(&p2wpkh), Some(294));
        let mut p2tr = vec![0x51, 0x20];
        p2tr.extend([1; 32]);
        assert_eq!(standard_dust(&p2tr), Some(330));
        assert_eq!(standard_dust(&[0x51]), None);
        let mut op_return = vec![0x6a, 0x20];
        op_return.extend([1; 32]);
        assert_eq!(standard_dust(&op_return), None);
    }
}
