//! The iPoW protocol on an EVM network, behind the node's one interface.

use alloy::eips::BlockId;
use alloy::network::EthereumWallet;
use alloy::primitives::{Address, Bytes, FixedBytes, U256};
use alloy::providers::{DynProvider, Provider, ProviderBuilder};
use alloy::signers::local::PrivateKeySigner;
use anyhow::Context;
use async_trait::async_trait;
use ipow_protocol_core::network::ProtocolNetwork;
use ipow_protocol_core::types::{
    Amount, BlockRef, Challenge, ChallengeKind, Evidence, Job, JobStatus, Proof, StoredBlock,
};

use crate::contracts::{iPoWLightClient, iPoWProtocol, IPoWLightClient, IPoWProtocol};

/// Headers per `extend`. The light client takes up to MAX_WALK (100), at
/// about 76,000 gas each (V7); 50 keeps one transaction well inside a block.
const MAX_EXTEND: usize = 50;

/// How long to wait for a transaction to be included before sending it
/// again with a higher fee.
const RECEIPT_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(90);
/// How much more each new try pays: a replacement needs at least 10% more.
const FEE_STEP: (u128, u128) = (13, 10);
/// A fee 30% higher, and at least 0.01 gwei more: a tip of zero must grow
/// too.
fn more(fee: u128) -> u128 {
    fee * FEE_STEP.0 / FEE_STEP.1 + 10_000_000
}

/// Tries of one transaction, each paying more: 1.3^5, about 3.7 times the
/// first fee at the last.
const TRIES: usize = 6;

/// The gas a transaction is sent with: the estimate and a margin. Only
/// `openJob` depends on the gas price (V14), and the node never opens jobs.
pub(crate) fn with_margin(estimate: u64) -> u64 {
    estimate + estimate / 4 + 30_000
}

pub struct EvmNetwork {
    receipt_timeout: std::time::Duration,
    /// The nonce and fees of the last transaction sent: a transaction with
    /// the same nonce replaces it only if it pays more.
    last_fees: std::sync::Mutex<Option<(u64, u128, u128)>>,
    name: String,
    provider: DynProvider,
    me: Address,
    protocol: Address,
    light_client: Address,
}

fn to_u128(v: U256) -> anyhow::Result<u128> {
    u128::try_from(v).map_err(|_| anyhow::anyhow!("the amount {v} does not fit 128 bits"))
}

fn to_ref(r: &iPoWProtocol::BlockRef) -> Option<BlockRef> {
    if r.hash.is_zero() {
        return None;
    }
    Some(BlockRef { hash: r.hash.0, height: r.height, epoch_time: r.epochTime })
}

fn p_ref(r: &BlockRef) -> iPoWProtocol::BlockRef {
    iPoWProtocol::BlockRef { hash: FixedBytes(r.hash), height: r.height, epochTime: r.epoch_time }
}

fn status_of(code: u8) -> anyhow::Result<JobStatus> {
    // The order of `JobStatus` in iPoWProtocol.sol.
    Ok(match code {
        1 => JobStatus::Auction,
        2 => JobStatus::Expired,
        3 => JobStatus::Assigned,
        4 => JobStatus::Proven,
        5 => JobStatus::Slashed,
        6 => JobStatus::Settled,
        other => anyhow::bail!("unknown job status {other}"),
    })
}

/// Whether a node refused a transaction because one with its nonce pays as
/// much or more.
fn is_underpriced(message: &str) -> bool {
    let m = message.to_lowercase();
    m.contains("underpriced") || m.contains("already known") || m.contains("replacement fee too low")
}

/// Why a call was rejected: the contract's error by name when it has one.
pub(crate) fn rejection(e: alloy::contract::Error) -> anyhow::Error {
    if let Some(err) = e.as_decoded_interface_error::<IPoWProtocol::IPoWProtocolErrors>() {
        return anyhow::anyhow!("rejected: {err:?}");
    }
    if let Some(err) = e.as_decoded_interface_error::<IPoWLightClient::IPoWLightClientErrors>() {
        return anyhow::anyhow!("rejected by the light client: {err:?}");
    }
    // Errors named alike in the protocol and Conversion (such as NotLinked)
    // share a selector; the name is given without saying which contract.
    if let Some(err) = e.as_decoded_interface_error::<crate::contracts::Conversion::ConversionErrors>() {
        return anyhow::anyhow!("rejected: {err:?}");
    }
    if let Some(err) = e.as_decoded_interface_error::<crate::contracts::IPoWVault::IPoWVaultErrors>() {
        return anyhow::anyhow!("rejected by the vault: {err:?}");
    }
    anyhow::Error::new(e).context("rejected")
}

/// Tries a call, then sends it with the gas it needs and waits for it to
/// be included. A call the contract would reject is not sent, so it costs
/// no gas, and the error names the reason. A transaction reverted anyway,
/// because the state moved between the two, is an error too.
///
/// The try runs in the next block, as the transaction will: a guardian's
/// note counts only from the block after it was sealed. A network that
/// tries in the latest block instead rejects that one round early.
macro_rules! send {
    // `match` keeps the call's temporaries alive for every step.
    ($net:expr, $call:expr) => {
        match $call.block(BlockId::pending()) {
            call => {
                call.call().await.map_err(rejection)?;
                let gas = call.estimate_gas().await.map_err(rejection)?;
                let receipt = $net.send_tx(call.gas(with_margin(gas)).into_transaction_request()).await?;
                anyhow::ensure!(receipt.status(), "the transaction was reverted");
                receipt
            }
        }
    };
}

pub(crate) use send;

impl EvmNetwork {
    /// `rpc_url` and `key_hex` come from the environment variables the
    /// settings name; they are never logged.
    pub fn connect(name: &str, rpc_url: &str, key_hex: &str, light_client: &str, protocol: &str) -> anyhow::Result<Self> {
        let signer: PrivateKeySigner = key_hex.trim().parse().context("the key is not a valid private key")?;
        let me = signer.address();
        // Every transaction asks the network for its nonce. A nonce kept in
        // memory moves on even when a transaction is refused, and every
        // later transaction then waits behind the gap.
        let provider = ProviderBuilder::default()
            .with_gas_estimation()
            .with_simple_nonce_management()
            .fetch_chain_id()
            .wallet(EthereumWallet::from(signer))
            .connect_http(rpc_url.parse().context("the endpoint is not a valid URL")?)
            .erased();
        Ok(EvmNetwork {
            receipt_timeout: RECEIPT_TIMEOUT,
            last_fees: std::sync::Mutex::new(None),
            name: name.to_string(),
            provider,
            me,
            protocol: protocol.parse().context("the protocol address is not valid")?,
            light_client: light_client.parse().context("the light client address is not valid")?,
        })
    }

    /// For tests: how long to wait before paying more.
    pub fn set_receipt_timeout(&mut self, timeout: std::time::Duration) {
        self.receipt_timeout = timeout;
    }

    /// Sends a transaction and waits for it to be mined. It takes the lowest
    /// nonce not mined yet, so it replaces a transaction left waiting by an
    /// earlier round instead of queueing behind it. While it is not mined it
    /// is sent again, with the same nonce and a fee 30% higher each time: a
    /// gas price spike cannot hold the node's transactions back.
    pub(crate) async fn send_tx(&self, request: alloy::rpc::types::TransactionRequest) -> anyhow::Result<alloy::rpc::types::TransactionReceipt> {
        let nonce = self.provider.get_transaction_count(self.me).latest().await?;
        let estimate = self.provider.estimate_eip1559_fees().await?;
        let (mut fee, mut tip) = (estimate.max_fee_per_gas, estimate.max_priority_fee_per_gas);
        let last = *self.last_fees.lock().unwrap();
        if let Some((n, f, p)) = last
            && n == nonce
        {
            // A transaction with this nonce may still wait: pay more than it.
            fee = fee.max(more(f));
            tip = tip.max(more(p));
        }
        let mut sent = vec![];
        for _ in 0..TRIES {
            let tx = request.clone().nonce(nonce).max_fee_per_gas(fee).max_priority_fee_per_gas(tip);
            match self.provider.send_transaction(tx).await {
                Ok(pending) => {
                    sent.push(*pending.tx_hash());
                    *self.last_fees.lock().unwrap() = Some((nonce, fee, tip));
                }
                // A waiting transaction with this nonce pays as much or more.
                Err(e) if is_underpriced(&e.to_string()) => {
                    tracing::debug!(network = %self.name, error = %e, "a transaction with this nonce pays more");
                }
                Err(e) => {
                    if let Some(r) = self.mined(&sent).await? {
                        return Ok(r);
                    }
                    return Err(anyhow::Error::new(e).context("sending the transaction failed"));
                }
            }
            // Wait for any of its tries to be mined.
            let until = std::time::Instant::now() + self.receipt_timeout;
            while std::time::Instant::now() < until {
                if let Some(r) = self.mined(&sent).await? {
                    return Ok(r);
                }
                tokio::time::sleep((self.receipt_timeout / 8).max(std::time::Duration::from_millis(50))).await;
            }
            if let Some(r) = self.mined(&sent).await? {
                return Ok(r);
            }
            // The nonce was used by another transaction of this node, from an
            // earlier round: this one is not needed in this form any more.
            if self.provider.get_transaction_count(self.me).latest().await? > nonce {
                anyhow::bail!("another transaction of this node took its place; the next round decides again");
            }
            tracing::debug!(network = %self.name, nonce, "not mined yet; paying more");
            fee = more(fee);
            tip = more(tip);
        }
        anyhow::bail!("the transaction was not mined, even after paying more {} times", TRIES - 1)
    }

    /// The receipt of whichever of these transactions was mined.
    async fn mined(&self, hashes: &[alloy::primitives::TxHash]) -> anyhow::Result<Option<alloy::rpc::types::TransactionReceipt>> {
        for h in hashes {
            if let Some(r) = self.provider.get_transaction_receipt(*h).await? {
                return Ok(Some(r));
            }
        }
        Ok(None)
    }

    pub(crate) fn provider(&self) -> &DynProvider {
        &self.provider
    }

    pub fn address(&self) -> Address {
        self.me
    }

    pub(crate) fn protocol(&self) -> IPoWProtocol::IPoWProtocolInstance<&DynProvider> {
        IPoWProtocol::new(self.protocol, &self.provider)
    }

    pub(crate) fn light(&self) -> IPoWLightClient::IPoWLightClientInstance<&DynProvider> {
        IPoWLightClient::new(self.light_client, &self.provider)
    }

    /// What a guardian's note holds as evidence, as the contract computes it.
    async fn evidence(&self, e: &Evidence) -> anyhow::Result<[u8; 32]> {
        let p = self.protocol();
        Ok(match e {
            Evidence::MissedDuty => [0u8; 32],
            Evidence::Parent(h) => p.parentEvidence(FixedBytes(*h)).call().await?.0,
            Evidence::Fork(h) => p.forkEvidence(FixedBytes(*h)).call().await?.0,
        })
    }
}

#[async_trait]
impl ProtocolNetwork for EvmNetwork {
    fn name(&self) -> &str {
        &self.name
    }

    fn me(&self) -> String {
        self.me.to_string()
    }

    async fn now(&self) -> anyhow::Result<i64> {
        let block = self
            .provider
            .get_block_by_number(alloy::eips::BlockNumberOrTag::Latest)
            .await?
            .context("the network returned no latest block")?;
        Ok(block.header.timestamp as i64)
    }

    async fn jobs_after(&self, after: u64, limit: usize) -> anyhow::Result<Vec<Job>> {
        let count = self.protocol().jobCount().call().await?.to::<u64>();
        let mut out = vec![];
        for id in (after + 1)..=count {
            if out.len() >= limit {
                break;
            }
            out.push(self.job(id).await?);
        }
        Ok(out)
    }

    async fn job(&self, id: u64) -> anyhow::Result<Job> {
        let p = self.protocol();
        let id_u = U256::from(id);
        let job = p.getJob(id_u).call().await?;
        let status = status_of(p.statusOf(id_u).call().await?)?;
        let duty = p.getDuty(id_u).call().await?;
        let operator = (job.operator != Address::ZERO).then(|| job.operator.to_string());
        let attester = (duty.attester != Address::ZERO).then(|| duty.attester.to_string());
        Ok(Job {
            id,
            application: job.application.to_string(),
            tag: job.tag.0,
            escrow: to_u128(job.escrow)?,
            commitment_fee: to_u128(job.commitmentFee)?,
            escrow_fee: to_u128(job.escrowFee)?,
            bid: to_u128(job.bid)?,
            operator,
            confirmations: job.confirmations,
            claim_kind: job.claimKind,
            status,
            auction_end: p.auctionEndOf(id_u).call().await?.to::<i64>(),
            deadline: p.deadlineOf(id_u).call().await?.to::<i64>(),
            anchor: to_ref(&duty.anchor),
            proof_block: to_ref(&duty.proofBlock),
            tip: to_ref(&duty.tip),
            txid: (!duty.txid.is_zero()).then_some(duty.txid.0),
            deepest: to_ref(&duty.deepest),
            parents_shown: duty.parentsShown,
            lock_end: duty.lockEnd.to::<i64>(),
            attester,
            open_challenges: p.openChallengesOf(id_u).call().await?.to::<u32>(),
        })
    }

    async fn my_bond(&self) -> anyhow::Result<(Amount, Amount)> {
        let bond = self.protocol().bondOf(self.me).call().await?;
        Ok((to_u128(bond.bond)?, to_u128(bond.locked)?))
    }

    async fn my_credit(&self) -> anyhow::Result<Amount> {
        to_u128(self.protocol().credit(self.me).call().await?)
    }

    async fn parent_deposit(&self, job_id: u64) -> anyhow::Result<Amount> {
        to_u128(self.protocol().parentDepositOf(U256::from(job_id)).call().await?)
    }

    async fn minimum_bid(&self, job_id: u64) -> anyhow::Result<Amount> {
        to_u128(self.protocol().minimumBidOf(U256::from(job_id)).call().await?)
    }

    async fn challenge_count(&self) -> anyhow::Result<u64> {
        Ok(self.protocol().challengeCount().call().await?.to::<u64>())
    }

    async fn challenge(&self, id: u64) -> anyhow::Result<Option<Challenge>> {
        let c = self.protocol().getChallenge(U256::from(id)).call().await?;
        // The order of `ChallengeKind` in iPoWProtocol.sol: None, Parent, Fork.
        let kind = match c.kind {
            0 => return Ok(None),
            1 => ChallengeKind::Parent,
            2 => ChallengeKind::Fork,
            other => anyhow::bail!("unknown challenge kind {other}"),
        };
        Ok(Some(Challenge {
            id,
            kind,
            job_id: c.jobId.to::<u64>(),
            guardian: c.guardian.to_string(),
            deposit: to_u128(c.deposit)?,
            opened_at: c.openedAt.to::<i64>(),
            asked: to_ref(&c.asked),
        }))
    }

    async fn stored_block(&self, at: &BlockRef) -> anyhow::Result<Option<StoredBlock>> {
        let l = self.light();
        let id = l.nodeId(FixedBytes(at.hash), at.height, at.epoch_time).call().await?;
        let n: iPoWLightClient::Node = l.getNode(id).call().await?;
        if n.storedAt.is_zero() {
            return Ok(None);
        }
        Ok(Some(StoredBlock {
            prev_hash: n.prevHash.0,
            merkle_root: n.merkleRoot.0,
            bits: n.bits,
            time: n.time,
            stored_at: n.storedAt.to::<i64>(),
        }))
    }

    async fn add_epoch_start(&self, headers: &[[u8; 80]], height: u32) -> anyhow::Result<()> {
        send!(self, self.light().addEpochStart(Bytes::from(headers.concat()), height));
        Ok(())
    }

    async fn jump(&self, header: &[u8; 80], epoch_start: &BlockRef, height: u32) -> anyhow::Result<()> {
        let l = self.light();
        let es = l.nodeId(FixedBytes(epoch_start.hash), epoch_start.height, epoch_start.epoch_time).call().await?;
        send!(self, l.jump(Bytes::copy_from_slice(header), es, height));
        Ok(())
    }

    async fn extend(&self, parent: &BlockRef, headers: &[[u8; 80]]) -> anyhow::Result<()> {
        // The contract takes at most MAX_WALK headers at a time.
        let mut on = *parent;
        for chunk in headers.chunks(MAX_EXTEND) {
            send!(self, self.light().extend(Bytes::from(chunk.concat()), on.height, on.epoch_time));
            for h in chunk {
                on = on.child(h);
            }
        }
        Ok(())
    }

    async fn extend_back(&self, child: &BlockRef, header: &[u8; 80], prev_epoch_time: u32) -> anyhow::Result<()> {
        send!(self, self.light().extendBack(
            Bytes::copy_from_slice(header),
            FixedBytes(child.hash),
            child.height,
            child.epoch_time,
            prev_epoch_time
        ));
        Ok(())
    }

    async fn lock_bond(&self, amount: Amount) -> anyhow::Result<()> {
        send!(self, self.protocol().lockBond(U256::from(amount)).value(U256::from(amount)));
        Ok(())
    }

    async fn withdraw_bond(&self, amount: Amount) -> anyhow::Result<()> {
        send!(self, self.protocol().withdrawBond(U256::from(amount)));
        Ok(())
    }

    async fn bid(&self, job_id: u64, amount: Amount) -> anyhow::Result<()> {
        send!(self, self.protocol().bid(U256::from(job_id), U256::from(amount)));
        Ok(())
    }

    async fn chain_head_commitment(&self) -> anyhow::Result<[u8; 32]> {
        Ok(self.protocol().chainHeadCommitment(self.me).call().await?.0)
    }

    async fn chain_head(&self) -> anyhow::Result<Option<([u8; 32], u32)>> {
        let h = self.protocol().chainHeadOf(self.me).call().await?;
        Ok(h.set.then_some((h.txid.0, h.vout)))
    }

    async fn tag_payload(&self, tag: &[u8; 32]) -> anyhow::Result<[u8; 32]> {
        Ok(self.protocol().tagPayload(FixedBytes(*tag)).call().await?.0)
    }

    async fn register_chain_head(&self, block: &BlockRef, raw_tx: &[u8], siblings: &[[u8; 32]], tx_index: u64, coin_index: u32, tag_index: u32) -> anyhow::Result<()> {
        send!(self, self.protocol().registerChainHead(
            p_ref(block),
            Bytes::copy_from_slice(raw_tx),
            siblings.iter().map(|s| FixedBytes(*s)).collect(),
            U256::from(tx_index),
            coin_index,
            tag_index
        ));
        Ok(())
    }

    async fn advance_chain_head(&self, block: &BlockRef, raw_tx: &[u8], siblings: &[[u8; 32]], tx_index: u64, head_index: u32) -> anyhow::Result<()> {
        send!(self, self.protocol().advanceChainHead(
            p_ref(block),
            Bytes::copy_from_slice(raw_tx),
            siblings.iter().map(|s| FixedBytes(*s)).collect(),
            U256::from(tx_index),
            head_index
        ));
        Ok(())
    }

    async fn anchor_job(&self, job_id: u64, anchor: &BlockRef) -> anyhow::Result<()> {
        send!(self, self.protocol().anchorJob(U256::from(job_id), p_ref(anchor)));
        Ok(())
    }

    async fn prove_job(&self, job_id: u64, proof: &Proof) -> anyhow::Result<()> {
        let p = iPoWProtocol::Proof {
            proofBlock: p_ref(&proof.proof_block),
            tip: p_ref(&proof.tip),
            prevEpochTime: proof.prev_epoch_time,
            rawTx: Bytes::copy_from_slice(&proof.raw_tx),
            siblings: proof.siblings.iter().map(|s| FixedBytes(*s)).collect(),
            txIndex: U256::from(proof.tx_index),
            headIndex: proof.head_index,
            tagIndex: proof.tag_index,
        };
        send!(self, self.protocol().proveJob(U256::from(job_id), p));
        Ok(())
    }

    async fn settle(&self, job_id: u64) -> anyhow::Result<()> {
        send!(self, self.protocol().settle(U256::from(job_id)));
        Ok(())
    }

    async fn attest(&self, job_id: u64) -> anyhow::Result<()> {
        send!(self, self.protocol().attest(U256::from(job_id)));
        Ok(())
    }

    async fn seal_note(&self, job_id: u64, evidence: &Evidence, salt: &[u8; 32]) -> anyhow::Result<()> {
        let p = self.protocol();
        let note = p
            .noteFor(self.me, U256::from(job_id), FixedBytes(self.evidence(evidence).await?), FixedBytes(*salt))
            .call()
            .await?;
        send!(self, p.sealNote(note));
        Ok(())
    }

    async fn report_missed_duty(&self, job_id: u64, salt: &[u8; 32]) -> anyhow::Result<()> {
        send!(self, self.protocol().reportMissedDuty(U256::from(job_id), FixedBytes(*salt)));
        Ok(())
    }

    async fn ask_parent(&self, job_id: u64, salt: &[u8; 32], deposit: Amount) -> anyhow::Result<u64> {
        let p = self.protocol();
        let receipt = send!(self, p.askParent(U256::from(job_id), FixedBytes(*salt)).value(U256::from(deposit)));
        // The id from the receipt: another challenge may have opened since.
        receipt
            .inner
            .logs()
            .iter()
            .find_map(|l| l.log_decode::<IPoWProtocol::ParentAsked>().ok())
            .map(|e| e.inner.data.challengeId.to::<u64>())
            .ok_or_else(|| anyhow::anyhow!("no ParentAsked event in the receipt"))
    }

    async fn show_parent(&self, challenge_id: u64, prev_epoch_time: u32) -> anyhow::Result<()> {
        send!(self, self.protocol().showParent(U256::from(challenge_id), prev_epoch_time));
        Ok(())
    }

    async fn challenge_fork(
        &self,
        job_id: u64,
        operator_block: &BlockRef,
        guardian_block: &BlockRef,
        guardian_tip: &BlockRef,
        prev_epoch_time: u32,
        salt: &[u8; 32],
        deposit: Amount,
    ) -> anyhow::Result<u64> {
        let p = self.protocol();
        let receipt = send!(self, p
            .challengeFork(
                U256::from(job_id),
                p_ref(operator_block),
                p_ref(guardian_block),
                p_ref(guardian_tip),
                prev_epoch_time,
                FixedBytes(*salt)
            )
            .value(U256::from(deposit)));
        receipt
            .inner
            .logs()
            .iter()
            .find_map(|l| l.log_decode::<IPoWProtocol::ForkChallenged>().ok())
            .map(|e| e.inner.data.challengeId.to::<u64>())
            .ok_or_else(|| anyhow::anyhow!("no ForkChallenged event in the receipt"))
    }

    async fn extend_branch(&self, challenge_id: u64, guardian_side: bool, from: &BlockRef, new_tip: &BlockRef, prev_epoch_time: u32) -> anyhow::Result<()> {
        send!(self, self.protocol().extendBranch(U256::from(challenge_id), guardian_side, p_ref(from), p_ref(new_tip), prev_epoch_time));
        Ok(())
    }

    async fn resolve_challenge(&self, challenge_id: u64) -> anyhow::Result<()> {
        send!(self, self.protocol().resolveChallenge(U256::from(challenge_id)));
        Ok(())
    }

    async fn withdraw_credit(&self) -> anyhow::Result<()> {
        send!(self, self.protocol().withdrawCredit());
        Ok(())
    }
}
