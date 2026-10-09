//! Tests of the Solana protocol, second piece: the chain head, the anchor,
//! the proof, the payment and the slash of a missed duty. They mirror
//! `test/protocol/iPoWProtocol.Duty.test.ts` on Ethereum. Spec:
//! docs/design/ipow-protocol.md, sections 3, 4.3, 4.4, 5 and 6.
//!
//! The Bitcoin blocks here are mined by the test at an easy difficulty,
//! which only the test build of the light client accepts.

use anchor_lang;
use anchor_lang::solana_program::instruction::{AccountMeta, Instruction};
use anchor_litesvm::{AnchorContext, AnchorLiteSVM, TestHelpers};
use sha2::{Digest, Sha256};
use solana_keypair::Keypair;
use solana_program::pubkey::Pubkey;
use solana_signer::Signer;

anchor_lang::declare_program!(ipow_protocol);
anchor_lang::declare_program!(ipow_light_client);

const SOL: u64 = 1_000_000_000;
const MINUTE: i64 = 60;
const HOUR: i64 = 3600;
const DAY: i64 = 24 * HOUR;
const T0: i64 = 1_800_000_000;
const EASY: u32 = 0x207f_ffff;
const SYSTEM: Pubkey = anchor_lang::solana_program::system_program::ID;

// ---------------------------------------------------------------------
// Bitcoin
// ---------------------------------------------------------------------

fn sha256d(data: &[u8]) -> [u8; 32] {
    let once = Sha256::digest(data);
    Sha256::digest(once).into()
}

fn sha256(parts: &[&[u8]]) -> [u8; 32] {
    let mut h = Sha256::new();
    for p in parts {
        h.update(p);
    }
    h.finalize().into()
}

fn target_be(bits: u32) -> [u8; 32] {
    let exp = (bits >> 24) as usize;
    let mant = bits & 0x007f_ffff;
    let mut t = [0u8; 32];
    let m = [(mant >> 16) as u8, (mant >> 8) as u8, mant as u8];
    for i in 0..3 {
        let pos = 32 - exp + i;
        if pos < 32 {
            t[pos] = m[i];
        }
    }
    t
}

fn mine(prev: [u8; 32], merkle_root: [u8; 32], time: u32) -> [u8; 80] {
    let mut h = [0u8; 80];
    h[0..4].copy_from_slice(&0x2000_0000u32.to_le_bytes());
    h[4..36].copy_from_slice(&prev);
    h[36..68].copy_from_slice(&merkle_root);
    h[68..72].copy_from_slice(&time.to_le_bytes());
    h[72..76].copy_from_slice(&EASY.to_le_bytes());
    let target = target_be(EASY);
    for nonce in 0u32.. {
        h[76..80].copy_from_slice(&nonce.to_le_bytes());
        let mut be = sha256d(&h);
        be.reverse();
        if be <= target {
            return h;
        }
    }
    unreachable!()
}

fn var_int(n: usize) -> Vec<u8> {
    assert!(n < 0xfd);
    vec![n as u8]
}

/// A transaction without witness data: one input, a coin at output 0 and an
/// OP_RETURN of 32 bytes at output 1.
fn build_tx(spends: ([u8; 32], u32), payload: [u8; 32]) -> Vec<u8> {
    let mut tx = vec![];
    tx.extend_from_slice(&2u32.to_le_bytes());
    tx.extend(var_int(1));
    tx.extend_from_slice(&spends.0);
    tx.extend_from_slice(&spends.1.to_le_bytes());
    tx.extend(var_int(0));
    tx.extend_from_slice(&[0xff; 4]);
    tx.extend(var_int(2));
    let coin = [&[0x00u8, 0x14][..], &[0x11; 20]].concat();
    tx.extend_from_slice(&546u64.to_le_bytes());
    tx.extend(var_int(coin.len()));
    tx.extend(coin);
    let op_return = [&[0x6au8, 0x20][..], &payload].concat();
    tx.extend_from_slice(&0u64.to_le_bytes());
    tx.extend(var_int(op_return.len()));
    tx.extend(op_return);
    tx.extend_from_slice(&[0u8; 4]);
    tx
}

/// The Merkle root of the txids and the proof of the one at `index`.
fn merkle(txids: &[[u8; 32]], index: usize) -> ([u8; 32], Vec<[u8; 32]>) {
    let mut level = txids.to_vec();
    let mut siblings = vec![];
    let mut at = index;
    while level.len() > 1 {
        if level.len() % 2 == 1 {
            level.push(*level.last().unwrap());
        }
        siblings.push(level[at ^ 1]);
        level = level.chunks(2).map(|p| sha256d(&[p[0], p[1]].concat())).collect();
        at /= 2;
    }
    (level[0], siblings)
}

fn tag_payload(tag: &[u8; 32]) -> [u8; 32] {
    sha256(&[b"iPoW job", tag])
}

fn chain_head_commitment(operator: &Pubkey) -> [u8; 32] {
    sha256(&[b"iPoW chain head", ipow_protocol::ID.as_ref(), operator.as_ref()])
}

fn note_for(guardian: &Pubkey, job_id: u64, evidence: &[u8; 32], salt: &[u8; 32]) -> [u8; 32] {
    sha256(&[guardian.as_ref(), &job_id.to_le_bytes(), evidence, salt])
}

// ---------------------------------------------------------------------
// Addresses
// ---------------------------------------------------------------------

fn lc(seeds: &[&[u8]]) -> Pubkey {
    Pubkey::find_program_address(seeds, &ipow_light_client::ID).0
}
fn pr(seeds: &[&[u8]]) -> Pubkey {
    Pubkey::find_program_address(seeds, &ipow_protocol::ID).0
}
fn node_pda(r: &Ref) -> Pubkey {
    lc(&[b"node", &r.hash, &r.height.to_le_bytes(), &r.epoch_time.to_le_bytes()])
}
fn operator_pda(o: &Pubkey) -> Pubkey {
    pr(&[b"operator", o.as_ref()])
}
fn job_pda(id: u64) -> Pubkey {
    pr(&[b"job", &id.to_le_bytes()])
}
fn credit_pda(o: &Pubkey) -> Pubkey {
    pr(&[b"credit", o.as_ref()])
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct Ref {
    hash: [u8; 32],
    height: u32,
    epoch_time: u32,
}

impl Ref {
    fn p(&self) -> ipow_protocol::types::BlockRef {
        ipow_protocol::types::BlockRef { hash: self.hash, height: self.height, epoch_time: self.epoch_time }
    }
    fn l(&self) -> ipow_light_client::types::BlockRef {
        ipow_light_client::types::BlockRef { hash: self.hash, height: self.height, epoch_time: self.epoch_time }
    }
}

fn compute_budget() -> Instruction {
    let mut data = vec![2u8];
    data.extend_from_slice(&1_400_000u32.to_le_bytes());
    Instruction { program_id: "ComputeBudget111111111111111111111111111111".parse().unwrap(), accounts: vec![], data }
}

fn code(name: &str) -> String {
    format!("Error Code: {name}")
}

fn expect_err<T: std::fmt::Debug>(r: Result<T, String>, needle: &str) {
    match r {
        Ok(v) => panic!("expected {needle}, got {v:?}"),
        Err(logs) => assert!(logs.contains(needle), "expected {needle}, got:\n{logs}"),
    }
}

// ---------------------------------------------------------------------
// The test world
// ---------------------------------------------------------------------

struct World {
    ctx: AnchorContext,
    app: Keypair,
    user: Keypair,
    operator: Keypair,
    guardian: Keypair,
    stranger: Keypair,
    tip: Ref,
    epoch_time: u32,
    epoch_start: Pubkey,
    blocks: std::collections::HashMap<[u8; 32], Vec<[u8; 32]>>,
    salt: u32,
    walks: u64,
}

impl World {
    fn new() -> Self {
        let ctx = AnchorLiteSVM::build_with_programs(&[
            (ipow_light_client::ID, include_bytes!("../../../../target/deploy-test/ipow_light_client.so")),
            (ipow_protocol::ID, include_bytes!("../../../../target/deploy/ipow_protocol.so")),
        ]);
        let mut w = World {
            ctx,
            app: Keypair::new(),
            user: Keypair::new(),
            operator: Keypair::new(),
            guardian: Keypair::new(),
            stranger: Keypair::new(),
            tip: Ref { hash: [0; 32], height: 0, epoch_time: 0 },
            epoch_time: 0,
            epoch_start: Pubkey::default(),
            blocks: Default::default(),
            salt: 0,
            walks: 0,
        };
        for k in ["app", "user", "operator", "guardian", "stranger"] {
            let funded = w.ctx.svm.create_funded_account(1_000 * SOL).unwrap();
            match k {
                "app" => w.app = funded,
                "user" => w.user = funded,
                "operator" => w.operator = funded,
                "guardian" => w.guardian = funded,
                _ => w.stranger = funded,
            }
        }
        w.set_now(T0);

        let easy = target_be(EASY);
        let payer = w.operator.insecure_clone();
        let ix = w.lc_ix(
            ipow_light_client::client::accounts::Initialize {
                config: lc(&[b"config"]),
                day_table: lc(&[b"days"]),
                authority: payer.pubkey(),
                program_data: SYSTEM,
                system_program: SYSTEM,
            },
            ipow_light_client::client::args::Initialize { min_height: 0, max_target: easy, pow_limit: easy },
        );
        w.send(ix, &[&payer]).unwrap();
        let ix = w.pr_ix(
            ipow_protocol::client::accounts::InitializeProtocol {
                protocol: pr(&[b"protocol"]),
                vault: pr(&[b"vault"]),
                payer: payer.pubkey(),
                system_program: SYSTEM,
            },
            ipow_protocol::client::args::InitializeProtocol {},
        );
        w.send(ix, &[&payer]).unwrap();
        let app = w.app.insecure_clone();
        let ix = w.pr_ix(
            ipow_protocol::client::accounts::RegisterApplication {
                application: pr(&[b"application", app.pubkey().as_ref()]),
                key: app.pubkey(),
                funder: app.pubkey(),
                system_program: SYSTEM,
            },
            ipow_protocol::client::args::RegisterApplication { challenge_periods: vec![7 * DAY as u32] },
        );
        w.send(ix, &[&app]).unwrap();
        w.start_chain();
        w
    }

    fn lc_ix<A: anchor_lang::ToAccountMetas, D: anchor_lang::InstructionData>(&self, a: A, d: D) -> Instruction {
        Instruction { program_id: ipow_light_client::ID, accounts: a.to_account_metas(None), data: d.data() }
    }

    fn pr_ix<A: anchor_lang::ToAccountMetas, D: anchor_lang::InstructionData>(&self, a: A, d: D) -> Instruction {
        Instruction { program_id: ipow_protocol::ID, accounts: a.to_account_metas(None), data: d.data() }
    }

    fn send(&mut self, ix: Instruction, signers: &[&Keypair]) -> Result<(), String> {
        self.ctx.svm.expire_blockhash();
        let r = self.ctx.execute_instructions(vec![compute_budget(), ix], signers).unwrap();
        if r.is_success() { Ok(()) } else { Err(r.logs().join("\n")) }
    }

    fn set_now(&mut self, t: i64) {
        let mut clock: solana_clock::Clock = self.ctx.svm.get_sysvar();
        clock.unix_timestamp = t;
        self.ctx.svm.set_sysvar(&clock);
    }

    fn now(&self) -> i64 {
        let c: solana_clock::Clock = self.ctx.svm.get_sysvar();
        c.unix_timestamp
    }

    /// Moves to the next slot, so that a note sealed before counts.
    fn next_slot(&mut self) {
        let mut clock: solana_clock::Clock = self.ctx.svm.get_sysvar();
        clock.slot += 1;
        self.ctx.svm.set_sysvar(&clock);
    }

    /// Records an epoch start whose last block is 10 minutes old.
    fn start_chain(&mut self) {
        self.epoch_time = (self.now() - 60 * MINUTE) as u32;
        let mut headers = vec![];
        let mut prev = [0u8; 32];
        for i in 0..6 {
            self.salt += 1;
            let h = mine(prev, sha256d(&self.salt.to_le_bytes()), self.epoch_time + i * 600);
            prev = sha256d(&h);
            headers.push(h);
        }
        let nodes: Vec<Pubkey> = headers
            .iter()
            .enumerate()
            .map(|(i, h)| node_pda(&Ref { hash: sha256d(h), height: i as u32, epoch_time: self.epoch_time }))
            .collect();
        let payer = self.operator.insecure_clone();
        self.epoch_start = lc(&[b"epoch_start", &sha256d(&headers[0]), &0u32.to_le_bytes(), &self.epoch_time.to_le_bytes()]);
        let mut ix = self.lc_ix(
            ipow_light_client::client::accounts::AddEpochStart {
                config: lc(&[b"config"]),
                day_table: lc(&[b"days"]),
                epoch_start: self.epoch_start,
                payer: payer.pubkey(),
                system_program: SYSTEM,
            },
            ipow_light_client::client::args::AddEpochStart { headers: headers.concat(), height: 0 },
        );
        for n in nodes {
            ix.accounts.push(AccountMeta::new(n, false));
        }
        self.send(ix, &[&payer]).unwrap();
        self.tip = Ref { hash: prev, height: 5, epoch_time: self.epoch_time };
    }

    /// Mines a block on `parent` (the tip unless stated) holding `txs`, and
    /// streams it. Returns its reference.
    fn add(&mut self, txs: &[Vec<u8>], parent: Option<Ref>) -> Ref {
        let on = parent.unwrap_or(self.tip);
        self.salt += 1;
        let coinbase = sha256d(&[b"coinbase".as_ref(), &self.salt.to_le_bytes()].concat());
        let mut txids = vec![coinbase];
        txids.extend(txs.iter().map(|t| sha256d(t)));
        let time = (self.now() + 1) as u32;
        let h = mine(on.hash, merkle(&txids, 0).0, time);
        let r = Ref { hash: sha256d(&h), height: on.height + 1, epoch_time: on.epoch_time };
        let payer = self.operator.insecure_clone();
        let mut ix = self.lc_ix(
            ipow_light_client::client::accounts::Extend {
                config: lc(&[b"config"]),
                parent: node_pda(&on),
                payer: payer.pubkey(),
                system_program: SYSTEM,
            },
            ipow_light_client::client::args::Extend { headers: h.to_vec() },
        );
        ix.accounts.push(AccountMeta::new(node_pda(&r), false));
        self.send(ix, &[&payer]).unwrap();
        self.blocks.insert(r.hash, txids);
        if parent.is_none() {
            self.tip = r;
        }
        r
    }

    fn proof_of(&self, block: &Ref, tx: &[u8]) -> (Vec<[u8; 32]>, u64) {
        let txids = &self.blocks[&block.hash];
        let i = txids.iter().position(|t| *t == sha256d(tx)).unwrap();
        (merkle(txids, i).1, i as u64)
    }

    /// Walks from `desc` down to `anc` along `path` (node refs, desc first).
    fn walk(&mut self, desc: Ref, anc: Ref, path: &[Ref]) -> Pubkey {
        self.walks += 1;
        let payer = self.operator.insecure_clone();
        let id = self.walks;
        let walk = lc(&[b"walk", payer.pubkey().as_ref(), &id.to_le_bytes()]);
        let ix = self.lc_ix(
            ipow_light_client::client::accounts::BeginWalk { walk, payer: payer.pubkey(), system_program: SYSTEM },
            ipow_light_client::client::args::BeginWalk { walk_id: id, descendant: desc.l(), ancestor: anc.l(), prev_epoch_time: 0 },
        );
        self.send(ix, &[&payer]).unwrap();
        for chunk in path.chunks(20) {
            let mut ix = self.lc_ix(
                ipow_light_client::client::accounts::WalkStep { config: lc(&[b"config"]), walk },
                ipow_light_client::client::args::WalkStep {},
            );
            for r in chunk {
                ix.accounts.push(AccountMeta::new_readonly(node_pda(r), false));
            }
            self.send(ix, &[&payer]).unwrap();
        }
        walk
    }

    fn lock_bond(&mut self, who: &Keypair, amount: u64) {
        let ix = self.pr_ix(
            ipow_protocol::client::accounts::LockBond {
                protocol: pr(&[b"protocol"]),
                operator: operator_pda(&who.pubkey()),
                vault: pr(&[b"vault"]),
                owner: who.pubkey(),
                system_program: SYSTEM,
            },
            ipow_protocol::client::args::LockBond { amount },
        );
        self.send(ix, &[who]).unwrap();
    }

    fn register_chain_head(&mut self, who: &Keypair) -> Result<Vec<u8>, String> {
        self.salt += 1;
        let tx = build_tx((sha256d(&self.salt.to_le_bytes()), 0), chain_head_commitment(&who.pubkey()));
        let block = self.add(&[tx.clone()], None);
        let (siblings, index) = self.proof_of(&block, &tx);
        let txid = sha256d(&tx);
        let ix = self.pr_ix(
            ipow_protocol::client::accounts::RegisterChainHead {
                operator: operator_pda(&who.pubkey()),
                node: node_pda(&block),
                used_tx: pr(&[b"used_tx", &txid]),
                owner: who.pubkey(),
                system_program: SYSTEM,
            },
            ipow_protocol::client::args::RegisterChainHead {
                txid,
                block: block.p(),
                raw_tx: tx.clone(),
                siblings,
                tx_index: index,
                coin_index: 0,
                tag_index: 1,
            },
        );
        self.send(ix, &[who])?;
        Ok(tx)
    }

    fn job_count(&self) -> u64 {
        let p: ipow_protocol::accounts::Protocol = self.ctx.get_account(&pr(&[b"protocol"])).unwrap();
        p.job_count
    }

    fn fee(confirmations: u16) -> u64 {
        (24 + confirmations as u64 + 20) * 5_000 * 3 / 2
    }

    /// Opens a job, lets the operator win it with `bid`, and waits until it
    /// is locked in. Returns the job id.
    fn assigned_job(&mut self, tag: [u8; 32], claim_kind: u16, bid: u64) -> u64 {
        let id = self.job_count() + 1;
        let app = self.app.insecure_clone();
        let ix = self.pr_ix(
            ipow_protocol::client::accounts::OpenJob {
                protocol: pr(&[b"protocol"]),
                application: pr(&[b"application", app.pubkey().as_ref()]),
                job: job_pda(id),
                tag_record: pr(&[b"tag", app.pubkey().as_ref(), &tag]),
                vault: pr(&[b"vault"]),
                key: app.pubkey(),
                funder: app.pubkey(),
                system_program: SYSTEM,
            },
            ipow_protocol::client::args::OpenJob {
                tag,
                escrow: SOL,
                escrow_fee_bps: 50,
                confirmations: 6,
                claim_kind,
                payer: self.user.pubkey(),
                paid: Self::fee(6) + SOL / 200,
            },
        );
        self.send(ix, &[&app]).unwrap();
        let opened = self.now();
        self.set_now(opened + MINUTE);
        let op = self.operator.insecure_clone();
        let ix = self.pr_ix(
            ipow_protocol::client::accounts::Bid {
                job: job_pda(id),
                operator: operator_pda(&op.pubkey()),
                previous: None,
                owner: op.pubkey(),
            },
            ipow_protocol::client::args::Bid { amount: bid },
        );
        self.send(ix, &[&op]).unwrap();
        self.set_now(opened + 2 * MINUTE);
        id
    }

    fn anchor_job(&mut self, id: u64, anchor: Ref, who: &Keypair) -> Result<(), String> {
        let ix = self.pr_ix(
            ipow_protocol::client::accounts::AnchorJob { job: job_pda(id), node: node_pda(&anchor), owner: who.pubkey() },
            ipow_protocol::client::args::AnchorJob { anchor: anchor.p() },
        );
        self.send(ix, &[who])
    }

    fn head(&self, who: &Pubkey) -> ([u8; 32], u32) {
        let o: ipow_protocol::accounts::Operator = self.ctx.get_account(&operator_pda(who)).unwrap();
        (o.chain_head_txid, o.chain_head_vout)
    }

    fn job(&self, id: u64) -> ipow_protocol::accounts::Job {
        self.ctx.get_account(&job_pda(id)).unwrap()
    }

    fn credit(&self, who: &Pubkey) -> u64 {
        self.ctx.get_account::<ipow_protocol::accounts::Credit>(&credit_pda(who)).map(|c| c.amount).unwrap_or(0)
    }

    fn bond(&self, who: &Pubkey) -> (u64, u64) {
        let o: ipow_protocol::accounts::Operator = self.ctx.get_account(&operator_pda(who)).unwrap();
        (o.bond, o.locked)
    }
}

/// The usual duty: a fresh anchor, the tagged transaction in block `in_block`
/// of the window, and `on_top` blocks after it. Returns what a proof needs.
struct Duty {
    anchor: Ref,
    proof_block: Ref,
    tip: Ref,
    tx: Vec<u8>,
    path: Vec<Ref>,
}

fn duty(w: &mut World, id: u64, tag: [u8; 32], in_block: u32, on_top: u32, payload: Option<[u8; 32]>) -> Duty {
    duty_spending(w, id, tag, in_block, on_top, payload, None)
}

fn duty_spending(
    w: &mut World,
    id: u64,
    tag: [u8; 32],
    in_block: u32,
    on_top: u32,
    payload: Option<[u8; 32]>,
    spends: Option<([u8; 32], u32)>,
) -> Duty {
    let op = w.operator.insecure_clone();
    let anchor = w.add(&[], None);
    w.anchor_job(id, anchor, &op).unwrap();
    let head = spends.unwrap_or_else(|| w.head(&op.pubkey()));
    let tx = build_tx(head, payload.unwrap_or_else(|| tag_payload(&tag)));
    let mut path = vec![anchor];
    for _ in 1..in_block {
        path.push(w.add(&[], None));
    }
    let proof_block = w.add(&[tx.clone()], None);
    path.push(proof_block);
    for _ in 0..on_top {
        path.push(w.add(&[], None));
    }
    let tip = w.tip;
    Duty { anchor, proof_block, tip, tx, path }
}

fn prove(w: &mut World, id: u64, d: &Duty) -> Result<(), String> {
    let op = w.operator.insecure_clone();
    prove_as(w, id, d, &op)
}

fn prove_as(w: &mut World, id: u64, d: &Duty, op: &Keypair) -> Result<(), String> {
    let to_proof: Vec<Ref> = d.path.iter().rev().skip_while(|r| r.height > d.proof_block.height).cloned().collect();
    let to_tip: Vec<Ref> = d.path.iter().rev().take_while(|r| r.height >= d.proof_block.height).cloned().collect();
    let walk_to_proof = w.walk(d.proof_block, d.anchor, &to_proof);
    let walk_to_tip = w.walk(d.tip, d.proof_block, &to_tip);
    let (siblings, index) = w.proof_of(&d.proof_block, &d.tx);
    let txid = sha256d(&d.tx);
    let ix = w.pr_ix(
        ipow_protocol::client::accounts::ProveJob {
            job: job_pda(id),
            application: pr(&[b"application", w.app.pubkey().as_ref()]),
            operator: operator_pda(&op.pubkey()),
            proof_node: node_pda(&d.proof_block),
            walk_to_proof,
            walk_to_tip,
            used_tx: pr(&[b"used_tx", &txid]),
            owner: op.pubkey(),
            system_program: SYSTEM,
        },
        ipow_protocol::client::args::ProveJob {
            proof: ipow_protocol::types::Proof {
                proof_block: d.proof_block.p(),
                tip: d.tip.p(),
                raw_tx: d.tx.clone(),
                txid,
                siblings,
                tx_index: index,
                head_index: 0,
                tag_index: 1,
            },
        },
    );
    w.send(ix, &[op])
}

fn settle(w: &mut World, id: u64) -> Result<(), String> {
    let job = w.job(id);
    let caller = w.stranger.insecure_clone();
    let ix = w.pr_ix(
        ipow_protocol::client::accounts::Settle {
            job: job_pda(id),
            operator: operator_pda(&job.operator),
            operator_credit: credit_pda(&job.operator),
            attester: None,
            attester_credit: None,
            funder: caller.pubkey(),
            system_program: SYSTEM,
        },
        ipow_protocol::client::args::Settle {},
    );
    w.send(ix, &[&caller])
}

fn seal(w: &mut World, who: &Keypair, note: [u8; 32]) {
    let ix = w.pr_ix(
        ipow_protocol::client::accounts::SealNote { record: pr(&[b"note", &note]), guardian: who.pubkey(), system_program: SYSTEM },
        ipow_protocol::client::args::SealNote { note },
    );
    w.send(ix, &[who]).unwrap();
}

fn report_missed(w: &mut World, who: &Keypair, id: u64, note: [u8; 32], salt: [u8; 32]) -> Result<(), String> {
    let job = w.job(id);
    let ix = w.pr_ix(
        ipow_protocol::client::accounts::ReportMissedDuty {
            job: job_pda(id),
            operator: operator_pda(&job.operator),
            note_record: pr(&[b"note", &note]),
            application_credit: credit_pda(&job.application),
            guardian_credit: credit_pda(&who.pubkey()),
            payer_credit: credit_pda(&job.payer),
            guardian: who.pubkey(),
            system_program: SYSTEM,
        },
        ipow_protocol::client::args::ReportMissedDuty { note, salt },
    );
    w.send(ix, &[who])
}

fn ready() -> World {
    let mut w = World::new();
    let op = w.operator.insecure_clone();
    w.lock_bond(&op, 3 * SOL);
    w.register_chain_head(&op).unwrap();
    w
}

const TAG: [u8; 32] = [7u8; 32];

// ---------------------------------------------------------------------
// Chain head (D30, D34)
// ---------------------------------------------------------------------

#[test]
fn registers_a_coin_made_by_a_transaction_that_names_the_operator() {
    let w = ready();
    let (txid, vout) = w.head(&w.operator.pubkey());
    assert_ne!(txid, [0u8; 32]);
    assert_eq!(vout, 0);
}

#[test]
fn rejects_a_registration_for_someone_else_and_a_second_one() {
    let mut w = ready();
    let op = w.operator.insecure_clone();
    expect_err(w.register_chain_head(&op), &code("ChainHeadExists"));

    // The stranger's commitment is not the operator's.
    let s = w.stranger.insecure_clone();
    w.lock_bond(&s, SOL);
    w.salt += 1;
    let tx = build_tx((sha256d(&w.salt.to_le_bytes()), 0), chain_head_commitment(&op.pubkey()));
    let block = w.add(&[tx.clone()], None);
    let (siblings, index) = w.proof_of(&block, &tx);
    let txid = sha256d(&tx);
    let ix = w.pr_ix(
        ipow_protocol::client::accounts::RegisterChainHead {
            operator: operator_pda(&s.pubkey()),
            node: node_pda(&block),
            used_tx: pr(&[b"used_tx", &txid]),
            owner: s.pubkey(),
            system_program: SYSTEM,
        },
        ipow_protocol::client::args::RegisterChainHead { txid, block: block.p(), raw_tx: tx, siblings, tx_index: index, coin_index: 0, tag_index: 1 },
    );
    expect_err(w.send(ix, &[&s]), &code("WrongTag"));
}

// ---------------------------------------------------------------------
// Anchor and proof (D6, D9, D15, D27, D39, D54)
// ---------------------------------------------------------------------

#[test]
fn proves_a_transaction_in_block_1_with_5_blocks_on_top() {
    let mut w = ready();
    let id = w.assigned_job(TAG, 0, SOL);
    let d = duty(&mut w, id, TAG, 1, 5, None);
    prove(&mut w, id, &d).unwrap();
    let job = w.job(id);
    assert_ne!(job.proven_at, 0);
    assert_eq!(job.lock_end, job.proven_at + 36 * HOUR);
    // The transaction's coin is the next chain head.
    assert_eq!(w.head(&w.operator.pubkey()), (sha256d(&d.tx), 0));
}

#[test]
fn rejects_4_blocks_on_top() {
    let mut w = ready();
    let id = w.assigned_job(TAG, 0, SOL);
    let d = duty(&mut w, id, TAG, 1, 4, None);
    expect_err(prove(&mut w, id, &d), &code("NotEnoughConfirmations"));
}

#[test]
fn accepts_block_25_and_rejects_block_26() {
    let mut w = ready();
    let first = w.assigned_job([1; 32], 0, SOL);
    let d = duty(&mut w, first, [1; 32], 25, 5, None);
    prove(&mut w, first, &d).unwrap();
    let second = w.assigned_job([2; 32], 0, SOL);
    let d = duty(&mut w, second, [2; 32], 26, 5, None);
    expect_err(prove(&mut w, second, &d), &code("OutsideProofRange"));
}

#[test]
fn rejects_another_tag_and_a_transaction_that_does_not_spend_the_chain_head() {
    let mut w = ready();
    let id = w.assigned_job(TAG, 0, SOL);
    let d = duty(&mut w, id, TAG, 1, 5, Some(tag_payload(&[9; 32])));
    expect_err(prove(&mut w, id, &d), &code("WrongTag"));
    // The application's tag as it is, not marked as a job's tag, is refused too.
    let id2 = w.assigned_job([3; 32], 0, SOL);
    let d = duty(&mut w, id2, [3; 32], 1, 5, Some([3; 32]));
    expect_err(prove(&mut w, id2, &d), &code("WrongTag"));
    // A transaction that spends someone else's coin (D30).
    let id3 = w.assigned_job([4; 32], 0, SOL);
    let d = duty_spending(&mut w, id3, [4; 32], 1, 5, None, Some(([0xab; 32], 0)));
    expect_err(prove(&mut w, id3, &d), &code("WrongChainHead"));
}

#[test]
fn rejects_a_proof_after_the_deadline_and_from_anyone_but_the_operator() {
    let mut w = ready();
    let id = w.assigned_job(TAG, 0, SOL);
    let d = duty(&mut w, id, TAG, 1, 5, None);
    // Someone else, with a bond and a chain head of its own, cannot submit it.
    let s = w.stranger.insecure_clone();
    w.lock_bond(&s, SOL);
    w.register_chain_head(&s).unwrap();
    expect_err(prove_as(&mut w, id, &d, &s), &code("NotOperator"));
    let lock_in = w.job(id).opened_at + 2 * MINUTE;
    w.set_now(lock_in + DAY + 1);
    expect_err(prove(&mut w, id, &d), &code("DeadlinePassed"));
}

#[test]
fn rejects_an_anchor_older_than_two_hours_or_named_by_someone_else() {
    let mut w = ready();
    let id = w.assigned_job(TAG, 0, SOL);
    let anchor = w.add(&[], None);
    let s = w.stranger.insecure_clone();
    expect_err(w.anchor_job(id, anchor, &s), &code("NotOperator"));
    let now = w.now();
    w.set_now(now + 2 * HOUR + 2);
    let op = w.operator.insecure_clone();
    expect_err(w.anchor_job(id, anchor, &op), &code("AnchorTooOld"));
}

#[test]
fn uses_a_transaction_for_one_job_only() {
    let mut w = ready();
    let id = w.assigned_job(TAG, 0, SOL);
    let d = duty(&mut w, id, TAG, 1, 5, None);
    // A second job with the same tag, from another application, won by the
    // same operator and anchored on the same block.
    let other = w.stranger.insecure_clone();
    let ix = w.pr_ix(
        ipow_protocol::client::accounts::RegisterApplication {
            application: pr(&[b"application", other.pubkey().as_ref()]),
            key: other.pubkey(),
            funder: other.pubkey(),
            system_program: SYSTEM,
        },
        ipow_protocol::client::args::RegisterApplication { challenge_periods: vec![] },
    );
    w.send(ix, &[&other]).unwrap();
    let saved = w.app.insecure_clone();
    w.app = other;
    let second = w.assigned_job(TAG, 0, SOL);
    w.app = saved;
    let op = w.operator.insecure_clone();
    w.anchor_job(second, d.anchor, &op).unwrap();

    prove(&mut w, id, &d).unwrap();
    // The same transaction cannot prove the second job: its record exists.
    let saved = w.app.insecure_clone();
    w.app = w.stranger.insecure_clone();
    let r = prove(&mut w, second, &d);
    w.app = saved;
    expect_err(r, "already in use");
}

// ---------------------------------------------------------------------
// The lock and the payment (D23, D50, D67)
// ---------------------------------------------------------------------

#[test]
fn pays_both_fees_and_frees_the_bond_after_36_hours() {
    let mut w = ready();
    let id = w.assigned_job(TAG, 0, 2 * SOL);
    let d = duty(&mut w, id, TAG, 1, 5, None);
    prove(&mut w, id, &d).unwrap();
    let lock_end = w.job(id).lock_end;
    w.set_now(lock_end - 1);
    expect_err(settle(&mut w, id), &code("LockNotEnded"));
    w.set_now(lock_end);
    settle(&mut w, id).unwrap();
    let op = w.operator.pubkey();
    assert_eq!(w.credit(&op), World::fee(6) + SOL / 200);
    assert_eq!(w.bond(&op), (3 * SOL, 0));
    expect_err(settle(&mut w, id), &code("NotProven"));
}

#[test]
fn locks_a_claim_for_its_whole_challenge_period() {
    let mut w = ready();
    let id = w.assigned_job(TAG, 1, SOL);
    let d = duty(&mut w, id, TAG, 1, 5, None);
    prove(&mut w, id, &d).unwrap();
    let job = w.job(id);
    assert_eq!(job.lock_end, job.proven_at + 7 * DAY);
}

// ---------------------------------------------------------------------
// A missed duty (D7, D26, D35, D36, D47, D53, D62)
// ---------------------------------------------------------------------

#[test]
fn slashes_the_full_escrow_of_a_missed_duty() {
    let mut w = ready();
    let id = w.assigned_job(TAG, 0, 2 * SOL);
    let g = w.guardian.insecure_clone();
    let salt = [5u8; 32];
    let note = note_for(&g.pubkey(), id, &[0u8; 32], &salt);
    seal(&mut w, &g, note);
    w.next_slot();

    let deadline = w.job(id).opened_at + 2 * MINUTE + DAY;
    w.set_now(deadline);
    expect_err(report_missed(&mut w, &g, id, note, salt), &code("DeadlineNotPassed"));
    w.set_now(deadline + 1);
    // Someone who saw the salt cannot use the guardian's note.
    let s = w.stranger.insecure_clone();
    expect_err(report_missed(&mut w, &s, id, note, salt), &code("NoNote"));
    report_missed(&mut w, &g, id, note, salt).unwrap();

    assert!(w.job(id).slashed);
    assert_eq!(w.credit(&w.app.pubkey()), SOL * 8 / 10);
    assert_eq!(w.credit(&g.pubkey()), SOL * 2 / 10);
    assert_eq!(w.credit(&w.user.pubkey()), World::fee(6) + SOL / 200);
    // x is slashed, not what was locked above it.
    assert_eq!(w.bond(&w.operator.pubkey()), (2 * SOL, 0));
}

#[test]
fn needs_the_note_sealed_in_an_earlier_slot() {
    let mut w = ready();
    let id = w.assigned_job(TAG, 0, SOL);
    let g = w.guardian.insecure_clone();
    let salt = [5u8; 32];
    let note = note_for(&g.pubkey(), id, &[0u8; 32], &salt);
    let deadline = w.job(id).opened_at + 2 * MINUTE + DAY;
    w.set_now(deadline + 1);
    seal(&mut w, &g, note);
    expect_err(report_missed(&mut w, &g, id, note, salt), &code("NoNote"));
    w.next_slot();
    report_missed(&mut w, &g, id, note, salt).unwrap();
}

#[test]
fn does_not_slash_a_job_whose_proof_was_accepted() {
    let mut w = ready();
    let id = w.assigned_job(TAG, 0, SOL);
    let g = w.guardian.insecure_clone();
    let salt = [5u8; 32];
    let note = note_for(&g.pubkey(), id, &[0u8; 32], &salt);
    seal(&mut w, &g, note);
    w.next_slot();
    let d = duty(&mut w, id, TAG, 1, 5, None);
    prove(&mut w, id, &d).unwrap();
    let deadline = w.job(id).opened_at + 2 * MINUTE + DAY;
    w.set_now(deadline + 1);
    expect_err(report_missed(&mut w, &g, id, note, salt), &code("NotAssigned"));
}

// ---------------------------------------------------------------------
// Challenge of a proof (D49, D81, D88, D90, D91, D98, D99)
// ---------------------------------------------------------------------

fn challenge_pda(id: u64) -> Pubkey {
    pr(&[b"challenge", &id.to_le_bytes()])
}

fn checkpoint_pda(cid: u64, side: u8, r: &Ref) -> Pubkey {
    pr(&[b"checkpoint", &cid.to_le_bytes(), &[side], &r.hash, &r.height.to_le_bytes(), &r.epoch_time.to_le_bytes()])
}

fn challenge_count(w: &World) -> u64 {
    let p: ipow_protocol::accounts::Protocol = w.ctx.get_account(&pr(&[b"protocol"])).unwrap();
    p.challenge_count
}

fn parent_evidence(child: &[u8; 32]) -> [u8; 32] {
    sha256(&[b"parent", child])
}

fn fork_evidence(block: &[u8; 32]) -> [u8; 32] {
    sha256(&[b"fork", block])
}

fn ask_parent(w: &mut World, who: &Keypair, id: u64, salt: [u8; 32], paid: u64) -> Result<u64, String> {
    let deepest = w.job(id).deepest;
    let note = note_for(&who.pubkey(), id, &parent_evidence(&deepest.hash), &salt);
    seal(w, who, note);
    w.next_slot();
    let cid = challenge_count(w) + 1;
    let ix = w.pr_ix(
        ipow_protocol::client::accounts::AskParent {
            protocol: pr(&[b"protocol"]),
            job: job_pda(id),
            challenge: challenge_pda(cid),
            note_record: pr(&[b"note", &note]),
            vault: pr(&[b"vault"]),
            guardian: who.pubkey(),
            system_program: SYSTEM,
        },
        ipow_protocol::client::args::AskParent { note, salt, paid },
    );
    w.send(ix, &[who])?;
    Ok(cid)
}

fn show_parent(w: &mut World, cid: u64) -> Result<(), String> {
    let c: ipow_protocol::accounts::Challenge = w.ctx.get_account(&challenge_pda(cid)).unwrap();
    let job = w.job(c.job_id);
    let asked = Ref { hash: c.asked.hash, height: c.asked.height, epoch_time: c.asked.epoch_time };
    let child: ipow_light_client::accounts::Node = w
        .ctx
        .get_account(&node_pda(&asked))
        .map_err(|e| format!("{e:?}"))?;
    let parent = Ref { hash: child.prev_hash, height: asked.height.saturating_sub(1), epoch_time: asked.epoch_time };
    let caller = w.stranger.insecure_clone();
    let ix = w.pr_ix(
        ipow_protocol::client::accounts::ShowParent {
            challenge: challenge_pda(cid),
            job: job_pda(c.job_id),
            child: node_pda(&asked),
            parent: node_pda(&parent),
            lc_config: lc(&[b"config"]),
            operator_credit: credit_pda(&job.operator),
            guardian: c.guardian,
            funder: caller.pubkey(),
            system_program: SYSTEM,
        },
        ipow_protocol::client::args::ShowParent { prev_epoch_time: 0 },
    );
    w.send(ix, &[&caller])
}

fn resolve(w: &mut World, cid: u64) -> Result<(), String> {
    let c: ipow_protocol::accounts::Challenge = w.ctx.get_account(&challenge_pda(cid)).unwrap();
    let job = w.job(c.job_id);
    let caller = w.stranger.insecure_clone();
    let ix = w.pr_ix(
        ipow_protocol::client::accounts::ResolveChallenge {
            challenge: challenge_pda(cid),
            job: job_pda(c.job_id),
            operator: operator_pda(&job.operator),
            attester: if job.has_attester && job.attester != job.operator { Some(operator_pda(&job.attester)) } else { None },
            operator_credit: credit_pda(&job.operator),
            application_credit: credit_pda(&job.application),
            guardian_credit: credit_pda(&c.guardian),
            payer_credit: credit_pda(&job.payer),
            guardian: c.guardian,
            funder: caller.pubkey(),
            system_program: SYSTEM,
        },
        ipow_protocol::client::args::ResolveChallenge {},
    );
    w.send(ix, &[&caller])
}

fn proven(w: &mut World, claim_kind: u16) -> (u64, Duty, Ref) {
    let id = w.assigned_job(TAG, claim_kind, SOL);
    let parent = w.tip;
    let d = duty(w, id, TAG, 1, 5, None);
    prove(w, id, &d).unwrap();
    (id, d, parent)
}

#[test]
fn answers_a_question_for_the_parent_and_pays_the_deposit_to_the_operator() {
    let mut w = ready();
    let (id, _, parent) = proven(&mut w, 0);
    let g = w.guardian.insecure_clone();
    let deposit = World::fee(6);
    expect_err(ask_parent(&mut w, &g, id, [1; 32], deposit - 1), &code("WrongDeposit"));
    let cid = ask_parent(&mut w, &g, id, [2; 32], deposit).unwrap();
    assert_eq!(w.job(id).open_challenges, 1);
    show_parent(&mut w, cid).unwrap();
    let job = w.job(id);
    assert_eq!(job.open_challenges, 0);
    assert_eq!(job.parents_shown, 1);
    assert_eq!(job.deepest.hash, parent.hash);
    assert_eq!(w.credit(&w.operator.pubkey()), deposit);

    // Question 2 costs 2 deposits (D91).
    expect_err(ask_parent(&mut w, &g, id, [3; 32], deposit), &code("WrongDeposit"));
    ask_parent(&mut w, &g, id, [4; 32], 2 * deposit).unwrap();
}

#[test]
fn makes_the_proof_false_when_nobody_shows_the_parent_in_12_hours() {
    let mut w = ready();
    let id = w.assigned_job(TAG, 0, SOL);
    // A forger's anchor: a block whose parent nobody has.
    w.salt += 1;
    let h = mine([0x42; 32], sha256d(&w.salt.to_le_bytes()), (w.now() + 1) as u32);
    let anchor = Ref { hash: sha256d(&h), height: 50, epoch_time: w.epoch_time };
    let payer = w.operator.insecure_clone();
    let ix = w.lc_ix(
        ipow_light_client::client::accounts::Jump {
            config: lc(&[b"config"]),
            day_table: lc(&[b"days"]),
            epoch_start: w.epoch_start,
            node: node_pda(&anchor),
            payer: payer.pubkey(),
            system_program: SYSTEM,
        },
        ipow_light_client::client::args::Jump { header: h, height: 50 },
    );
    w.send(ix, &[&payer]).unwrap();
    w.tip = anchor;
    let op = w.operator.insecure_clone();
    w.anchor_job(id, anchor, &op).unwrap();
    let head = w.head(&op.pubkey());
    let tx = build_tx(head, tag_payload(&TAG));
    let proof_block = w.add(&[tx.clone()], None);
    let mut path = vec![anchor, proof_block];
    for _ in 0..5 {
        path.push(w.add(&[], None));
    }
    let d = Duty { anchor, proof_block, tip: w.tip, tx, path };
    prove(&mut w, id, &d).unwrap();

    let g = w.guardian.insecure_clone();
    let deposit = World::fee(6);
    let asked_at = w.now();
    let cid = ask_parent(&mut w, &g, id, [1; 32], deposit).unwrap();
    expect_err(show_parent(&mut w, cid), &code("NoParent"));
    w.set_now(asked_at + 12 * HOUR - 1);
    expect_err(resolve(&mut w, cid), &code("ResponseTimeNotOver"));
    w.set_now(asked_at + 12 * HOUR);
    resolve(&mut w, cid).unwrap();
    assert!(w.job(id).slashed);
    assert_eq!(w.credit(&w.app.pubkey()), SOL * 8 / 10);
    assert_eq!(w.credit(&g.pubkey()), SOL * 2 / 10 + deposit);
}

#[test]
fn opens_no_challenge_in_the_last_12_hours_and_pays_nobody_while_one_is_open() {
    let mut w = ready();
    let (id, _, _) = proven(&mut w, 0);
    let lock_end = w.job(id).lock_end;
    let g = w.guardian.insecure_clone();
    let deposit = World::fee(6);
    w.set_now(lock_end - 12 * HOUR + 1);
    expect_err(ask_parent(&mut w, &g, id, [1; 32], deposit), &code("ChallengeWindowClosed"));
    w.set_now(lock_end - 12 * HOUR);
    let cid = ask_parent(&mut w, &g, id, [2; 32], deposit).unwrap();
    w.set_now(lock_end);
    expect_err(settle(&mut w, id), &code("ChallengeOpen"));
    // Nobody answered in time: the proof is false, although the chain was real.
    resolve(&mut w, cid).unwrap();
    assert!(w.job(id).slashed);
}

fn fork(w: &mut World, id: u64, d: &Duty, parent: Ref, g: &[Ref], operator_block: Ref) -> Result<u64, String> {
    let guardian = w.guardian.insecure_clone();
    let job = w.job(id);
    let tip = Ref { hash: job.tip.hash, height: job.tip.height, epoch_time: job.tip.epoch_time };
    let g_block = g[0];
    let g_tip = *g.last().unwrap();
    let note = note_for(&guardian.pubkey(), id, &fork_evidence(&g_block.hash), &[9; 32]);
    seal(w, &guardian, note);
    w.next_slot();
    let to_op: Vec<Ref> = d.path.iter().rev().filter(|r| r.height <= d.proof_block.height && r.height >= operator_block.height).cloned().collect();
    let to_tip: Vec<Ref> = d.path.iter().rev().filter(|r| r.height >= d.proof_block.height).cloned().collect();
    let g_path: Vec<Ref> = g.iter().rev().cloned().collect();
    let w1 = w.walk(d.proof_block, operator_block, &to_op);
    let w2 = w.walk(tip, d.proof_block, &to_tip);
    let w3 = w.walk(g_tip, g_block, &g_path);
    let cid = challenge_count(w) + 1;
    let _ = parent;
    let ix = w.pr_ix(
        ipow_protocol::client::accounts::ChallengeFork {
            protocol: pr(&[b"protocol"]),
            job: job_pda(id),
            challenge: challenge_pda(cid),
            note_record: pr(&[b"note", &note]),
            operator_node: node_pda(&operator_block),
            guardian_node: node_pda(&g_block),
            walk_proof_to_operator: w1,
            walk_tip_to_proof: w2,
            walk_guardian: w3,
            operator_checkpoint: checkpoint_pda(cid, 0, &tip),
            guardian_checkpoint: checkpoint_pda(cid, 1, &g_tip),
            proof_checkpoint: checkpoint_pda(cid, 0, &d.proof_block),
            guardian_first_checkpoint: checkpoint_pda(cid, 1, &g_block),
            vault: pr(&[b"vault"]),
            guardian: guardian.pubkey(),
            system_program: SYSTEM,
        },
        ipow_protocol::client::args::ChallengeFork {
            note,
            salt: [9; 32],
            paid: World::fee(6),
            args: ipow_protocol::types::ForkArgs {
                operator_block: operator_block.p(),
                guardian_block: g_block.p(),
                guardian_tip: g_tip.p(),
            },
        },
    );
    w.send(ix, &[&guardian])?;
    Ok(cid)
}

fn extend_branch(w: &mut World, cid: u64, guardian_side: bool, from: Ref, blocks: &[Ref]) -> Result<(), String> {
    let new_tip = *blocks.last().unwrap();
    let mut path: Vec<Ref> = blocks.iter().rev().cloned().collect();
    path.push(from);
    let walk = w.walk(new_tip, from, &path);
    let caller = w.stranger.insecure_clone();
    let side = guardian_side as u8;
    let c: ipow_protocol::accounts::Challenge = w.ctx.get_account(&challenge_pda(cid)).unwrap();
    let ix = w.pr_ix(
        ipow_protocol::client::accounts::ExtendBranch {
            challenge: challenge_pda(cid),
            job: job_pda(c.job_id),
            from_checkpoint: checkpoint_pda(cid, side, &from),
            new_checkpoint: checkpoint_pda(cid, side, &new_tip),
            walk,
            funder: caller.pubkey(),
            system_program: SYSTEM,
        },
        ipow_protocol::client::args::ExtendBranch { guardian_side, from: from.p(), new_tip: new_tip.p() },
    );
    w.send(ix, &[&caller])
}

fn branch(w: &mut World, from: Ref, count: usize) -> Vec<Ref> {
    let mut on = from;
    let mut out = vec![];
    for _ in 0..count {
        on = w.add(&[], Some(on));
        out.push(on);
    }
    out
}

#[test]
fn fails_a_competing_branch_against_real_blocks_that_keep_the_lead() {
    let mut w = ready();
    let (id, d, parent) = proven(&mut w, 0);
    // The operator's branch from the anchor: anchor, proof block, 5 on top = 7.
    let g = branch(&mut w, parent, 8);
    let cid = fork(&mut w, id, &d, parent, &g, d.anchor).unwrap();
    // Someone shows a dead end on the operator's tip; the real chain grows
    // from the tip anyway.
    let dead = branch(&mut w, d.tip, 1);
    extend_branch(&mut w, cid, false, d.tip, &dead).unwrap();
    let real = branch(&mut w, d.tip, 2);
    extend_branch(&mut w, cid, false, d.tip, &real).unwrap();
    // A block never shown on a side cannot be built on.
    expect_err(extend_branch(&mut w, cid, false, g[3], &g[4..]), "AccountNotInitialized");

    let lock_end = w.job(id).lock_end;
    w.set_now(lock_end - 1);
    expect_err(resolve(&mut w, cid), &code("LockNotEnded"));
    w.set_now(lock_end);
    resolve(&mut w, cid).unwrap();
    assert_eq!(w.credit(&w.operator.pubkey()), World::fee(6));
    settle(&mut w, id).unwrap();
}

#[test]
fn makes_the_proof_false_when_the_guardians_branch_has_more_work_at_the_end() {
    let mut w = ready();
    let (id, d, parent) = proven(&mut w, 0);
    let g = branch(&mut w, parent, 8);
    let cid = fork(&mut w, id, &d, parent, &g, d.anchor).unwrap();
    w.set_now(w.job(id).lock_end);
    resolve(&mut w, cid).unwrap();
    assert!(w.job(id).slashed);
    assert_eq!(w.credit(&w.guardian.pubkey()), SOL * 2 / 10 + World::fee(6));
}

#[test]
fn rejects_a_branch_without_more_work_or_that_does_not_share_the_parent() {
    let mut w = ready();
    let (id, d, parent) = proven(&mut w, 0);
    let short = branch(&mut w, parent, 7);
    expect_err(fork(&mut w, id, &d, parent, &short, d.anchor), &code("NotHeavier"));
    // A branch from one block lower names another parent.
    let grand: ipow_light_client::accounts::Node = w.ctx.get_account(&node_pda(&parent)).unwrap();
    let low_from = Ref { hash: grand.prev_hash, height: parent.height - 1, epoch_time: parent.epoch_time };
    let low = branch(&mut w, low_from, 10);
    expect_err(fork(&mut w, id, &d, parent, &low, d.anchor), &code("NotCompeting"));
}

// ---------------------------------------------------------------------
// Attest and official (D11, D42, D73, D95, D96, D97)
// ---------------------------------------------------------------------

fn attest(w: &mut World, who: &Keypair, id: u64) -> Result<(), String> {
    let job = w.job(id);
    let ix = w.pr_ix(
        ipow_protocol::client::accounts::Attest {
            job: job_pda(id),
            attester: operator_pda(&who.pubkey()),
            operator: if who.pubkey() == job.operator { None } else { Some(operator_pda(&job.operator)) },
            owner: who.pubkey(),
        },
        ipow_protocol::client::args::Attest {},
    );
    w.send(ix, &[who])
}

fn official(w: &mut World, id: u64) -> bool {
    let job = w.job(id);
    let ix = w.pr_ix(
        ipow_protocol::client::accounts::RequireOfficial {
            job: job_pda(id),
            application: pr(&[b"application", job.application.as_ref()]),
        },
        ipow_protocol::client::args::RequireOfficial {},
    );
    let caller = w.stranger.insecure_clone();
    w.send(ix, &[&caller]).is_ok()
}

fn settle_attested(w: &mut World, id: u64) -> Result<(), String> {
    let job = w.job(id);
    let own = job.attester == job.operator;
    let caller = w.stranger.insecure_clone();
    let ix = w.pr_ix(
        ipow_protocol::client::accounts::Settle {
            job: job_pda(id),
            operator: operator_pda(&job.operator),
            operator_credit: credit_pda(&job.operator),
            attester: if own { None } else { Some(operator_pda(&job.attester)) },
            attester_credit: if own { None } else { Some(credit_pda(&job.attester)) },
            funder: caller.pubkey(),
            system_program: SYSTEM,
        },
        ipow_protocol::client::args::Settle {},
    );
    w.send(ix, &[&caller])
}

#[test]
fn lets_an_attester_take_the_operators_place_and_pays_it_for_the_time_it_covered() {
    let mut w = ready();
    let (id, _, _) = proven(&mut w, 1);
    let s = w.stranger.insecure_clone();
    w.lock_bond(&s, 2 * SOL);
    assert!(!official(&mut w, id));

    let job = w.job(id);
    let half = job.proven_at + (job.lock_end - job.proven_at) / 2;
    w.set_now(half);
    attest(&mut w, &s, id).unwrap();
    assert!(official(&mut w, id));
    assert_eq!(w.bond(&s.pubkey()), (2 * SOL, SOL));
    assert_eq!(w.bond(&w.operator.pubkey()), (3 * SOL, 0));
    expect_err(attest(&mut w, &s, id), &code("AlreadyAttested"));

    w.set_now(job.lock_end);
    settle_attested(&mut w, id).unwrap();
    let share = (SOL / 200) * 4000 / 10_000 / 2;
    assert_eq!(w.credit(&s.pubkey()), share);
    assert_eq!(w.credit(&w.operator.pubkey()), World::fee(6) + SOL / 200 - share);
    assert_eq!(w.bond(&s.pubkey()), (2 * SOL, 0));
}

#[test]
fn makes_a_claim_official_only_when_its_lock_has_ended_with_no_challenge_open() {
    let mut w = ready();
    let (id, _, _) = proven(&mut w, 1);
    let lock_end = w.job(id).lock_end;
    w.set_now(lock_end - 1);
    assert!(!official(&mut w, id));
    w.set_now(lock_end);
    assert!(official(&mut w, id));
}

#[test]
fn makes_a_settlement_official_at_its_proof() {
    let mut w = ready();
    let (id, _, _) = proven(&mut w, 0);
    assert!(official(&mut w, id));
}

#[test]
fn slashes_the_attester_not_the_operator_when_the_proof_is_proven_fake() {
    let mut w = ready();
    let (id, d, parent) = proven(&mut w, 0);
    let s = w.stranger.insecure_clone();
    w.lock_bond(&s, 2 * SOL);
    attest(&mut w, &s, id).unwrap();
    let g = branch(&mut w, parent, 8);
    let cid = fork(&mut w, id, &d, parent, &g, d.anchor).unwrap();
    w.set_now(w.job(id).lock_end);
    resolve(&mut w, cid).unwrap();
    assert_eq!(w.bond(&s.pubkey()), (SOL, 0));
    assert_eq!(w.bond(&w.operator.pubkey()), (3 * SOL, 0));
    assert!(!official(&mut w, id));
}

// ---------------------------------------------------------------------
// Found by the review of the Solana program
// ---------------------------------------------------------------------

#[test]
fn pays_an_operator_that_attested_its_own_job() {
    let mut w = ready();
    let (id, _, _) = proven(&mut w, 1);
    let op = w.operator.insecure_clone();
    attest(&mut w, &op, id).unwrap();
    assert!(official(&mut w, id));
    // Its bid is freed and x is locked in its place.
    assert_eq!(w.bond(&op.pubkey()), (3 * SOL, SOL));
    w.set_now(w.job(id).lock_end);
    settle_attested(&mut w, id).unwrap();
    // It receives both fees: its own share and the attester's.
    assert_eq!(w.credit(&op.pubkey()), World::fee(6) + SOL / 200);
    assert_eq!(w.bond(&op.pubkey()), (3 * SOL, 0));
}

#[test]
fn slashes_an_operator_that_attested_its_own_fake_proof() {
    let mut w = ready();
    let (id, d, parent) = proven(&mut w, 1);
    let op = w.operator.insecure_clone();
    attest(&mut w, &op, id).unwrap();
    let g = branch(&mut w, parent, 8);
    let cid = fork(&mut w, id, &d, parent, &g, d.anchor).unwrap();
    w.set_now(w.job(id).lock_end);
    resolve(&mut w, cid).unwrap();
    assert!(w.job(id).slashed);
    assert!(!official(&mut w, id));
    assert_eq!(w.bond(&op.pubkey()), (2 * SOL, 0));
    assert_eq!(w.credit(&w.app.pubkey()), SOL * 8 / 10);
}

#[test]
fn needs_x_free_while_the_bid_is_still_locked_when_the_operator_attests() {
    let mut w = World::new();
    let op = w.operator.insecure_clone();
    w.lock_bond(&op, SOL);
    w.register_chain_head(&op).unwrap();
    let id = w.assigned_job(TAG, 1, SOL);
    let d = duty(&mut w, id, TAG, 1, 5, None);
    prove(&mut w, id, &d).unwrap();
    // All of its bond is locked for the bid, so x is not free.
    expect_err(attest(&mut w, &op, id), &code("BondNotFree"));
}

#[test]
fn grows_the_operators_side_from_its_proofs_block_when_its_tip_was_abandoned() {
    let mut w = ready();
    let (id, d, parent) = proven(&mut w, 0);
    let g = branch(&mut w, parent, 8);
    let cid = fork(&mut w, id, &d, parent, &g, d.anchor).unwrap();
    // Bitcoin abandoned the blocks above the proof's block; the real chain
    // continues from the proof's block itself.
    let real = branch(&mut w, d.proof_block, 7);
    extend_branch(&mut w, cid, false, d.proof_block, &real).unwrap();
    let c: ipow_protocol::accounts::Challenge = w.ctx.get_account(&challenge_pda(cid)).unwrap();
    assert!(c.operator_work > c.guardian_work);
    // The guardian's side can grow too, from its first block.
    let more = branch(&mut w, g[0], 9);
    extend_branch(&mut w, cid, true, g[0], &more).unwrap();
}

#[test]
fn gives_the_deposit_back_to_a_second_guardian_when_the_first_one_slashed_the_job() {
    let mut w = ready();
    let (id, d, parent) = proven(&mut w, 0);
    let g = branch(&mut w, parent, 8);
    let first = fork(&mut w, id, &d, parent, &g, d.anchor).unwrap();
    // A second guardian asks a question at the same time.
    let s = w.stranger.insecure_clone();
    let second = ask_parent(&mut w, &s, id, [8; 32], World::fee(6)).unwrap();
    w.set_now(w.job(id).lock_end);
    resolve(&mut w, first).unwrap();
    assert!(w.job(id).slashed);
    resolve(&mut w, second).unwrap();
    assert_eq!(w.credit(&s.pubkey()), World::fee(6));
    assert_eq!(w.job(id).open_challenges, 0);
}
