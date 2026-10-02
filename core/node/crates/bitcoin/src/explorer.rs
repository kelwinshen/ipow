//! Bitcoin read from an explorer that serves the Esplora API
//! (mempool.space, blockstream.info, or one's own).
//!
//! An explorer can lie. Nothing read here is trusted on its own: the light
//! client checks every header's work and every Merkle proof, so a lie costs
//! the node a failed transaction, never a wrong proof accepted.

use anyhow::{Context, bail};
use async_trait::async_trait;
use serde::Deserialize;

use crate::reversed;
use crate::tx::strip_witness;
use crate::view::{BitcoinView, Coin, TxStatus};

/// Explorers in order of preference: a read or a broadcast that cannot
/// reach one, or that it fails with a server error, goes to the next. An
/// answer, including "not known", is taken from the first that gives one,
/// so two explorers that differ do not make it flip.
pub struct Explorer {
    bases: Vec<String>,
    http: reqwest::Client,
}

/// A hash as an explorer prints it, turned to header byte order.
fn from_display(hex_str: &str) -> anyhow::Result<[u8; 32]> {
    let bytes: [u8; 32] = hex::decode(hex_str.trim())
        .context("not hex")?
        .try_into()
        .map_err(|_| anyhow::anyhow!("not 32 bytes"))?;
    Ok(reversed(bytes))
}

/// A hash in header byte order, as an explorer prints it.
fn to_display(h: &[u8; 32]) -> String {
    hex::encode(reversed(*h))
}

#[derive(Deserialize)]
struct BlockStatus {
    in_best_chain: bool,
    height: Option<u32>,
}

#[derive(Deserialize)]
struct RawTxStatus {
    confirmed: bool,
    block_height: Option<u32>,
    block_hash: Option<String>,
}

#[derive(Deserialize)]
struct Outspend {
    spent: bool,
    txid: Option<String>,
}

#[derive(Deserialize)]
struct Utxo {
    txid: String,
    vout: u32,
    value: u64,
    status: RawTxStatus,
}

impl Explorer {
    /// `base` is the API root, e.g. `https://mempool.space/api`.
    pub fn new(base: &str) -> anyhow::Result<Self> {
        Self::with_fallbacks(base, &[])
    }

    /// `base` first, then each of `fallbacks` when one cannot answer.
    pub fn with_fallbacks(base: &str, fallbacks: &[String]) -> anyhow::Result<Self> {
        let http = reqwest::Client::builder().timeout(std::time::Duration::from_secs(20)).build()?;
        let bases = std::iter::once(base).chain(fallbacks.iter().map(String::as_str)).map(|b| b.trim_end_matches('/').to_string()).collect();
        Ok(Explorer { bases, http })
    }

    /// The body of a GET, or `None` for a 404.
    async fn get(&self, path: &str) -> anyhow::Result<Option<String>> {
        let mut last = None;
        for base in &self.bases {
            let url = format!("{base}{path}");
            match self.http.get(&url).send().await {
                Err(e) => last = Some(anyhow::Error::new(e).context(format!("GET {url}"))),
                Ok(res) if res.status() == reqwest::StatusCode::NOT_FOUND => return Ok(None),
                Ok(res) if res.status().is_server_error() => last = Some(anyhow::anyhow!("GET {url}: {}", res.status())),
                Ok(res) => {
                    let status = res.status();
                    let body = res.text().await?;
                    if !status.is_success() {
                        bail!("GET {url}: {status}: {}", body.trim());
                    }
                    return Ok(Some(body));
                }
            }
        }
        Err(last.unwrap_or_else(|| anyhow::anyhow!("no explorer")))
    }

    async fn need(&self, path: &str) -> anyhow::Result<String> {
        self.get(path).await?.ok_or_else(|| anyhow::anyhow!("{path} is not known to the explorer"))
    }

    fn status_of(s: RawTxStatus) -> anyhow::Result<TxStatus> {
        Ok(match (s.confirmed, s.block_hash, s.block_height) {
            (true, Some(h), Some(height)) => TxStatus::Confirmed { block: from_display(&h)?, height },
            (true, ..) => bail!("a confirmed transaction without its block"),
            (false, ..) => TxStatus::Unconfirmed,
        })
    }
}

#[async_trait]
impl BitcoinView for Explorer {
    async fn tip_height(&self) -> anyhow::Result<u32> {
        Ok(self.need("/blocks/tip/height").await?.trim().parse()?)
    }

    async fn block_hash(&self, height: u32) -> anyhow::Result<[u8; 32]> {
        from_display(&self.need(&format!("/block-height/{height}")).await?)
    }

    async fn header(&self, hash: &[u8; 32]) -> anyhow::Result<[u8; 80]> {
        let raw = hex::decode(self.need(&format!("/block/{}/header", to_display(hash))).await?.trim())?;
        let header: [u8; 80] = raw.try_into().map_err(|_| anyhow::anyhow!("a header is 80 bytes"))?;
        // The one check that costs nothing: the header is the block asked for.
        if crate::sha256d(&header) != *hash {
            bail!("the explorer sent a header of another block");
        }
        Ok(header)
    }

    async fn best_chain_height(&self, hash: &[u8; 32]) -> anyhow::Result<Option<u32>> {
        let Some(body) = self.get(&format!("/block/{}/status", to_display(hash))).await? else {
            return Ok(None);
        };
        let s: BlockStatus = serde_json::from_str(&body)?;
        Ok(if s.in_best_chain { s.height } else { None })
    }

    async fn txids(&self, block: &[u8; 32]) -> anyhow::Result<Vec<[u8; 32]>> {
        let list: Vec<String> = serde_json::from_str(&self.need(&format!("/block/{}/txids", to_display(block))).await?)?;
        list.iter().map(|t| from_display(t)).collect()
    }

    async fn raw_tx(&self, txid: &[u8; 32]) -> anyhow::Result<Option<Vec<u8>>> {
        let Some(body) = self.get(&format!("/tx/{}/hex", to_display(txid))).await? else {
            return Ok(None);
        };
        let raw = strip_witness(&hex::decode(body.trim())?)?;
        if crate::tx::txid(&raw) != *txid {
            bail!("the explorer sent another transaction");
        }
        Ok(Some(raw))
    }

    async fn tx_status(&self, txid: &[u8; 32]) -> anyhow::Result<Option<TxStatus>> {
        let Some(body) = self.get(&format!("/tx/{}/status", to_display(txid))).await? else {
            return Ok(None);
        };
        Ok(Some(Self::status_of(serde_json::from_str(&body)?)?))
    }

    async fn spender(&self, txid: &[u8; 32], vout: u32) -> anyhow::Result<Option<[u8; 32]>> {
        let Some(body) = self.get(&format!("/tx/{}/outspend/{vout}", to_display(txid))).await? else {
            return Ok(None);
        };
        let o: Outspend = serde_json::from_str(&body)?;
        match (o.spent, o.txid) {
            (true, Some(t)) => Ok(Some(from_display(&t)?)),
            (true, None) => bail!("a spent coin without its spender"),
            (false, _) => Ok(None),
        }
    }

    async fn coins(&self, address: &str) -> anyhow::Result<Vec<Coin>> {
        let list: Vec<Utxo> = serde_json::from_str(&self.need(&format!("/address/{address}/utxo")).await?)?;
        list.into_iter()
            .map(|u| Ok(Coin { txid: from_display(&u.txid)?, vout: u.vout, value: u.value, confirmed: u.status.confirmed }))
            .collect()
    }

    async fn fee_rate(&self, blocks: u16) -> anyhow::Result<f64> {
        let rates: std::collections::HashMap<String, f64> = serde_json::from_str(&self.need("/fee-estimates").await?)?;
        // The rate for the nearest target at or under `blocks`.
        rates
            .iter()
            .filter_map(|(k, v)| k.parse::<u16>().ok().filter(|k| *k <= blocks).map(|k| (k, *v)))
            .max_by_key(|(k, _)| *k)
            .map(|(_, v)| v)
            .ok_or_else(|| anyhow::anyhow!("the explorer gave no fee rate"))
    }

    async fn broadcast(&self, raw: &[u8]) -> anyhow::Result<[u8; 32]> {
        // The same transaction sent to two explorers is harmless; a refusal
        // of it is final.
        let mut last = None;
        for base in &self.bases {
            let url = format!("{base}/tx");
            match self.http.post(&url).body(hex::encode(raw)).send().await {
                Err(e) => last = Some(anyhow::Error::new(e).context(format!("POST {url}"))),
                Ok(res) if res.status().is_server_error() => last = Some(anyhow::anyhow!("POST {url}: {}", res.status())),
                Ok(res) => {
                    let status = res.status();
                    let body = res.text().await?;
                    if !status.is_success() {
                        bail!("the explorer refused the transaction: {status}: {}", body.trim());
                    }
                    return from_display(&body);
                }
            }
        }
        Err(last.unwrap_or_else(|| anyhow::anyhow!("no explorer")))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Reads real Bitcoin mainnet. Run with `cargo test -- --ignored` and a
    /// connection to the internet.
    #[tokio::test]
    #[ignore]
    async fn reads_mainnet() {
        let e = Explorer::new("https://mempool.space/api").unwrap();
        // Block 100,000 and its 4 transactions, as every explorer prints them.
        let hash = e.block_hash(100_000).await.unwrap();
        assert_eq!(to_display(&hash), "000000000003ba27aa200b1cecaad478d2b00432346c3f1f3986da1afd33e506");
        let header = e.header(&hash).await.unwrap();
        assert_eq!(e.best_chain_height(&hash).await.unwrap(), Some(100_000));
        let txids = e.txids(&hash).await.unwrap();
        assert_eq!(txids.len(), 4);
        let (root, siblings) = crate::merkle::root_and_proof(&txids, 2);
        assert_eq!(root, crate::Header(&header).merkle_root());
        assert_eq!(crate::merkle::root_from(txids[2], &siblings, 2), root);
        let raw = e.raw_tx(&txids[2]).await.unwrap().unwrap();
        assert_eq!(crate::tx::txid(&raw), txids[2]);
        assert_eq!(e.tx_status(&txids[2]).await.unwrap(), Some(TxStatus::Confirmed { block: hash, height: 100_000 }));
        // The first output of block 100,000's second transaction was spent.
        let spent_by = e.spender(&txids[1], 0).await.unwrap();
        assert!(spent_by.is_some());
        assert!(e.tip_height().await.unwrap() > 900_000);
        assert!(e.fee_rate(6).await.unwrap() > 0.0);
    }

    #[test]
    fn turns_hashes_between_the_two_orders() {
        let shown = "000000000003ba27aa200b1cecaad478d2b00432346c3f1f3986da1afd33e506";
        let h = from_display(shown).unwrap();
        assert_eq!(h[31], 0x00);
        assert_eq!(to_display(&h), shown);
    }
}
