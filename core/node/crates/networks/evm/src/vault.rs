//! The protocol's vault on an EVM network (`contracts/protocol/iPoWVault.sol`
//! and its parts `vault/VaultHome.sol` and `vault/VaultReceipts.sol`, spec
//! section 11), for the node's operator and guardian.

use std::sync::Arc;

use alloy::eips::BlockId;
use alloy::primitives::{Address, Bytes, FixedBytes, U256};
use alloy::providers::Provider;
use alloy::rpc::types::Filter;
use alloy::sol_types::SolEvent;
use async_trait::async_trait;
use ipow_protocol_core::types::{Amount, BlockRef};
use ipow_protocol_core::vault::{
    decode, AssetInfo, Chain, Claim, FastLock, Lock, Position, Record, Request, TxProof, VaultApp, SOLANA,
};
use tokio::sync::OnceCell;

use crate::contracts::vault::{iPoWProtocol, iPoWVault, IPoWVault};
use crate::contracts::{IPoWProtocol, VaultHome, VaultReceipts, IERC20};
use crate::network::{rejection, send, with_margin, EvmNetwork};

pub struct EvmVault {
    net: Arc<EvmNetwork>,
    address: Address,
    /// This network's number and the pair's other one, read from the vault
    /// (D132, D133).
    here: u8,
    peer: u8,
    parts: OnceCell<(Address, Address)>,
}

/// An asset, by its home network and number, as the core keys it.
fn key(home: u8, asset: u32) -> alloy::primitives::aliases::U40 {
    alloy::primitives::aliases::U40::from(((home as u64) << 32) | asset as u64)
}

impl EvmVault {
    /// The vault at `address`: one pair's, its two network numbers read from
    /// it.
    pub async fn connect(net: Arc<EvmNetwork>, address: &str) -> anyhow::Result<Self> {
        let address: Address = address.parse().map_err(|_| anyhow::anyhow!("the vault address is not valid"))?;
        let c = IPoWVault::new(address, net.provider());
        let (here, peer) = (c.here().call().await?, c.peer().call().await?);
        Ok(EvmVault { net, address, here, peer, parts: OnceCell::new() })
    }

    fn contract(&self) -> IPoWVault::IPoWVaultInstance<&alloy::providers::DynProvider> {
        IPoWVault::new(self.address, self.net.provider())
    }

    /// The vault's parts: `home` and `receipts`, made by the core.
    async fn parts(&self) -> anyhow::Result<(Address, Address)> {
        Ok(*self
            .parts
            .get_or_try_init(|| async {
                let c = self.contract();
                anyhow::Ok((c.home().call().await?, c.receipts().call().await?))
            })
            .await?)
    }

    async fn home(&self) -> anyhow::Result<VaultHome::VaultHomeInstance<&alloy::providers::DynProvider>> {
        Ok(VaultHome::new(self.parts().await?.0, self.net.provider()))
    }

    async fn receipts(&self) -> anyhow::Result<VaultReceipts::VaultReceiptsInstance<&alloy::providers::DynProvider>> {
        Ok(VaultReceipts::new(self.parts().await?.1, self.net.provider()))
    }

    /// The operator on the peer in the vault's 32 bytes: a Solana key as it
    /// is, an EVM address in the last 20 (section 11.3).
    fn peer_word(&self, peer: &[u8]) -> anyhow::Result<[u8; 32]> {
        match (self.peer == SOLANA, peer.len()) {
            (true, 32) => Ok(peer.try_into().unwrap()),
            (false, 20) => {
                let mut w = [0u8; 32];
                w[12..].copy_from_slice(peer);
                Ok(w)
            }
            _ => anyhow::bail!("an operator on network {} is {} bytes, not {}", self.peer, if self.peer == SOLANA { 32 } else { 20 }, peer.len()),
        }
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

fn record_bytes(r: &Record) -> Bytes {
    Bytes::from(r.bytes())
}

#[async_trait]
impl VaultApp for EvmVault {
    fn network_id(&self) -> u8 {
        self.here
    }

    fn peer_id(&self) -> u8 {
        self.peer
    }

    fn vault_id(&self) -> [u8; 32] {
        self.address.into_word().0
    }

    async fn peer_vault(&self) -> anyhow::Result<[u8; 32]> {
        Ok(self.contract().peerVault().call().await?.0)
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
        Ok(self.contract().pairCommitment(self.net.address(), FixedBytes(self.peer_word(peer)?)).call().await?.0)
    }

    async fn chain(&self, operator: &str) -> anyhow::Result<Option<Chain>> {
        let op = Self::operator(operator)?;
        let c = self.contract().getChain(op).call().await?;
        if !c.registered {
            return Ok(None);
        }
        let mut positions = vec![];
        for k in self.contract().assetsOf(op).call().await? {
            let k = k.to::<u64>();
            positions.push(self.position(operator, (k >> 32) as u8, k as u32).await?);
        }
        Ok(Some(Chain {
            // An EVM peer's operator is an address: the last 20 bytes.
            peer: if self.peer == SOLANA { c.peerOperator.0.to_vec() } else { c.peerOperator.0[12..].to_vec() },
            coin: (c.coinTxid.0, c.coinVout),
            messages: c.messages,
            exited: c.exited,
            slashed: c.slashed,
            refused: c.refused,
            positions,
            open_claims: c.openClaims,
            deposits: to_u128(c.deposits)?,
        }))
    }

    async fn position(&self, operator: &str, home: u8, asset: u32) -> anyhow::Result<Position> {
        let p = self.contract().getPosition(Self::operator(operator)?, key(home, asset)).call().await?;
        Ok(Position {
            home,
            asset,
            bond: p.bond,
            stated: p.stated,
            peer_bond: p.peerBond,
            peer_bond_carried: p.peerBondCarried,
            open_value: p.openValue,
        })
    }

    async fn register_chain(&self, peer: &[u8], tx: &TxProof, coin_index: u32, tag_index: u32) -> anyhow::Result<()> {
        send!(self.net, self.contract().registerChain(FixedBytes(self.peer_word(peer)?), btc(tx), coin_index, tag_index));
        Ok(())
    }

    async fn add_bond(&self, home: u8, asset: u32, amount: u64) -> anyhow::Result<()> {
        let mut value = U256::ZERO;
        if home == self.here {
            let a = self.home().await?.getAsset(asset).call().await?;
            let native = U256::from(amount) * a.unit;
            if a.token == Address::ZERO {
                value = native;
            } else {
                let token = IERC20::new(a.token, self.net.provider());
                if token.allowance(self.net.address(), self.address).call().await? < native {
                    send!(self.net, token.approve(self.address, U256::MAX));
                }
            }
        }
        send!(self.net, self.contract().addBond(key(home, asset), amount).value(value));
        Ok(())
    }

    async fn add_deposits(&self, amount: Amount) -> anyhow::Result<()> {
        send!(self.net, self.contract().addDeposits(U256::from(amount)).value(U256::from(amount)));
        Ok(())
    }

    async fn settle_slash(&self, operator: &str, home: u8, asset: u32) -> anyhow::Result<()> {
        send!(self.net, self.contract().settleSlash(Self::operator(operator)?, key(home, asset)));
        Ok(())
    }

    async fn slash_pending(&self, operator: &str, home: u8, asset: u32) -> anyhow::Result<u64> {
        Ok(self.contract().slashBacking(Self::operator(operator)?, key(home, asset)).call().await?)
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
        let receipt = send!(self.net, self.contract().openCheckpoint(confirmations, U256::from(paid)).value(U256::from(paid)));
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

    async fn message_batch(&self, operator: &str, index: u64) -> anyhow::Result<Option<Vec<u8>>> {
        let op = Self::operator(operator)?;
        let c = self.contract().getChain(op).call().await?;
        if index >= c.messages {
            return Ok(None);
        }
        // Each message's event names the block of the one before: walked
        // back from the last, one block at a time.
        let mut block = c.lastMessageBlock;
        while block != 0 {
            let filter = Filter::new()
                .address(self.address)
                .event_signature(IPoWVault::MessageBatch::SIGNATURE_HASH)
                .topic1(op.into_word())
                .from_block(block)
                .to_block(block);
            // Several messages may share a block: the lowest of them names the
            // block of the one before it.
            let mut lowest: Option<(u64, u64)> = None;
            for log in self.net.provider().get_logs(&filter).await? {
                let e = IPoWVault::MessageBatch::decode_log(&log.inner)?.data;
                if e.index == index {
                    return Ok(Some(e.batch.to_vec()));
                }
                if lowest.is_none_or(|(i, _)| e.index < i) {
                    lowest = Some((e.index, e.prevBlock));
                }
            }
            match lowest {
                Some((i, prev)) if i > index && prev != 0 && prev < block => block = prev,
                _ => break,
            }
        }
        Ok(None)
    }

    async fn assets(&self) -> anyhow::Result<Vec<AssetInfo>> {
        let home = self.home().await?;
        let count = home.assetCount().call().await?;
        let mut out = vec![];
        for n in 0..count {
            let a = home.getAsset(n).call().await?;
            out.push(AssetInfo { number: n, token: a.token.into_word().0, decimals: a.recordDecimals });
        }
        Ok(out)
    }

    async fn final_locks_after(&self, after: u64, limit: usize) -> anyhow::Result<Vec<Lock>> {
        // A lock's number is only settled once its block is final: a
        // reorganisation could give the number another lock (section 11.5).
        let count = self.home().await?.lockCount().block(BlockId::finalized()).call().await?;
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
        let l = self.home().await?.getLock(id).call().await?;
        if l.owner == Address::ZERO {
            return Ok(None);
        }
        Ok(Some(Lock {
            id,
            asset: l.asset,
            amount: l.amount,
            recipient: l.recipient.0,
            fee: l.fee,
            fast_fee: l.fastFee,
            locked_at: l.lockedAt,
            fee_paid: l.feePaid,
            returned: l.returned,
        }))
    }

    async fn take_fee(&self, _record: &Record) -> anyhow::Result<()> {
        // Credited when the message is processed: `withdraw_credit` takes it.
        Ok(())
    }

    async fn request_paid(&self, id: u64) -> anyhow::Result<bool> {
        Ok(self.home().await?.requestPaid(id).call().await?)
    }

    async fn has_receipt(&self, asset: u32) -> anyhow::Result<bool> {
        Ok(self.receipts().await?.receiptOf(asset).call().await? != Address::ZERO)
    }

    async fn receipt_balance(&self, asset: u32) -> anyhow::Result<u64> {
        let r = self.receipts().await?.receiptOf(asset).call().await?;
        if r == Address::ZERO {
            return Ok(0);
        }
        let b = IERC20::new(r, self.net.provider()).balanceOf(self.net.address()).call().await?;
        u64::try_from(b).map_err(|_| anyhow::anyhow!("a receipt balance fits 64 bits"))
    }

    async fn make_receipt(&self, claim: u64, record: &Record) -> anyhow::Result<()> {
        send!(self.net, self.receipts().await?.makeReceipt(U256::from(claim), record_bytes(record)));
        Ok(())
    }

    async fn requests_after(&self, after: u64, limit: usize) -> anyhow::Result<Vec<Request>> {
        // Final, as locks: a reorganisation could give the number another
        // burn.
        let count = self.receipts().await?.burnCount().block(BlockId::finalized()).call().await?;
        let mut out = vec![];
        for id in (after + 1)..=count {
            if out.len() >= limit {
                break;
            }
            if let Some(r) = self.request(id).await? {
                out.push(r);
            }
        }
        Ok(out)
    }

    async fn request(&self, id: u64) -> anyhow::Result<Option<Request>> {
        let b = self.receipts().await?.getBurn(id).call().await?;
        if b.owner == Address::ZERO {
            return Ok(None);
        }
        Ok(Some(Request {
            id,
            asset: b.asset,
            amount: b.amount,
            to: b.to.0,
            fee: b.fee,
            fast_fee: b.fastFee,
            requested_at: b.at,
            fee_paid: b.feePaid,
        }))
    }

    async fn given_up(&self, id: u64) -> anyhow::Result<bool> {
        Ok(self.receipts().await?.givenUp(id).block(BlockId::finalized()).call().await?)
    }

    async fn receipt_issued(&self, id: u64) -> anyhow::Result<bool> {
        Ok(self.receipts().await?.getMark(id).call().await?.issued)
    }

    async fn lock_attests(&self, id: u64) -> anyhow::Result<Option<(i64, Vec<FastLock>)>> {
        let m = self.receipts().await?.getMark(id).call().await?;
        if m.attests == 0 {
            return Ok(None);
        }
        // Settled in the order made: the open ones are the newest.
        let mut open = vec![];
        let mut at = m.lastAttest;
        while at != 0 {
            let Some(f) = self.fast_lock(at).await? else { break };
            at = f.prev;
            open.push(f);
        }
        Ok(Some((m.firstAt as i64, open)))
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
        // only its records from Solana belong to the claim here.
        let filter = Filter::new()
            .address(self.address)
            .event_signature(IPoWVault::ClaimBatch::SIGNATURE_HASH)
            .topic1(U256::from(id))
            .from_block(c.openedBlock)
            .to_block(c.openedBlock);
        let logs = self.net.provider().get_logs(&filter).await?;
        let log = logs.first().ok_or_else(|| anyhow::anyhow!("the batch of claim {id} was not found"))?;
        let batch = IPoWVault::ClaimBatch::decode_log(&log.inner)?.data.batch;
        let records = decode(&batch).unwrap_or_default().into_iter().filter(|r| r.fact() == Some(self.peer)).collect();
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

    async fn attest_lock(&self, record: &Record) -> anyhow::Result<u64> {
        let receipts = self.receipts().await?;
        let receipt = send!(self.net, receipts.attestLock(record_bytes(record)));
        // Its number from the event: another attest may land first.
        for log in receipt.inner.logs() {
            if let Ok(e) = VaultReceipts::Attested::decode_log(&log.inner) {
                return Ok(e.data.attestId);
            }
        }
        anyhow::bail!("the attest's event was not found")
    }

    async fn attest_count(&self) -> anyhow::Result<u64> {
        Ok(self.receipts().await?.attestCount().call().await?)
    }

    async fn fast_lock(&self, id: u64) -> anyhow::Result<Option<FastLock>> {
        let a = self.receipts().await?.getAttest(id).call().await?;
        if a.attester == Address::ZERO || a.closed {
            return Ok(None);
        }
        Ok(Some(FastLock {
            id,
            attester: a.attester.to_string(),
            lock_id: a.lockId,
            stated: a.recordHash.0,
            asset: a.asset,
            collateral: a.collateral,
            attested_at: a.attestedAt as i64,
            claim: a.claim.to::<u64>(),
            prev: a.prev,
            burned: a.burned,
        }))
    }

    async fn link_fast(&self, claim: u64, attest: u64, _linked: Option<u64>) -> anyhow::Result<()> {
        send!(self.net, self.receipts().await?.linkFast(attest, U256::from(claim)));
        Ok(())
    }

    async fn burn_fast(&self, attest: u64, _linked: Option<u64>) -> anyhow::Result<()> {
        send!(self.net, self.receipts().await?.burnFast(attest));
        Ok(())
    }

    async fn settle_fast(&self, claim: u64, attest: u64) -> anyhow::Result<()> {
        let receipts = self.receipts().await?;
        let a = receipts.getAttest(attest).call().await?;
        // The claim's LOCK record of the attest's lock.
        let c = self.claim(claim).await?;
        let record = c
            .records
            .iter()
            .find(|r| matches!(r, Record::Lock { id, .. } if *id == a.lockId))
            .ok_or_else(|| anyhow::anyhow!("claim {claim} does not carry lock {}", a.lockId))?;
        send!(self.net, receipts.settleFast(attest, U256::from(claim), record_bytes(record)));
        Ok(())
    }

    async fn fast_pay(&self, record: &Record) -> anyhow::Result<()> {
        let Record::Request { asset, amount, .. } = record else { anyhow::bail!("only a burn is paid at once") };
        let home = self.home().await?;
        let a = home.getAsset(*asset).call().await?;
        let native = U256::from(*amount) * a.unit;
        if a.token == Address::ZERO {
            send!(self.net, home.fastPay(record_bytes(record)).value(native));
        } else {
            let token = IERC20::new(a.token, self.net.provider());
            if token.allowance(self.net.address(), *home.address()).call().await? < native {
                send!(self.net, token.approve(*home.address(), U256::MAX));
            }
            send!(self.net, home.fastPay(record_bytes(record)));
        }
        Ok(())
    }

    async fn fast_paid_by(&self, record: &Record) -> anyhow::Result<Option<String>> {
        let Record::Request { id, .. } = record else { return Ok(None) };
        let f = self.home().await?.getFastPay(*id, record_bytes(record)).call().await?;
        Ok((f.attester != Address::ZERO).then(|| f.attester.to_string()))
    }

    async fn pay_request(&self, claim: u64, record: &Record) -> anyhow::Result<()> {
        send!(self.net, self.home().await?.payRequest(U256::from(claim), record_bytes(record)));
        Ok(())
    }

    async fn credit(&self, home: u8, asset: u32) -> anyhow::Result<u128> {
        // The core's (deposits won, a slasher's share), and for a receipt the
        // receipts part's too (settled attests).
        let me = self.net.address();
        let mut total = to_u128(self.contract().credit(me, key(home, asset)).call().await?)?;
        // Fees earned, and what is owed for burns paid at once: in the
        // asset on its home, in receipts on the other.
        total += if home == self.peer {
            to_u128(self.receipts().await?.credit(me, asset).call().await?)?
        } else {
            to_u128(self.home().await?.credit(me, asset).call().await?)?
        };
        Ok(total)
    }

    async fn withdraw_credit(&self, home: u8, asset: u32) -> anyhow::Result<()> {
        if !self.contract().credit(self.net.address(), key(home, asset)).call().await?.is_zero() {
            send!(self.net, self.contract().withdrawCredit(key(home, asset)));
        }
        if home == self.peer {
            let receipts = self.receipts().await?;
            if !receipts.credit(self.net.address(), asset).call().await?.is_zero() {
                send!(self.net, receipts.withdrawCredit(asset));
            }
        } else {
            let h = self.home().await?;
            if !h.credit(self.net.address(), asset).call().await?.is_zero() {
                send!(self.net, h.withdrawCredit(asset));
            }
        }
        Ok(())
    }
}
