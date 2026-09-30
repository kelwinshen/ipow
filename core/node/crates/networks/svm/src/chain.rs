//! How the adapter reaches Solana: read an account, send instructions, read
//! the clock. The node uses the RPC; tests use an in-process Solana.

use anchor_lang::prelude::Pubkey;
use anchor_lang::solana_program::instruction::Instruction;
use async_trait::async_trait;
use solana_client::client_error::ClientErrorKind;
use solana_client::nonblocking::rpc_client::RpcClient;
use solana_client::rpc_request::{RpcError, RpcResponseErrorData};
use solana_sdk::clock::Clock;
use solana_commitment_config::CommitmentConfig;
use solana_sdk::hash::Hash;
use solana_sdk::message::{AddressLookupTableAccount, VersionedMessage, v0};
use solana_sdk::signature::Keypair;
use solana_sdk::signer::Signer;
use solana_sdk::transaction::{Transaction, VersionedTransaction};

/// The largest transaction Solana accepts, in bytes.
pub const PACKET_DATA_SIZE: usize = 1232;

/// A signed transaction of the instructions, after the compute budget. With
/// a lookup table, the accounts it holds take one byte each instead of 32.
pub fn build(instructions: &[Instruction], payer: &Keypair, blockhash: Hash, table: Option<&AddressLookupTableAccount>) -> anyhow::Result<VersionedTransaction> {
    let ixs: Vec<Instruction> = compute_budget().into_iter().chain(instructions.iter().cloned()).collect();
    Ok(match table {
        None => Transaction::new_signed_with_payer(&ixs, Some(&payer.pubkey()), &[payer], blockhash).into(),
        Some(t) => {
            let message = v0::Message::try_compile(&payer.pubkey(), &ixs, std::slice::from_ref(t), blockhash)?;
            VersionedTransaction::try_new(VersionedMessage::V0(message), &[payer])?
        }
    })
}

/// The size of the transaction as sent.
pub fn size(tx: &VersionedTransaction) -> anyhow::Result<usize> {
    Ok(bincode::serialize(tx)?.len())
}

/// Compute units for every transaction: the most Solana allows. Walking and
/// checking headers needs many.
const COMPUTE_UNITS: u32 = 1_400_000;
/// The priority fee, in micro-lamports per compute unit: 10,000 lamports on
/// top of the signature fee at the full limit. Without one, a busy network
/// can drop a proof or an answer while the clock runs.
const PRIORITY_PRICE: u64 = 7_000;

fn compute_budget_ix(data: Vec<u8>) -> Instruction {
    Instruction { program_id: "ComputeBudget111111111111111111111111111111".parse().unwrap(), accounts: vec![], data }
}

/// The compute-budget instructions every transaction starts with: the
/// limit and the priority fee.
pub fn compute_budget() -> Vec<Instruction> {
    let mut limit = vec![2u8];
    limit.extend_from_slice(&COMPUTE_UNITS.to_le_bytes());
    let mut price = vec![3u8];
    price.extend_from_slice(&PRIORITY_PRICE.to_le_bytes());
    vec![compute_budget_ix(limit), compute_budget_ix(price)]
}

/// The program's error name from its logs, e.g. "DeadlineNotPassed".
pub fn error_name(logs: &[String]) -> Option<String> {
    logs.iter().find_map(|l| l.split("Error Code: ").nth(1).map(|s| s.split('.').next().unwrap_or(s).to_string()))
}

#[async_trait]
pub trait Chain: Send + Sync {
    /// The data of an account, or `None` when it does not exist.
    async fn account(&self, key: &Pubkey) -> anyhow::Result<Option<Vec<u8>>>;
    /// The data of an account as of Solana's last finalized block: what a
    /// record about Solana's own facts must rest on, since a confirmed
    /// block can still be rolled back.
    async fn account_final(&self, key: &Pubkey) -> anyhow::Result<Option<Vec<u8>>> {
        self.account(key).await
    }
    /// The program that owns an account, and its lamports, or `None` when it
    /// does not exist.
    async fn owner_and_lamports(&self, key: &Pubkey) -> anyhow::Result<Option<(Pubkey, u64)>>;
    /// Sends the instructions in one transaction paid and signed by
    /// `payer`, through `table` when given. An error names the program's
    /// error when there is one.
    async fn send(&self, instructions: &[Instruction], payer: &Keypair, table: Option<&AddressLookupTableAccount>) -> anyhow::Result<()>;
    async fn clock(&self) -> anyhow::Result<Clock>;
    /// Waits until the network is past `slot`: a lookup table extended in a
    /// slot can be used from the next one.
    async fn wait_past(&self, slot: u64) -> anyhow::Result<()>;
    /// The lookup tables whose authority is `authority`: found again after a
    /// restart, so that their rent is not lost.
    async fn tables_of(&self, authority: &Pubkey) -> anyhow::Result<Vec<Pubkey>>;
}

/// Where a lookup table's authority sits in its account: after the
/// discriminator (4 bytes), two slots (16), an index (1) and the option tag
/// (1).
pub const TABLE_AUTHORITY_OFFSET: usize = 22;

pub struct Rpc(RpcClient);

impl Rpc {
    pub fn new(url: &str) -> Self {
        Rpc(RpcClient::new_with_commitment(url.to_string(), CommitmentConfig::confirmed()))
    }
}

#[async_trait]
impl Chain for Rpc {
    async fn account(&self, key: &Pubkey) -> anyhow::Result<Option<Vec<u8>>> {
        Ok(self.0.get_account_with_commitment(key, self.0.commitment()).await?.value.map(|a| a.data))
    }

    async fn account_final(&self, key: &Pubkey) -> anyhow::Result<Option<Vec<u8>>> {
        Ok(self.0.get_account_with_commitment(key, CommitmentConfig::finalized()).await?.value.map(|a| a.data))
    }

    async fn owner_and_lamports(&self, key: &Pubkey) -> anyhow::Result<Option<(Pubkey, u64)>> {
        Ok(self.0.get_account_with_commitment(key, self.0.commitment()).await?.value.map(|a| (a.owner, a.lamports)))
    }

    async fn send(&self, instructions: &[Instruction], payer: &Keypair, table: Option<&AddressLookupTableAccount>) -> anyhow::Result<()> {
        let blockhash = self.0.get_latest_blockhash().await?;
        let tx = build(instructions, payer, blockhash, table)?;
        // The RPC tries the transaction first; a rejection costs nothing.
        match self.0.send_and_confirm_transaction(&tx).await {
            Ok(_) => Ok(()),
            Err(e) => {
                if let ClientErrorKind::RpcError(RpcError::RpcResponseError {
                    data: RpcResponseErrorData::SendTransactionPreflightFailure(sim),
                    ..
                }) = e.kind()
                    && let Some(name) = sim.logs.as_deref().and_then(error_name)
                {
                    anyhow::bail!("rejected by the program: {name}");
                }
                Err(anyhow::Error::new(e).context("the transaction failed"))
            }
        }
    }

    async fn clock(&self) -> anyhow::Result<Clock> {
        let data = self
            .0
            .get_account_with_commitment(&solana_sdk::sysvar::clock::ID, self.0.commitment())
            .await?
            .value
            .ok_or_else(|| anyhow::anyhow!("no clock"))?
            .data;
        Ok(bincode::deserialize(&data)?)
    }

    async fn wait_past(&self, slot: u64) -> anyhow::Result<()> {
        for _ in 0..100 {
            if self.0.get_slot().await? > slot {
                return Ok(());
            }
            tokio::time::sleep(std::time::Duration::from_millis(400)).await;
        }
        anyhow::bail!("the network did not move past slot {slot}")
    }

    async fn tables_of(&self, authority: &Pubkey) -> anyhow::Result<Vec<Pubkey>> {
        use solana_client::rpc_config::{RpcAccountInfoConfig, RpcProgramAccountsConfig};
        use solana_client::rpc_filter::{Memcmp, RpcFilterType};
        let config = RpcProgramAccountsConfig {
            filters: Some(vec![RpcFilterType::Memcmp(Memcmp::new_raw_bytes(TABLE_AUTHORITY_OFFSET, authority.to_bytes().to_vec()))]),
            account_config: RpcAccountInfoConfig { commitment: Some(self.0.commitment()), ..Default::default() },
            ..Default::default()
        };
        let found = self.0.get_program_ui_accounts_with_config(&solana_address_lookup_table_interface::program::ID, config).await?;
        Ok(found.into_iter().map(|(k, _)| k).collect())
    }
}
