//! The protocol's vault on Solana (`programs/ipow-vault`, spec section 11),
//! for the node's operator and guardian.

use std::sync::Arc;

use anchor_lang::prelude::Pubkey;
use anchor_lang::solana_program::instruction::{AccountMeta, Instruction};
use anchor_lang::{InstructionData, ToAccountMetas};
use anchor_spl::associated_token::get_associated_token_address as ata;
use async_trait::async_trait;
use ipow_protocol_core::types::{Amount, BlockRef};
use ipow_protocol_core::vault::{decode, Chain, Claim, FastLock, Lock, Record, Request, TxProof, VaultApp, ETHEREUM, SOLANA};
use sha2::{Digest, Sha256};

use crate::network::SvmNetwork;
use crate::programs::ipow_protocol as pr;
use crate::programs::ipow_vault as vt;

const SYSTEM: Pubkey = anchor_lang::solana_program::system_program::ID;
const TOKEN: Pubkey = anchor_spl::token::ID;

pub struct SvmVault {
    net: Arc<SvmNetwork>,
}

fn sha256(parts: &[&[u8]]) -> [u8; 32] {
    let mut h = Sha256::new();
    for p in parts {
        h.update(p);
    }
    h.finalize().into()
}

fn pda(seeds: &[&[u8]]) -> Pubkey {
    Pubkey::find_program_address(seeds, &vt::ID).0
}
fn config() -> Pubkey {
    pda(&[b"config"])
}
fn mint() -> Pubkey {
    pda(&[b"veth"])
}
fn holding() -> Pubkey {
    pda(&[b"holding"])
}
fn chain_pda(o: &Pubkey) -> Pubkey {
    pda(&[b"chain", o.as_ref()])
}
fn claim_pda(id: u64) -> Pubkey {
    pda(&[b"claim", &id.to_le_bytes()])
}
fn stake_pda(id: u64, who: &Pubkey) -> Pubkey {
    pda(&[b"stake", &id.to_le_bytes(), who.as_ref()])
}
fn credit_pda(who: &Pubkey) -> Pubkey {
    pda(&[b"credit", who.as_ref()])
}
fn request_pda(id: u64) -> Pubkey {
    pda(&[b"request", &id.to_le_bytes()])
}
fn message_pda(operator: &Pubkey, index: u64) -> Pubkey {
    pda(&[b"message", operator.as_ref(), &index.to_le_bytes()])
}
fn lock_pda(id: u64) -> Pubkey {
    pda(&[b"lock", &id.to_le_bytes()])
}
fn fast_pda(n: u64) -> Pubkey {
    pda(&[b"fast", &n.to_le_bytes()])
}
fn real_pda(r: &BlockRef) -> Pubkey {
    pda(&[b"real", &r.hash, &r.height.to_le_bytes(), &r.epoch_time.to_le_bytes()])
}
fn buffer_pda(owner: &Pubkey) -> Pubkey {
    pda(&[b"buffer", owner.as_ref()])
}
/// Above this many bytes of transaction and batch together, a message is
/// uploaded to a buffer first: with a proof from a full block, more does not
/// fit in one Solana transaction.
const INLINE: usize = 300;
/// Bytes written to a buffer per transaction.
const CHUNK: usize = 700;
fn v_ref(r: &BlockRef) -> vt::types::BlockRef {
    vt::types::BlockRef { hash: r.hash, height: r.height, epoch_time: r.epoch_time }
}
fn checkpoint_tag(n: u64) -> [u8; 32] {
    sha256(&[b"iPoW checkpoint", vt::ID.as_ref(), &n.to_le_bytes()])
}

impl SvmVault {
    pub fn new(net: Arc<SvmNetwork>, program: &str) -> anyhow::Result<Self> {
        anyhow::ensure!(program == vt::ID.to_string(), "the vault is {program}, this node is built for {}", vt::ID);
        Ok(SvmVault { net })
    }

    fn ix(accounts: impl ToAccountMetas, data: impl InstructionData) -> Instruction {
        Instruction { program_id: vt::ID, accounts: accounts.to_account_metas(None), data: data.data() }
    }

    fn operator(s: &str) -> anyhow::Result<Pubkey> {
        s.parse().map_err(|_| anyhow::anyhow!("{s} is not a Solana address"))
    }

    async fn config_record(&self) -> anyhow::Result<vt::accounts::Config> {
        self.net.read_account(&config()).await?.ok_or_else(|| anyhow::anyhow!("the vault is not initialized"))
    }

    /// The walk from `tx.real` down to `tx.block`, or none when they are the
    /// same block. The caller closes it.
    async fn walk_for(&self, tx: &TxProof) -> anyhow::Result<Option<Pubkey>> {
        if tx.block == tx.real {
            return Ok(None);
        }
        Ok(Some(self.net.walk(&tx.real, &tx.block).await?))
    }

    fn btc(tx: &TxProof) -> vt::types::Btc {
        vt::types::Btc { block: v_ref(&tx.block), raw_tx: tx.raw_tx.clone(), siblings: tx.siblings.clone(), tx_index: tx.tx_index, real: v_ref(&tx.real) }
    }

    /// Sends `ix`, then closes `walk`, whatever the outcome.
    async fn send_then_close(&self, ix: Instruction, walk: Option<Pubkey>) -> anyhow::Result<()> {
        let outcome = self.net.send_ix(ix).await;
        if let Some(w) = walk {
            self.net.close_walks(&[w]).await;
        }
        outcome
    }

    /// Uploads a message's transaction and batch to this node's buffer,
    /// opened fresh.
    async fn upload(&self, raw_tx: &[u8], batch: &[u8]) -> anyhow::Result<()> {
        let me = self.net.me_key();
        // A buffer left by a failed attempt is closed first.
        if self.net.account_data(&buffer_pda(&me)).await?.is_some_and(|d| !d.is_empty()) {
            self.close_buffer().await;
        }
        self.net
            .send_ix(Self::ix(
                vt::client::accounts::OpenBuffer { buffer: buffer_pda(&me), owner: me, system_program: SYSTEM },
                vt::client::args::OpenBuffer {},
            ))
            .await?;
        for (raw, data) in [(true, raw_tx), (false, batch)] {
            for chunk in data.chunks(CHUNK) {
                let ix = Self::ix(
                    vt::client::accounts::WriteBuffer { buffer: buffer_pda(&me), owner: me },
                    vt::client::args::WriteBuffer { raw, data: chunk.to_vec() },
                );
                if let Err(e) = self.net.send_ix(ix).await {
                    self.close_buffer().await;
                    return Err(e);
                }
            }
        }
        Ok(())
    }

    /// Closes this node's buffer and takes its rent back. A failure only
    /// costs rent until the next upload closes it.
    async fn close_buffer(&self) {
        let me = self.net.me_key();
        let ix = Self::ix(vt::client::accounts::CloseBuffer { buffer: buffer_pda(&me), owner: me }, vt::client::args::CloseBuffer {});
        if let Err(e) = self.net.send_ix(ix).await {
            tracing::warn!(error = %ipow_protocol_core::secrets::redact(&e), "closing the vault buffer failed");
        }
    }

    async fn side(&self, id: u64, object: bool) -> anyhow::Result<()> {
        let me = self.net.me_key();
        let ix = if object {
            Self::ix(
                vt::client::accounts::Object { claim: claim_pda(id), stake: stake_pda(id, &me), config: config(), who: me, system_program: SYSTEM },
                vt::client::args::Object { claim_id: id },
            )
        } else {
            Self::ix(
                vt::client::accounts::Answer { claim: claim_pda(id), stake: stake_pda(id, &me), config: config(), who: me, system_program: SYSTEM },
                vt::client::args::Answer { claim_id: id },
            )
        };
        self.net.send_ix(ix).await
    }
}

#[async_trait]
impl VaultApp for SvmVault {
    fn network_id(&self) -> u8 {
        SOLANA
    }

    fn me(&self) -> String {
        self.net.me_key().to_string()
    }

    fn me_bytes(&self) -> Vec<u8> {
        self.net.me_key().to_bytes().to_vec()
    }

    async fn deposit(&self) -> anyhow::Result<Amount> {
        Ok(self.config_record().await?.deposit as Amount)
    }

    async fn pair_commitment(&self, peer: &[u8]) -> anyhow::Result<[u8; 32]> {
        let peer: [u8; 20] = peer.try_into().map_err(|_| anyhow::anyhow!("an operator on Ethereum is 20 bytes"))?;
        let ethereum_vault = self.config_record().await?.ethereum_vault;
        Ok(sha256(&[b"iPoW pair", &ethereum_vault, &peer, vt::ID.as_ref(), self.net.me_key().as_ref()]))
    }

    async fn chain(&self, operator: &str) -> anyhow::Result<Option<Chain>> {
        let Some(c) = self.net.read_account::<vt::accounts::Chain>(&chain_pda(&Self::operator(operator)?)).await? else {
            return Ok(None);
        };
        Ok(Some(Chain {
            peer: c.peer_operator.to_vec(),
            coin: (c.coin_txid, c.coin_vout),
            messages: c.messages,
            exited: c.exited,
            slashed: c.slashed,
            refused: c.refused,
            bond: c.bond as u128,
            stated: c.stated as u128,
            peer_bond: c.peer_bond,
            peer_bond_carried: c.peer_bond_carried,
            open_value: c.open_value,
            open_claims: c.open_claims,
            deposits: c.deposits as Amount,
        }))
    }

    async fn register_chain(&self, peer: &[u8], tx: &TxProof, coin_index: u32, tag_index: u32) -> anyhow::Result<()> {
        let peer_operator: [u8; 20] = peer.try_into().map_err(|_| anyhow::anyhow!("an operator on Ethereum is 20 bytes"))?;
        let me = self.net.me_key();
        let walk = self.walk_for(tx).await?;
        let ix = Self::ix(
            vt::client::accounts::RegisterChain {
                config: config(),
                chain: chain_pda(&me),
                real: real_pda(&tx.real),
                walk: walk.unwrap_or(SYSTEM),
                node: SvmNetwork::node_pda(&tx.block),
                operator: me,
                system_program: SYSTEM,
            },
            vt::client::args::RegisterChain { peer_operator, btc: Self::btc(tx), coin_index, tag_index },
        );
        self.send_then_close(ix, walk).await
    }

    async fn add_bond(&self, amount: u128) -> anyhow::Result<()> {
        let me = self.net.me_key();
        let amount = u64::try_from(amount).map_err(|_| anyhow::anyhow!("a vETH amount fits 64 bits"))?;
        self.net
            .send_ix(Self::ix(
                vt::client::accounts::AddBond { chain: chain_pda(&me), mint: mint(), from: ata(&me, &mint()), holding: holding(), operator: me, token_program: TOKEN },
                vt::client::args::AddBond { amount },
            ))
            .await
    }

    async fn add_deposits(&self, amount: Amount) -> anyhow::Result<()> {
        let me = self.net.me_key();
        let amount = u64::try_from(amount).map_err(|_| anyhow::anyhow!("lamports fit 64 bits"))?;
        self.net
            .send_ix(Self::ix(
                vt::client::accounts::AddDeposits { chain: chain_pda(&me), config: config(), operator: me, system_program: SYSTEM },
                vt::client::args::AddDeposits { amount },
            ))
            .await
    }

    async fn is_real(&self, block: &BlockRef) -> anyhow::Result<bool> {
        Ok(self.net.account_data(&real_pda(block)).await?.is_some_and(|d| !d.is_empty()))
    }

    async fn min_certifying_escrow(&self) -> anyhow::Result<Amount> {
        Ok(self.config_record().await?.min_certifying_escrow as Amount)
    }

    async fn record_real_from_job(&self, job_id: u64) -> anyhow::Result<()> {
        let job = self.net.job_record(job_id).await?;
        let pb = BlockRef { hash: job.proof_block.hash, height: job.proof_block.height, epoch_time: job.proof_block.epoch_time };
        self.net
            .send_ix(Self::ix(
                vt::client::accounts::RecordRealFromJob { config: config(), job: SvmNetwork::job_pda(job_id), real: real_pda(&pb), payer: self.net.me_key(), system_program: SYSTEM },
                vt::client::args::RecordRealFromJob { job_id },
            ))
            .await
    }

    async fn record_real(&self, low: &BlockRef, high: &BlockRef) -> anyhow::Result<()> {
        let walk = self.net.walk(high, low).await?;
        let ix = Self::ix(
            vt::client::accounts::RecordReal { high_real: real_pda(high), walk, low_real: real_pda(low), payer: self.net.me_key(), system_program: SYSTEM },
            vt::client::args::RecordReal { low: v_ref(low), high: v_ref(high) },
        );
        self.send_then_close(ix, Some(walk)).await
    }

    async fn open_checkpoint(&self, confirmations: u16, paid: Amount) -> anyhow::Result<u64> {
        let n = self.config_record().await?.checkpoint_count + 1;
        let protocol: pr::accounts::Protocol =
            self.net.read_account(&SvmNetwork::pr_pda(&[b"protocol"])).await?.ok_or_else(|| anyhow::anyhow!("the protocol is not initialized"))?;
        let job_id = protocol.job_count + 1;
        let me = self.net.me_key();
        let paid = u64::try_from(paid).map_err(|_| anyhow::anyhow!("lamports fit 64 bits"))?;
        self.net
            .send_ix(Self::ix(
                vt::client::accounts::OpenCheckpoint {
                    config: config(),
                    protocol: SvmNetwork::pr_pda(&[b"protocol"]),
                    application: SvmNetwork::pr_pda(&[b"application", config().as_ref()]),
                    job: SvmNetwork::job_pda(job_id),
                    tag_record: SvmNetwork::pr_pda(&[b"tag", config().as_ref(), &checkpoint_tag(n)]),
                    protocol_vault: SvmNetwork::pr_pda(&[b"vault"]),
                    funder: me,
                    protocol_program: pr::ID,
                    system_program: SYSTEM,
                },
                vt::client::args::OpenCheckpoint { confirmations, paid },
            ))
            .await?;
        Ok(job_id)
    }

    async fn submit_message(&self, operator: &str, tx: &TxProof, input_index: u32, tag_index: u32, batch: &[u8]) -> anyhow::Result<()> {
        let op = Self::operator(operator)?;
        let me = self.net.me_key();
        let id = self.config_record().await?.claim_count + 1;
        let index = self.net.read_account::<vt::accounts::Chain>(&chain_pda(&op)).await?.map_or(0, |c| c.messages);
        // The accounts the records about Solana are judged with, in order.
        let mut extra = vec![];
        for r in decode(batch).unwrap_or_default() {
            match r {
                Record::Request { id, .. } => extra.push(request_pda(id)),
                Record::Cancel { id } => extra.push(lock_pda(id)),
                _ => {}
            }
        }
        let buffered = tx.raw_tx.len() + batch.len() > INLINE;
        if buffered {
            self.upload(&tx.raw_tx, batch).await?;
        }
        let walk = match self.walk_for(tx).await {
            Ok(w) => w,
            Err(e) => {
                if buffered {
                    self.close_buffer().await;
                }
                return Err(e);
            }
        };
        let mut btc = Self::btc(tx);
        if buffered {
            btc.raw_tx = vec![];
        }
        let mut ix = Self::ix(
            vt::client::accounts::SubmitMessage {
                config: config(),
                chain: chain_pda(&op),
                message: message_pda(&op, index),
                real: real_pda(&tx.real),
                walk: walk.unwrap_or(SYSTEM),
                node: SvmNetwork::node_pda(&tx.block),
                claim: claim_pda(id),
                stake: stake_pda(id, &op),
                operator_credit: credit_pda(&op),
                submitter_credit: credit_pda(&me),
                mint: mint(),
                holding: holding(),
                submitter: me,
                buffer: buffered.then(|| buffer_pda(&me)),
                token_program: TOKEN,
                system_program: SYSTEM,
            },
            vt::client::args::SubmitMessage { operator: op, btc, input_index, tag_index, batch: if buffered { vec![] } else { batch.to_vec() } },
        );
        ix.accounts.extend(extra.into_iter().map(|k| AccountMeta::new(k, false)));
        let outcome = self.send_then_close(ix, walk).await;
        if buffered {
            self.close_buffer().await;
        }
        outcome
    }

    async fn message_batch(&self, operator: &str, index: u64) -> anyhow::Result<Option<Vec<u8>>> {
        let op = Self::operator(operator)?;
        let m: Option<vt::accounts::Message> = self.net.read_account(&message_pda(&op, index)).await?;
        Ok(m.filter(|m| m.operator == op && m.index == index).map(|m| m.batch))
    }

    async fn final_locks_after(&self, _after: u64, _limit: usize) -> anyhow::Result<Vec<Lock>> {
        Ok(vec![])
    }

    async fn lock(&self, _id: u64) -> anyhow::Result<Option<Lock>> {
        Ok(None)
    }

    /// Only requests in a finalized block: a record of one Solana could
    /// still roll back would be false, and slashed (section 11.5).
    async fn requests_after(&self, after: u64, limit: usize) -> anyhow::Result<Vec<Request>> {
        let config: vt::accounts::Config = self.net.read_final(&config()).await?.ok_or_else(|| anyhow::anyhow!("the vault is not initialized"))?;
        let count = config.request_count;
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
        let r: Option<vt::accounts::Request> = self.net.read_final(&request_pda(id)).await?;
        Ok(r.map(|r| Request { id, amount: r.amount, to: r.to, fee: r.fee, fast_fee: r.fast_fee, requested_at: r.requested_at as u64, fee_paid: r.fee_paid }))
    }

    /// As of a finalized block, for the same reason as requests.
    async fn given_up(&self, id: u64) -> anyhow::Result<bool> {
        let m: Option<vt::accounts::LockMark> = self.net.read_final(&lock_pda(id)).await?;
        Ok(m.is_some_and(|m| m.given_up))
    }

    async fn receipt_issued(&self, id: u64) -> anyhow::Result<bool> {
        let m: Option<vt::accounts::LockMark> = self.net.read_account(&lock_pda(id)).await?;
        Ok(m.is_some_and(|m| m.issued))
    }

    async fn request_paid(&self, _id: u64) -> anyhow::Result<bool> {
        Ok(false)
    }

    async fn attest_lock(&self, l: &Lock) -> anyhow::Result<u64> {
        let me = self.net.me_key();
        let recipient = Pubkey::new_from_array(l.recipient);
        let create = anchor_spl::associated_token::spl_associated_token_account::instruction::create_associated_token_account_idempotent(
            &me, &recipient, &mint(), &TOKEN,
        );
        self.net.send_ix(create).await?;
        let n = self.config_record().await?.attest_count + 1;
        self.net
            .send_ix(Self::ix(
                vt::client::accounts::AttestLock {
                    config: config(),
                    chain: chain_pda(&me),
                    mark: lock_pda(l.id),
                    fast: fast_pda(n),
                    mint: mint(),
                    holding: holding(),
                    from: ata(&me, &mint()),
                    to: ata(&recipient, &mint()),
                    attester: me,
                    token_program: TOKEN,
                    system_program: SYSTEM,
                },
                vt::client::args::AttestLock { lock_id: l.id, amount: l.amount, recipient, fee: l.fee, fast_fee: l.fast_fee, locked_at: l.locked_at as i64 },
            ))
            .await?;
        Ok(n)
    }

    async fn lock_attest(&self, id: u64) -> anyhow::Result<Option<u64>> {
        let m: Option<vt::accounts::LockMark> = self.net.read_account(&lock_pda(id)).await?;
        Ok(m.and_then(|m| (m.attests != 0).then_some(m.attests as u64)))
    }

    async fn lock_attests(&self, id: u64) -> anyhow::Result<Option<(i64, Vec<FastLock>)>> {
        let Some(m): Option<vt::accounts::LockMark> = self.net.read_account(&lock_pda(id)).await? else { return Ok(None) };
        if m.attests == 0 {
            return Ok(None);
        }
        // Settled in the order made: the open ones are the newest.
        let mut open = vec![];
        let mut at = m.last_attest;
        while at != 0 {
            let Some(f) = self.fast_lock(at).await? else { break };
            at = f.prev;
            open.push(f);
        }
        Ok(Some((m.first_at, open)))
    }

    async fn attest_count(&self) -> anyhow::Result<u64> {
        Ok(self.config_record().await?.attest_count)
    }

    async fn fast_lock(&self, id: u64) -> anyhow::Result<Option<FastLock>> {
        let f: Option<vt::accounts::FastLock> = self.net.read_account(&fast_pda(id)).await?;
        Ok(f.map(|f| FastLock {
            id: f.id,
            attester: f.attester.to_string(),
            record: Record::Lock { id: f.lock_id, amount: f.amount, recipient: f.recipient.to_bytes(), fee: f.fee, fast_fee: f.fast_fee, at: f.locked_at as u64 },
            collateral: f.collateral,
            attested_at: f.attested_at,
            claim: f.claim,
            prev: f.prev,
            burned: f.burned,
        }))
    }

    async fn link_fast(&self, claim: u64, attest: u64, linked: Option<u64>) -> anyhow::Result<()> {
        self.net
            .send_ix(Self::ix(
                vt::client::accounts::LinkFast { fast: fast_pda(attest), claim: claim_pda(claim), linked: linked.map(claim_pda), attester: self.net.me_key() },
                vt::client::args::LinkFast { claim_id: claim, attest },
            ))
            .await
    }

    async fn burn_fast(&self, attest: u64, linked: Option<u64>) -> anyhow::Result<()> {
        let me = self.net.me_key();
        self.net
            .send_ix(Self::ix(
                vt::client::accounts::BurnFast {
                    config: config(),
                    fast: fast_pda(attest),
                    linked: linked.map(claim_pda),
                    mint: mint(),
                    holding: holding(),
                    caller_credit: credit_pda(&me),
                    caller: me,
                    token_program: TOKEN,
                    system_program: SYSTEM,
                },
                vt::client::args::BurnFast { attest },
            ))
            .await
    }

    async fn settle_fast(&self, claim: u64, attest: u64) -> anyhow::Result<()> {
        let me = self.net.me_key();
        let f: vt::accounts::FastLock = self.net.read_account(&fast_pda(attest)).await?.ok_or_else(|| anyhow::anyhow!("attest {attest} does not exist"))?;
        let c = self.claim(claim).await?;
        let record = c
            .records
            .iter()
            .find(|r| matches!(r, Record::Lock { id, .. } if *id == f.lock_id))
            .ok_or_else(|| anyhow::anyhow!("claim {claim} does not carry lock {}", f.lock_id))?;
        // The true record's recipient may be owed the rest of the fast fee,
        // or its receipt.
        let Record::Lock { recipient, .. } = record else { unreachable!() };
        let recipient = Pubkey::new_from_array(*recipient);
        let create = anchor_spl::associated_token::spl_associated_token_account::instruction::create_associated_token_account_idempotent(
            &me, &recipient, &mint(), &TOKEN,
        );
        self.net.send_ix(create).await?;
        let to = Some(ata(&recipient, &mint()));
        self.net
            .send_ix(Self::ix(
                vt::client::accounts::SettleFast {
                    config: config(),
                    claim: claim_pda(claim),
                    fast: fast_pda(attest),
                    mark: lock_pda(f.lock_id),
                    prev: (f.prev != 0).then(|| fast_pda(f.prev)),
                    attester: f.attester,
                    mint: mint(),
                    holding: holding(),
                    to,
                    attester_credit: credit_pda(&f.attester),
                    caller_credit: credit_pda(&me),
                    caller: me,
                    token_program: TOKEN,
                    system_program: SYSTEM,
                },
                vt::client::args::SettleFast { claim_id: claim, attest },
            ))
            .await
    }

    async fn veth_credit(&self) -> anyhow::Result<u64> {
        let c: Option<vt::accounts::Credit> = self.net.read_account(&credit_pda(&self.net.me_key())).await?;
        Ok(c.map_or(0, |c| c.veth))
    }

    async fn withdraw_credit(&self) -> anyhow::Result<()> {
        let me = self.net.me_key();
        self.net
            .send_ix(Self::ix(
                vt::client::accounts::WithdrawCredit {
                    credit: credit_pda(&me),
                    config: config(),
                    mint: mint(),
                    holding: holding(),
                    to: Some(ata(&me, &mint())),
                    owner: me,
                    token_program: TOKEN,
                },
                vt::client::args::WithdrawCredit {},
            ))
            .await
    }

    async fn claim_count(&self) -> anyhow::Result<u64> {
        Ok(self.config_record().await?.claim_count)
    }

    async fn claim_status(&self, id: u64) -> anyhow::Result<Claim> {
        self.claim(id).await
    }

    async fn claim(&self, id: u64) -> anyhow::Result<Claim> {
        let c: vt::accounts::Claim = self.net.read_account(&claim_pda(id)).await?.ok_or_else(|| anyhow::anyhow!("claim {id} does not exist"))?;
        let mut records = decode(&c.records).unwrap_or_default();
        if c.peer_bond != 0 {
            records.push(Record::Bond { network: ETHEREUM, amount: c.peer_bond });
        }
        Ok(Claim { id, operator: c.operator.to_string(), last_at: c.last_at, held: c.held, decided: c.decided, accepted: c.accepted, records })
    }

    async fn object(&self, id: u64) -> anyhow::Result<()> {
        self.side(id, true).await
    }

    async fn answer(&self, id: u64) -> anyhow::Result<()> {
        self.side(id, false).await
    }

    async fn decide(&self, id: u64) -> anyhow::Result<()> {
        let c: vt::accounts::Claim = self.net.read_account(&claim_pda(id)).await?.ok_or_else(|| anyhow::anyhow!("claim {id} does not exist"))?;
        self.net
            .send_ix(Self::ix(vt::client::accounts::Decide { claim: claim_pda(id), chain: chain_pda(&c.operator), config: config() }, vt::client::args::Decide { claim_id: id }))
            .await
    }

    async fn collect(&self, id: u64) -> anyhow::Result<()> {
        let me = self.net.me_key();
        let c: vt::accounts::Claim = self.net.read_account(&claim_pda(id)).await?.ok_or_else(|| anyhow::anyhow!("claim {id} does not exist"))?;
        let Some(s) = self.net.read_account::<vt::accounts::Stake>(&stake_pda(id, &me)).await? else { return Ok(()) };
        let mine = if c.accepted { s.answers } else { s.objections };
        if mine == 0 {
            return Ok(());
        }
        self.net
            .send_ix(Self::ix(
                vt::client::accounts::Collect { claim: claim_pda(id), stake: stake_pda(id, &me), credit: credit_pda(&me), who: me, system_program: SYSTEM },
                vt::client::args::Collect { claim_id: id },
            ))
            .await
    }
}
