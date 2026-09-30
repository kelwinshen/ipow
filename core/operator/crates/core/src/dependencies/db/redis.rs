use ::redis::{AsyncCommands, Client};
use anyhow::{Context, Result};
use tracing::info;

use crate::dependencies::config::CoreConfig;

pub struct RedisStorage {
    client: Client,
}

impl RedisStorage {
    pub async fn init(config: &CoreConfig) -> Result<Self> {
        let client = Client::open(config.redis_url.as_str())
            .with_context(|| "Invalid Redis URL")?;

        let mut conn = client.get_multiplexed_async_connection().await?;
        let _: String = ::redis::cmd("PING").query_async(&mut conn).await?;

        info!("Redis storage initialized");
        Ok(Self { client })
    }

    fn key(&self, network: &str, suffix: &str) -> String {
        format!("{}:{}", network.to_lowercase(), suffix)
    }

    // --- RECEIVE STATE ---
    pub async fn get_or_create_index_for_tx(
        &self,
        network: &str,
        tx_id: &str,
    ) -> Result<u32> {
        let mut conn = self.client.get_multiplexed_async_connection().await?;
        let mapping_key = self.key(network, "receive:mapping");
        let counter_key = self.key(network, "receive:next_index");

        if let Some(idx) =
            conn.hget::<_, _, Option<u32>>(&mapping_key, tx_id).await?
        {
            return Ok(idx);
        }

        // hset here just reserves the mapping key (INCR already claimed the slot);
        // the real index gets written below once we know the 0-based value.
        let (new_count, _): (u32, i32) = ::redis::pipe()
            .atomic()
            .incr(&counter_key, 1)
            .hset(&mapping_key, tx_id, 0)
            .query_async(&mut conn)
            .await?;

        let assigned_idx = new_count - 1;
        let _: () = conn.hset(&mapping_key, tx_id, assigned_idx).await?;

        Ok(assigned_idx)
    }

    // --- CONVERSIONS ---
    pub async fn set_conversion_processed(
        &self,
        network: &str,
        tx_id: &str,
        btc_tx_id: &str,
    ) -> Result<()> {
        let mut conn = self.client.get_multiplexed_async_connection().await?;
        let key = self.key(network, "conversions:processed");

        // HSET returns 1 if new, 0 if updated
        let _: () = conn.hset(key, tx_id, btc_tx_id).await?;
        Ok(())
    }

    pub async fn filter_processed_ids(
        &self,
        network: &str,
        ids: &[String],
    ) -> Result<Vec<bool>> {
        let mut conn = self.client.get_multiplexed_async_connection().await?;
        let key = self.key(network, "conversions:processed");

        if ids.is_empty() {
            return Ok(vec![]);
        }

        let results: Vec<Option<String>> = conn.hmget(key, ids).await?;
        Ok(results.iter().map(|r| r.is_some()).collect())
    }

    pub async fn get_btc_tx_ids(
        &self,
        network: &str,
        tx_ids: &[String],
    ) -> Result<Vec<Option<String>>> {
        if tx_ids.is_empty() {
            return Ok(vec![]);
        }

        let mut conn = self.client.get_multiplexed_async_connection().await?;
        let key = self.key(network, "conversions:processed");

        let results: Vec<Option<String>> = redis::cmd("HMGET")
            .arg(&key)
            .arg(tx_ids)
            .query_async(&mut conn)
            .await?;

        Ok(results)
    }

    // --- SWEEP HISTORY  ---
    /// Gets the last index that was successfully swept
    pub async fn get_last_swept_index(&self, network: &str) -> Result<u32> {
        let mut conn = self.client.get_multiplexed_async_connection().await?;
        let key = self.key(network, "sweep:last_index");
        let val: Option<u32> = conn.get(key).await?;
        Ok(val.unwrap_or(0))
    }

    /// Updates the last swept index
    pub async fn set_last_swept_index(
        &self,
        network: &str,
        index: u32,
    ) -> Result<()> {
        let mut conn = self.client.get_multiplexed_async_connection().await?;
        let key = self.key(network, "sweep:last_index");
        let _: () = conn.set(key, index).await?;
        Ok(())
    }

    /// Helper to get the current derivation counter
    pub async fn get_next_derivation_index(
        &self,
        network: &str,
    ) -> Result<u32> {
        let mut conn = self.client.get_multiplexed_async_connection().await?;
        let key = self.key(network, "receive:next_index");
        let val: Option<u32> = conn.get(key).await?;
        Ok(val.unwrap_or(0))
    }

    // --- Mempool Checkpoint  ---
    pub async fn set_mempool_checkpoint(
        &self,
        network: &str,
        btc_tx_id: &str,
        height: u64,
    ) -> Result<()> {
        let mut conn = self.client.get_multiplexed_async_connection().await?;
        let key = self.key(network, "mempool:first_seen");
        let _: () = conn.hset(key, btc_tx_id, height).await?;
        Ok(())
    }

    pub async fn get_mempool_checkpoint(
        &self,
        network: &str,
        btc_tx_id: &str,
    ) -> Result<Option<u64>> {
        let mut conn = self.client.get_multiplexed_async_connection().await?;
        let key = self.key(network, "mempool:first_seen");
        let val: Option<u64> = conn.hget(key, btc_tx_id).await?;
        Ok(val)
    }

    pub async fn remove_mempool_checkpoint(
        &self,
        network: &str,
        btc_tx_id: &str,
    ) -> Result<()> {
        let mut conn = self.client.get_multiplexed_async_connection().await?;
        let key = self.key(network, "mempool:first_seen");
        let _: () = conn.hdel(key, btc_tx_id).await?;
        Ok(())
    }

    // --- BETA: statement-chain party head tracking ---
    // Keyed by network (a Beta identity is per-network, not global — every
    // `BetaVault`/`BetaHub` deployment has its own independent party
    // registry) + hex party id, matching the "no shared chain across
    // independent spoke deployments" lesson from this session's real
    // 5-network mint (design/ipow-implementation.md §8.20).

    /// Current chain-head outpoint (`txid_be:vout`) for a Beta party
    /// identity on one network — `None` if never set here (fall back to
    /// the party's on-chain `registerParty` outpoint).
    pub async fn get_beta_chain_head(
        &self,
        network: &str,
        party_id_hex: &str,
    ) -> Result<Option<String>> {
        let mut conn = self.client.get_multiplexed_async_connection().await?;
        let key = self.key(network, "beta:chain_head");
        let val: Option<String> = conn.hget(key, party_id_hex).await?;
        Ok(val)
    }

    pub async fn set_beta_chain_head(
        &self,
        network: &str,
        party_id_hex: &str,
        outpoint: &str,
    ) -> Result<()> {
        let mut conn = self.client.get_multiplexed_async_connection().await?;
        let key = self.key(network, "beta:chain_head");
        let _: () = conn.hset(key, party_id_hex, outpoint).await?;
        Ok(())
    }

    /// Idempotency guard: has a statement anchor already been built and
    /// broadcast for this `pendingId`+`componentIndex`? Prevents a
    /// crashed/restarted engine from double-spending the chain head or
    /// double-claiming the same pending mint.
    pub async fn get_beta_statement_broadcast(
        &self,
        network: &str,
        pending_id_hex: &str,
        component_index: u8,
    ) -> Result<Option<String>> {
        let mut conn = self.client.get_multiplexed_async_connection().await?;
        let key = self.key(network, "beta:statement_broadcast");
        let field = format!("{pending_id_hex}:{component_index}");
        let val: Option<String> = conn.hget(key, field).await?;
        Ok(val)
    }

    pub async fn set_beta_statement_broadcast(
        &self,
        network: &str,
        pending_id_hex: &str,
        component_index: u8,
        btc_txid_be: &str,
    ) -> Result<()> {
        let mut conn = self.client.get_multiplexed_async_connection().await?;
        let key = self.key(network, "beta:statement_broadcast");
        let field = format!("{pending_id_hex}:{component_index}");
        let _: () = conn.hset(key, field, btc_txid_be).await?;
        Ok(())
    }
}
