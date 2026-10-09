//! What the roles read from Bitcoin, and send to it. Behind a trait so a
//! test can hand a role a Bitcoin of its own.
//!
//! Every hash here is in header byte order, as the contracts use it; an
//! implementation that reads an explorer reverses what the explorer prints.

use async_trait::async_trait;

/// Where a transaction is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TxStatus {
    /// Waiting to be mined.
    Unconfirmed,
    Confirmed { block: [u8; 32], height: u32 },
}

/// A coin a wallet can spend.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Coin {
    pub txid: [u8; 32],
    pub vout: u32,
    pub value: u64,
    pub confirmed: bool,
}

#[async_trait]
pub trait BitcoinView: Send + Sync {
    /// The height of the best block.
    async fn tip_height(&self) -> anyhow::Result<u32>;
    /// The hash of the best chain's block at `height`.
    async fn block_hash(&self, height: u32) -> anyhow::Result<[u8; 32]>;
    async fn header(&self, hash: &[u8; 32]) -> anyhow::Result<[u8; 80]>;
    /// The height of a block, or `None` when it is not in the best chain.
    async fn best_chain_height(&self, hash: &[u8; 32]) -> anyhow::Result<Option<u32>>;
    /// The txids of a block, in order.
    async fn txids(&self, block: &[u8; 32]) -> anyhow::Result<Vec<[u8; 32]>>;
    /// A transaction without its witness data, or `None` when unknown.
    async fn raw_tx(&self, txid: &[u8; 32]) -> anyhow::Result<Option<Vec<u8>>>;
    async fn tx_status(&self, txid: &[u8; 32]) -> anyhow::Result<Option<TxStatus>>;
    /// The transaction that spends a coin, mined or waiting, or `None`.
    async fn spender(&self, txid: &[u8; 32], vout: u32) -> anyhow::Result<Option<[u8; 32]>>;
    /// The coins of an address.
    async fn coins(&self, address: &str) -> anyhow::Result<Vec<Coin>>;
    /// The fee rate, in satoshis per virtual byte, to be mined within
    /// `blocks` blocks.
    async fn fee_rate(&self, blocks: u16) -> anyhow::Result<f64>;
    /// Sends a signed transaction (with its witness data) to the network.
    /// Returns its txid.
    async fn broadcast(&self, raw: &[u8]) -> anyhow::Result<[u8; 32]>;
}
