//! What the roles share about real Bitcoin: whether a block is real, and
//! putting real blocks into a network's light client.

use ipow_bitcoin::Header;
use ipow_bitcoin::view::BitcoinView;
use ipow_protocol_core::network::ProtocolNetwork;
use ipow_protocol_core::types::BlockRef;

/// Blocks shown on a side of a challenge at once: below the light client's
/// MAX_WALK of 100, so every walk fits.
pub const STEP: u32 = 90;

/// Whether a block is in Bitcoin's best chain. Its stated height is not
/// checked: a real block is real whatever height it was stated at.
pub async fn is_real(btc: &dyn BitcoinView, block: &BlockRef) -> anyhow::Result<bool> {
    Ok(btc.best_chain_height(&block.hash).await?.is_some())
}

/// Whether a block is made up: `Some(true)` when it is not in Bitcoin's
/// best chain and Bitcoin has another block at its stated height,
/// `Some(false)` when it is in the best chain, and `None` when that cannot
/// be told yet (Bitcoin has no block at that height). An explorer that
/// merely does not know a block does not make it made up.
pub async fn made_up(btc: &dyn BitcoinView, block: &BlockRef) -> anyhow::Result<Option<bool>> {
    if is_real(btc, block).await? {
        return Ok(Some(false));
    }
    if block.height > btc.tip_height().await? {
        return Ok(None);
    }
    Ok((btc.block_hash(block.height).await? != block.hash).then_some(true))
}

/// Whether the blocks are stated at their real heights, with their real
/// epoch times: what a proof needs for every question about it to have an
/// answer.
pub async fn at_real_places(btc: &dyn BitcoinView, blocks: &[BlockRef]) -> anyhow::Result<bool> {
    for b in blocks {
        if btc.best_chain_height(&b.hash).await? != Some(b.height) {
            return Ok(false);
        }
        if crate::light::epoch_time_at(btc, b.height).await? != b.epoch_time {
            return Ok(false);
        }
    }
    Ok(true)
}

/// The value a walk across these blocks needs: the time of the older epoch
/// when they lie in two epochs, zero otherwise. Epoch times only grow, so
/// the older epoch has the smallest.
pub fn crossing(blocks: &[BlockRef]) -> u32 {
    let low = blocks.iter().map(|b| b.epoch_time).min().unwrap_or(0);
    let high = blocks.iter().map(|b| b.epoch_time).max().unwrap_or(0);
    if low != high { low } else { 0 }
}

/// Stores `count` real blocks on top of `from`, a stored real block, and
/// returns where each is in the light client. `from` is stated at the
/// height the light client knows it by; the explorer is asked by its real
/// height. Returns `None` when Bitcoin does not have that many blocks yet.
pub async fn stream_real(
    net: &dyn ProtocolNetwork,
    btc: &dyn BitcoinView,
    from: &BlockRef,
    count: u32,
) -> anyhow::Result<Option<Vec<BlockRef>>> {
    let Some(real_height) = btc.best_chain_height(&from.hash).await? else {
        anyhow::bail!("block {} is not in Bitcoin's best chain", hex(&from.hash));
    };
    if btc.tip_height().await? < real_height + count {
        return Ok(None);
    }
    let mut headers = Vec::with_capacity(count as usize);
    let mut refs = Vec::with_capacity(count as usize);
    let mut on = *from;
    for h in real_height + 1..=real_height + count {
        let header = btc.header(&btc.block_hash(h).await?).await?;
        // The explorer may have moved to another branch between two reads.
        anyhow::ensure!(Header(&header).prev() == on.hash, "Bitcoin changed while its blocks were read");
        on = on.child(&header);
        refs.push(on);
        headers.push(header);
    }
    if !headers.is_empty() {
        net.extend(from, &headers).await?;
    }
    Ok(Some(refs))
}

/// A hash as explorers print it, for logs.
pub fn hex(h: &[u8; 32]) -> String {
    let mut r = *h;
    r.reverse();
    r.iter().map(|b| format!("{b:02x}")).collect()
}
