//! The protocol's vault on an EVM network (`contracts/protocol/iPoWVault.sol`,
//! spec section 11), for the node's operator and guardian.

use std::sync::Arc;

use alloy::eips::BlockId;
use alloy::primitives::{Address, Bytes, FixedBytes, U256};
use alloy::providers::Provider;
use alloy::rpc::types::Filter;
use alloy::sol_types::SolEvent;
use async_trait::async_trait;
use ipow_protocol_core::types::{Amount, BlockRef};
use ipow_protocol_core::vault::{decode, Chain, Claim, Lock, Record, Request, TxProof, VaultApp, ETHEREUM};

use crate::contracts::vault::{iPoWProtocol, iPoWVault, IPoWVault};
use crate::contracts::IPoWProtocol;
use crate::network::{rejection, send, with_margin, EvmNetwork};

pub struct EvmVault {
    net: Arc<EvmNetwork>,
    address: Address,
}

impl EvmVault {
    pub fn new(net: Arc<EvmNetwork>, address: &str) -> anyhow::Result<Self> {
        Ok(EvmVault { net, address: address.parse().map_err(|_| anyhow::anyhow!("the vault address is not valid"))? })
    }

    fn contract(&self) -> IPoWVault::IPoWVaultInstance<&alloy::providers::DynProvider> {
        IPoWVault::new(self.address, self.net.provider())
    }

    fn operator(s: &str) -> anyhow::Result<Address> {
        s.parse().map_err(|_| anyhow::anyhow!("{s} is not an address"))
    }
}

fn v_ref(r: &BlockRef) -> iPoWProtocol::BlockRef {
    iPoWProtocol::BlockRef { hash: FixedBytes(r.hash), height: r.height, epochTime: r.epoch_time }
}

/// What a walk between two blocks needs: the older epoch's time when they
/// lie in two epochs, zero otherwise. A walk of at most 100 blocks crosses
/// at most one boundary.
fn prev_epoch_time(high: &BlockRef, low: &BlockRef) -> u32 {
    let epoch = ipow_protocol_core::types::EPOCH_BLOCKS;
    if high.height / epoch != low.height / epoch { low.epoch_time } else { 0 }
}

fn btc(tx: &TxProof) -> iPoWVault::BitcoinTx {
    iPoWVault::BitcoinTx {
        block: v_ref(&tx.block),
        rawTx: Bytes::copy_from_slice(&tx.raw_tx),
        siblings: tx.siblings.iter().map(|s| FixedBytes(*s)).collect(),
        txIndex: U256::from(tx.tx_index),
        real: v_ref(&tx.real),
        prevEpochTime: prev_epoch_time(&tx.real, &tx.block),
    }
}

fn to_u128(v: U256) -> anyhow::Result<u128> {
    u128::try_from(v).map_err(|_| anyhow::anyhow!("the amount {v} does not fit 128 bits"))
}

const GWEI: u128 = 1_000_000_000;

fn to_gwei(v: U256) -> anyhow::Result<u64> {
    u64::try_from(to_u128(v)? / GWEI).map_err(|_| anyhow::anyhow!("the amount {v} does not fit in gwei"))
}

#[async_trait]
impl VaultApp for EvmVault {
    fn network_id(&self) -> u8 {
        ETHEREUM
    }

    fn me(&self) -> String {
        self.net.address().to_string()
    }

    fn me_bytes(&self) -> Vec<u8> {
        self.net.address().to_vec()
    }

    async fn deposit(&self) -> anyhow::Result<Amount> {
        to_u128(self.contract().deposit().call().await?)
    }

    async fn pair_commitment(&self, peer: &[u8]) -> anyhow::Result<[u8; 32]> {
        let peer: [u8; 32] = peer.try_into().map_err(|_| anyhow::anyhow!("an operator on Solana is 32 bytes"))?;
        Ok(self.contract().pairCommitment(self.net.address(), FixedBytes(peer)).call().await?.0)
    }

    async fn chain(&self, operator: &str) -> anyhow::Result<Option<Chain>> {
        let c = self.contract().getChain(Self::operator(operator)?).call().await?;
        if !c.registered {
            return Ok(None);
        }
        Ok(Some(Chain {
            peer: c.peerOperator.0.to_vec(),
            coin: (c.coinTxid.0, c.coinVout),
            messages: c.messages,
            exited: c.exited,
            slashed: c.slashed,
            refused: c.refused,
            bond: to_u128(c.bond)?,
            stated: to_u128(c.stated)?,
            peer_bond: to_gwei(c.peerBond)?,
            peer_bond_carried: c.peerBondCarried,
            open_value: to_gwei(c.openValue)?,
            open_claims: c.openClaims,
            deposits: to_u128(c.deposits)?,
        }))
    }

    async fn register_chain(&self, peer: &[u8], tx: &TxProof, coin_index: u32, tag_index: u32) -> anyhow::Result<()> {
        let peer: [u8; 32] = peer.try_into().map_err(|_| anyhow::anyhow!("an operator on Solana is 32 bytes"))?;
        send!(self.net, self.contract().registerChain(FixedBytes(peer), btc(tx), coin_index, tag_index));
        Ok(())
    }

    async fn add_bond(&self, amount: u128) -> anyhow::Result<()> {
        send!(self.net, self.contract().addBond().value(U256::from(amount)));
        Ok(())
    }

    async fn add_deposits(&self, amount: Amount) -> anyhow::Result<()> {
        send!(self.net, self.contract().addDeposits().value(U256::from(amount)));
        Ok(())
    }

    async fn is_real(&self, block: &BlockRef) -> anyhow::Result<bool> {
        let id = self.net.light().nodeId(FixedBytes(block.hash), block.height, block.epoch_time).call().await?;
        Ok(self.contract().isReal(id).call().await?)
    }

    async fn min_certifying_escrow(&self) -> anyhow::Result<Amount> {
        to_u128(self.contract().minCertifyingEscrow().call().await?)
    }

    async fn record_real_from_job(&self, job_id: u64) -> anyhow::Result<()> {
        send!(self.net, self.contract().recordRealFromJob(U256::from(job_id)));
        Ok(())
    }

    async fn record_real(&self, low: &BlockRef, high: &BlockRef) -> anyhow::Result<()> {
        send!(self.net, self.contract().recordReal(v_ref(low), v_ref(high), prev_epoch_time(high, low)));
        Ok(())
    }

    async fn open_checkpoint(&self, confirmations: u16, paid: Amount) -> anyhow::Result<u64> {
        let receipt = send!(self.net, self.contract().openCheckpoint(confirmations).value(U256::from(paid)));
        for log in receipt.inner.logs() {
            if let Ok(e) = IPoWProtocol::JobOpened::decode_log(&log.inner) {
                return Ok(e.data.jobId.to::<u64>());
            }
        }
        anyhow::bail!("the checkpoint's job was not opened")
    }

    async fn submit_message(&self, operator: &str, tx: &TxProof, input_index: u32, tag_index: u32, batch: &[u8]) -> anyhow::Result<()> {
        send!(
            self.net,
            self.contract().submitMessage(Self::operator(operator)?, btc(tx), input_index, tag_index, Bytes::copy_from_slice(batch))
        );
        Ok(())
    }

    async fn final_locks_after(&self, after: u64, limit: usize) -> anyhow::Result<Vec<Lock>> {
        // A lock's number is only settled once its block is final: a
        // reorganisation could give the number another lock (section 11.5).
        let count = self.contract().lockCount().block(BlockId::finalized()).call().await?.to::<u64>();
        let mut out = vec![];
        for id in (after + 1)..=count {
            if out.len() >= limit {
                break;
            }
            if let Some(l) = self.lock(id).await? {
                out.push(l);
            }
        }
        Ok(out)
    }

    async fn lock(&self, id: u64) -> anyhow::Result<Option<Lock>> {
        let l = self.contract().getLock(U256::from(id)).call().await?;
        if l.owner == Address::ZERO {
            return Ok(None);
        }
        Ok(Some(Lock { id, amount: l.amount, recipient: l.recipient.0, fee: l.fee, fee_paid: l.feePaid, returned: l.returned }))
    }

    async fn requests_after(&self, _after: u64, _limit: usize) -> anyhow::Result<Vec<Request>> {
        Ok(vec![])
    }

    async fn request(&self, _id: u64) -> anyhow::Result<Option<Request>> {
        Ok(None)
    }

    async fn given_up(&self, _id: u64) -> anyhow::Result<bool> {
        Ok(false)
    }

    async fn receipt_issued(&self, _id: u64) -> anyhow::Result<bool> {
        Ok(false)
    }

    async fn request_paid(&self, id: u64) -> anyhow::Result<bool> {
        Ok(self.contract().requestPaid(id).call().await?)
    }

    async fn claim_count(&self) -> anyhow::Result<u64> {
        Ok(self.contract().claimCount().call().await?.to::<u64>())
    }

    async fn claim_status(&self, id: u64) -> anyhow::Result<Claim> {
        let c = self.contract().getClaim(U256::from(id)).call().await?;
        anyhow::ensure!(c.operator != Address::ZERO, "claim {id} does not exist");
        Ok(Claim {
            id,
            operator: c.operator.to_string(),
            last_at: c.lastAt.to::<i64>(),
            held: c.held,
            decided: c.decided,
            accepted: c.accepted,
            records: vec![],
        })
    }

    async fn claim(&self, id: u64) -> anyhow::Result<Claim> {
        let c = self.contract().getClaim(U256::from(id)).call().await?;
        anyhow::ensure!(c.operator != Address::ZERO, "claim {id} does not exist");
        // The batch is named in an event of the block the claim opened in;
        // only its acting records belong to the claim here.
        let filter = Filter::new()
            .address(self.address)
            .event_signature(IPoWVault::ClaimBatch::SIGNATURE_HASH)
            .topic1(U256::from(id))
            .from_block(c.openedBlock)
            .to_block(c.openedBlock);
        let logs = self.net.provider().get_logs(&filter).await?;
        let log = logs.first().ok_or_else(|| anyhow::anyhow!("the batch of claim {id} was not found"))?;
        let batch = IPoWVault::ClaimBatch::decode_log(&log.inner)?.data.batch;
        let records = decode(&batch)
            .unwrap_or_default()
            .into_iter()
            .filter(|r| match r {
                Record::Request { .. } | Record::Cancel { .. } => true,
                Record::Bond { network, .. } => *network != ETHEREUM,
                _ => false,
            })
            .collect();
        Ok(Claim {
            id,
            operator: c.operator.to_string(),
            last_at: c.lastAt.to::<i64>(),
            held: c.held,
            decided: c.decided,
            accepted: c.accepted,
            records,
        })
    }

    async fn object(&self, id: u64) -> anyhow::Result<()> {
        let d = self.contract().deposit().call().await?;
        send!(self.net, self.contract().object(U256::from(id)).value(d));
        Ok(())
    }

    async fn answer(&self, id: u64) -> anyhow::Result<()> {
        let d = self.contract().deposit().call().await?;
        send!(self.net, self.contract().answer(U256::from(id)).value(d));
        Ok(())
    }

    async fn decide(&self, id: u64) -> anyhow::Result<()> {
        send!(self.net, self.contract().decide(U256::from(id)));
        Ok(())
    }

    async fn collect(&self, id: u64) -> anyhow::Result<()> {
        let c = self.contract();
        let claim = c.getClaim(U256::from(id)).call().await?;
        let me = self.net.address();
        let mine = if claim.accepted {
            c.answersOf(U256::from(id), me).call().await?
        } else {
            c.objectionsOf(U256::from(id), me).call().await?
        };
        if mine == 0 {
            return Ok(());
        }
        send!(self.net, c.collect(U256::from(id)));
        Ok(())
    }
}
