//! Test helpers shared by every network. A test network (Hardhat, or an
//! in-process Solana) implements `World`; the helpers here then work the
//! same on each.

use std::collections::HashMap;

use async_trait::async_trait;
use ipow_bitcoin::{merkle, sha256d, tx};
use ipow_protocol_core::network::ProtocolNetwork;
use ipow_protocol_core::types::{Amount, BlockRef, Proof};

pub use ipow_bitcoin::memory::{EASY, mine_easy as mine};

/// A test network with the contracts or programs in place, and an
/// application that can open jobs.
#[async_trait]
pub trait World: Send + Sync {
    /// One coin in the network's smallest unit: 10^18 wei, 10^9 lamports.
    fn unit(&self) -> Amount;
    /// The node of the operator, and the node of a guardian or attester.
    fn operator(&self) -> &dyn ProtocolNetwork;
    fn guardian(&self) -> &dyn ProtocolNetwork;
    /// Application `app` (0 or 1) opens a settlement job for 1 coin with 6
    /// confirmations and tag `[1; 32]`. Each application once.
    async fn open_job(&self, app: u8);
    async fn increase_time(&self, seconds: u64);
    async fn latest_time(&self) -> u32;

    /// Conversion on this network, as the operator's node sees it.
    async fn conversion(&self) -> std::sync::Arc<dyn ipow_protocol_core::conversion::ConversionApp> {
        unimplemented!("this test network has no Conversion")
    }
    /// A user sells `amount` of the coin for at least `sats` paid to `script`.
    async fn user_sell(&self, _amount: Amount, _sats: u64, _script: &[u8]) {
        unimplemented!("this test network has no Conversion")
    }
    /// A user buys `amount` of the coin for `sats`.
    async fn user_buy(&self, _amount: Amount, _sats: u64) {
        unimplemented!("this test network has no Conversion")
    }
}

/// A Bitcoin chain the test mines and streams to the light client.
pub struct TestChain {
    /// The first block of the epoch start.
    pub first: BlockRef,
    pub tip: BlockRef,
    pub blocks: HashMap<[u8; 32], Vec<[u8; 32]>>,
    /// Every header mined, with its height and transactions.
    pub mined: Vec<(u32, [u8; 80], Vec<Vec<u8>>)>,
    count: u32,
}

impl TestChain {
    /// Records an epoch start whose last block is 10 minutes old.
    pub async fn start(n: &dyn ProtocolNetwork, now: u32) -> Self {
        let epoch_time = now - 3600;
        let mut headers = vec![];
        let mut prev = [0u8; 32];
        let mut first = BlockRef { hash: [0; 32], height: 0, epoch_time };
        for i in 0..6 {
            let h = mine(prev, sha256d(&[i as u8]), epoch_time + i * 600);
            prev = sha256d(&h);
            if i == 0 {
                first.hash = prev;
            }
            headers.push(h);
        }
        n.add_epoch_start(&headers, 0).await.unwrap();
        let mined = headers.iter().enumerate().map(|(i, h)| (i as u32, *h, vec![])).collect();
        TestChain { first, tip: BlockRef { hash: prev, height: 5, epoch_time }, blocks: HashMap::new(), mined, count: 0 }
    }

    /// Mines a block with these transactions on the tip, and streams it.
    pub async fn add(&mut self, n: &dyn ProtocolNetwork, time: u32, txs: &[&[u8]]) -> BlockRef {
        // The first transaction of a block is the miner's own.
        self.count += 1;
        let coinbase = tx::build(&[([0; 32], 0xffffffff)], &[tx::Output { value: self.count as u64, script: vec![0x51] }]);
        let txids: Vec<[u8; 32]> = std::iter::once(tx::txid(&coinbase)).chain(txs.iter().map(|t| tx::txid(t))).collect();
        let (root, _) = merkle::root_and_proof(&txids, 0);
        let h = mine(self.tip.hash, root, time);
        n.extend(&self.tip, &[h]).await.unwrap();
        self.tip = self.tip.child(&h);
        self.blocks.insert(self.tip.hash, txids);
        self.mined.push((self.tip.height, h, std::iter::once(coinbase).chain(txs.iter().map(|t| t.to_vec())).collect()));
        self.tip
    }

    pub fn proof_of(&self, block: &BlockRef, raw: &[u8]) -> (Vec<[u8; 32]>, u64) {
        let txids = &self.blocks[&block.hash];
        let i = txids.iter().position(|t| *t == tx::txid(raw)).unwrap();
        (merkle::root_and_proof(txids, i).1, i as u64)
    }
}

/// A coin to spend, and `OP_RETURN` with `payload`: the shape of every
/// tagged transaction. Output 0 is the next chain head.
pub fn tagged_tx(spends: ([u8; 32], u32), payload: &[u8; 32]) -> Vec<u8> {
    let mut coin = vec![0x00, 0x14];
    coin.extend_from_slice(&[0x11; 20]);
    tx::build(&[spends], &[tx::Output { value: 546, script: coin }, tx::op_return(payload)])
}

/// A job proven by the world's operator, with 3 coins of bond: its anchor,
/// then the block with the tagged transaction and 5 blocks on top. Returns
/// the chain, the anchor and the block of the proof.
pub async fn proven_job(w: &dyn World) -> (TestChain, BlockRef, BlockRef) {
    let operator = w.operator();
    let mut chain = TestChain::start(operator, w.latest_time().await).await;
    operator.lock_bond(3 * w.unit()).await.unwrap();
    let first = tagged_tx(([9; 32], 0), &operator.chain_head_commitment().await.unwrap());
    let block = chain.add(operator, w.latest_time().await + 1, &[&first]).await;
    let (siblings, index) = chain.proof_of(&block, &first);
    operator.register_chain_head(&block, &first, &siblings, index, 0, 1).await.unwrap();

    w.open_job(0).await;
    operator.bid(1, w.unit()).await.unwrap();
    w.increase_time(61).await;
    let anchor = chain.add(operator, w.latest_time().await + 1, &[]).await;
    operator.anchor_job(1, &anchor).await.unwrap();
    let head = operator.chain_head().await.unwrap().unwrap();
    let raw = tagged_tx(head, &operator.tag_payload(&[1; 32]).await.unwrap());
    let proof_block = chain.add(operator, w.latest_time().await + 1, &[&raw]).await;
    for _ in 0..5 {
        chain.add(operator, w.latest_time().await + 1, &[]).await;
    }
    let (siblings, tx_index) = chain.proof_of(&proof_block, &raw);
    let proof = Proof { proof_block, tip: chain.tip, prev_epoch_time: 0, raw_tx: raw, siblings, tx_index, head_index: 0, tag_index: 1 };
    operator.prove_job(1, &proof).await.unwrap();
    (chain, anchor, proof_block)
}
pub mod suite;
