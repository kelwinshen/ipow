//! `BetaSpokeAdapter` impl against a real deployed `BetaVault`.

use crate::beta::context::VaultHandles;
use crate::network::EvmNetwork;
use anyhow::{Context, Result};
use ethers::types::U256;
use ipow_core::traits::beta_adapter::{
    BetaSpokeAdapter, PartyStatus, RemoteLockState,
};
use std::sync::Arc;

pub struct EvmBetaSpokeAdapter {
    pub handles: VaultHandles,
    pub ipow_headers_address: String,
    pub evm_network: EvmNetwork,
}

fn lock_state_from_u8(v: u8) -> RemoteLockState {
    match v {
        1 => RemoteLockState::Final,
        2 => RemoteLockState::Refunded,
        _ => RemoteLockState::Pending,
    }
}

#[async_trait::async_trait]
impl BetaSpokeAdapter for EvmBetaSpokeAdapter {
    fn ipow_headers_address(&self) -> String {
        self.ipow_headers_address.clone()
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
        let mut call = self.handles.write.process_anchor(
            party_id,
            statement.into(),
            tx_raw.into(),
            U256::from(block_height),
            branch_le,
            U256::from(index),
        );
        // Tempo's RPC rejects eth_estimateGas whenever EIP-1559 fee
        // fields are present in the estimation call — confirmed against
        // real deploy transactions there (design/ipow-implementation.md §8.15); an
        // explicit gas limit skips ethers-rs's automatic estimation
        // entirely, the same workaround `tempo/scripts/deploy_raw.mjs`
        // uses. Unverified for a *call* (only proven for deploys) — flag
        // any failure here as the first real signal either way.
        if self.evm_network == EvmNetwork::Tempo {
            call = call.gas(U256::from(2_000_000u64));
        }
        let pending_tx =
            call.send().await.context("processAnchor tx failed to send")?;
        let receipt = pending_tx
            .await
            .context("processAnchor tx failed to confirm")?
            .ok_or_else(|| anyhow::anyhow!("processAnchor: no receipt"))?;
        Ok(format!("{:?}", receipt.transaction_hash))
    }

    async fn get_lock_state(&self, lock_id: u64) -> Result<RemoteLockState> {
        let lock = self
            .handles
            .read
            .locks(lock_id)
            .call()
            .await
            .context("locks() call failed")?;
        Ok(lock_state_from_u8(lock.4))
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

pub fn new_spoke_adapter(
    handles: VaultHandles,
    ipow_headers_address: String,
    evm_network: EvmNetwork,
) -> Arc<dyn BetaSpokeAdapter> {
    Arc::new(EvmBetaSpokeAdapter { handles, ipow_headers_address, evm_network })
}
