//! What the node needs from the Conversion application on a network
//! (docs/specs/ipow-conversion-app.md). The same on every network; each
//! network's adapter implements it.

use async_trait::async_trait;

use crate::types::Amount;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Side {
    /// The user sells the coin for BTC: the operator pays BTC.
    Sell,
    /// The user buys the coin with BTC: the operator supplies the coin.
    Buy,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SwapState {
    Open,
    Funded,
    Done,
    Refunded,
    Cancelled,
    Reclaimed,
}

#[derive(Clone, Debug)]
pub struct Swap {
    pub id: u64,
    pub side: Side,
    pub state: SwapState,
    pub user: String,
    /// `None` for the network's own coin.
    pub token: Option<String>,
    pub amount: Amount,
    /// Sell: the least the user accepts. Buy: what the user pays.
    pub sats: u64,
    pub job_id: u64,
    /// Sell: the user's script. Buy: the operator's, once it funded.
    pub script: Vec<u8>,
    /// Sell in a tunnel only: the Bitcoin blocks its payment must be mined
    /// in (docs/specs/ipow-conversion-tunnel.md, T1); `None` otherwise.
    pub pay_window: Option<(u32, u32)>,
    /// Buy only: the note its user's app wrote for operators; a tunnel's
    /// buy names the sale that will pay it (the SDK's `tunnelMemo`).
    pub memo: Vec<u8>,
}

#[async_trait]
pub trait ConversionApp: Send + Sync {
    /// The application's key in the protocol: `Job::application` of its jobs.
    fn application(&self) -> String;
    /// Swaps with an id above `after`, oldest first, at most `limit`.
    async fn swaps_after(&self, after: u64, limit: usize) -> anyhow::Result<Vec<Swap>>;
    async fn swap(&self, id: u64) -> anyhow::Result<Swap>;
    /// How much of `token` (or the coin) this node holds, to supply buys.
    async fn my_balance(&self, token: Option<&str>) -> anyhow::Result<Amount>;
    /// What a buy opened now pays in job fees, in the network's coin.
    async fn buy_fees(&self) -> anyhow::Result<Amount>;
    /// Buy: locks the coin and names the script the user pays.
    async fn fund(&self, swap_id: u64, script: &[u8]) -> anyhow::Result<()>;
    /// Sell: collects the coin once the proof's lock has ended.
    async fn complete_sell(&self, swap_id: u64, raw_tx: &[u8]) -> anyhow::Result<()>;
    /// Buy: gives the user the coin, the receipt spending their payment.
    async fn complete_buy(&self, swap_id: u64, receipt_raw: &[u8], payment_raw: &[u8], vout: u32) -> anyhow::Result<()>;
    /// Buy: takes the coin back when the user did not pay.
    async fn reclaim(&self, swap_id: u64) -> anyhow::Result<()>;
}
