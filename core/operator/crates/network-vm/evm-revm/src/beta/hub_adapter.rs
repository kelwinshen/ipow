//! `BetaHubAdapter` impl against a real deployed `BetaHub`.
//!
//! One real design point worth flagging: unlike Conversion's `tx_id`
//! (a sequential counter, trivially enumerable via `TxIdFilter`), Beta's
//! `pendingId = keccak256(user, nonce)` has no on-chain enumeration
//! function — there's no `getAllPending()`. So `find_claimable_pending`/
//! `find_exercisable_pending` scan the `PendingApproved` event log over a
//! recent block window and re-read each candidate's live `pending()`
//! state, rather than iterating a counter. This is a reasonable v1 (real
//! block windows aren't huge on these testnets), not a permanent design —
//! a production version should track a persisted "last scanned block"
//! cursor (e.g. in Redis, the same store Conversion's adapters already use
//! for their own idempotency bookkeeping) instead of a fixed lookback.

use crate::{beta::context::HubHandles, bindings::beta_hub as hub_bindings};
use anyhow::{Context, Result};
use ethers::providers::Middleware;
use ethers::types::{Address, U256};
use ipow_core::traits::beta_adapter::{
    AnchorStatus, BetaHubAdapter, ComponentInfo, PartyStatus, PendingSummary,
};
use std::sync::Arc;

/// How far back to scan `PendingApproved` logs each tick, in blocks. A
/// fixed window rather than a persisted cursor — see module doc.
const EVENT_LOOKBACK_BLOCKS: u64 = 5_000;

pub struct EvmBetaHubAdapter {
    pub handles: HubHandles,
    pub ipow_headers_address: String,
}

fn pending_key(user: Address, nonce: u64) -> [u8; 32] {
    // Matches BetaHub._pendingKey: keccak256(abi.encodePacked(user, nonce))
    let mut packed = Vec::with_capacity(20 + 8);
    packed.extend_from_slice(user.as_bytes());
    packed.extend_from_slice(&nonce.to_be_bytes());
    ethers::utils::keccak256(&packed)
}

/// The trait's `user: [u8; 32]` is a real Solana pubkey on an SVM hub, or
/// a zero-padded EVM address on an EVM hub (matching the MINT statement's
/// own `hubUser` wire format — bytes [12:32], zeros in [0:12]). These two
/// helpers convert at the boundary; `_pendingKey`'s own keccak256 always
/// uses the raw unpadded 20 bytes, per `BetaHub.sol`'s
/// `abi.encodePacked(address, uint64)`.
fn address_to_wide(addr: Address) -> [u8; 32] {
    let mut wide = [0u8; 32];
    wide[12..].copy_from_slice(addr.as_bytes());
    wide
}

fn address_from_wide(wide: [u8; 32]) -> Address {
    let mut bytes = [0u8; 20];
    bytes.copy_from_slice(&wide[12..]);
    Address::from(bytes)
}

fn anchor_status_from_u8(v: u8) -> AnchorStatus {
    match v {
        1 => AnchorStatus::Queued,
        2 => AnchorStatus::Exercised,
        3 => AnchorStatus::Slashed,
        4 => AnchorStatus::Skipped,
        _ => AnchorStatus::None,
    }
}

#[async_trait::async_trait]
impl BetaHubAdapter for EvmBetaHubAdapter {
    fn ipow_headers_address(&self) -> String {
        self.ipow_headers_address.clone()
    }

    async fn get_composition(
        &self,
        composition_id: u64,
    ) -> Result<Vec<ComponentInfo>> {
        let components = self
            .handles
            .read
            .get_composition(composition_id)
            .call()
            .await
            .context("getComposition call failed")?;

        let self_network_id: U256 = self
            .handles
            .read
            .self_network_id()
            .call()
            .await
            .unwrap_or_default();

        Ok(components
            .into_iter()
            .map(|c| {
                let mut token_id = [0u8; 32];
                token_id[12..].copy_from_slice(c.token_id.as_bytes());
                ComponentInfo {
                    network_id: c.network_id.as_u64(),
                    token_id,
                    amount_per_unit: c.amount_per_unit.as_u128(),
                    is_local: c.network_id == self_network_id,
                }
            })
            .collect())
    }

    async fn find_claimable_pending(&self) -> Result<Vec<PendingSummary>> {
        let all = self.scan_recent_pending().await?;
        Ok(all
            .into_iter()
            .filter(|p| p.approved && p.queued_by.is_none())
            .collect())
    }

    async fn get_pending(
        &self,
        user: [u8; 32],
        nonce: u64,
    ) -> Result<PendingSummary> {
        let pending_id = pending_key(address_from_wide(user), nonce);
        self.read_pending(pending_id).await
    }

    async fn submit_process_anchor(
        &self,
        party_id: [u8; 32],
        statement: Vec<u8>,
        tx_raw: Vec<u8>,
        block_height: u64,
        branch_le: Vec<[u8; 32]>,
        index: u64,
    ) -> Result<String> {
        let call = self.handles.write.process_anchor(
            party_id,
            statement.into(),
            tx_raw.into(),
            U256::from(block_height),
            branch_le,
            U256::from(index),
        );
        let pending_tx =
            call.send().await.context("processAnchor tx failed to send")?;
        let receipt = pending_tx
            .await
            .context("processAnchor tx failed to confirm")?
            .ok_or_else(|| anyhow::anyhow!("processAnchor: no receipt"))?;
        Ok(format!("{:?}", receipt.transaction_hash))
    }

    async fn get_anchor_status(
        &self,
        txid_le: [u8; 32],
    ) -> Result<AnchorStatus> {
        let a = self
            .handles
            .read
            .anchors(txid_le)
            .call()
            .await
            .context("anchors() call failed")?;
        // Generated tuple order matches AnchorsReturn's field order.
        Ok(anchor_status_from_u8(a.1))
    }

    async fn is_attested(&self, txid_le: [u8; 32]) -> Result<bool> {
        let attester = self
            .handles
            .read
            .mint_attester(txid_le)
            .call()
            .await
            .context("mintAttester() call failed")?;
        Ok(attester != [0u8; 32])
    }

    async fn find_exercisable_pending(&self) -> Result<Vec<PendingSummary>> {
        let all = self.scan_recent_pending().await?;
        let mut ready = Vec::new();
        for p in all {
            if !p.approved || p.queued_by.is_none() {
                continue;
            }
            if p.remote_anchor_txid_le.iter().any(|t| t.is_none()) {
                continue;
            }
            let mut all_ready = true;
            for txid in p.remote_anchor_txid_le.iter().flatten() {
                let status = self.get_anchor_status(*txid).await?;
                if status != AnchorStatus::Queued {
                    all_ready = false;
                    break;
                }
                // exerciseMint accepts once either attested or the
                // challenge window has closed — checking attested here is
                // the fast path; the window-closed path is also accepted
                // on-chain even if this returns false, so this is a
                // conservative (never-false-positive) readiness check.
                if !self.is_attested(*txid).await? {
                    all_ready = false;
                    break;
                }
            }
            if all_ready {
                ready.push(p);
            }
        }
        Ok(ready)
    }

    async fn exercise_mint(
        &self,
        user: [u8; 32],
        nonce: u64,
    ) -> Result<String> {
        let call = self
            .handles
            .write
            .exercise_mint(address_from_wide(user), nonce);
        let pending_tx =
            call.send().await.context("exerciseMint tx failed to send")?;
        let receipt = pending_tx
            .await
            .context("exerciseMint tx failed to confirm")?
            .ok_or_else(|| anyhow::anyhow!("exerciseMint: no receipt"))?;
        Ok(format!("{:?}", receipt.transaction_hash))
    }

    async fn get_own_party_status(
        &self,
        party_id: [u8; 32],
    ) -> Result<PartyStatus> {
        let p = self
            .handles
            .read
            .parties(party_id)
            .call()
            .await
            .context("parties() call failed")?;
        Ok(PartyStatus { exists: p.0, dead: p.7, bond: p.6.as_u128() })
    }
}

impl EvmBetaHubAdapter {
    async fn read_pending(&self, pending_id: [u8; 32]) -> Result<PendingSummary> {
        let p = self
            .handles
            .read
            .pending(pending_id)
            .call()
            .await
            .context("pending() call failed")?;
        // (user, nonce, compositionId, units, deadline, approved, queuedBy, createdAt)
        let (user, nonce, composition_id, units, deadline, approved, queued_by, _created_at) = p;

        let remote_lock_id = self
            .handles
            .read
            .pending_remote_lock_id(pending_id)
            .call()
            .await
            .unwrap_or_default();
        let remote_anchor_txid: Vec<[u8; 32]> = self
            .handles
            .read
            .pending_remote_anchor_txid(pending_id)
            .call()
            .await
            .unwrap_or_default();

        Ok(PendingSummary {
            user: address_to_wide(user),
            nonce,
            composition_id,
            units,
            deadline: deadline as i64,
            approved,
            queued_by: (queued_by != [0u8; 32]).then_some(queued_by),
            remote_lock_id,
            remote_anchor_txid_le: remote_anchor_txid
                .into_iter()
                .map(|t| (t != [0u8; 32]).then_some(t))
                .collect(),
        })
    }

    /// Scans `PendingApproved` events over the recent lookback window and
    /// returns each distinct pending's current live state.
    async fn scan_recent_pending(&self) -> Result<Vec<PendingSummary>> {
        let tip = self.handles.read.client().get_block_number().await?;
        let from = tip.saturating_sub(EVENT_LOOKBACK_BLOCKS.into());

        let events: Vec<hub_bindings::PendingApprovedFilter> = self
            .handles
            .read
            .event::<hub_bindings::PendingApprovedFilter>()
            .from_block(from.as_u64())
            .to_block(tip.as_u64())
            .query()
            .await
            .context("PendingApproved log query failed")?;

        let mut seen = std::collections::HashSet::new();
        let mut out = Vec::new();
        for ev in events {
            if !seen.insert(ev.pending_id) {
                continue;
            }
            match self.read_pending(ev.pending_id).await {
                Ok(p) if p.user != [0u8; 32] => out.push(p),
                Ok(_) => {}, // already exercised/expired — user zeroed out
                Err(e) => {
                    tracing::warn!(
                        pending_id = ?ev.pending_id,
                        error = %e,
                        "failed to read pending state for scanned event"
                    );
                },
            }
        }
        Ok(out)
    }
}

pub fn new_hub_adapter(
    handles: HubHandles,
    ipow_headers_address: String,
) -> Arc<dyn BetaHubAdapter> {
    Arc::new(EvmBetaHubAdapter { handles, ipow_headers_address })
}
