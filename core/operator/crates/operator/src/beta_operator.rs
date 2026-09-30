//! Beta's own engine, parallel to `chain_operator.rs` but driving Beta's
//! claim → relay → exercise lifecycle instead of Conversion's. See
//! design/ipow-implementation.md's Beta-engine plan for the full design rationale.
//!
//! **Deliberate v1 simplifications** (flagged, not silently assumed):
//! - **One global Bitcoin statement chain**, tracked as a single Redis
//!   key (`network="global"`), not per-`party_id`/per-network. The
//!   `party_id` a network's `Party` is registered under is an EVM/Solana
//!   registration concept only — the Bitcoin chain itself is just one
//!   linear sequence of transactions from the operator's one BTC wallet
//!   (`OPERATOR_BTC_WALLET_PRIVATE_KEY`/`_ADDRESS`, the same identity used
//!   by hand all session via `anchor_statement.rs`).
//! - **Remote legs are anchored sequentially, not forked in parallel.**
//!   This session's real 5-network mint hit a genuine bug from sharing
//!   one linear chain across multiple spokes anchored "simultaneously"
//!   (each spoke's own `Party` pointer could only advance one link at a
//!   time — see design/ipow-implementation.md §8.20's "shared chain" lesson, fixed there
//!   by hand with a second registered party per spoke). Rather than
//!   solve the general N-parallel-leg case here, this engine anchors one
//!   remote component at a time, waiting for each to be relayed before
//!   starting the next — slower, but avoids the whole class of bug.
//! - **The chain head must be seeded once, externally**, via
//!   `RedisStorage::set_beta_chain_head` — this engine does not try to
//!   discover an operator's currently-registered on-chain outpoint on its
//!   own; it errors clearly if no head is set rather than guessing.
//! - Header freshness is driven inline within each tick (calling
//!   `handle.streaming` directly) rather than as its own scheduled phase
//!   — Beta's ticks are infrequent enough that this is simpler than a
//!   fourth spawned task, unlike Conversion's higher-frequency engine.

use anyhow::{Context, Result};
use bitcoin::{PrivateKey as BtcPrivateKey, Txid};
use ipow_core::{
    btc::{
        btc_service::{build_proof_bundle, ProofBundle, TxStatus},
        statement_chain::{
            build_and_sign_statement_anchor, broadcast, fetch_utxo_value,
            parse_address, parse_wif, ChainHead, StatementAnchorParams,
        },
    },
    consts::beta_statement::{
        AttestStatement, MintStatement, KIND_ATTEST,
    },
    dependencies::context::CoreContext,
    traits::beta_adapter::PendingSummary,
};
use std::{str::FromStr, sync::Arc};
use tracing::{info, warn};

/// Simple, protocol-only "is this confirmed, and if so give me its
/// verified proof bundle" — deliberately not `btc_service`'s
/// `check_confirmation_and_build_proof` (that one is Conversion-specific:
/// `tx_id: U256`, RBF fee-rate logic mixed in). Mirrors
/// `examples/build_proof.rs`'s exact flow, just via `CoreContext`'s own
/// `build_proof_bundle` instead of a hand-rolled duplicate.
pub(crate) async fn fetch_confirmed_proof(
    core_ctx: &CoreContext,
    txid_be: &str,
) -> Result<Option<ProofBundle>> {
    let status: TxStatus = core_ctx
        .http
        .get(format!("{ESPLORA_BASE}/tx/{txid_be}/status"))
        .send()
        .await?
        .json()
        .await?;
    if !status.confirmed {
        return Ok(None);
    }
    let block_height = status
        .block_height
        .context("confirmed status missing block_height")?;
    let block_hash_be = status
        .block_hash
        .context("confirmed status missing block_hash")?;

    let tip = ipow_core::btc::btc_service::btc_tip_height(core_ctx).await?;
    if (tip as u64).saturating_sub(block_height) + 1 < MIN_CONFIRMATIONS {
        return Ok(None);
    }

    let bundle = build_proof_bundle(
        core_ctx,
        txid_be,
        1, // OP_RETURN is always output[1] in a statement anchor
        &block_hash_be,
        block_height,
    )
    .await?;
    Ok(Some(bundle))
}

use crate::beta_registry::{BetaNetworkHandle, BetaRegistry};

pub(crate) const ESPLORA_BASE: &str = "https://blockstream.info/api";
const HEAD_SATS: u64 = 294;
const FEE_SATS: u64 = 500;
/// Blocks of confirmation required before treating a statement anchor as
/// safe to relay — deliberately conservative for testnet reorg risk,
/// matching this session's own practice throughout.
pub(crate) const MIN_CONFIRMATIONS: u64 = 1;

pub struct BetaOperator;

impl BetaOperator {
    pub async fn run(core_ctx: Arc<CoreContext>) -> Result<()> {
        let registry = Arc::new(BetaRegistry::load(core_ctx.clone()).await?);

        if registry.by_self_network_id.is_empty() {
            anyhow::bail!(
                "no networks have Beta config (beta_networks.* in config.yml) — nothing to run"
            );
        }

        let mut interval = tokio::time::interval(std::time::Duration::from_secs(29));
        loop {
            interval.tick().await;
            if let Err(e) = tick_all(core_ctx.clone(), registry.clone()).await {
                warn!(error = %e, "Beta engine tick failed");
            }
        }
    }
}

async fn tick_all(
    core_ctx: Arc<CoreContext>,
    registry: Arc<BetaRegistry>,
) -> Result<()> {
    for handle in registry.by_self_network_id.values() {
        if let Some(hub) = &handle.hub {
            // Header freshness for this hub's own `iPoW`, before doing
            // anything that reads/writes anchor state against it.
            let tip = ipow_core::btc::btc_service::btc_tip_height(&core_ctx)
                .await
                .unwrap_or(0) as u64;
            if tip > 0 {
                let _ = handle.streaming.stream_headers_to_height(0, tip, 200).await;
            }

            if let Err(e) = tick_claim(&core_ctx, registry.as_ref(), handle, hub.as_ref()).await {
                warn!(network = handle.network_label, error = %e, "tick_claim failed");
            }
            if let Err(e) = tick_relay(&core_ctx, registry.as_ref(), handle, hub.as_ref()).await {
                warn!(network = handle.network_label, error = %e, "tick_relay failed");
            }
            if let Err(e) = tick_exercise(hub.as_ref()).await {
                warn!(network = handle.network_label, error = %e, "tick_exercise failed");
            }
        }
    }
    Ok(())
}

pub(crate) fn btc_credentials() -> Result<(BtcPrivateKey, bitcoin::Address, String)> {
    let wif = std::env::var("OPERATOR_BTC_WALLET_PRIVATE_KEY")
        .context("OPERATOR_BTC_WALLET_PRIVATE_KEY not set")?;
    let address_str = std::env::var("OPERATOR_BTC_WALLET_ADDRESS")
        .context("OPERATOR_BTC_WALLET_ADDRESS not set")?;
    let key = parse_wif(&wif)?;
    let address = parse_address(&address_str, bitcoin::Network::Bitcoin)?;
    Ok((key, address, address_str))
}

/// EVM-only: matches `BetaHub._pendingKey`'s `keccak256(abi.encodePacked(
/// address, uint64))`, which packs the raw 20-byte address, not the wide
/// 32-byte (zero-padded) form `PendingSummary.user` now carries. Not
/// meaningful for an SVM hub, whose own `pending` PDA is already
/// addressed by the real 32-byte pubkey directly — this helper is only
/// ever called from this file's EVM-hub tick logic.
fn pending_id_hex(user: [u8; 32], nonce: u64) -> String {
    let mut packed = Vec::with_capacity(20 + 8);
    packed.extend_from_slice(&user[12..]);
    packed.extend_from_slice(&nonce.to_be_bytes());
    hex::encode(ethers::utils::keccak256(&packed))
}

/// Phase 1: claim approved pendings by anchoring the next un-anchored
/// remote component's MINT statement onto the global Bitcoin chain.
async fn tick_claim(
    core_ctx: &CoreContext,
    registry: &BetaRegistry,
    handle: &BetaNetworkHandle,
    hub: &dyn ipow_core::traits::beta_adapter::BetaHubAdapter,
) -> Result<()> {
    let claimable = hub.find_claimable_pending().await?;
    if claimable.is_empty() {
        return Ok(());
    }

    for pending in claimable {
        if let Err(e) = claim_one(core_ctx, registry, handle, hub, &pending).await {
            warn!(
                user = ?pending.user,
                nonce = pending.nonce,
                error = %e,
                "failed to claim pending"
            );
        }
    }
    Ok(())
}

async fn claim_one(
    core_ctx: &CoreContext,
    registry: &BetaRegistry,
    handle: &BetaNetworkHandle,
    hub: &dyn ipow_core::traits::beta_adapter::BetaHubAdapter,
    pending: &PendingSummary,
) -> Result<()> {
    let composition = hub.get_composition(pending.composition_id).await?;
    let pid_hex = pending_id_hex(pending.user, pending.nonce);

    for (component_index, component) in composition.iter().enumerate() {
        if component.is_local {
            continue;
        }
        if pending
            .remote_anchor_txid_le
            .get(component_index)
            .and_then(|t| *t)
            .is_some()
        {
            continue; // already anchored
        }

        let already = core_ctx
            .redis_storage
            .get_beta_statement_broadcast(
                "global",
                &pid_hex,
                component_index as u8,
            )
            .await?;
        if already.is_some() {
            info!(
                pid = %pid_hex,
                component_index,
                "statement already broadcast, waiting for confirmation"
            );
            return Ok(()); // one at a time — wait for this leg to relay first
        }

        let Some(remote) = registry.by_self_network_id.get(&component.network_id)
        else {
            warn!(
                network_id = component.network_id,
                "no Beta config for this composition's remote network — cannot claim"
            );
            return Ok(());
        };

        let lock_id = *pending
            .remote_lock_id
            .get(component_index)
            .context("missing remoteLockId for this component")?;

        // target_user is the *hub's own* pending identity regardless of
        // which chain the hub itself runs on — pending.user is already in
        // the correct wide, wire-format shape from whichever adapter
        // produced it (EVM zero-pads to 32 bytes; SVM's is already a real
        // 32-byte pubkey), so no chain-specific conversion belongs here.
        let stmt = MintStatement {
            composition_id: pending.composition_id,
            component_index: component_index as u8,
            lock_id,
            target_user: pending.user,
            nonce: pending.nonce,
            units: pending.units,
            deadline: pending.deadline,
        }
        .encode();

        let (op_key, op_address, op_address_str) = btc_credentials()?;
        let client = reqwest::Client::new();

        let head_str = core_ctx
            .redis_storage
            .get_beta_chain_head("global", "main")
            .await?
            .context(
                "no Beta chain head seeded — set one via \
                 RedisStorage::set_beta_chain_head before running the engine",
            )?;
        let (head_txid_str, head_vout_str) = head_str
            .split_once(':')
            .context("malformed stored chain head, expected txid:vout")?;
        let head_txid = Txid::from_str(head_txid_str)?;
        let head_vout: u32 = head_vout_str.parse()?;
        let head_value = fetch_utxo_value(
            &client,
            ESPLORA_BASE,
            &op_address_str,
            &head_txid,
            head_vout,
        )
        .await?;

        let result = build_and_sign_statement_anchor(StatementAnchorParams {
            head: ChainHead { txid: head_txid, vout: head_vout, value_sats: head_value },
            funding: None,
            kind: ipow_core::consts::beta_statement::KIND_MINT,
            statement: stmt,
            head_sats: HEAD_SATS,
            fee_sats: FEE_SATS,
            extra_out_sats: 0,
            operator_private_key: op_key,
            operator_address: op_address,
        })?;

        broadcast(&client, ESPLORA_BASE, &result.witness_tx_hex).await?;

        core_ctx
            .redis_storage
            .set_beta_chain_head(
                "global",
                "main",
                &format!("{}:{}", result.new_head.txid, result.new_head.vout),
            )
            .await?;
        core_ctx
            .redis_storage
            .set_beta_statement_broadcast(
                "global",
                &pid_hex,
                component_index as u8,
                &result.txid.to_string(),
            )
            .await?;

        info!(
            hub_network = handle.network_label,
            remote_network = remote.network_label,
            component_index,
            btc_txid = %result.txid,
            "broadcast MINT statement for remote component"
        );
        return Ok(()); // one leg per tick — see module doc
    }
    Ok(())
}

/// Phase 2: once a broadcast statement confirms, build its proof and
/// submit `processAnchor` on both the hub and the relevant spoke, then
/// submit the paired ATTEST anchor on the hub.
async fn tick_relay(
    core_ctx: &CoreContext,
    registry: &BetaRegistry,
    handle: &BetaNetworkHandle,
    hub: &dyn ipow_core::traits::beta_adapter::BetaHubAdapter,
) -> Result<()> {
    let claimable_and_claimed = {
        let mut v = hub.find_claimable_pending().await?;
        v.extend(hub.find_exercisable_pending().await?);
        v
    };

    for pending in claimable_and_claimed {
        let composition = hub.get_composition(pending.composition_id).await?;
        let pid_hex = pending_id_hex(pending.user, pending.nonce);

        for (component_index, component) in composition.iter().enumerate() {
            if component.is_local {
                continue;
            }
            let already_anchored = pending
                .remote_anchor_txid_le
                .get(component_index)
                .and_then(|t| *t)
                .is_some();
            if already_anchored {
                continue;
            }

            let Some(btc_txid) = core_ctx
                .redis_storage
                .get_beta_statement_broadcast(
                    "global",
                    &pid_hex,
                    component_index as u8,
                )
                .await?
            else {
                continue; // not claimed yet — tick_claim's job
            };

            let Some(remote) =
                registry.by_self_network_id.get(&component.network_id)
            else {
                continue;
            };
            let Some(spoke) = &remote.spoke else {
                warn!(
                    network = remote.network_label,
                    "remote component's network has no spoke configured"
                );
                continue;
            };

            if let Err(e) = relay_one(
                core_ctx,
                handle,
                hub,
                remote,
                spoke.as_ref(),
                &btc_txid,
                &pending,
                component_index as u8,
                pending.composition_id,
            )
            .await
            {
                info!(btc_txid = %btc_txid, error = %e, "not yet relayable");
            }
        }
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
async fn relay_one(
    core_ctx: &CoreContext,
    hub_handle: &BetaNetworkHandle,
    hub: &dyn ipow_core::traits::beta_adapter::BetaHubAdapter,
    remote_handle: &BetaNetworkHandle,
    spoke: &dyn ipow_core::traits::beta_adapter::BetaSpokeAdapter,
    btc_txid_be: &str,
    pending: &PendingSummary,
    component_index: u8,
    composition_id: u64,
) -> Result<()> {
    let proof = fetch_confirmed_proof(core_ctx, btc_txid_be)
        .await?
        .context("statement not yet confirmed / proof not ready")?;

    let lock_id = *pending
        .remote_lock_id
        .get(component_index as usize)
        .context("missing remoteLockId")?;
    let statement = MintStatement {
        composition_id,
        component_index,
        lock_id,
        target_user: pending.user,
        nonce: pending.nonce,
        units: pending.units,
        deadline: pending.deadline,
    }
    .encode();

    let decoded = decode_proof(&proof)?;

    // Header freshness on both sides before submitting.
    let _ = remote_handle
        .streaming
        .stream_headers_to_height(0, decoded.block_height, 50)
        .await;
    let _ = hub_handle
        .streaming
        .stream_headers_to_height(0, decoded.block_height, 50)
        .await;

    spoke
        .submit_process_anchor(
            remote_handle.party_id,
            statement.clone(),
            decoded.tx_raw.clone(),
            decoded.block_height,
            decoded.branch_le.clone(),
            decoded.index,
        )
        .await
        .context("spoke processAnchor failed")?;

    hub.submit_process_anchor(
        hub_handle.party_id,
        statement,
        decoded.tx_raw,
        decoded.block_height,
        decoded.branch_le,
        decoded.index,
    )
    .await
    .context("hub processAnchor failed")?;

    info!(
        remote_network = remote_handle.network_label,
        hub_network = hub_handle.network_label,
        btc_txid = %btc_txid_be,
        "relayed MINT anchor to hub + spoke"
    );

    // Now ATTEST the MINT anchor on the hub to skip its challenge window.
    let mint_txid_le = ethers_txid_le(btc_txid_be)?;
    if hub.is_attested(mint_txid_le).await.unwrap_or(false) {
        return Ok(());
    }
    submit_attest(core_ctx, hub_handle, hub, mint_txid_le).await
}

async fn submit_attest(
    core_ctx: &CoreContext,
    hub_handle: &BetaNetworkHandle,
    hub: &dyn ipow_core::traits::beta_adapter::BetaHubAdapter,
    target_txid_le: [u8; 32],
) -> Result<()> {
    let target_hex = hex::encode(target_txid_le);
    let already = core_ctx
        .redis_storage
        .get_beta_statement_broadcast("global", "attest", 0)
        .await?;
    if already.as_deref() == Some(target_hex.as_str()) {
        return Ok(());
    }

    let (op_key, op_address, op_address_str) = btc_credentials()?;
    let client = reqwest::Client::new();

    let head_str = core_ctx
        .redis_storage
        .get_beta_chain_head("global", "main")
        .await?
        .context("no chain head seeded")?;
    let (head_txid_str, head_vout_str) =
        head_str.split_once(':').context("malformed chain head")?;
    let head_txid = Txid::from_str(head_txid_str)?;
    let head_vout: u32 = head_vout_str.parse()?;
    let head_value = fetch_utxo_value(
        &client,
        ESPLORA_BASE,
        &op_address_str,
        &head_txid,
        head_vout,
    )
    .await?;

    let stmt = AttestStatement { target_txid_le }.encode();
    let result = build_and_sign_statement_anchor(StatementAnchorParams {
        head: ChainHead { txid: head_txid, vout: head_vout, value_sats: head_value },
        funding: None,
        kind: KIND_ATTEST,
        statement: stmt,
        head_sats: HEAD_SATS,
        fee_sats: FEE_SATS,
        extra_out_sats: 0,
        operator_private_key: op_key,
        operator_address: op_address,
    })?;
    broadcast(&client, ESPLORA_BASE, &result.witness_tx_hex).await?;

    core_ctx
        .redis_storage
        .set_beta_chain_head(
            "global",
            "main",
            &format!("{}:{}", result.new_head.txid, result.new_head.vout),
        )
        .await?;

    // Once this ATTEST tx itself confirms and is relayed to the hub via a
    // future tick (reusing the same claim/relay bookkeeping keyed under a
    // dedicated "attest" pseudo-component), exerciseMint becomes eligible
    // immediately instead of waiting for the challenge window.
    core_ctx
        .redis_storage
        .set_beta_statement_broadcast(
            "global",
            "attest",
            0,
            &target_hex,
        )
        .await?;

    match fetch_confirmed_proof(core_ctx, &result.txid.to_string()).await {
        Ok(Some(proof)) => {
            let decoded = decode_proof(&proof)?;
            let _ = hub_handle
                .streaming
                .stream_headers_to_height(0, decoded.block_height, 50)
                .await;
            hub.submit_process_anchor(
                hub_handle.party_id,
                AttestStatement { target_txid_le }.encode(),
                decoded.tx_raw,
                decoded.block_height,
                decoded.branch_le,
                decoded.index,
            )
            .await
            .context("hub ATTEST processAnchor failed")?;
            info!(target = %target_hex, "ATTEST relayed to hub");
        },
        _ => {
            info!(target = %target_hex, "ATTEST broadcast, awaiting confirmation");
        },
    }

    Ok(())
}

pub(crate) struct DecodedProof {
    pub(crate) tx_raw: Vec<u8>,
    pub(crate) branch_le: Vec<[u8; 32]>,
    pub(crate) block_height: u64,
    pub(crate) index: u64,
}

pub(crate) fn decode_proof(proof: &ProofBundle) -> Result<DecodedProof> {
    let legacy_hex = proof.legacy_0x.strip_prefix("0x").unwrap_or(&proof.legacy_0x);
    let tx_raw = hex::decode(legacy_hex)?;
    let branch_le = proof
        .branch
        .iter()
        .map(|h| {
            let raw = hex::decode(h.strip_prefix("0x").unwrap_or(h))?;
            let mut out = [0u8; 32];
            if raw.len() != 32 {
                anyhow::bail!("branch element is not 32 bytes");
            }
            out.copy_from_slice(&raw);
            Ok(out)
        })
        .collect::<Result<Vec<_>>>()?;
    Ok(DecodedProof {
        tx_raw,
        branch_le,
        block_height: proof.block_height,
        index: proof.index,
    })
}

pub(crate) fn ethers_txid_le(btc_txid_be: &str) -> Result<[u8; 32]> {
    let be = hex::decode(btc_txid_be)?;
    if be.len() != 32 {
        anyhow::bail!("txid must be 32 bytes");
    }
    let mut le = [0u8; 32];
    for i in 0..32 {
        le[i] = be[31 - i];
    }
    Ok(le)
}

/// Phase 3: exercise every pending whose remote legs are all ready.
async fn tick_exercise(
    hub: &dyn ipow_core::traits::beta_adapter::BetaHubAdapter,
) -> Result<()> {
    let ready = hub.find_exercisable_pending().await?;
    for pending in ready {
        match hub.exercise_mint(pending.user, pending.nonce).await {
            Ok(tx_hash) => {
                info!(
                    user = ?pending.user,
                    nonce = pending.nonce,
                    tx_hash = %tx_hash,
                    "exerciseMint submitted"
                );
            },
            Err(e) => {
                warn!(
                    user = ?pending.user,
                    nonce = pending.nonce,
                    error = %e,
                    "exerciseMint failed"
                );
            },
        }
    }
    Ok(())
}
