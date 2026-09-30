//! The operator's Bitcoin wallet, shared by its tasks on every network. One
//! lock orders every transaction it builds, so two networks never spend the
//! same coin.
//!
//! It also joins messages. The jobs of one transfer on two networks carry
//! the same tag, so the same payload (D80). When this operator holds both,
//! one Bitcoin transaction spends both chain heads (spec sections 3 and
//! 4.3): each network's task says it is interested in a payload when it
//! bids, and ready when its job is anchored; the transaction goes out once
//! every interested network is ready, or once a Bitcoin block has passed
//! since the first was, so that no network waits long for another.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

use ipow_bitcoin::view::BitcoinView;
use ipow_bitcoin::wallet::{CHAIN_HEAD_VALUE, Spend, Wallet};
use tokio::sync::Mutex;

/// Blocks within which a tagged transaction should be mined. The proof
/// range is 25 blocks (D68); aiming at the next 2 leaves room to pay more.
const FEE_TARGET_BLOCKS: u16 = 2;

/// How long a coin spent by this node stays reserved. By then the explorer
/// shows it spent, or the transaction that spent it was replaced and the
/// coin is free again.
const RESERVED_FOR: Duration = Duration::from_secs(10 * 60);

/// A transaction sent, and the fee rate it pays.
#[derive(Clone, Copy, Debug)]
pub struct Sent {
    pub txid: [u8; 32],
    pub rate: f64,
}

/// The networks that want a payload carried, and the chain head each will
/// spend once ready.
#[derive(Default)]
struct Message {
    networks: HashMap<String, Option<Spend>>,
    /// Bitcoin's height when the first network was ready.
    first_ready: Option<u32>,
}

pub struct SharedWallet {
    wallet: Wallet,
    btc: Arc<dyn BitcoinView>,
    max_fee_rate: f64,
    messages: Mutex<HashMap<[u8; 32], Message>>,
    /// The fee rate of every transaction sent and perhaps not mined yet, on
    /// any network: a replacement must pay for all it evicts (BIP125 rule
    /// 3), and fee coins chain the transactions of every network together.
    rates: Mutex<HashMap<[u8; 32], f64>>,
    /// Per-swap keys derived so far, by their script: a replacement signs
    /// their coins again.
    derived: std::sync::Mutex<HashMap<Vec<u8>, Arc<Wallet>>>,
    /// Coins spent by transactions this node sent, which the explorer may
    /// not show as spent yet.
    spent: Mutex<HashMap<(u32, [u8; 32]), Instant>>,
}

impl SharedWallet {
    pub fn new(wallet: Wallet, btc: Arc<dyn BitcoinView>, max_fee_rate: f64) -> Self {
        SharedWallet {
            wallet,
            btc,
            max_fee_rate,
            messages: Mutex::new(HashMap::new()),
            rates: Mutex::new(HashMap::new()),
            derived: std::sync::Mutex::new(HashMap::new()),
            spent: Mutex::new(HashMap::new()),
        }
    }

    pub fn address(&self) -> String {
        self.wallet.address()
    }

    /// The script of the wallet's own address.
    pub fn script(&self) -> &[u8] {
        self.wallet.script()
    }

    /// The rate to pay: the explorer's, at least `min_rate`, never above the
    /// cap.
    async fn rate(&self, min_rate: f64) -> anyhow::Result<f64> {
        Ok(self.btc.fee_rate(FEE_TARGET_BLOCKS).await?.max(min_rate).min(self.max_fee_rate))
    }

    /// Whether the wallet holds a coin that can pay a fee at the cap.
    pub async fn can_pay(&self) -> anyhow::Result<bool> {
        let spent = self.spent.lock().await;
        let enough = 2 * CHAIN_HEAD_VALUE + (self.max_fee_rate * Wallet::vsize(3, 2) as f64).ceil() as u64;
        Ok(self
            .btc
            .coins(&self.wallet.address())
            .await?
            .iter()
            .any(|c| c.value >= enough && !spent.get(&(c.vout, c.txid)).is_some_and(|at| at.elapsed() < RESERVED_FOR)))
    }

    /// The key of one swap, derived from the wallet's key at `path`, and
    /// remembered.
    pub fn derive(&self, path: &[u32]) -> anyhow::Result<Arc<Wallet>> {
        let w = Arc::new(self.wallet.derive(path)?);
        self.derived.lock().unwrap().insert(w.script().to_vec(), w.clone());
        Ok(w)
    }

    /// The wallet whose key spends a coin paying `script`.
    fn owner_of(&self, script: &[u8]) -> Option<Arc<Wallet>> {
        if script == self.wallet.script() {
            return None;
        }
        self.derived.lock().unwrap().get(script).cloned()
    }

    /// Signs each input with its own key (`None`: the wallet's) and sends.
    async fn sign_and_send_with(
        &self,
        inputs: &[(Spend, Option<Arc<Wallet>>)],
        heads: usize,
        payload: &[u8; 32],
        extra: &[(u64, Vec<u8>)],
        rate: f64,
        max_stripped: Option<usize>,
    ) -> anyhow::Result<Sent> {
        let fee = (rate * Wallet::vsize_outputs(inputs.len(), heads, extra) as f64).ceil() as u64;
        let keyed: Vec<(Spend, &Wallet)> = inputs.iter().map(|(s, w)| (*s, w.as_deref().unwrap_or(&self.wallet))).collect();
        let signed = self.wallet.build(&keyed, heads, payload, extra, fee)?;
        if let Some(max) = max_stripped {
            let size = ipow_bitcoin::tx::strip_witness(&signed.raw)?.len();
            anyhow::ensure!(size <= max, "the transaction would be {size} bytes without witness data, above {max}: the wallet's coins are too many and small");
        }
        let txid = self.btc.broadcast(&signed.raw).await?;
        anyhow::ensure!(txid == signed.txid, "the explorer reported another txid");
        self.rates.lock().await.insert(txid, rate);
        Ok(Sent { txid, rate })
    }

    /// The sum of the fee rates of the transactions still waiting to be
    /// mined, `except` one: what a replacement of `except` may evict.
    pub async fn waiting_rates_except(&self, except: &[u8; 32]) -> anyhow::Result<f64> {
        let mut rates = self.rates.lock().await;
        let mut sum = 0.0;
        let mut done = vec![];
        for (txid, rate) in rates.iter() {
            match self.btc.tx_status(txid).await? {
                Some(ipow_bitcoin::view::TxStatus::Unconfirmed) => {
                    if txid != except {
                        sum += rate;
                    }
                }
                _ => done.push(*txid),
            }
        }
        for t in done {
            rates.remove(&t);
        }
        Ok(sum)
    }

    /// A payload a network may soon want carried: it bid on a job with it.
    /// After a restart a won job does not say so again, and its network
    /// sends alone: only a fee is lost.
    pub async fn interested(&self, payload: &[u8; 32], network: &str) {
        self.messages.lock().await.entry(*payload).or_default().networks.entry(network.to_string()).or_insert(None);
    }

    /// The network no longer wants the payload carried.
    pub async fn forget(&self, payload: &[u8; 32], network: &str) {
        let mut messages = self.messages.lock().await;
        if let Some(m) = messages.get_mut(payload) {
            m.networks.remove(network);
            if m.networks.is_empty() {
                messages.remove(payload);
            }
        }
    }

    /// The network is ready: its job is anchored, and `head` is the chain
    /// head to spend. Returns the transaction when it goes out, carrying the
    /// payload for every ready network, and the input that spends `head`;
    /// `None` while it waits for another interested network, at most until
    /// Bitcoin mines the next block. `urgent` sends at once: the job's proof
    /// range is about to end.
    pub async fn ready(&self, payload: &[u8; 32], network: &str, head: Spend, best: u32, urgent: bool) -> anyhow::Result<Option<(Sent, u32)>> {
        let mut messages = self.messages.lock().await;
        let m = messages.entry(*payload).or_default();
        m.networks.insert(network.to_string(), Some(head));
        let first = *m.first_ready.get_or_insert(best);
        let all = m.networks.values().all(|h| h.is_some());
        if !all && best <= first && !urgent {
            return Ok(None);
        }
        // Another network's head may have been spent since it said it was
        // ready, by a transaction of its own; it is left out, and says
        // again in its next round.
        let mut ready: Vec<(String, Spend)> = vec![];
        for (n, h) in m.networks.iter() {
            if let Some(h) = h
                && (n == network || self.btc.spender(&h.txid, h.vout).await?.is_none())
            {
                ready.push((n.clone(), *h));
            }
        }
        // A fixed order: input k is network k's head.
        ready.sort_by(|a, b| a.0.cmp(&b.0));
        let heads: Vec<Spend> = ready.iter().map(|(_, h)| *h).collect();
        let (sent, carried) = match self.send(&heads, payload).await {
            Ok(sent) => (sent, ready),
            // One bad head must not hold this network back: send alone.
            Err(e) if ready.len() > 1 => {
                tracing::warn!(error = %crate::secrets::redact(&e), "the transaction for several networks failed; sending alone");
                let own = ready.into_iter().filter(|(n, _)| n == network).collect::<Vec<_>>();
                (self.send(&[head], payload).await?, own)
            }
            Err(e) => return Err(e),
        };
        let index = carried.iter().position(|(n, _)| n == network).expect("the caller is carried") as u32;
        // The networks it carried are done; the others send later.
        for (n, h) in m.networks.iter_mut() {
            if carried.iter().any(|(c, _)| c == n) {
                *h = None;
            }
        }
        m.networks.retain(|n, _| !carried.iter().any(|(c, _)| c == n));
        m.first_ready = None;
        if m.networks.is_empty() {
            messages.remove(payload);
        }
        Ok(Some((sent, index)))
    }

    /// Sends a transaction whose first inputs are `heads`, one chain head per
    /// network (none for a first chain head, which spends a funding coin),
    /// with the next chain heads at the same output numbers, then `OP_RETURN
    /// payload`.
    pub async fn send(&self, heads: &[Spend], payload: &[u8; 32]) -> anyhow::Result<Sent> {
        self.send_with(heads, &[], payload, &[]).await
    }

    /// `send`, with coins an application asks to spend (each with its own
    /// key: a user's payment to a per-swap address) and outputs it asks for
    /// (BTC paid to a user). The wallet's own coins pay the rest: the
    /// largest first, as many as needed.
    pub async fn send_with(&self, heads: &[Spend], extra_inputs: &[(Spend, Arc<Wallet>)], payload: &[u8; 32], extra: &[(u64, Vec<u8>)]) -> anyhow::Result<Sent> {
        self.send_limited(heads, extra_inputs, payload, extra, None).await
    }

    /// `send`, refusing to broadcast a transaction longer than
    /// `max_stripped` bytes without its witness data.
    pub async fn send_within(&self, heads: &[Spend], payload: &[u8; 32], max_stripped: usize) -> anyhow::Result<Sent> {
        self.send_limited(heads, &[], payload, &[], Some(max_stripped)).await
    }

    async fn send_limited(
        &self,
        heads: &[Spend],
        extra_inputs: &[(Spend, Arc<Wallet>)],
        payload: &[u8; 32],
        extra: &[(u64, Vec<u8>)],
        max_stripped: Option<usize>,
    ) -> anyhow::Result<Sent> {
        let mut spent = self.spent.lock().await;
        spent.retain(|_, at| at.elapsed() < RESERVED_FOR);
        let rate = self.rate(0.0).await?;
        let head_count = heads.len().max(1);
        let payments: u64 = extra.iter().map(|(v, _)| *v).sum();
        let brought: u64 = heads.iter().map(|h| h.value).sum::<u64>() + extra_inputs.iter().map(|(s, _)| s.value).sum::<u64>();

        // Coins of the chain head's size are never used to pay.
        let mut coins: Vec<_> = self
            .btc
            .coins(&self.wallet.address())
            .await?
            .into_iter()
            .filter(|c| c.value > CHAIN_HEAD_VALUE && !spent.contains_key(&(c.vout, c.txid)))
            .collect();
        coins.sort_by_key(|c| std::cmp::Reverse(c.value));
        let mut funding = vec![];
        let mut have = brought;
        for c in coins {
            let inputs = heads.len() + extra_inputs.len() + funding.len() + 1;
            let fee = (rate * Wallet::vsize_outputs(inputs, head_count, extra) as f64).ceil() as u64;
            funding.push(Spend { txid: c.txid, vout: c.vout, value: c.value });
            have += c.value;
            if have >= CHAIN_HEAD_VALUE * head_count as u64 + payments + fee {
                break;
            }
        }
        anyhow::ensure!(!funding.is_empty(), "the wallet {} has no coin to pay the fee", self.wallet.address());

        let mut inputs: Vec<(Spend, Option<Arc<Wallet>>)> = heads.iter().map(|h| (*h, None)).collect();
        inputs.extend(extra_inputs.iter().map(|(s, w)| (*s, Some(w.clone()))));
        inputs.extend(funding.iter().map(|f| (*f, None)));
        let sent = self.sign_and_send_with(&inputs, head_count, payload, extra, rate, max_stripped).await?;
        for (i, _) in &inputs {
            spent.insert((i.vout, i.txid), Instant::now());
        }
        Ok(sent)
    }

    /// Whether the wallet can pay `sats` to someone, with the fee at the cap.
    pub async fn can_pay_sats(&self, sats: u64) -> anyhow::Result<bool> {
        let spent = self.spent.lock().await;
        let need = sats + 2 * CHAIN_HEAD_VALUE + (self.max_fee_rate * Wallet::vsize_with(4, 1, 1) as f64).ceil() as u64;
        let have: u64 = self
            .btc
            .coins(&self.wallet.address())
            .await?
            .iter()
            .filter(|c| c.value > CHAIN_HEAD_VALUE && !spent.get(&(c.vout, c.txid)).is_some_and(|at| at.elapsed() < RESERVED_FOR))
            .map(|c| c.value)
            .sum();
        Ok(have >= need)
    }

    /// Replaces a waiting transaction with one that spends the same coins
    /// and pays at least `min_rate` (replace-by-fee). The chain head it
    /// spends stays the same.
    /// With `keep_payments` false, the replacement leaves out the payments
    /// the transaction made: a sell whose payment would come too late.
    pub async fn replace(&self, old: &[u8; 32], payload: &[u8; 32], min_rate: f64, keep_payments: bool) -> anyhow::Result<Sent> {
        let _order = self.spent.lock().await;
        let raw = self.btc.raw_tx(old).await?.ok_or_else(|| anyhow::anyhow!("the transaction to replace is unknown"))?;
        let tx: bitcoin::Transaction = bitcoin::consensus::deserialize(&raw)?;
        let mut inputs = vec![];
        for i in &tx.input {
            let prev_txid = bitcoin::hashes::Hash::to_byte_array(i.previous_output.txid);
            let prev_raw = self.btc.raw_tx(&prev_txid).await?.ok_or_else(|| anyhow::anyhow!("a coin of the transaction is unknown"))?;
            let prev: bitcoin::Transaction = bitcoin::consensus::deserialize(&prev_raw)?;
            let out = prev.output.get(i.previous_output.vout as usize).ok_or_else(|| anyhow::anyhow!("no such output"))?;
            // Each coin signed again by the key that owns it.
            let owner = self.owner_of(out.script_pubkey.as_bytes());
            inputs.push((Spend { txid: prev_txid, vout: i.previous_output.vout, value: out.value.to_sat() }, owner));
        }
        // As many chain heads as the transaction replaced carried, and the
        // same payments after its OP_RETURN; its change is left out.
        let heads = tag_index(&raw).ok_or_else(|| anyhow::anyhow!("the transaction to replace carries no payload"))? as usize;
        let mut extra: Vec<(u64, Vec<u8>)> = tx.output[heads + 1..].iter().map(|o| (o.value.to_sat(), o.script_pubkey.as_bytes().to_vec())).collect();
        if extra.last().is_some_and(|(_, s)| s.as_slice() == self.wallet.script()) {
            extra.pop();
        }
        if !keep_payments {
            extra.clear();
        }
        let rate = self.rate(min_rate).await?;
        self.sign_and_send_with(&inputs, heads, payload, &extra, rate, None).await
    }

    /// The payload of a transaction's `OP_RETURN`, if it has one.
    pub fn payload_of(raw: &[u8]) -> Option<[u8; 32]> {
        let t: bitcoin::Transaction = bitcoin::consensus::deserialize(raw).ok()?;
        let script = t.output.get(tag_index(raw)? as usize)?.script_pubkey.as_bytes();
        Some(script[2..].try_into().unwrap())
    }

    pub fn btc(&self) -> &dyn BitcoinView {
        self.btc.as_ref()
    }
}

fn is_payload(script: &[u8]) -> bool {
    script.len() == 34 && script[0] == 0x6a && script[1] == 0x20
}

/// The output that carries the payload: after the chain heads.
pub fn tag_index(raw: &[u8]) -> Option<u32> {
    let t: bitcoin::Transaction = bitcoin::consensus::deserialize(raw).ok()?;
    t.output.iter().position(|o| is_payload(o.script_pubkey.as_bytes())).map(|i| i as u32)
}

/// The input that spends `coin`: for a chain head, also the output that is
/// the next chain head (N23).
pub fn input_spending(raw: &[u8], coin: ([u8; 32], u32)) -> Option<u32> {
    let t: bitcoin::Transaction = bitcoin::consensus::deserialize(raw).ok()?;
    t.input
        .iter()
        .position(|i| bitcoin::hashes::Hash::to_byte_array(i.previous_output.txid) == coin.0 && i.previous_output.vout == coin.1)
        .map(|i| i as u32)
}

/// The chain head as a coin to spend.
pub fn head_spend(txid: [u8; 32], vout: u32) -> Spend {
    Spend { txid, vout, value: CHAIN_HEAD_VALUE }
}

/// Where in a block a transaction is, and its Merkle proof.
pub async fn merkle_proof(btc: &dyn BitcoinView, block: &[u8; 32], txid: &[u8; 32]) -> anyhow::Result<(Vec<[u8; 32]>, u64)> {
    let txids = btc.txids(block).await?;
    let index = txids.iter().position(|t| t == txid).ok_or_else(|| anyhow::anyhow!("the transaction is not in the block"))?;
    let (_, siblings) = ipow_bitcoin::merkle::root_and_proof(&txids, index);
    Ok((siblings, index as u64))
}

#[cfg(test)]
mod tests {
    use super::*;
    use ipow_bitcoin::memory::{MemoryBitcoin, mine_easy};
    use bitcoin::hashes::Hash;
    use ipow_bitcoin::{merkle, tx};

    const WIF: &str = "KwDiBf89QgGbjEhKnhXJuH7LrciVrZi3qYjgd9M7rFU73sVHnoWn";

    /// A wallet with one coin of 1,000,000 satoshis.
    fn funded() -> (Arc<MemoryBitcoin>, SharedWallet) {
        let wallet = Wallet::from_wif(WIF).unwrap();
        let btc = Arc::new(MemoryBitcoin::default());
        let funding = tx::build(&[([7; 32], 0)], &[tx::Output { value: 1_000_000, script: wallet.script().to_vec() }]);
        let root = merkle::root_and_proof(&[tx::txid(&funding)], 0).0;
        btc.add(0, mine_easy([0; 32], root, 1_800_000_000), &[funding]);
        let shared = SharedWallet::new(wallet, btc.clone(), 200.0);
        (btc, shared)
    }

    fn head(n: u8) -> Spend {
        Spend { txid: [n; 32], vout: 0, value: CHAIN_HEAD_VALUE }
    }

    fn parse(raw: &[u8]) -> bitcoin::Transaction {
        bitcoin::consensus::deserialize(raw).unwrap()
    }

    #[tokio::test]
    async fn sends_once_every_interested_network_is_ready() {
        let (btc, w) = funded();
        let p = [1u8; 32];
        w.interested(&p, "ethereum").await;
        w.interested(&p, "solana").await;
        assert!(w.ready(&p, "solana", head(2), 10, false).await.unwrap().is_none());
        let (_, index) = w.ready(&p, "ethereum", head(1), 10, false).await.unwrap().unwrap();
        // Inputs in the order of the networks' names: ethereum, then solana.
        assert_eq!(index, 0);
        let t = parse(&btc.sent()[0]);
        assert_eq!(t.input.len(), 3);
        assert_eq!(t.input[1].previous_output.txid.to_byte_array(), [2; 32]);
        assert_eq!(tag_index(&tx::strip_witness(&btc.sent()[0]).unwrap()), Some(2));
    }

    #[tokio::test]
    async fn sends_alone_after_a_block_or_when_urgent() {
        let (btc, w) = funded();
        let p = [1u8; 32];
        w.interested(&p, "ethereum").await;
        w.interested(&p, "solana").await;
        assert!(w.ready(&p, "ethereum", head(1), 10, false).await.unwrap().is_none());
        // A block later it goes alone.
        let (_, index) = w.ready(&p, "ethereum", head(1), 11, false).await.unwrap().unwrap();
        assert_eq!(index, 0);
        assert_eq!(parse(&btc.sent()[0]).input.len(), 2);

        // Near the end of its proof range it does not wait at all.
        let q = [2u8; 32];
        w.interested(&q, "ethereum").await;
        w.interested(&q, "solana").await;
        assert!(w.ready(&q, "solana", head(3), 20, true).await.unwrap().is_some());
    }

    #[tokio::test]
    async fn leaves_out_a_head_spent_since_it_was_ready() {
        let (btc, w) = funded();
        let p = [1u8; 32];
        w.interested(&p, "ethereum").await;
        w.interested(&p, "solana").await;
        assert!(w.ready(&p, "ethereum", head(1), 10, false).await.unwrap().is_none());
        // Ethereum's head is spent by another of its transactions meanwhile.
        btc.broadcast(&tx::build(&[([1; 32], 0)], &[tx::Output { value: 1, script: vec![0x51] }])).await.unwrap();
        let (_, index) = w.ready(&p, "solana", head(2), 10, false).await.unwrap().unwrap();
        assert_eq!(index, 0);
        let t = parse(btc.sent().last().unwrap());
        assert_eq!(t.input.len(), 2, "Solana's head and a fee coin");
        assert_eq!(t.input[0].previous_output.txid.to_byte_array(), [2; 32]);
    }

    #[tokio::test]
    async fn forgets_a_network_that_no_longer_carries_the_message() {
        let (_, w) = funded();
        let p = [1u8; 32];
        w.interested(&p, "ethereum").await;
        w.interested(&p, "solana").await;
        w.forget(&p, "ethereum").await;
        // Solana is the only one left: it sends at once.
        assert!(w.ready(&p, "solana", head(2), 10, false).await.unwrap().is_some());
    }
}
