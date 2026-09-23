//! `BetaHubAdapter` impl against the real, live `beta-factory` program —
//! Solana's counterpart to `evm-revm`'s `hub_adapter.rs`. Scoped to
//! exactly what the automated tick loop needs, mirroring the EVM
//! adapter's own scope: MINT-kind statement submission and exercising a
//! ready mint. VETO/ATTEST/CLEAR/ALIVE are auditor-triggered manual
//! actions on both chains, not part of this loop.
//!
//! Unlike `BetaHub.sol`, `beta-factory` emits no event for a newly
//! approved `Pending` — discovery here uses `getProgramAccounts` filtered
//! by `Pending`'s Anchor discriminator instead of an event-log scan. This
//! is a real, heavier RPC call (scans every live account of that shape
//! under the program); acceptable for now, same "v1 simplification, not
//! a permanent design" caveat the EVM adapter's own lookback-window scan
//! already carries.

use crate::beta::context::BetaSvmContext;
use crate::bindings::beta_factory_types::beta_factory;
use anchor_lang::solana_program::system_program;
use anchor_lang::{AccountDeserialize, Discriminator, InstructionData};
use anyhow::{Context, Result};
use ipow_core::consts::beta_statement::{MintStatement, KIND_MINT};
use ipow_core::traits::beta_adapter::{
    AnchorStatus as CoreAnchorStatus, BetaHubAdapter, ComponentInfo,
    PartyStatus, PendingSummary,
};
use sha2::{Digest, Sha256};
use solana_client::rpc_config::{RpcAccountInfoConfig, RpcProgramAccountsConfig};
use solana_client::rpc_filter::{Memcmp, RpcFilterType};
use solana_client::rpc_response::UiAccountEncoding;
use solana_sdk::{
    instruction::{AccountMeta, Instruction},
    pubkey::Pubkey,
    signer::Signer,
    transaction::Transaction,
};
use std::sync::Arc;

/// Well-known, fixed program ids — not worth a new crate dependency for
/// two constants. Identical on devnet and mainnet.
const SPL_TOKEN_PROGRAM: &str = "TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA";
const SPL_ASSOCIATED_TOKEN_PROGRAM: &str = "ATokenGPvbdGVxr1b2hvZbsiqW5xWH25efTNsLJA8knL";

fn spl_token_id() -> Pubkey {
    SPL_TOKEN_PROGRAM.parse().expect("valid hardcoded pubkey")
}

/// Standard ATA derivation: `[wallet, token_program, mint]` under the
/// associated-token program — replicated here rather than pulling in
/// `spl-associated-token-account` for one formula.
fn associated_token_address(wallet: &Pubkey, mint: &Pubkey) -> Pubkey {
    let ata_program: Pubkey =
        SPL_ASSOCIATED_TOKEN_PROGRAM.parse().expect("valid hardcoded pubkey");
    let token_program = spl_token_id();
    Pubkey::find_program_address(
        &[wallet.as_ref(), token_program.as_ref(), mint.as_ref()],
        &ata_program,
    )
    .0
}

pub struct SvmBetaHubAdapter {
    pub ctx: Arc<BetaSvmContext>,
}

fn txid_le_of(tx_raw: &[u8]) -> [u8; 32] {
    let once = Sha256::digest(tx_raw);
    let twice = Sha256::digest(once);
    let mut out = [0u8; 32];
    out.copy_from_slice(&twice);
    out
}

fn anchor_status_to_core(v: beta_factory::types::AnchorStatus) -> CoreAnchorStatus {
    use beta_factory::types::AnchorStatus as SolStatus;
    match v {
        SolStatus::Queued => CoreAnchorStatus::Queued,
        SolStatus::Exercised => CoreAnchorStatus::Exercised,
        SolStatus::Slashed => CoreAnchorStatus::Slashed,
        SolStatus::Skipped => CoreAnchorStatus::Skipped,
        // `Cancelled` (a v3 held-and-never-cleared MINT, settled away) has
        // no EVM equivalent in this enum — closest is None, since the
        // slot is no longer meaningfully "processed" for exercise
        // purposes.
        SolStatus::Cancelled => CoreAnchorStatus::None,
    }
}

fn pending_to_summary(pubkey: Pubkey, p: beta_factory::accounts::Pending) -> PendingSummary {
    let queued_by = (p.queued_by != [0u8; 32]).then_some(p.queued_by);
    let _ = pubkey;
    PendingSummary {
        user: p.user.to_bytes(),
        nonce: p.nonce,
        composition_id: p.composition_id,
        units: p.units,
        deadline: p.deadline,
        approved: p.approved,
        queued_by,
        remote_lock_id: p.remote_lock_id,
        remote_anchor_txid_le: p
            .remote_anchor_txid
            .into_iter()
            .map(|t| (t != [0u8; 32]).then_some(t))
            .collect(),
    }
}

impl SvmBetaHubAdapter {
    async fn fetch_pending(&self, pending_pda: Pubkey) -> Result<beta_factory::accounts::Pending> {
        let data = self
            .ctx
            .rpc_client
            .get_account_data(&pending_pda)
            .await
            .context("pending account fetch failed")?;
        beta_factory::accounts::Pending::try_deserialize(&mut &data[..])
            .context("pending account deserialize failed")
    }

    async fn fetch_processed_anchor(
        &self,
        txid_le: &[u8; 32],
    ) -> Result<Option<beta_factory::accounts::ProcessedAnchor>> {
        let pda = self.ctx.processed_anchor_pda(txid_le);
        match self.ctx.rpc_client.get_account_data(&pda).await {
            Ok(data) => Ok(Some(
                beta_factory::accounts::ProcessedAnchor::try_deserialize(&mut &data[..])
                    .context("processed anchor deserialize failed")?,
            )),
            Err(_) => Ok(None),
        }
    }

    /// All live `Pending` accounts under the hub program — see module doc
    /// for why this scans rather than following an event log.
    async fn scan_all_pending(&self) -> Result<Vec<(Pubkey, beta_factory::accounts::Pending)>> {
        let filters = vec![RpcFilterType::Memcmp(Memcmp::new_base58_encoded(
            0,
            &beta_factory::accounts::Pending::DISCRIMINATOR,
        ))];
        let config = RpcProgramAccountsConfig {
            filters: Some(filters),
            account_config: RpcAccountInfoConfig {
                encoding: Some(UiAccountEncoding::Base64),
                ..Default::default()
            },
            ..Default::default()
        };
        let accounts = self
            .ctx
            .rpc_client
            .get_program_accounts_with_config(&self.ctx.hub_program_id, config)
            .await
            .context("getProgramAccounts(Pending) failed")?;

        let mut out = Vec::with_capacity(accounts.len());
        for (pubkey, account) in accounts {
            match beta_factory::accounts::Pending::try_deserialize(&mut &account.data[..]) {
                Ok(p) => out.push((pubkey, p)),
                Err(e) => tracing::warn!(%pubkey, error = %e, "failed to deserialize Pending account"),
            }
        }
        Ok(out)
    }
}

#[async_trait::async_trait]
impl BetaHubAdapter for SvmBetaHubAdapter {
    fn ipow_headers_address(&self) -> String {
        self.ctx.ipow_program_id.to_string()
    }

    async fn get_composition(&self, composition_id: u64) -> Result<Vec<ComponentInfo>> {
        let pda = self.ctx.composition_pda(composition_id);
        let data = self
            .ctx
            .rpc_client
            .get_account_data(&pda)
            .await
            .context("composition account fetch failed")?;
        let comp = beta_factory::accounts::Composition::try_deserialize(&mut &data[..])
            .context("composition deserialize failed")?;

        Ok(comp
            .components
            .into_iter()
            .map(|c| ComponentInfo {
                network_id: c.network_id,
                token_id: c.token_id,
                amount_per_unit: c.amount_per_unit as u128,
                // network_id == 0 is Solana's own convention for "this
                // hub's local leg" (BetaHub.sol's SELF_NETWORK_ID plays
                // the same role on EVM, just non-zero there).
                is_local: c.network_id == 0,
            })
            .collect())
    }

    async fn find_claimable_pending(&self) -> Result<Vec<PendingSummary>> {
        let all = self.scan_all_pending().await?;
        Ok(all
            .into_iter()
            .filter(|(_, p)| p.approved && p.queued_by == [0u8; 32])
            .map(|(pk, p)| pending_to_summary(pk, p))
            .collect())
    }

    async fn get_pending(&self, user: [u8; 32], nonce: u64) -> Result<PendingSummary> {
        let user_pk = Pubkey::new_from_array(user);
        let pda = self.ctx.pending_pda(&user_pk, nonce);
        let p = self.fetch_pending(pda).await?;
        Ok(pending_to_summary(pda, p))
    }

    /// MINT-only — see module doc. `party_id`/`branch_le`/`index` are
    /// passed straight through; `block_height` doubles as the header
    /// PDA's own seed.
    async fn submit_process_anchor(
        &self,
        party_id: [u8; 32],
        statement: Vec<u8>,
        tx_raw: Vec<u8>,
        block_height: u64,
        branch_le: Vec<[u8; 32]>,
        index: u64,
    ) -> Result<String> {
        if statement.is_empty() || statement[0] != KIND_MINT {
            anyhow::bail!(
                "SvmBetaHubAdapter::submit_process_anchor only handles MINT statements this pass"
            );
        }
        let mint = MintStatement::decode(&statement)
            .context("failed to decode MINT statement")?;
        let sol_user = Pubkey::new_from_array(mint.target_user);
        let txid_le = txid_le_of(&tx_raw);

        let party_pda = self.ctx.party_pda();
        let party_data = self
            .ctx
            .rpc_client
            .get_account_data(&party_pda)
            .await
            .context("party account fetch failed")?;
        let party = beta_factory::accounts::Party::try_deserialize(&mut &party_data[..])
            .context("party deserialize failed")?;

        let processed_pda = self.ctx.processed_anchor_pda(&txid_le);
        let header_pda = self.ctx.header_pda(block_height);
        let pending_pda = self.ctx.pending_pda(&sol_user, mint.nonce);

        let args = beta_factory::client::args::ProcessAnchor {
            txid_le,
            statement,
            tx_raw,
            proof_block_height: block_height,
            branch_le,
            index,
        };
        let data = args.data();

        // Account order matches ProcessAnchor's #[derive(Accounts)]
        // declaration exactly, read directly from process_anchor.rs.
        // Kind-specific slots this adapter never populates (burn,
        // target_party, target_anchor, prior_party — VETO/RELEASE/
        // takeover only, see module doc) pass the program id itself,
        // Anchor's own "this Option is None" convention (same pattern
        // `converting_adapter.rs` already uses for its optional header
        // account).
        let none_placeholder = AccountMeta::new_readonly(self.ctx.hub_program_id, false);
        let accounts = vec![
            AccountMeta::new(self.ctx.config_pda, false),
            AccountMeta::new(party_pda, false),
            AccountMeta::new(party.owner, false),
            AccountMeta::new(processed_pda, false),
            AccountMeta::new_readonly(header_pda, false),
            AccountMeta::new(self.ctx.bond_escrow_pda, false),
            AccountMeta::new(self.ctx.insurance_pda, false),
            AccountMeta::new(self.ctx.reward_pool_pda, false),
            AccountMeta::new(self.ctx.fees_pda, false),
            AccountMeta::new(self.ctx.operator.pubkey(), true),
            AccountMeta::new_readonly(system_program::id(), false),
            AccountMeta::new(pending_pda, false),
            AccountMeta::new(sol_user, false),
            none_placeholder.clone(), // burn
            none_placeholder.clone(), // target_party
            none_placeholder.clone(), // target_anchor
            none_placeholder,         // prior_party
        ];

        let ix = Instruction { program_id: self.ctx.hub_program_id, accounts, data };
        let bh = self.ctx.rpc_client.get_latest_blockhash().await?;
        let tx = Transaction::new_signed_with_payer(
            &[ix],
            Some(&self.ctx.operator.pubkey()),
            &[&*self.ctx.operator],
            bh,
        );
        let sig = self
            .ctx
            .rpc_client
            .send_and_confirm_transaction(&tx)
            .await
            .context("processAnchor tx failed")?;
        let _ = party_id; // already encoded via party_pda; kept for trait-shape parity with the EVM adapter
        Ok(sig.to_string())
    }

    async fn get_anchor_status(&self, txid_le: [u8; 32]) -> Result<CoreAnchorStatus> {
        match self.fetch_processed_anchor(&txid_le).await? {
            Some(pa) => Ok(anchor_status_to_core(pa.status)),
            None => Ok(CoreAnchorStatus::None),
        }
    }

    async fn is_attested(&self, txid_le: [u8; 32]) -> Result<bool> {
        match self.fetch_processed_anchor(&txid_le).await? {
            Some(pa) => Ok(pa.attested_by != [0u8; 32]),
            None => Ok(false),
        }
    }

    async fn find_exercisable_pending(&self) -> Result<Vec<PendingSummary>> {
        let all = self.scan_all_pending().await?;
        let mut ready = Vec::new();
        for (pk, p) in all {
            if !p.approved || p.queued_by == [0u8; 32] {
                continue;
            }
            if p.remote_anchor_txid.iter().any(|t| *t == [0u8; 32]) {
                continue;
            }
            let mut all_ready = true;
            for txid in &p.remote_anchor_txid {
                let Some(pa) = self.fetch_processed_anchor(txid).await? else {
                    all_ready = false;
                    break;
                };
                if !matches!(pa.status, beta_factory::types::AnchorStatus::Queued) || pa.held {
                    all_ready = false;
                    break;
                }
                // Same conservative (never-false-positive) readiness check
                // the EVM adapter uses: attested is the fast path; the
                // window-closed path is also accepted on-chain even if
                // this says false.
                if pa.attested_by == [0u8; 32] {
                    all_ready = false;
                    break;
                }
            }
            if all_ready {
                ready.push(pending_to_summary(pk, p));
            }
        }
        Ok(ready)
    }

    /// Local-leg accounts (`beta_mint`/`user_beta`/`mint_authority`/spl
    /// vaults) aren't part of `BetaHubAdapter`'s chain-agnostic
    /// signature — resolved here directly from `FactoryConfig` and the
    /// ATA convention `exercise_mint.rs` itself uses.
    async fn exercise_mint(&self, user: [u8; 32], nonce: u64) -> Result<String> {
        let user_pk = Pubkey::new_from_array(user);
        let pending_pda = self.ctx.pending_pda(&user_pk, nonce);
        let pending = self.fetch_pending(pending_pda).await?;

        let config_data = self
            .ctx
            .rpc_client
            .get_account_data(&self.ctx.config_pda)
            .await
            .context("config account fetch failed")?;
        let config = beta_factory::accounts::FactoryConfig::try_deserialize(&mut &config_data[..])
            .context("config deserialize failed")?;

        let composition_pda = self.ctx.composition_pda(pending.composition_id);
        let user_beta_ata = associated_token_address(&user_pk, &config.beta_mint);
        let (mint_authority, _) =
            Pubkey::find_program_address(&[b"mint_authority"], &self.ctx.hub_program_id);

        let remote_count = pending.remote_lock_id.len();
        let party_slot = if remote_count > 0 {
            AccountMeta::new_readonly(self.ctx.party_pda(), false)
        } else {
            AccountMeta::new_readonly(self.ctx.hub_program_id, false)
        };

        let mut remote_slots = Vec::with_capacity(7);
        for i in 0..7 {
            if i < remote_count {
                let pda = self.ctx.processed_anchor_pda(&pending.remote_anchor_txid[i]);
                remote_slots.push(AccountMeta::new(pda, false));
            } else {
                remote_slots.push(AccountMeta::new_readonly(self.ctx.hub_program_id, false));
            }
        }

        let args = beta_factory::client::args::ExerciseMint {};
        let data = args.data();

        let mut accounts = vec![
            AccountMeta::new(self.ctx.config_pda, false),
            AccountMeta::new(pending_pda, false),
            AccountMeta::new_readonly(composition_pda, false),
            party_slot,
            AccountMeta::new(user_pk, false),
            AccountMeta::new(user_beta_ata, false),
            AccountMeta::new(config.beta_mint, false),
            AccountMeta::new_readonly(mint_authority, false),
            AccountMeta::new(self.ctx.fees_pda, false),
            AccountMeta::new_readonly(spl_token_id(), false),
            AccountMeta::new_readonly(system_program::id(), false),
        ];
        accounts.extend(remote_slots);

        let ix = Instruction { program_id: self.ctx.hub_program_id, accounts, data };
        let bh = self.ctx.rpc_client.get_latest_blockhash().await?;
        let tx = Transaction::new_signed_with_payer(
            &[ix],
            Some(&self.ctx.operator.pubkey()),
            &[&*self.ctx.operator],
            bh,
        );
        let sig = self
            .ctx
            .rpc_client
            .send_and_confirm_transaction(&tx)
            .await
            .context("exerciseMint tx failed")?;
        Ok(sig.to_string())
    }

    async fn get_own_party_status(&self, party_id: [u8; 32]) -> Result<PartyStatus> {
        let pda = Pubkey::find_program_address(&[b"party", &party_id], &self.ctx.hub_program_id).0;
        match self.ctx.rpc_client.get_account_data(&pda).await {
            Ok(data) => {
                let p = beta_factory::accounts::Party::try_deserialize(&mut &data[..])
                    .context("party deserialize failed")?;
                Ok(PartyStatus { exists: true, dead: p.dead, bond: p.bond as u128 })
            },
            Err(_) => Ok(PartyStatus { exists: false, dead: false, bond: 0 }),
        }
    }
}

pub fn new_hub_adapter(ctx: Arc<BetaSvmContext>) -> Arc<dyn BetaHubAdapter> {
    Arc::new(SvmBetaHubAdapter { ctx })
}
