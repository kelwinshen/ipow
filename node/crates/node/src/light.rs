//! Keeps one network's light client supplied with real Bitcoin blocks, for
//! an operator (D13). Only while the operator has something to prove:
//! every block costs a transaction.

use std::collections::BTreeMap;

use ipow_bitcoin::Header;
use ipow_bitcoin::view::BitcoinView;
use ipow_protocol_core::network::ProtocolNetwork;
use ipow_protocol_core::types::{BlockRef, EPOCH_BLOCKS};
use tracing::info;

use crate::bitcoin::stream_real;

/// Blocks stored in one round at most, so a round stays short.
const MAX_PER_ROUND: u32 = 500;
/// How far below the lowest known block `at` reaches down.
pub const MAX_REACH_DOWN: u32 = 200;
/// Blocks behind Bitcoin's best block beyond which the feed jumps instead
/// of streaming: about 2 hours of blocks.
const JUMP_GAP: u32 = 12;
/// Blocks remembered.
const KEEP: usize = 3_000;

/// The real blocks this node knows are stored, by height. They name each
/// other in order, with no gap.
#[derive(Default)]
pub struct Feed {
    known: BTreeMap<u32, BlockRef>,
}

impl Feed {
    /// The lowest block known to be stored.
    pub fn lowest(&self) -> Option<u32> {
        self.known.first_key_value().map(|(h, _)| *h)
    }

    /// Brings the light client up to Bitcoin's best block and returns where
    /// that block is stored. `None` when it cannot yet: the first blocks of
    /// a new epoch are still being mined.
    pub async fn tip(&mut self, net: &dyn ProtocolNetwork, btc: &dyn BitcoinView) -> anyhow::Result<Option<BlockRef>> {
        let best = btc.tip_height().await?;
        // Blocks that left the best chain are forgotten.
        while let Some((&h, r)) = self.known.last_key_value() {
            if h <= best && btc.block_hash(h).await? == r.hash {
                break;
            }
            self.known.pop_last();
        }
        match self.known.last_key_value().map(|(h, r)| (*h, *r)) {
            None => {
                if !self.jump(net, btc, best).await? {
                    return Ok(None);
                }
            }
            // Far behind: jumping is one transaction, streaming would be
            // one per block, and an anchor must be at most 2 hours old.
            Some((h, _)) if best - h > JUMP_GAP => {
                self.known.clear();
                if !self.jump(net, btc, best).await? {
                    return Ok(None);
                }
            }
            Some((h, r)) if h < best => {
                let count = (best - h).min(MAX_PER_ROUND);
                if let Some(refs) = stream_real(net, btc, &r, count).await? {
                    for (i, b) in refs.into_iter().enumerate() {
                        self.known.insert(h + 1 + i as u32, b);
                    }
                }
            }
            Some(_) => {}
        }
        while self.known.len() > KEEP {
            self.known.pop_first();
        }
        Ok(self.known.last_key_value().map(|(_, r)| *r))
    }

    /// Where the real block at `height` is stored, storing it first when
    /// needed. `None` when Bitcoin has no block there yet.
    pub async fn at(&mut self, net: &dyn ProtocolNetwork, btc: &dyn BitcoinView, height: u32) -> anyhow::Result<Option<BlockRef>> {
        if self.known.last_key_value().is_none_or(|(h, _)| *h < height) && self.tip(net, btc).await?.is_none() {
            return Ok(None);
        }
        let Some((&low, _)) = self.known.first_key_value() else { return Ok(None) };
        anyhow::ensure!(low <= height || low - height <= MAX_REACH_DOWN, "block {height} is too far below what is stored");
        let mut low = low;
        while low > height {
            let child = self.known[&low];
            let prev = net.stored_block(&child).await?.ok_or_else(|| anyhow::anyhow!("a known block is not stored"))?.prev_hash;
            let header = btc.header(&prev).await?;
            let prev_epoch_time = if child.height % EPOCH_BLOCKS == 0 { epoch_time_at(btc, low - 1).await? } else { 0 };
            net.extend_back(&child, &header, prev_epoch_time).await?;
            let epoch_time = if child.height % EPOCH_BLOCKS == 0 { prev_epoch_time } else { child.epoch_time };
            low -= 1;
            self.known.insert(low, BlockRef { hash: prev, height: low, epoch_time });
        }
        Ok(self.known.get(&height).copied())
    }

    /// Records the epoch start of the epoch of `best` and jumps to `best`.
    async fn jump(&mut self, net: &dyn ProtocolNetwork, btc: &dyn BitcoinView, best: u32) -> anyhow::Result<bool> {
        let first = best - best % EPOCH_BLOCKS;
        if best < first + 5 {
            info!(network = net.name(), "waiting for the first 6 blocks of the new epoch");
            return Ok(false);
        }
        let mut headers = vec![];
        for h in first..first + 6 {
            headers.push(btc.header(&btc.block_hash(h).await?).await?);
        }
        let epoch_start = BlockRef { hash: ipow_bitcoin::sha256d(&headers[0]), height: first, epoch_time: Header(&headers[0]).time() };
        // Recording it again changes nothing on either network. Its first
        // block being stored does not mean it is recorded: streaming across
        // an epoch boundary stores that block without a record.
        net.add_epoch_start(&headers, first).await?;
        let header = btc.header(&btc.block_hash(best).await?).await?;
        net.jump(&header, &epoch_start, best).await?;
        self.known.insert(best, BlockRef { hash: ipow_bitcoin::sha256d(&header), height: best, epoch_time: epoch_start.epoch_time });
        info!(network = net.name(), height = best, "jumped to Bitcoin's best block");
        Ok(true)
    }
}

/// The time of the first block of the epoch that holds `height`.
pub async fn epoch_time_at(btc: &dyn BitcoinView, height: u32) -> anyhow::Result<u32> {
    let first = height - height % EPOCH_BLOCKS;
    Ok(Header(&btc.header(&btc.block_hash(first).await?).await?).time())
}
