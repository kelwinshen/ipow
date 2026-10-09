//! The protocol's vault on Solana (`programs/protocol/ipow-vault`, spec section 11),
//! for the node's operator and guardian.

use std::sync::Arc;

use anchor_lang::prelude::Pubkey;
use anchor_lang::solana_program::instruction::{AccountMeta, Instruction};
use anchor_lang::{InstructionData, ToAccountMetas};
use anchor_spl::associated_token::get_associated_token_address_with_program_id as ata_of;
use async_trait::async_trait;
use ipow_protocol_core::types::{Amount, BlockRef};
use ipow_protocol_core::vault::{
    decode, record_hash, AssetInfo, Chain, Claim, FastLock, Lock, Position, Record, Request, TxProof, VaultApp, MAX_NETWORK, SOLANA,
};
use sha2::{Digest, Sha256};

use crate::network::SvmNetwork;
use crate::programs::ipow_protocol as pr;
use crate::programs::ipow_vault as vt;

const SYSTEM: Pubkey = anchor_lang::solana_program::system_program::ID;
const TOKEN: Pubkey = anchor_spl::token::ID;

pub struct SvmVault {
    net: Arc<SvmNetwork>,
    /// The pair's other network, and its accounts here (D132).
    peer: u8,
    p: Pdas,
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

/// The accounts of one pair (D132): the program keeps a configuration per
/// peer, `["config", peer]`, and every account of the pair is keyed by it.
#[derive(Clone, Copy)]
pub struct Pdas {
    pub config: Pubkey,
}

impl Pdas {
    pub fn new(peer: u8) -> Self {
        Pdas { config: pda(&[b"config", &[peer]]) }
    }
    fn at(&self, seeds: &[&[u8]]) -> Pubkey {
        let mut all: Vec<&[u8]> = vec![seeds[0], self.config.as_ref()];
        all.extend_from_slice(&seeds[1..]);
        pda(&all)
    }
    pub fn config(&self) -> Pubkey {
        self.config
    }
    /// The receipt here of the peer's asset `n`, and the vault's account of
    /// it.
    pub fn receipt(&self, n: u32) -> Pubkey {
        self.at(&[b"receipt", &n.to_le_bytes()])
    }
    pub fn holding(&self, n: u32) -> Pubkey {
        self.at(&[b"holding", &n.to_le_bytes()])
    }
    pub fn asset_pda(&self, n: u32) -> Pubkey {
        self.at(&[b"asset", &n.to_le_bytes()])
    }
    pub fn home_tokens(&self, mint: &Pubkey) -> Pubkey {
        self.at(&[b"home_tokens", mint.as_ref()])
    }
    pub fn home_lock_pda(&self, id: u64) -> Pubkey {
        self.at(&[b"home_lock", &id.to_le_bytes()])
    }
    pub fn paid_pda(&self, id: u64) -> Pubkey {
        self.at(&[b"paid", &id.to_le_bytes()])
    }
    pub fn fast_pay_pda(&self, id: u64, record: &[u8]) -> Pubkey {
        self.at(&[b"fast_pay", &id.to_le_bytes(), &sha256(&[record])])
    }
    pub fn chain_pda(&self, o: &Pubkey) -> Pubkey {
        self.at(&[b"chain", o.as_ref()])
    }
    pub fn claim_pda(&self, id: u64) -> Pubkey {
        self.at(&[b"claim", &id.to_le_bytes()])
    }
    pub fn stake_pda(&self, id: u64, who: &Pubkey) -> Pubkey {
        self.at(&[b"stake", &id.to_le_bytes(), who.as_ref()])
    }
    pub fn credit_pda(&self, who: &Pubkey, home: u8, asset: u32) -> Pubkey {
        self.at(&[b"credit", who.as_ref(), &[home], &asset.to_le_bytes()])
    }
    pub fn request_pda(&self, id: u64) -> Pubkey {
        self.at(&[b"request", &id.to_le_bytes()])
    }
    pub fn message_pda(&self, operator: &Pubkey, index: u64) -> Pubkey {
        self.at(&[b"message", operator.as_ref(), &index.to_le_bytes()])
    }
    pub fn lock_pda(&self, id: u64) -> Pubkey {
        self.at(&[b"lock", &id.to_le_bytes()])
    }
    pub fn fast_pda(&self, id: u64) -> Pubkey {
        self.at(&[b"fast", &id.to_le_bytes()])
    }
    pub fn real_pda(&self, r: &BlockRef) -> Pubkey {
        self.at(&[b"real", &r.hash, &r.height.to_le_bytes(), &r.epoch_time.to_le_bytes()])
    }

    /// The accounts a batch's records are judged and acted on with here, in
    /// order (`submit_message`). Up to the first record that does not parse:
    /// the program reads the accounts of those before it, then judges the
    /// batch false, so a batch that does not parse can still be slashed.
    fn accounts_of(&self, peer: u8, batch: &[u8]) -> Vec<Pubkey> {
        let mut out = vec![];
        let mut o = 0;
        while o < batch.len() {
            let len = match batch[o] {
                1 | 2 => 78,
                3 => 10,
                4 => 15,
                6 => 39,
                _ => break,
            };
            let Some(records) = batch.get(o..o + len).and_then(decode) else { break };
            o += len;
            for r in records {
                match r {
                    Record::Lock { home: SOLANA, id, .. } => out.push(self.home_lock_pda(id)),
                    Record::Request { net: SOLANA, id, .. } => out.push(self.request_pda(id)),
                    Record::Cancel { net: SOLANA, id } => out.push(self.lock_pda(id)),
                    Record::Asset { home: SOLANA, asset, .. } => out.push(self.asset_pda(asset)),
                    Record::Request { net, asset, .. } if net == peer => out.push(self.asset_pda(asset)),
                    Record::Cancel { net, id } if net == peer => out.push(self.home_lock_pda(id)),
                    _ => {}
                }
            }
        }
        out
    }
}

/// What a registration carries (section 11.3, D133): each side its
/// network, its vault and its operator in 32 bytes, the lower number first;
/// Solana's vault is the pair's configuration. The program's
/// `util::pair_commitment` computes the same bytes.
pub fn pair_commitment(peer: u8, peer_vault: &[u8; 20], peer_operator: &[u8; 20], config: &Pubkey, operator: &Pubkey) -> [u8; 32] {
    let pad = |a: &[u8; 20]| {
        let mut out = [0u8; 32];
        out[12..].copy_from_slice(a);
        out
    };
    let (theirs_vault, theirs_op) = (pad(peer_vault), pad(peer_operator));
    let mine: [&[u8]; 3] = [&[SOLANA], config.as_ref(), operator.as_ref()];
    let theirs: [&[u8]; 3] = [&[peer], &theirs_vault, &theirs_op];
    let (a, b) = if SOLANA < peer { (mine, theirs) } else { (theirs, mine) };
    sha256(&[b"iPoW pair", a[0], a[1], a[2], b[0], b[1], b[2]])
}

/// Whether network `peer` can be Solana's peer: one of the list (D133, D141),
/// not Solana itself.
pub fn can_pair(peer: u8) -> bool {
    peer != SOLANA && (1..=MAX_NETWORK).contains(&peer)
}

impl SvmVault {
    /// The vault program's pair with network `peer`.
    pub fn new(net: Arc<SvmNetwork>, program: &str, peer: u8) -> anyhow::Result<Self> {
        anyhow::ensure!(program == vt::ID.to_string(), "the vault is {program}, this node is built for {}", vt::ID);
        anyhow::ensure!(can_pair(peer), "network {peer} cannot be Solana's peer");
        Ok(SvmVault { net, peer, p: Pdas::new(peer) })
    }

    /// The pair's accounts.
    pub fn pdas(&self) -> Pdas {
        self.p
    }

    fn ix(accounts: impl ToAccountMetas, data: impl InstructionData) -> Instruction {
        Instruction { program_id: vt::ID, accounts: accounts.to_account_metas(None), data: data.data() }
    }

    fn operator(s: &str) -> anyhow::Result<Pubkey> {
        s.parse().map_err(|_| anyhow::anyhow!("{s} is not a Solana address"))
    }

    async fn config_record(&self) -> anyhow::Result<vt::accounts::Config> {
        self.net.read_account(&self.p.config()).await?.ok_or_else(|| anyhow::anyhow!("the vault is not initialized"))
    }

    async fn home_asset(&self, n: u32) -> anyhow::Result<vt::accounts::HomeAsset> {
        self.net.read_account(&self.p.asset_pda(n)).await?.ok_or_else(|| anyhow::anyhow!("asset {n} is not registered here"))
    }

    /// For a token whose home is Solana: the vault's account of it, its mint
    /// and its program; none for SOL.
    fn token_accounts(&self, a: &vt::accounts::HomeAsset) -> (Option<Pubkey>, Option<Pubkey>, Option<Pubkey>) {
        if a.number == 0 { (None, None, None) } else { (Some(self.p.home_tokens(&a.mint)), Some(a.mint), Some(a.token_program)) }
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
                vt::client::accounts::Object { claim: self.p.claim_pda(id), stake: self.p.stake_pda(id, &me), config: self.p.config(), who: me, system_program: SYSTEM },
                vt::client::args::Object { claim_id: id },
            )
        } else {
            Self::ix(
                vt::client::accounts::Answer { claim: self.p.claim_pda(id), stake: self.p.stake_pda(id, &me), config: self.p.config(), who: me, system_program: SYSTEM },
                vt::client::args::Answer { claim_id: id },
            )
        };
        self.net.send_ix(ix).await
    }

    /// Creates `owner`'s associated account of `mint` if it does not exist.
    async fn ensure_ata(&self, owner: &Pubkey, mint: &Pubkey, program: &Pubkey) -> anyhow::Result<Pubkey> {
        let create = anchor_spl::associated_token::spl_associated_token_account::instruction::create_associated_token_account_idempotent(
            &self.net.me_key(),
            owner,
            mint,
            program,
        );
        self.net.send_ix(create).await?;
        Ok(ata_of(owner, mint, program))
    }
}

#[async_trait]
impl VaultApp for SvmVault {
    fn network_id(&self) -> u8 {
        SOLANA
    }

    fn peer_id(&self) -> u8 {
        self.peer
    }

    fn vault_id(&self) -> [u8; 32] {
        self.p.config.to_bytes()
    }

    async fn peer_vault(&self) -> anyhow::Result<[u8; 32]> {
        let mut out = [0u8; 32];
        out[12..].copy_from_slice(&self.config_record().await?.peer_vault);
        Ok(out)
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
        let peer: [u8; 20] = peer.try_into().map_err(|_| anyhow::anyhow!("an operator on an EVM network is 20 bytes"))?;
        let c = self.config_record().await?;
        Ok(pair_commitment(c.peer, &c.peer_vault, &peer, &self.p.config, &self.net.me_key()))
    }

    async fn chain(&self, operator: &str) -> anyhow::Result<Option<Chain>> {
        let Some(c) = self.net.read_account::<vt::accounts::Chain>(&self.p.chain_pda(&Self::operator(operator)?)).await? else {
            return Ok(None);
        };
        Ok(Some(Chain {
            peer: c.peer_operator.to_vec(),
            coin: (c.coin_txid, c.coin_vout),
            messages: c.messages,
            exited: c.exited,
            slashed: c.slashed,
            refused: c.refused,
            positions: c
                .positions
                .iter()
                .map(|p| Position {
                    home: p.home,
                    asset: p.asset,
                    bond: p.bond,
                    stated: p.stated,
                    peer_bond: p.peer_bond,
                    peer_bond_carried: p.peer_bond_carried,
                    open_value: p.open_value,
                })
                .collect(),
            open_claims: c.open_claims,
            deposits: c.deposits as Amount,
        }))
    }

    async fn position(&self, operator: &str, home: u8, asset: u32) -> anyhow::Result<Position> {
        Ok(self.chain(operator).await?.map(|c| c.position(home, asset)).unwrap_or(Position { home, asset, ..Default::default() }))
    }

    async fn register_chain(&self, peer: &[u8], tx: &TxProof, coin_index: u32, tag_index: u32) -> anyhow::Result<()> {
        let peer_operator: [u8; 20] = peer.try_into().map_err(|_| anyhow::anyhow!("an operator on an EVM network is 20 bytes"))?;
        let me = self.net.me_key();
        let walk = self.walk_for(tx).await?;
        let ix = Self::ix(
            vt::client::accounts::RegisterChain {
                config: self.p.config(),
                chain: self.p.chain_pda(&me),
                real: self.p.real_pda(&tx.real),
                walk: walk.unwrap_or(SYSTEM),
                node: SvmNetwork::node_pda(&tx.block),
                operator: me,
                system_program: SYSTEM,
            },
            vt::client::args::RegisterChain { peer_operator, btc: Self::btc(tx), coin_index, tag_index },
        );
        self.send_then_close(ix, walk).await
    }

    async fn add_bond(&self, home: u8, asset: u32, amount: u64) -> anyhow::Result<()> {
        let me = self.net.me_key();
        if home == self.peer {
            return self
                .net
                .send_ix(Self::ix(
                    vt::client::accounts::AddBondReceipt { config: self.p.config(),
                        chain: self.p.chain_pda(&me),
                        mint: self.p.receipt(asset),
                        from: ata_of(&me, &self.p.receipt(asset), &TOKEN),
                        holding: self.p.holding(asset),
                        operator: me,
                        token_program: TOKEN,
                    },
                    vt::client::args::AddBondReceipt { asset, amount },
                ))
                .await;
        }
        let a = self.home_asset(asset).await?;
        let (tokens, mint, program) = self.token_accounts(&a);
        self.net
            .send_ix(Self::ix(
                vt::client::accounts::AddBondHome {
                    chain: self.p.chain_pda(&me),
                    config: self.p.config(),
                    home_asset: self.p.asset_pda(asset),
                    from: mint.map(|m| ata_of(&me, &m, &a.token_program)),
                    tokens,
                    mint,
                    token_program: program,
                    operator: me,
                    system_program: SYSTEM,
                },
                vt::client::args::AddBondHome { asset, amount },
            ))
            .await
    }

    async fn add_deposits(&self, amount: Amount) -> anyhow::Result<()> {
        let me = self.net.me_key();
        let amount = u64::try_from(amount).map_err(|_| anyhow::anyhow!("lamports fit 64 bits"))?;
        self.net
            .send_ix(Self::ix(
                vt::client::accounts::AddDeposits { chain: self.p.chain_pda(&me), config: self.p.config(), operator: me, system_program: SYSTEM },
                vt::client::args::AddDeposits { amount },
            ))
            .await
    }

    async fn settle_slash(&self, operator: &str, home: u8, asset: u32) -> anyhow::Result<()> {
        let op = Self::operator(operator)?;
        let c: vt::accounts::Chain = self.net.read_account(&self.p.chain_pda(&op)).await?.ok_or_else(|| anyhow::anyhow!("no chain of {operator}"))?;
        let sol = home == SOLANA;
        self.net
            .send_ix(Self::ix(
                vt::client::accounts::SettleSlash {
                    chain: self.p.chain_pda(&op),
                    config: self.p.config(),
                    home_asset: sol.then(|| self.p.asset_pda(asset)),
                    mint: (!sol).then(|| self.p.receipt(asset)),
                    holding: (!sol).then(|| self.p.holding(asset)),
                    slasher_credit: self.p.credit_pda(&c.slasher, home, asset),
                    payer: self.net.me_key(),
                    token_program: (!sol).then_some(TOKEN),
                    system_program: SYSTEM,
                },
                vt::client::args::SettleSlash { home, asset },
            ))
            .await
    }

    async fn slash_pending(&self, operator: &str, home: u8, asset: u32) -> anyhow::Result<u64> {
        let c: Option<vt::accounts::Chain> = self.net.read_account(&self.p.chain_pda(&Self::operator(operator)?)).await?;
        Ok(c.and_then(|c| c.positions.iter().find(|p| p.home == home && p.asset == asset).map(|p| p.slash_backing + p.slash_share)).unwrap_or(0))
    }

    async fn is_real(&self, block: &BlockRef) -> anyhow::Result<bool> {
        Ok(self.net.account_data(&self.p.real_pda(block)).await?.is_some_and(|d| !d.is_empty()))
    }

    async fn min_certifying_escrow(&self) -> anyhow::Result<Amount> {
        Ok(self.config_record().await?.min_certifying_escrow as Amount)
    }

    async fn record_real_from_job(&self, job_id: u64) -> anyhow::Result<()> {
        let job = self.net.job_record(job_id).await?;
        let pb = BlockRef { hash: job.proof_block.hash, height: job.proof_block.height, epoch_time: job.proof_block.epoch_time };
        self.net
            .send_ix(Self::ix(
                vt::client::accounts::RecordRealFromJob { config: self.p.config(), job: SvmNetwork::job_pda(job_id), real: self.p.real_pda(&pb), payer: self.net.me_key(), system_program: SYSTEM },
                vt::client::args::RecordRealFromJob { job_id },
            ))
            .await
    }

    async fn record_real(&self, low: &BlockRef, high: &BlockRef) -> anyhow::Result<()> {
        let walk = self.net.walk(high, low).await?;
        let ix = Self::ix(
            vt::client::accounts::RecordReal { config: self.p.config(), high_real: self.p.real_pda(high), walk, low_real: self.p.real_pda(low), payer: self.net.me_key(), system_program: SYSTEM },
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
                    config: self.p.config(),
                    protocol: SvmNetwork::pr_pda(&[b"protocol"]),
                    application: SvmNetwork::pr_pda(&[b"application", self.p.config().as_ref()]),
                    job: SvmNetwork::job_pda(job_id),
                    tag_record: SvmNetwork::pr_pda(&[b"tag", self.p.config().as_ref(), &checkpoint_tag(n)]),
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
        let index = self.net.read_account::<vt::accounts::Chain>(&self.p.chain_pda(&op)).await?.map_or(0, |c| c.messages);
        let extra = self.p.accounts_of(self.peer, batch);
        let buffered = tx.raw_tx.len() + batch.len() > INLINE;
        // Another pair's submission on this key may be using the buffer.
        let _buffer = if buffered { Some(self.net.buffer_lock().await) } else { None };
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
                config: self.p.config(),
                chain: self.p.chain_pda(&op),
                real: self.p.real_pda(&tx.real),
                walk: walk.unwrap_or(SYSTEM),
                node: SvmNetwork::node_pda(&tx.block),
                message: self.p.message_pda(&op, index),
                claim: self.p.claim_pda(id),
                stake: self.p.stake_pda(id, &op),
                submitter: me,
                buffer: buffered.then(|| buffer_pda(&me)),
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
        let m: Option<vt::accounts::Message> = self.net.read_account(&self.p.message_pda(&op, index)).await?;
        Ok(m.filter(|m| m.operator == op && m.index == index).map(|m| m.batch))
    }

    async fn assets(&self) -> anyhow::Result<Vec<AssetInfo>> {
        let count = self.config_record().await?.asset_count;
        let mut out = vec![];
        for n in 0..count {
            let a = self.home_asset(n).await?;
            out.push(AssetInfo { number: n, token: a.mint.to_bytes(), decimals: a.record_decimals });
        }
        Ok(out)
    }

    /// Only locks in a finalized block: a record of one Solana could still
    /// roll back would be false, and slashed (section 11.5).
    async fn final_locks_after(&self, after: u64, limit: usize) -> anyhow::Result<Vec<Lock>> {
        let config: vt::accounts::Config = self.net.read_final(&self.p.config()).await?.ok_or_else(|| anyhow::anyhow!("the vault is not initialized"))?;
        let mut out = vec![];
        for id in (after + 1)..=config.lock_count {
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
        let l: Option<vt::accounts::HomeLock> = self.net.read_final(&self.p.home_lock_pda(id)).await?;
        Ok(l.map(|l| Lock {
            id,
            asset: l.asset,
            amount: l.amount,
            recipient: l.recipient,
            fee: l.fee,
            fast_fee: l.fast_fee,
            locked_at: l.locked_at as u64,
            fee_paid: l.fee_paid,
            returned: l.returned,
        }))
    }

    async fn take_fee(&self, record: &Record) -> anyhow::Result<()> {
        let me = self.net.me_key();
        match *record {
            Record::Lock { home: SOLANA, id, asset, .. } => {
                let Some(l) = self.net.read_account::<vt::accounts::HomeLock>(&self.p.home_lock_pda(id)).await? else { return Ok(()) };
                if !l.fee_paid || l.fee_taken || l.fee_to != me || l.fee == 0 {
                    return Ok(());
                }
                let a = self.home_asset(asset).await?;
                let (to, tokens, mint, program) = if a.number == 0 {
                    (None, None, None, None)
                } else {
                    (Some(self.ensure_ata(&me, &a.mint, &a.token_program).await?), Some(self.p.home_tokens(&a.mint)), Some(a.mint), Some(a.token_program))
                };
                self.net
                    .send_ix(Self::ix(
                        vt::client::accounts::TakeLockFee {
                            config: self.p.config(),
                            lock: self.p.home_lock_pda(id),
                            asset: self.p.asset_pda(asset),
                            to,
                            tokens,
                            mint,
                            token_program: program,
                            operator: me,
                        },
                        vt::client::args::TakeLockFee { lock_id: id },
                    ))
                    .await
            }
            Record::Request { net: SOLANA, id, asset, .. } => {
                let Some(r) = self.net.read_account::<vt::accounts::Request>(&self.p.request_pda(id)).await? else { return Ok(()) };
                if !r.fee_paid || r.fee_taken || r.fee_to != me || r.fee == 0 {
                    return Ok(());
                }
                let to = self.ensure_ata(&me, &self.p.receipt(asset), &TOKEN).await?;
                self.net
                    .send_ix(Self::ix(
                        vt::client::accounts::TakeRequestFee {
                            config: self.p.config(),
                            request: self.p.request_pda(id),
                            mint: self.p.receipt(asset),
                            holding: self.p.holding(asset),
                            to,
                            operator: me,
                            token_program: TOKEN,
                        },
                        vt::client::args::TakeRequestFee { request_id: id },
                    ))
                    .await
            }
            _ => Ok(()),
        }
    }

    async fn request_paid(&self, id: u64) -> anyhow::Result<bool> {
        Ok(self.net.account_data(&self.p.paid_pda(id)).await?.is_some_and(|d| !d.is_empty()))
    }

    async fn has_receipt(&self, asset: u32) -> anyhow::Result<bool> {
        Ok(self.net.account_data(&self.p.receipt(asset)).await?.is_some_and(|d| !d.is_empty()))
    }

    /// Only burns in a finalized block, as locks.
    async fn receipt_balance(&self, asset: u32) -> anyhow::Result<u64> {
        let a: Option<anchor_spl::token::TokenAccount> = self.net.read_account(&ata_of(&self.net.me_key(), &self.p.receipt(asset), &TOKEN)).await?;
        Ok(a.map_or(0, |a| a.amount))
    }

    async fn make_receipt(&self, claim: u64, record: &Record) -> anyhow::Result<()> {
        let Record::Asset { home, asset, .. } = record else { anyhow::bail!("only an ASSET record makes a receipt") };
        anyhow::ensure!(*home == self.peer, "only an asset of the peer has its receipt here");
        self.net
            .send_ix(Self::ix(
                vt::client::accounts::MakeReceipt {
                    config: self.p.config(),
                    claim: self.p.claim_pda(claim),
                    mint: self.p.receipt(*asset),
                    holding: self.p.holding(*asset),
                    payer: self.net.me_key(),
                    token_program: TOKEN,
                    system_program: SYSTEM,
                },
                vt::client::args::MakeReceipt { claim_id: claim, asset: *asset, record: record.bytes() },
            ))
            .await
    }

    async fn requests_after(&self, after: u64, limit: usize) -> anyhow::Result<Vec<Request>> {
        let config: vt::accounts::Config = self.net.read_final(&self.p.config()).await?.ok_or_else(|| anyhow::anyhow!("the vault is not initialized"))?;
        let mut out = vec![];
        for id in (after + 1)..=config.request_count {
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
        let r: Option<vt::accounts::Request> = self.net.read_final(&self.p.request_pda(id)).await?;
        Ok(r.map(|r| Request {
            id,
            asset: r.asset,
            amount: r.amount,
            to: r.to,
            fee: r.fee,
            fast_fee: r.fast_fee,
            requested_at: r.requested_at as u64,
            fee_paid: r.fee_paid,
        }))
    }

    /// As of a finalized block, for the same reason as requests.
    async fn given_up(&self, id: u64) -> anyhow::Result<bool> {
        let m: Option<vt::accounts::LockMark> = self.net.read_final(&self.p.lock_pda(id)).await?;
        Ok(m.is_some_and(|m| m.given_up))
    }

    async fn receipt_issued(&self, id: u64) -> anyhow::Result<bool> {
        let m: Option<vt::accounts::LockMark> = self.net.read_account(&self.p.lock_pda(id)).await?;
        Ok(m.is_some_and(|m| m.issued))
    }

    async fn lock_attests(&self, id: u64) -> anyhow::Result<Option<(i64, Vec<FastLock>)>> {
        let Some(m): Option<vt::accounts::LockMark> = self.net.read_account(&self.p.lock_pda(id)).await? else { return Ok(None) };
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

    async fn claim_count(&self) -> anyhow::Result<u64> {
        Ok(self.config_record().await?.claim_count)
    }

    async fn claim_status(&self, id: u64) -> anyhow::Result<Claim> {
        self.claim(id).await
    }

    async fn claim(&self, id: u64) -> anyhow::Result<Claim> {
        let c: vt::accounts::Claim = self.net.read_account(&self.p.claim_pda(id)).await?.ok_or_else(|| anyhow::anyhow!("claim {id} does not exist"))?;
        let mut records = decode(&c.records).unwrap_or_default();
        // A BOND carried in it, from Ethereum.
        for a in &c.assets {
            if a.peer_bond != 0 {
                records.retain(|r| !matches!(r, Record::Bond { home, asset, .. } if *home == a.home && *asset == a.asset));
                records.push(Record::Bond { net: self.peer, home: a.home, asset: a.asset, amount: a.peer_bond });
            }
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
        let c: vt::accounts::Claim = self.net.read_account(&self.p.claim_pda(id)).await?.ok_or_else(|| anyhow::anyhow!("claim {id} does not exist"))?;
        self.net
            .send_ix(Self::ix(vt::client::accounts::Decide { claim: self.p.claim_pda(id), chain: self.p.chain_pda(&c.operator), config: self.p.config() }, vt::client::args::Decide { claim_id: id }))
            .await
    }

    async fn collect(&self, id: u64) -> anyhow::Result<()> {
        let me = self.net.me_key();
        let c: vt::accounts::Claim = self.net.read_account(&self.p.claim_pda(id)).await?.ok_or_else(|| anyhow::anyhow!("claim {id} does not exist"))?;
        let Some(s) = self.net.read_account::<vt::accounts::Stake>(&self.p.stake_pda(id, &me)).await? else { return Ok(()) };
        let mine = if c.accepted { s.answers } else { s.objections };
        if mine == 0 {
            return Ok(());
        }
        self.net
            .send_ix(Self::ix(
                vt::client::accounts::Collect { config: self.p.config(), claim: self.p.claim_pda(id), stake: self.p.stake_pda(id, &me), credit: self.p.credit_pda(&me, SOLANA, 0), who: me, system_program: SYSTEM },
                vt::client::args::Collect { claim_id: id },
            ))
            .await
    }

    async fn attest_lock(&self, record: &Record) -> anyhow::Result<u64> {
        let Record::Lock { home, asset, id, recipient, .. } = record else { anyhow::bail!("only a lock is attested") };
        anyhow::ensure!(*home == self.peer, "only a lock on the peer is attested here");
        let me = self.net.me_key();
        let recipient = Pubkey::new_from_array(*recipient);
        let to = self.ensure_ata(&recipient, &self.p.receipt(*asset), &TOKEN).await?;
        let n = self.config_record().await?.attest_count + 1;
        self.net
            .send_ix(Self::ix(
                vt::client::accounts::AttestLock {
                    config: self.p.config(),
                    chain: self.p.chain_pda(&me),
                    mark: self.p.lock_pda(*id),
                    fast: self.p.fast_pda(n),
                    mint: self.p.receipt(*asset),
                    holding: self.p.holding(*asset),
                    from: ata_of(&me, &self.p.receipt(*asset), &TOKEN),
                    to,
                    attester: me,
                    token_program: TOKEN,
                    system_program: SYSTEM,
                },
                vt::client::args::AttestLock { asset: *asset, record: record.bytes() },
            ))
            .await?;
        Ok(n)
    }

    async fn attest_count(&self) -> anyhow::Result<u64> {
        Ok(self.config_record().await?.attest_count)
    }

    async fn fast_lock(&self, id: u64) -> anyhow::Result<Option<FastLock>> {
        let f: Option<vt::accounts::FastLock> = self.net.read_account(&self.p.fast_pda(id)).await?;
        Ok(f.map(|f| {
            let stated = Record::Lock {
                home: self.peer,
                asset: f.asset,
                id: f.lock_id,
                amount: f.amount,
                recipient: f.recipient.to_bytes(),
                fee: f.fee,
                fast_fee: f.fast_fee,
                at: f.locked_at as u64,
            };
            FastLock {
                id: f.id,
                attester: f.attester.to_string(),
                lock_id: f.lock_id,
                stated: record_hash(&stated),
                asset: f.asset,
                collateral: f.collateral,
                attested_at: f.attested_at,
                claim: f.claim,
                prev: f.prev,
                burned: f.burned,
            }
        }))
    }

    async fn link_fast(&self, claim: u64, attest: u64, linked: Option<u64>) -> anyhow::Result<()> {
        self.net
            .send_ix(Self::ix(
                vt::client::accounts::LinkFast { config: self.p.config(), fast: self.p.fast_pda(attest), claim: self.p.claim_pda(claim), linked: linked.map(|c| self.p.claim_pda(c)), attester: self.net.me_key() },
                vt::client::args::LinkFast { claim_id: claim, attest },
            ))
            .await
    }

    async fn burn_fast(&self, attest: u64, linked: Option<u64>) -> anyhow::Result<()> {
        let me = self.net.me_key();
        let f: vt::accounts::FastLock = self.net.read_account(&self.p.fast_pda(attest)).await?.ok_or_else(|| anyhow::anyhow!("attest {attest} does not exist"))?;
        self.net
            .send_ix(Self::ix(
                vt::client::accounts::BurnFast {
                    config: self.p.config(),
                    fast: self.p.fast_pda(attest),
                    linked: linked.map(|c| self.p.claim_pda(c)),
                    mint: self.p.receipt(f.asset),
                    holding: self.p.holding(f.asset),
                    caller_credit: self.p.credit_pda(&me, self.peer, f.asset),
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
        let f: vt::accounts::FastLock = self.net.read_account(&self.p.fast_pda(attest)).await?.ok_or_else(|| anyhow::anyhow!("attest {attest} does not exist"))?;
        let c = self.claim(claim).await?;
        let record = c
            .records
            .iter()
            .find(|r| matches!(r, Record::Lock { id, .. } if *id == f.lock_id))
            .ok_or_else(|| anyhow::anyhow!("claim {claim} does not carry lock {}", f.lock_id))?;
        // The true record's recipient may be owed the rest of the fast fee,
        // or its receipt, in the attest's asset.
        let Record::Lock { recipient, .. } = record else { unreachable!() };
        let to = self.ensure_ata(&Pubkey::new_from_array(*recipient), &self.p.receipt(f.asset), &TOKEN).await?;
        self.net
            .send_ix(Self::ix(
                vt::client::accounts::SettleFast {
                    config: self.p.config(),
                    claim: self.p.claim_pda(claim),
                    fast: self.p.fast_pda(attest),
                    mark: self.p.lock_pda(f.lock_id),
                    attester: f.attester,
                    mint: self.p.receipt(f.asset),
                    holding: self.p.holding(f.asset),
                    prev: (f.prev != 0).then(|| self.p.fast_pda(f.prev)),
                    to: Some(to),
                    attester_credit: self.p.credit_pda(&f.attester, self.peer, f.asset),
                    caller_credit: self.p.credit_pda(&me, self.peer, f.asset),
                    caller: me,
                    token_program: TOKEN,
                    system_program: SYSTEM,
                },
                vt::client::args::SettleFast { claim_id: claim, attest },
            ))
            .await
    }

    async fn fast_pay(&self, record: &Record) -> anyhow::Result<()> {
        let Record::Request { net, asset, id, to, .. } = record else { anyhow::bail!("only a burn is paid") };
        anyhow::ensure!(*net == self.peer, "only a burn on the peer is paid here");
        let me = self.net.me_key();
        let bytes = record.bytes();
        let a = self.home_asset(*asset).await?;
        let to_key = Pubkey::new_from_array(*to);
        let (to_account, from, mint, program) = if a.number == 0 {
            (to_key, None, None, None)
        } else {
            (self.ensure_ata(&to_key, &a.mint, &a.token_program).await?, Some(ata_of(&me, &a.mint, &a.token_program)), Some(a.mint), Some(a.token_program))
        };
        self.net
            .send_ix(Self::ix(
                vt::client::accounts::FastPay { config: self.p.config(),
                    home_asset: self.p.asset_pda(*asset),
                    fast_pay: self.p.fast_pay_pda(*id, &bytes),
                    paid: self.p.paid_pda(*id),
                    to: to_account,
                    from,
                    mint,
                    token_program: program,
                    attester: me,
                    system_program: SYSTEM,
                },
                vt::client::args::FastPay { request_id: *id, asset: *asset, record_hash: sha256(&[&bytes]), record: bytes.clone() },
            ))
            .await
    }

    async fn fast_paid_by(&self, record: &Record) -> anyhow::Result<Option<String>> {
        let Record::Request { id, .. } = record else { return Ok(None) };
        let f: Option<vt::accounts::FastPay> = self.net.read_account(&self.p.fast_pay_pda(*id, &record.bytes())).await?;
        Ok(f.map(|f| f.attester.to_string()))
    }

    async fn pay_request(&self, claim: u64, record: &Record) -> anyhow::Result<()> {
        let Record::Request { net, asset, id, to, .. } = record else { anyhow::bail!("only a burn is paid") };
        anyhow::ensure!(*net == self.peer, "only a burn on the peer is paid here");
        let bytes = record.bytes();
        let a = self.home_asset(*asset).await?;
        let attester = self.fast_paid_by(record).await?.map(|s| Self::operator(&s)).transpose()?;
        let to_key = Pubkey::new_from_array(*to);
        let (to_account, attester_account) = if a.number == 0 {
            (to_key, attester)
        } else {
            let t = self.ensure_ata(&to_key, &a.mint, &a.token_program).await?;
            let at = match attester {
                Some(k) => Some(self.ensure_ata(&k, &a.mint, &a.token_program).await?),
                None => None,
            };
            (t, at)
        };
        let (tokens, mint, program) = self.token_accounts(&a);
        self.net
            .send_ix(Self::ix(
                vt::client::accounts::PayRequest {
                    config: self.p.config(),
                    claim: self.p.claim_pda(claim),
                    home_asset: self.p.asset_pda(*asset),
                    paid: self.p.paid_pda(*id),
                    fast_pay: self.p.fast_pay_pda(*id, &bytes),
                    to: to_account,
                    attester: attester_account,
                    tokens,
                    mint,
                    token_program: program,
                    payer: self.net.me_key(),
                    system_program: SYSTEM,
                },
                vt::client::args::PayRequest { claim_id: claim, request_id: *id, asset: *asset, record: bytes },
            ))
            .await
    }

    async fn credit(&self, home: u8, asset: u32) -> anyhow::Result<u128> {
        let c: Option<vt::accounts::Credit> = self.net.read_account(&self.p.credit_pda(&self.net.me_key(), home, asset)).await?;
        Ok(c.map_or(0, |c| c.amount as u128))
    }

    async fn withdraw_credit(&self, home: u8, asset: u32) -> anyhow::Result<()> {
        let me = self.net.me_key();
        if home == SOLANA {
            let a = self.home_asset(asset).await?;
            let (to, tokens, mint, program) = if a.number == 0 {
                (None, None, None, None)
            } else {
                (Some(self.ensure_ata(&me, &a.mint, &a.token_program).await?), Some(self.p.home_tokens(&a.mint)), Some(a.mint), Some(a.token_program))
            };
            return self
                .net
                .send_ix(Self::ix(
                    vt::client::accounts::WithdrawCreditHome {
                        credit: self.p.credit_pda(&me, SOLANA, asset),
                        config: self.p.config(),
                        home_asset: self.p.asset_pda(asset),
                        to,
                        tokens,
                        mint,
                        token_program: program,
                        owner: me,
                    },
                    vt::client::args::WithdrawCreditHome { asset },
                ))
                .await;
        }
        let to = self.ensure_ata(&me, &self.p.receipt(asset), &TOKEN).await?;
        self.net
            .send_ix(Self::ix(
                vt::client::accounts::WithdrawCreditReceipt {
                    credit: self.p.credit_pda(&me, self.peer, asset),
                    config: self.p.config(),
                    mint: self.p.receipt(asset),
                    holding: self.p.holding(asset),
                    to,
                    owner: me,
                    token_program: TOKEN,
                },
                vt::client::args::WithdrawCreditReceipt { asset },
            ))
            .await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ipow_protocol_core::vault::{encode, ETHEREUM};

    #[test]
    fn pairs_with_every_network_of_the_list_up_to_arbitrum() {
        // D133, D141: 1 and 3 to 9; not Solana, not 0, not past Arbitrum.
        for peer in [1u8, 3, 4, 5, 6, 7, 8, 9] {
            assert!(super::can_pair(peer), "{peer}");
        }
        for peer in [0u8, 2, 10, 255] {
            assert!(!super::can_pair(peer), "{peer}");
        }
    }

    #[test]
    fn reads_the_accounts_of_a_batch_up_to_where_it_stops_parsing() {
        let lock = Record::Lock { home: SOLANA, asset: 0, id: 5, amount: 1, recipient: [1; 32], fee: 0, fast_fee: 0, at: 0 };
        let cancel = Record::Cancel { net: ETHEREUM, id: 7 };
        let p = Pdas::new(ETHEREUM);
        // Another pair's accounts are others.
        assert_ne!(Pdas::new(3).home_lock_pda(5), p.home_lock_pda(5));
        let good = encode(&[lock.clone(), cancel.clone()]);
        assert_eq!(p.accounts_of(ETHEREUM, &good), vec![p.home_lock_pda(5), p.home_lock_pda(7)]);
        // A kind no vault knows, and a record cut short: the program still
        // reads the accounts before it, then judges the batch false.
        let mut bad = encode(std::slice::from_ref(&lock));
        bad.push(0xff);
        assert_eq!(p.accounts_of(ETHEREUM, &bad), vec![p.home_lock_pda(5)]);
        let mut short = encode(&[lock, cancel]);
        short.pop();
        assert_eq!(p.accounts_of(ETHEREUM, &short), vec![p.home_lock_pda(5)]);
    }
}

#[cfg(test)]
mod pair_tests {
    use super::*;
    use ipow_protocol_core::vault::{BASE, ETHEREUM};

    fn side(net: u8, vault: &[u8], op: &[u8]) -> Vec<u8> {
        let pad = |a: &[u8]| {
            let mut out = vec![0u8; 32 - a.len()];
            out.extend_from_slice(a);
            out
        };
        [vec![net], pad(vault), pad(op)].concat()
    }

    #[test]
    fn names_the_lower_network_first_in_a_registration() {
        let (vault, op_there, me) = ([0x11; 20], [0x22; 20], Pubkey::new_from_array([0x33; 32]));
        // With Ethereum (1): Ethereum's side first.
        let c = Pdas::new(ETHEREUM).config;
        let expected = sha256(&[b"iPoW pair", &side(ETHEREUM, &vault, &op_there), &side(SOLANA, c.as_ref(), me.as_ref())]);
        assert_eq!(pair_commitment(ETHEREUM, &vault, &op_there, &c, &me), expected);
        // With Base (3): Solana's side first, and Base's own configuration.
        let c3 = Pdas::new(BASE).config;
        let expected = sha256(&[b"iPoW pair", &side(SOLANA, c3.as_ref(), me.as_ref()), &side(BASE, &vault, &op_there)]);
        assert_eq!(pair_commitment(BASE, &vault, &op_there, &c3, &me), expected);
        assert_ne!(c, c3);
    }

    #[test]
    fn keys_every_account_by_the_pairs_configuration() {
        let (p1, p3) = (Pdas::new(ETHEREUM), Pdas::new(BASE));
        assert_eq!(p3.config, pda(&[b"config", &[BASE]]));
        // As the program seeds them: the configuration after the first seed.
        assert_eq!(p3.claim_pda(7), pda(&[b"claim", p3.config.as_ref(), &7u64.to_le_bytes()]));
        assert_ne!(p1.claim_pda(7), p3.claim_pda(7));
        assert_ne!(p1.receipt(0), p3.receipt(0));
    }
}
