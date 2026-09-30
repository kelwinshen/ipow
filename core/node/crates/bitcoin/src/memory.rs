//! A Bitcoin kept in memory, for tests: a best chain the test builds or
//! mines, the transactions of its blocks, and the ones waiting to be mined.

use std::collections::HashMap;
use std::str::FromStr;
use std::sync::Mutex;

use async_trait::async_trait;
use bitcoin::hashes::Hash;

use crate::view::{BitcoinView, Coin, TxStatus};
use crate::{Header, sha256d, tx};

/// The lowest difficulty Bitcoin can state. Only test light clients accept it.
pub const EASY: u32 = 0x207fffff;

/// Mines a header at the easy test difficulty.
pub fn mine_easy(prev: [u8; 32], merkle_root: [u8; 32], time: u32) -> [u8; 80] {
    let mut h = [0u8; 80];
    h[..4].copy_from_slice(&0x20000000u32.to_le_bytes());
    h[4..36].copy_from_slice(&prev);
    h[36..68].copy_from_slice(&merkle_root);
    h[68..72].copy_from_slice(&time.to_le_bytes());
    h[72..76].copy_from_slice(&EASY.to_le_bytes());
    for nonce in 0u32.. {
        h[76..].copy_from_slice(&nonce.to_le_bytes());
        // At this difficulty a hash counts when its top bit is clear.
        if sha256d(&h)[31] < 0x80 {
            return h;
        }
    }
    unreachable!()
}

#[derive(Default)]
struct Inner {
    /// The best chain, by height from `first_height`.
    chain: Vec<[u8; 80]>,
    first_height: u32,
    /// The txids of each block.
    txids: HashMap<[u8; 32], Vec<[u8; 32]>>,
    /// Every transaction, mined or waiting, without witness data.
    txs: HashMap<[u8; 32], Vec<u8>>,
    /// Waiting to be mined, in the order they were sent.
    waiting: Vec<[u8; 32]>,
    /// Everything handed to `broadcast`, with witness data.
    sent: Vec<Vec<u8>>,
    blocks_mined: u32,
}

#[derive(Default)]
pub struct MemoryBitcoin(Mutex<Inner>);

fn parse(raw: &[u8]) -> bitcoin::Transaction {
    bitcoin::consensus::deserialize(raw).expect("a transaction the test made")
}

impl MemoryBitcoin {
    /// Adds a block on top of the best chain, or starts the chain at
    /// `height` when it is empty. A block that does not name the tip as its
    /// parent replaces the chain above its parent: a reorganisation.
    pub fn add(&self, height: u32, header: [u8; 80], txs: &[Vec<u8>]) {
        let mut b = self.0.lock().unwrap();
        if b.chain.is_empty() {
            b.first_height = height;
        } else {
            let keep = (height - b.first_height) as usize;
            b.chain.truncate(keep);
            assert_eq!(Header(&header).prev(), sha256d(b.chain.last().unwrap()), "the block does not name the block below it");
        }
        b.chain.push(header);
        let mut ids = vec![];
        for t in txs {
            let stripped = tx::strip_witness(t).unwrap();
            let id = tx::txid(&stripped);
            b.txs.insert(id, stripped);
            b.waiting.retain(|w| *w != id);
            ids.push(id);
        }
        b.txids.insert(sha256d(&header), ids);
    }

    /// Mines a block on the tip with every waiting transaction, at `time`.
    /// Returns its height.
    pub fn mine(&self, time: u32) -> u32 {
        self.mine_block(time, true)
    }

    /// Mines a block that leaves the waiting transactions waiting, as when
    /// they pay too little.
    pub fn mine_empty(&self, time: u32) -> u32 {
        self.mine_block(time, false)
    }

    /// Mines a block with every waiting transaction and `filler` more, as
    /// full as a busy mainnet block: its Merkle proofs are as long.
    pub fn mine_full(&self, time: u32, filler: usize) -> u32 {
        let (tip, height, waiting) = {
            let mut b = self.0.lock().unwrap();
            b.blocks_mined += 1;
            let tip = *b.chain.last().expect("start the chain first");
            let height = b.first_height + b.chain.len() as u32;
            let waiting: Vec<Vec<u8>> = b.waiting.iter().map(|id| b.txs[id].clone()).collect();
            (tip, height, waiting)
        };
        let coinbase = tx::build(&[([0; 32], 0xffffffff)], &[tx::Output { value: height as u64, script: vec![0x51] }]);
        let fill = (0..filler).map(|i| tx::build(&[(sha256d(&(i as u64).to_le_bytes()), height)], &[tx::Output { value: 1, script: vec![0x51] }]));
        let txs: Vec<Vec<u8>> = std::iter::once(coinbase).chain(waiting).chain(fill).collect();
        let ids: Vec<[u8; 32]> = txs.iter().map(|t| tx::txid(t)).collect();
        let (root, _) = crate::merkle::root_and_proof(&ids, 0);
        self.add(height, mine_easy(sha256d(&tip), root, time), &txs);
        height
    }

    fn mine_block(&self, time: u32, with_waiting: bool) -> u32 {
        let (tip, height, waiting, n) = {
            let mut b = self.0.lock().unwrap();
            b.blocks_mined += 1;
            let tip = *b.chain.last().expect("start the chain first");
            let height = b.first_height + b.chain.len() as u32;
            let waiting: Vec<Vec<u8>> = if with_waiting { b.waiting.iter().map(|id| b.txs[id].clone()).collect() } else { vec![] };
            (tip, height, waiting, b.blocks_mined)
        };
        // The first transaction of a block is the miner's own.
        let coinbase = tx::build(&[([0; 32], 0xffffffff)], &[tx::Output { value: n as u64, script: vec![0x51] }]);
        let txs: Vec<Vec<u8>> = std::iter::once(coinbase).chain(waiting).collect();
        let ids: Vec<[u8; 32]> = txs.iter().map(|t| tx::txid(t)).collect();
        let (root, _) = crate::merkle::root_and_proof(&ids, 0);
        self.add(height, mine_easy(sha256d(&tip), root, time), &txs);
        height
    }

    /// Transactions handed to `broadcast`, in order.
    pub fn sent(&self) -> Vec<Vec<u8>> {
        self.0.lock().unwrap().sent.clone()
    }

    /// Removes a waiting transaction and every waiting one that spends it.
    fn evict(b: &mut Inner, id: [u8; 32]) {
        b.waiting.retain(|w| *w != id);
        let raw = b.txs.remove(&id).unwrap();
        for vout in 0..parse(&raw).output.len() as u32 {
            if let Some(child) = Self::spender_in(b, &id, vout) {
                Self::evict(b, child);
            }
        }
    }

    fn height_of(b: &Inner, hash: &[u8; 32]) -> Option<u32> {
        b.chain.iter().position(|h| sha256d(h) == *hash).map(|i| b.first_height + i as u32)
    }

    fn block_of(b: &Inner, txid: &[u8; 32]) -> Option<([u8; 32], u32)> {
        b.chain.iter().enumerate().find_map(|(i, header)| {
            let hash = sha256d(header);
            b.txids.get(&hash).is_some_and(|ids| ids.contains(txid)).then_some((hash, b.first_height + i as u32))
        })
    }

    fn spender_in(b: &Inner, txid: &[u8; 32], vout: u32) -> Option<[u8; 32]> {
        b.txs.iter().find_map(|(id, raw)| {
            parse(raw)
                .input
                .iter()
                .any(|i| i.previous_output.txid.to_byte_array() == *txid && i.previous_output.vout == vout)
                .then_some(*id)
        })
    }
}

#[async_trait]
impl BitcoinView for MemoryBitcoin {
    async fn tip_height(&self) -> anyhow::Result<u32> {
        let b = self.0.lock().unwrap();
        anyhow::ensure!(!b.chain.is_empty(), "no blocks yet");
        Ok(b.first_height + b.chain.len() as u32 - 1)
    }

    async fn block_hash(&self, height: u32) -> anyhow::Result<[u8; 32]> {
        let b = self.0.lock().unwrap();
        let h = b.chain.get(height.checked_sub(b.first_height).ok_or_else(|| anyhow::anyhow!("below the chain"))? as usize);
        Ok(sha256d(h.ok_or_else(|| anyhow::anyhow!("no block at {height}"))?))
    }

    async fn header(&self, hash: &[u8; 32]) -> anyhow::Result<[u8; 80]> {
        let b = self.0.lock().unwrap();
        b.chain.iter().find(|h| sha256d(*h) == *hash).copied().ok_or_else(|| anyhow::anyhow!("unknown block"))
    }

    async fn best_chain_height(&self, hash: &[u8; 32]) -> anyhow::Result<Option<u32>> {
        Ok(Self::height_of(&self.0.lock().unwrap(), hash))
    }

    async fn txids(&self, block: &[u8; 32]) -> anyhow::Result<Vec<[u8; 32]>> {
        self.0.lock().unwrap().txids.get(block).cloned().ok_or_else(|| anyhow::anyhow!("unknown block"))
    }

    async fn raw_tx(&self, txid: &[u8; 32]) -> anyhow::Result<Option<Vec<u8>>> {
        Ok(self.0.lock().unwrap().txs.get(txid).cloned())
    }

    async fn tx_status(&self, txid: &[u8; 32]) -> anyhow::Result<Option<TxStatus>> {
        let b = self.0.lock().unwrap();
        if let Some((block, height)) = Self::block_of(&b, txid) {
            return Ok(Some(TxStatus::Confirmed { block, height }));
        }
        Ok(b.waiting.contains(txid).then_some(TxStatus::Unconfirmed))
    }

    async fn spender(&self, txid: &[u8; 32], vout: u32) -> anyhow::Result<Option<[u8; 32]>> {
        Ok(Self::spender_in(&self.0.lock().unwrap(), txid, vout))
    }

    async fn coins(&self, address: &str) -> anyhow::Result<Vec<Coin>> {
        let script = bitcoin::Address::from_str(address)?.assume_checked().script_pubkey();
        let b = self.0.lock().unwrap();
        let mut out = vec![];
        for (id, raw) in &b.txs {
            for (vout, o) in parse(raw).output.iter().enumerate() {
                if o.script_pubkey == script && Self::spender_in(&b, id, vout as u32).is_none() {
                    let confirmed = Self::block_of(&b, id).is_some();
                    out.push(Coin { txid: *id, vout: vout as u32, value: o.value.to_sat(), confirmed });
                }
            }
        }
        Ok(out)
    }

    async fn fee_rate(&self, _blocks: u16) -> anyhow::Result<f64> {
        Ok(2.0)
    }

    async fn broadcast(&self, raw: &[u8]) -> anyhow::Result<[u8; 32]> {
        let mut b = self.0.lock().unwrap();
        let stripped = tx::strip_witness(raw)?;
        let id = tx::txid(&stripped);
        // A coin spent by a mined transaction is refused, as a real node
        // would. One spent by a waiting transaction is a replacement: the
        // waiting one, and what spends its coins, leave the pool.
        for i in parse(&stripped).input {
            if let Some(other) = Self::spender_in(&b, &i.previous_output.txid.to_byte_array(), i.previous_output.vout) {
                anyhow::ensure!(b.waiting.contains(&other), "a coin already spent");
                Self::evict(&mut b, other);
            }
        }
        b.sent.push(raw.to_vec());
        b.txs.insert(id, stripped);
        b.waiting.push(id);
        Ok(id)
    }
}
