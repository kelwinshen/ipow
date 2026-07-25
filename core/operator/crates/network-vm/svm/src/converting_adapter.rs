use std::{collections::HashMap, sync::Arc};

use anchor_lang::solana_program::system_program;
use anchor_lang::{AccountDeserialize, InstructionData};
use async_trait::async_trait;
use ethers::types::{Bytes, H160, U256};
use ipow_core::{
    btc::btc_service::{
        BitcoinMerkleProofPayload, btc_tip_height,
        check_confirmation_and_build_proof, send_to_user_program,
    },
    consts::{
        supported_network_enum::SupportedNetwork,
        transaction_phase::TransactionPhase, transaction_type::TransactionType,
    },
    dependencies::context::CoreContext,
    models::conversion::Conversion as CoreConversion,
    traits::{
        chain_provider_adapter::{ChainProviderAdapter, TxIdFilter},
        converting_adapter::ConvertingAdapter,
    },
};
use solana_sdk::{
    instruction::{AccountMeta, Instruction},
    pubkey::Pubkey,
    signer::Signer,
    transaction::Transaction,
};
use futures::stream::{self, StreamExt};
use tracing::{error, info, warn};

// Caps how many read-only RPC calls we fan out at once when fetching per-tx_id
// data concurrently, so a large batch doesn't hammer a rate-limited provider.
const REMOTE_FETCH_CONCURRENCY: usize = 8;

use crate::{
    bindings::ipow_types::ipow::{
        accounts::Conversion as SolConversion,
        client::args::SubmitBitcoinProofCache, types::ConversionStatus,
    },
    dependencies::context::SolanaContext,
};
use anyhow::Result;

pub struct SolanaConvertingAdapter {
    pub ctx: Arc<SolanaContext>,
    pub core_ctx: Arc<CoreContext>,
    pub chain_provider: Arc<dyn ChainProviderAdapter>,
}

impl SolanaConvertingAdapter {
    fn map_to_core(sol: SolConversion) -> CoreConversion {
        // Safe mapping of Solana 32-byte Pubkey to EVM-style Address (first 20 bytes)
        let user_bytes = sol.user.to_bytes();
        let mut eth_addr = [0u8; 20];
        eth_addr.copy_from_slice(&user_bytes[0..20]);

        CoreConversion {
            user: H160::from(eth_addr),
            is_native_to_bitcoin: sol.is_native_to_bitcoin,
            user_program: Bytes::from(sol.user_program),
            ipow_receive_program: Bytes::from(sol.ipow_receive_program),
            network_address: Bytes::from(sol.network_address),
            network_id: U256::from(sol.network_id),
            native_amount: U256::from(sol.native_amount),
            bitcoin_amount: U256::from(sol.bitcoin_amount),
            commit_fee: U256::from(sol.commit_fee),
            reserved_native: U256::from(sol.reserved_native),
            created_at: U256::from(sol.created_at as u64),
            approved_at: U256::from(sol.approved_at as u64),
            deposited_at: U256::from(sol.deposited_at as u64),
            operator_duty_expires_at: U256::from(
                sol.operator_duty_expires_at as u64,
            ),
            approved: matches!(
                sol.status,
                ConversionStatus::Approved
                    | ConversionStatus::Deposited
                    | ConversionStatus::Completed
            ),
            deposited: sol.deposited_at > 0,
            completed: matches!(sol.status, ConversionStatus::Completed),
            refunded: matches!(sol.status, ConversionStatus::Refunded),
        }
    }
}

#[async_trait]
impl ConvertingAdapter for SolanaConvertingAdapter {
    async fn mark_processed(
        &self,
        tx_id: U256,
        btc_tx_id: Option<String>,
    ) -> Result<()> {
        let tx_id_str = tx_id.to_string();
        let btc_id = btc_tx_id.unwrap_or_else(|| "unknown".to_string());
        let network = self.ctx.cfg.network.string_identifier();

        info!(tx_id = %tx_id, btc_tx_id = %btc_id, "Marking transaction as processed in storage");

        if let Err(e) = self
            .core_ctx
            .redis_storage
            .set_conversion_processed(network, &tx_id_str, &btc_id)
            .await
        {
            error!(tx_id = %tx_id, error = %e, "Failed to mark transaction as processed");
            return Err(e.into());
        }

        info!(tx_id = %tx_id, "Transaction marked successfully");
        Ok(())
    }

    async fn find_native_to_btc_ready(
        &self,
        to_tx_id: U256,
        dest_network: Option<SupportedNetwork>,
    ) -> Result<Vec<(U256, CoreConversion)>> {
        let mut ready = Vec::new();
        let tx_types = [
            // TransactionType::NATIVE_TO_BITCOIN, //Temporary turn off not to send BTC to user
            TransactionType::NATIVE_TO_NATIVE_OUT,
        ];

        for tx_type in tx_types {
            let active_ids: Vec<U256> = self
                .chain_provider
                .get_tx_ids_by_filter(TxIdFilter {
                    type_filter: tx_type,
                    phase_filter: TransactionPhase::ACTIVE_WAITING_PROOF,
                    dest_network,
                    to_tx_id,
                    ..Default::default()
                })
                .await?;

            if active_ids.is_empty() {
                continue;
            }

            info!(
                tx_type = ?tx_type,
                count = active_ids.len(),
                "Processing active IDs from contract"
            );

            let ctx = self.ctx.clone();
            let fetched: Vec<(U256, Option<Vec<u8>>)> = stream::iter(active_ids)
                .map(|tx_id| {
                    let ctx = ctx.clone();
                    async move {
                        let tx_id_u64 = tx_id.as_u64();
                        let (conversion_pda, _) = Pubkey::find_program_address(
                            &[b"conversion", &tx_id_u64.to_le_bytes()],
                            &ctx.program_id,
                        );

                        match ctx
                            .rpc_client
                            .get_account_data(&conversion_pda)
                            .await
                        {
                            Ok(data) => (tx_id, Some(data)),
                            Err(e) => {
                                warn!(tx_id = %tx_id, error = %e, "Conversion PDA not found");
                                (tx_id, None)
                            },
                        }
                    }
                })
                .buffer_unordered(REMOTE_FETCH_CONCURRENCY)
                .collect()
                .await;

            for (tx_id, account_data) in fetched {
                let Some(account_data) = account_data else {
                    continue;
                };

                let sol_conv = match SolConversion::try_deserialize(
                    &mut &account_data[..],
                ) {
                    Ok(conv) => conv,
                    Err(e) => {
                        warn!(tx_id = %tx_id, error = %e, "Failed to deserialize conversion");
                        continue;
                    },
                };

                if tx_type == TransactionType::NATIVE_TO_BITCOIN
                    && !sol_conv.is_native_to_bitcoin
                {
                    continue;
                }

                if !matches!(
                    sol_conv.status,
                    ConversionStatus::Approved | ConversionStatus::Deposited
                ) {
                    continue;
                }

                if sol_conv.deposited_at == 0 {
                    continue;
                }

                let core_conv = Self::map_to_core(sol_conv);
                ready.push((tx_id, core_conv));
            }
        }

        if !ready.is_empty() {
            info!(
                to_tx_id = %to_tx_id,
                count = ready.len(),
                tx_ids = ?ready.iter().map(|r| r.0).collect::<Vec<_>>(),
                "Found ready conversions awaiting payout (mapped to Core)"
            );
        }

        Ok(ready)
    }

    async fn find_btc_to_native_completed(
        &self,
        to_tx_id: U256,
        dest_network: Option<SupportedNetwork>,
    ) -> Result<Vec<(U256, CoreConversion)>> {
        let mut completed = Vec::new();
        let network = self.ctx.cfg.network.string_identifier();

        for tx_type in [
            TransactionType::BITCOIN_TO_NATIVE,
            TransactionType::NATIVE_TO_NATIVE_IN,
        ] {
            let mut ids: Vec<U256> = self
                .chain_provider
                .get_tx_ids_by_filter(TxIdFilter {
                    type_filter: tx_type,
                    phase_filter: TransactionPhase::COMPLETED,
                    dest_network,
                    to_tx_id,
                    ..Default::default()
                })
                .await?;

            completed.append(&mut ids);
        }

        info!(
            to_tx_id = %to_tx_id,
            count = completed.len(),
            tx_ids = ?completed,
            "Contract returned COMPLETED BTC→Native txs"
        );

        let mut ready = Vec::new();
        let completed_id_strs: Vec<String> =
            completed.iter().map(|tx_id| tx_id.to_string()).collect();

        let processed_flags = self
            .core_ctx
            .redis_storage
            .filter_processed_ids(network, &completed_id_strs)
            .await?;

        for (idx, tx_id) in completed.iter().enumerate() {
            if processed_flags[idx] {
                continue;
            }

            let tx_id_u64 = tx_id.as_u64();
            let (conversion_pda, _) = Pubkey::find_program_address(
                &[b"conversion", &tx_id_u64.to_le_bytes()],
                &self.ctx.program_id,
            );

            let account_data = match self
                .ctx
                .rpc_client
                .get_account_data(&conversion_pda)
                .await
            {
                Ok(data) => data,
                Err(_) => continue,
            };

            let sol_conv =
                match SolConversion::try_deserialize(&mut &account_data[..]) {
                    Ok(conv) => conv,
                    Err(_) => continue,
                };

            if sol_conv.is_native_to_bitcoin {
                continue;
            }

            if !matches!(sol_conv.status, ConversionStatus::Completed) {
                continue;
            }

            ready.push((*tx_id, Self::map_to_core(sol_conv)));
        }

        if !ready.is_empty() {
            info!(
                to_tx_id = %to_tx_id,
                count = ready.len(),
                tx_ids = ?ready.iter().map(|r| r.0).collect::<Vec<_>>(),
                "Found BTC→Native conversions completed (user got Native)"
            );
        }

        Ok(ready)
    }

    async fn handle_native_to_btc_conversion(
        &self,
        tx_id: U256,
        conv: CoreConversion,
    ) -> Result<()> {
        let user_program: Vec<u8> = if conv.user_program.0.is_empty() {
            vec![]
        } else {
            conv.user_program.0.to_vec()
        };

        let program_status =
            if user_program.is_empty() { "empty" } else { "non-empty" };
        info!(
            amount_sats = %conv.bitcoin_amount,
            program_status, "Sending BTC to user's script program"
        );

        let amount_sats: u64 =
            conv.bitcoin_amount.try_into().map_err(|_| {
                anyhow::anyhow!("btc_amount overflow: {}", conv.bitcoin_amount)
            })?;

        let anchor = self.chain_provider.anchor_info(tx_id).await?;
        let btc_tip = btc_tip_height(&self.core_ctx).await?;
        let btc_tip_u256 = U256::from(btc_tip);
        let limit = anchor.anchor_height + U256::from(20);

        if btc_tip_u256 < limit {
            match send_to_user_program(
                &self.core_ctx,
                &user_program,
                amount_sats,
            )
            .await
            {
                Ok(result) => {
                    info!(
                        tx_id = %tx_id,
                        btc_txid = %result,
                        "BTC sent successfully to user program"
                    );
                    self.mark_processed(tx_id, Some(result)).await?;
                },
                Err(e) => {
                    let err_msg = e.to_string();

                    if err_msg.contains("Not enough funds") {
                        warn!(tx_id = %tx_id, "Insufficient BTC. Triggering provider-level emergency sweep.");

                        if let Err(e) =
                            self.chain_provider.trigger_btc_sweep().await
                        {
                            error!(tx_id = %tx_id, error = %e, "Emergency sweep execution failed");
                        } else {
                            info!(tx_id = %tx_id, "Emergency sweep completed successfully.");
                        }
                    }
                    error!(error = %e, tx_id = %tx_id, "Failed to send BTC to user program");
                    return Err(e.into());
                },
            }
        } else {
            warn!(
                %tx_id,
                btc_tip,
                anchor_height = %anchor.anchor_height,
                "BTC tip height too high relative to anchor; skipping send"
            );
        }

        Ok(())
    }

    async fn handle_btc_to_native_conversion(&self, tx_id: U256) -> Result<()> {
        info!(%tx_id, "Marking BTC→NATIVE conversion as processed in storage.");
        let status_str = "processed_btc_to_native".to_string();
        self.mark_processed(tx_id, Some(status_str.clone())).await?;
        info!(%tx_id, status = %status_str, "Successfully marked conversion.");
        Ok(())
    }

    async fn check_confirmation_and_build_proof(
        &self,
        tx_id: U256,
        btc_txid: &str,
    ) -> Result<Option<BitcoinMerkleProofPayload>> {
        check_confirmation_and_build_proof(&self.core_ctx, tx_id, btc_txid)
            .await
    }

    async fn submit_merkle_proof(
        &self,
        tx_id: U256,
        proof: BitcoinMerkleProofPayload,
    ) -> Result<()> {
        let tx_id_u64 = tx_id.as_u64();
        let target_height = proof.block_height.as_u64();
        tracing::info!(
            tx_id = %tx_id,
            block_height = target_height,
             "Submitting merkle proof cache to Solana program"
        );

        let (global_state_pda, _) =
            Pubkey::find_program_address(&[b"global_state"], &self.ctx.program_id);
        let (escrow_vault_pda, _) =
            Pubkey::find_program_address(&[b"escrow"], &self.ctx.program_id);
        let (conversion_pda, _) = Pubkey::find_program_address(
            &[b"conversion", &tx_id_u64.to_le_bytes()],
            &self.ctx.program_id,
        );
        let (proof_cache_pda, _) = Pubkey::find_program_address(
            &[b"proof", &tx_id_u64.to_le_bytes()],
            &self.ctx.program_id,
        );
        let (height_tracker_pda, _) = Pubkey::find_program_address(
            &[b"tracker", &target_height.to_le_bytes()],
            &self.ctx.program_id,
        );

        let (expected_header_pda, _) = Pubkey::find_program_address(
            &[b"header", &target_height.to_le_bytes()],
            &self.ctx.program_id,
        );

        let header_pda = if self
            .ctx
            .rpc_client
            .get_account(&expected_header_pda)
            .await
            .is_ok()
        {
            expected_header_pda
        } else {
            tracing::info!(
                tx_id = %tx_id,
                block_height = target_height,
                "Header not synced yet, passing Program self.ctx.program_id as None to cache proof for later"
            );
            self.ctx.program_id
        };

        let conv_data =
            self.ctx.rpc_client.get_account_data(&conversion_pda).await?;
        let mut user_bytes = [0u8; 32];
        user_bytes.copy_from_slice(&conv_data[16..48]);
        let user_pubkey = Pubkey::new_from_array(user_bytes);

        let mut branch_array = Vec::new();
        for b in &proof.branch {
            let mut buf = [0u8; 32];
            let len = std::cmp::min(b.len(), 32);
            buf[..len].copy_from_slice(&b[..len]);
            branch_array.push(buf);
        }

        let ix_args = SubmitBitcoinProofCache {
            tx_raw: proof.legacy_tx.to_vec(),
            vout_index: proof.vout_index.as_u64(),
            proof_block_height: target_height,
            branch_le: branch_array,
            index: proof.index.as_u64(),
        };
        let data = ix_args.data();

        let ix = Instruction {
            program_id: self.ctx.program_id,
            accounts: vec![
                AccountMeta::new(global_state_pda, false),
                AccountMeta::new(escrow_vault_pda, false),
                AccountMeta::new(conversion_pda, false),
                AccountMeta::new(proof_cache_pda, false),
                AccountMeta::new(height_tracker_pda, false),
                AccountMeta::new_readonly(header_pda, false),
                AccountMeta::new(self.ctx.operator.pubkey(), false),
                AccountMeta::new(user_pubkey, false),
                AccountMeta::new(self.ctx.operator.pubkey(), true),
                AccountMeta::new_readonly(system_program::id(), false),
            ],
            data,
        };

        let compute_budget_program_id = std::str::FromStr::from_str(
            "ComputeBudget111111111111111111111111111111",
        )
        .unwrap();

        let mut compute_data = vec![2u8];
        compute_data.extend_from_slice(&1_000_000u32.to_le_bytes());

        let compute_budget_ix = Instruction {
            program_id: compute_budget_program_id,
            accounts: vec![],
            data: compute_data,
        };

        let bh = self.ctx.rpc_client.get_latest_blockhash().await?;
        let tx = Transaction::new_signed_with_payer(
            &[compute_budget_ix, ix],
            Some(&self.ctx.operator.pubkey()),
            &[&*self.ctx.operator],
            bh,
        );

        match self.ctx.rpc_client.send_and_confirm_transaction(&tx).await {
            Ok(sig) => {
                tracing::info!(
                    tx_id = %tx_id,
                    contract_tx_hash = ?sig,
                    "Merkle proof cache submitted successfully (Inline Verified!)"
                );
                Ok(())
            },
            Err(e) => {
                tracing::error!(tx_id = %tx_id, error = %e, "Failed to send merkle proof cache transaction");
                Err(anyhow::anyhow!("RPC Error: {}", e))
            },
        }
    }

    async fn get_processed_native_to_btc(
        &self,
        tx_ids: &[U256],
    ) -> Result<HashMap<U256, String>> {
        if tx_ids.is_empty() {
            return Ok(HashMap::new());
        }

        let tx_id_strs: Vec<String> =
            tx_ids.iter().map(|id| id.to_string()).collect();
        let network = self.ctx.cfg.network.string_identifier();

        let btc_tx_ids = self
            .core_ctx
            .redis_storage
            .get_btc_tx_ids(network, &tx_id_strs)
            .await?;

        let mut result = HashMap::new();

        for (i, btc_tx_id) in btc_tx_ids.into_iter().enumerate() {
            if let Some(btc_id) = btc_tx_id {
                if btc_id != "unknown" && btc_id != "processed_btc_to_native" {
                    result.insert(tx_ids[i], btc_id);
                }
            }
        }

        Ok(result)
    }
}
