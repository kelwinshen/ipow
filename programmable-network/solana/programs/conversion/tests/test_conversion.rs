//! Tests of Conversion on Solana: the same cases as
//! `test/Conversion.test.ts` on Ethereum. Design:
//! docs/drafts/ipow-conversion-app.md. The Bitcoin blocks are mined by the
//! test at an easy difficulty, which only the test build of the light
//! client accepts.

use anchor_lang;
use anchor_lang::solana_program::instruction::{AccountMeta, Instruction};
use anchor_litesvm::{AnchorContext, AnchorLiteSVM, TestHelpers};
use sha2::{Digest, Sha256};
use solana_keypair::Keypair;
use solana_program::pubkey::Pubkey;
use solana_signer::Signer;

anchor_lang::declare_program!(conversion);
anchor_lang::declare_program!(ipow_protocol);
anchor_lang::declare_program!(ipow_light_client);

const SOL: u64 = 1_000_000_000;
const HOUR: i64 = 3600;
const T0: i64 = 1_800_000_000;
const EASY: u32 = 0x207f_ffff;
const SYSTEM: Pubkey = anchor_lang::solana_program::system_program::ID;
const FEES: u64 = SOL / 10;
const USER_SCRIPT: [u8; 22] = {
    let mut s = [0x22u8; 22];
    s[0] = 0x00;
    s[1] = 0x14;
    s
};
const OPERATOR_SCRIPT: [u8; 22] = {
    let mut s = [0x33u8; 22];
    s[0] = 0x00;
    s[1] = 0x14;
    s
};
const COIN_SCRIPT: [u8; 22] = {
    let mut s = [0x11u8; 22];
    s[0] = 0x00;
    s[1] = 0x14;
    s
};

// ---------------------------------------------------------------------
// Bitcoin
// ---------------------------------------------------------------------

fn sha256d(data: &[u8]) -> [u8; 32] {
    Sha256::digest(Sha256::digest(data)).into()
}

fn sha256(parts: &[&[u8]]) -> [u8; 32] {
    let mut h = Sha256::new();
    for p in parts {
        h.update(p);
    }
    h.finalize().into()
}

fn mine(prev: [u8; 32], merkle_root: [u8; 32], time: u32) -> [u8; 80] {
    let mut h = [0u8; 80];
    h[0..4].copy_from_slice(&0x2000_0000u32.to_le_bytes());
    h[4..36].copy_from_slice(&prev);
    h[36..68].copy_from_slice(&merkle_root);
    h[68..72].copy_from_slice(&time.to_le_bytes());
    h[72..76].copy_from_slice(&EASY.to_le_bytes());
    for nonce in 0u32.. {
        h[76..80].copy_from_slice(&nonce.to_le_bytes());
        if sha256d(&h)[31] < 0x80 {
            return h;
        }
    }
    unreachable!()
}

/// A transaction without witness data.
fn tx(inputs: &[([u8; 32], u32)], outputs: &[(u64, &[u8])]) -> Vec<u8> {
    let mut t = vec![];
    t.extend_from_slice(&2u32.to_le_bytes());
    t.push(inputs.len() as u8);
    for (txid, vout) in inputs {
        t.extend_from_slice(txid);
        t.extend_from_slice(&vout.to_le_bytes());
        t.push(0);
        t.extend_from_slice(&[0xff; 4]);
    }
    t.push(outputs.len() as u8);
    for (value, script) in outputs {
        t.extend_from_slice(&value.to_le_bytes());
        t.push(script.len() as u8);
        t.extend_from_slice(script);
    }
    t.extend_from_slice(&[0u8; 4]);
    t
}

fn op_return(payload: &[u8; 32]) -> Vec<u8> {
    [&[0x6au8, 0x20][..], payload].concat()
}

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

// ---------------------------------------------------------------------
// Addresses
// ---------------------------------------------------------------------

fn lc(seeds: &[&[u8]]) -> Pubkey {
    Pubkey::find_program_address(seeds, &ipow_light_client::ID).0
}
fn pr(seeds: &[&[u8]]) -> Pubkey {
    Pubkey::find_program_address(seeds, &ipow_protocol::ID).0
}
fn cv(seeds: &[&[u8]]) -> Pubkey {
    Pubkey::find_program_address(seeds, &conversion::ID).0
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
    fn c(&self) -> conversion::types::BlockRef {
        conversion::types::BlockRef { hash: self.hash, height: self.height, epoch_time: self.epoch_time }
    }
    fn l(&self) -> ipow_light_client::types::BlockRef {
        ipow_light_client::types::BlockRef { hash: self.hash, height: self.height, epoch_time: self.epoch_time }
    }
}

fn node_pda(r: &Ref) -> Pubkey {
    lc(&[b"node", &r.hash, &r.height.to_le_bytes(), &r.epoch_time.to_le_bytes()])
}
fn job_pda(id: u64) -> Pubkey {
    pr(&[b"job", &id.to_le_bytes()])
}
fn operator_pda(o: &Pubkey) -> Pubkey {
    pr(&[b"operator", o.as_ref()])
}
fn config() -> Pubkey {
    cv(&[b"config"])
}
fn swap_pda(id: u64) -> Pubkey {
    cv(&[b"swap", &id.to_le_bytes()])
}
fn tag_of(swap_id: u64) -> [u8; 32] {
    sha256(&[b"iPoW conversion", conversion::ID.as_ref(), &swap_id.to_le_bytes()])
}

fn compute_budget() -> Instruction {
    let mut data = vec![2u8];
    data.extend_from_slice(&1_400_000u32.to_le_bytes());
    Instruction { program_id: "ComputeBudget111111111111111111111111111111".parse().unwrap(), accounts: vec![], data }
}

fn expect_err<T: std::fmt::Debug>(r: Result<T, String>, name: &str) {
    match r {
        Ok(v) => panic!("expected {name}, got {v:?}"),
        Err(logs) => assert!(logs.contains(&format!("Error Code: {name}")), "expected {name}, got:\n{logs}"),
    }
}

// ---------------------------------------------------------------------
// The test world
// ---------------------------------------------------------------------

struct World {
    ctx: AnchorContext,
    user: Keypair,
    operator: Keypair,
    guardian: Keypair,
    stranger: Keypair,
    tip: Ref,
    blocks: std::collections::HashMap<[u8; 32], Vec<[u8; 32]>>,
    /// Each block's parent, so a walk can follow any stretch of the chain.
    parents: std::collections::HashMap<[u8; 32], Ref>,
    salt: u32,
    walks: u64,
}

impl World {
    fn new() -> Self {
        let ctx = AnchorLiteSVM::build_with_programs(&[
            (ipow_light_client::ID, include_bytes!("../../../target/deploy-test/ipow_light_client.so")),
            (ipow_protocol::ID, include_bytes!("../../../target/deploy/ipow_protocol.so")),
            (conversion::ID, include_bytes!("../../../target/deploy/conversion.so")),
        ]);
        let mut w = World {
            ctx,
            user: Keypair::new(),
            operator: Keypair::new(),
            guardian: Keypair::new(),
            stranger: Keypair::new(),
            tip: Ref { hash: [0; 32], height: 0, epoch_time: 0 },
            blocks: Default::default(),
            parents: Default::default(),
            salt: 0,
            walks: 0,
        };
        w.user = w.ctx.svm.create_funded_account(1_000 * SOL).unwrap();
        w.operator = w.ctx.svm.create_funded_account(1_000 * SOL).unwrap();
        w.guardian = w.ctx.svm.create_funded_account(1_000 * SOL).unwrap();
        w.stranger = w.ctx.svm.create_funded_account(1_000 * SOL).unwrap();
        w.set_now(T0);

        let mut easy = [0u8; 32];
        easy[..3].copy_from_slice(&[0x7f, 0xff, 0xff]);
        let payer = w.stranger.insecure_clone();
        w.send(
            w.ix(
                ipow_light_client::ID,
                ipow_light_client::client::accounts::Initialize {
                    config: lc(&[b"config"]),
                    day_table: lc(&[b"days"]),
                    authority: payer.pubkey(),
                    program_data: SYSTEM,
                    system_program: SYSTEM,
                },
                ipow_light_client::client::args::Initialize { min_height: 0, max_target: easy, pow_limit: easy },
            ),
            &[&payer],
        )
        .unwrap();
        w.send(
            w.ix(
                ipow_protocol::ID,
                ipow_protocol::client::accounts::InitializeProtocol {
                    protocol: pr(&[b"protocol"]),
                    vault: pr(&[b"vault"]),
                    payer: payer.pubkey(),
                    system_program: SYSTEM,
                },
                ipow_protocol::client::args::InitializeProtocol {},
            ),
            &[&payer],
        )
        .unwrap();
        w.send(
            w.ix(
                conversion::ID,
                conversion::client::accounts::Initialize {
                    config: config(),
                    application: pr(&[b"application", config().as_ref()]),
                    payer: payer.pubkey(),
                    protocol_program: ipow_protocol::ID,
                    system_program: SYSTEM,
                },
                conversion::client::args::Initialize {},
            ),
            &[&payer],
        )
        .unwrap();
        w.start_chain();

        // The operator's bond and first chain head.
        let op = w.operator.insecure_clone();
        w.send(
            w.ix(
                ipow_protocol::ID,
                ipow_protocol::client::accounts::LockBond {
                    protocol: pr(&[b"protocol"]),
                    operator: operator_pda(&op.pubkey()),
                    vault: pr(&[b"vault"]),
                    owner: op.pubkey(),
                    system_program: SYSTEM,
                },
                ipow_protocol::client::args::LockBond { amount: 3 * SOL },
            ),
            &[&op],
        )
        .unwrap();
        let commitment = sha256(&[b"iPoW chain head", ipow_protocol::ID.as_ref(), op.pubkey().as_ref()]);
        let first = tx(&[([9; 32], 0)], &[(546, &COIN_SCRIPT), (0, &op_return(&commitment))]);
        let block = w.add(&[first.clone()], None);
        let (siblings, index) = w.proof_of(&block, &first);
        let txid = sha256d(&first);
        w.send(
            w.ix(
                ipow_protocol::ID,
                ipow_protocol::client::accounts::RegisterChainHead {
                    operator: operator_pda(&op.pubkey()),
                    node: node_pda(&block),
                    used_tx: pr(&[b"used_tx", &txid]),
                    owner: op.pubkey(),
                    system_program: SYSTEM,
                },
                ipow_protocol::client::args::RegisterChainHead {
                    txid,
                    block: block.p(),
                    raw_tx: first,
                    siblings,
                    tx_index: index,
                    coin_index: 0,
                    tag_index: 1,
                },
            ),
            &[&op],
        )
        .unwrap();
        w
    }

    fn ix<A: anchor_lang::ToAccountMetas, D: anchor_lang::InstructionData>(&self, program: Pubkey, a: A, d: D) -> Instruction {
        Instruction { program_id: program, accounts: a.to_account_metas(None), data: d.data() }
    }

    fn send(&mut self, ix: Instruction, signers: &[&Keypair]) -> Result<(), String> {
        self.ctx.svm.expire_blockhash();
        let r = self.ctx.execute_instructions(vec![compute_budget(), ix], signers).unwrap();
        self.next_slot();
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

    fn next_slot(&mut self) {
        let mut clock: solana_clock::Clock = self.ctx.svm.get_sysvar();
        clock.slot += 1;
        self.ctx.svm.set_sysvar(&clock);
    }

    fn start_chain(&mut self) {
        let epoch_time = (self.now() - HOUR) as u32;
        let mut headers = vec![];
        let mut prev = [0u8; 32];
        for i in 0..6 {
            self.salt += 1;
            let h = mine(prev, sha256d(&self.salt.to_le_bytes()), epoch_time + i * 600);
            prev = sha256d(&h);
            headers.push(h);
        }
        let nodes: Vec<Pubkey> =
            headers.iter().enumerate().map(|(i, h)| node_pda(&Ref { hash: sha256d(h), height: i as u32, epoch_time })).collect();
        let payer = self.stranger.insecure_clone();
        let mut ix = self.ix(
            ipow_light_client::ID,
            ipow_light_client::client::accounts::AddEpochStart {
                config: lc(&[b"config"]),
                day_table: lc(&[b"days"]),
                epoch_start: lc(&[b"epoch_start", &sha256d(&headers[0]), &0u32.to_le_bytes(), &epoch_time.to_le_bytes()]),
                payer: payer.pubkey(),
                system_program: SYSTEM,
            },
            ipow_light_client::client::args::AddEpochStart { headers: headers.concat(), height: 0 },
        );
        for n in nodes {
            ix.accounts.push(AccountMeta::new(n, false));
        }
        self.send(ix, &[&payer]).unwrap();
        self.tip = Ref { hash: prev, height: 5, epoch_time };
    }

    /// Mines a block on `parent` (the tip unless stated) holding `txs`, and
    /// streams it.
    fn add(&mut self, txs: &[Vec<u8>], parent: Option<Ref>) -> Ref {
        let on = parent.unwrap_or(self.tip);
        self.salt += 1;
        let coinbase = sha256d(&[b"coinbase".as_ref(), &self.salt.to_le_bytes()].concat());
        let mut txids = vec![coinbase];
        txids.extend(txs.iter().map(|t| sha256d(t)));
        let h = mine(on.hash, merkle(&txids, 0).0, (self.now() + 1) as u32);
        let r = Ref { hash: sha256d(&h), height: on.height + 1, epoch_time: on.epoch_time };
        let payer = self.stranger.insecure_clone();
        let mut ix = self.ix(
            ipow_light_client::ID,
            ipow_light_client::client::accounts::Extend { config: lc(&[b"config"]), parent: node_pda(&on), payer: payer.pubkey(), system_program: SYSTEM },
            ipow_light_client::client::args::Extend { headers: h.to_vec() },
        );
        ix.accounts.push(AccountMeta::new(node_pda(&r), false));
        self.send(ix, &[&payer]).unwrap();
        self.blocks.insert(r.hash, txids);
        self.parents.insert(r.hash, on);
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

    /// A finished walk from `desc` down to `anc` along `path` (desc first).
    fn walk(&mut self, desc: Ref, anc: Ref, path: &[Ref]) -> Pubkey {
        self.walks += 1;
        let payer = self.stranger.insecure_clone();
        let id = self.walks;
        let walk = lc(&[b"walk", payer.pubkey().as_ref(), &id.to_le_bytes()]);
        let ix = self.ix(
            ipow_light_client::ID,
            ipow_light_client::client::accounts::BeginWalk { walk, payer: payer.pubkey(), system_program: SYSTEM },
            ipow_light_client::client::args::BeginWalk { walk_id: id, descendant: desc.l(), ancestor: anc.l(), prev_epoch_time: 0 },
        );
        self.send(ix, &[&payer]).unwrap();
        for chunk in path.chunks(20) {
            let mut ix = self.ix(
                ipow_light_client::ID,
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

    fn job(&self, id: u64) -> ipow_protocol::accounts::Job {
        self.ctx.get_account(&job_pda(id)).unwrap()
    }

    fn swap(&self, id: u64) -> conversion::accounts::Swap {
        self.ctx.get_account(&swap_pda(id)).unwrap()
    }

    fn balance(&self, who: &Pubkey) -> u64 {
        self.ctx.svm.get_balance(who).unwrap_or(0)
    }

    fn job_count(&self) -> u64 {
        let p: ipow_protocol::accounts::Protocol = self.ctx.get_account(&pr(&[b"protocol"])).unwrap();
        p.job_count
    }

    fn swap_count(&self) -> u64 {
        let c: conversion::accounts::Config = self.ctx.get_account(&config()).unwrap();
        c.swap_count
    }

    fn open_accounts(&self, swap_id: u64) -> (Pubkey, Pubkey, Pubkey) {
        let job = job_pda(self.job_count() + 1);
        let tag_record = pr(&[b"tag", config().as_ref(), &tag_of(swap_id)]);
        (swap_pda(swap_id), job, tag_record)
    }

    fn sell(&mut self, amount: u64, sats: u64) -> u64 {
        let id = self.swap_count() + 1;
        let (swap, job, tag_record) = self.open_accounts(id);
        let user = self.user.insecure_clone();
        let ix = self.ix(
            conversion::ID,
            conversion::client::accounts::Sell {
                config: config(),
                swap,
                user: user.pubkey(),
                protocol: pr(&[b"protocol"]),
                application: pr(&[b"application", config().as_ref()]),
                job,
                tag_record,
                protocol_vault: pr(&[b"vault"]),
                protocol_program: ipow_protocol::ID,
                system_program: SYSTEM,
                mint: None,
                escrow: None,
                from: None,
                token_program: None,
                associated_token_program: None,
            },
            conversion::client::args::Sell { amount, sats, script: USER_SCRIPT.to_vec(), confirmations: 6, paid: FEES },
        );
        self.send(ix, &[&user]).unwrap();
        id
    }

    fn buy(&mut self, amount: u64, sats: u64) -> Result<u64, String> {
        let id = self.swap_count() + 1;
        let (swap, job, tag_record) = self.open_accounts(id);
        let user = self.user.insecure_clone();
        let ix = self.ix(
            conversion::ID,
            conversion::client::accounts::Buy {
                config: config(),
                swap,
                user: user.pubkey(),
                protocol: pr(&[b"protocol"]),
                application: pr(&[b"application", config().as_ref()]),
                job,
                tag_record,
                protocol_vault: pr(&[b"vault"]),
                protocol_program: ipow_protocol::ID,
                system_program: SYSTEM,
                mint: None,
            },
            conversion::client::args::Buy { amount, sats, confirmations: 6, paid: FEES },
        );
        self.send(ix, &[&user])?;
        Ok(id)
    }

    /// The operator wins the swap's job and is locked in.
    fn win(&mut self, swap_id: u64) -> u64 {
        let job_id = self.swap(swap_id).job_id;
        let job = self.job(job_id);
        let op = self.operator.insecure_clone();
        let ix = self.ix(
            ipow_protocol::ID,
            ipow_protocol::client::accounts::Bid { job: job_pda(job_id), operator: operator_pda(&op.pubkey()), previous: None, owner: op.pubkey() },
            ipow_protocol::client::args::Bid { amount: job.escrow },
        );
        self.send(ix, &[&op]).unwrap();
        let t = self.now();
        self.set_now(t + 61);
        job_id
    }

    fn anchor(&mut self, job_id: u64, anchor: Ref) -> Result<(), String> {
        let op = self.operator.insecure_clone();
        let ix = self.ix(
            ipow_protocol::ID,
            ipow_protocol::client::accounts::AnchorJob { job: job_pda(job_id), node: node_pda(&anchor), owner: op.pubkey() },
            ipow_protocol::client::args::AnchorJob { anchor: anchor.p() },
        );
        self.send(ix, &[&op])
    }

    fn fund(&mut self, swap_id: u64, script: &[u8], who: &Keypair) -> Result<(), String> {
        let job = self.job(self.swap(swap_id).job_id);
        let anchor = Ref { hash: job.anchor.hash, height: job.anchor.height, epoch_time: job.anchor.epoch_time };
        let ix = self.ix(
            conversion::ID,
            conversion::client::accounts::Fund {
                swap: swap_pda(swap_id),
                job: job_pda(job.id),
                script_record: cv(&[b"script", &sha256(&[script])]),
                anchor_node: node_pda(&anchor),
                operator: who.pubkey(),
                system_program: SYSTEM,
                mint: None,
                escrow: None,
                from: None,
                token_program: None,
                associated_token_program: None,
            },
            conversion::client::args::Fund { script: script.to_vec() },
        );
        self.send(ix, &[who])
    }

    /// The operator's duty: its tagged transaction (with `extra` outputs,
    /// spending `also`) after `before` blocks, then 5 on top, proven.
    /// Returns (anchor, the blocks before, the transaction, its block).
    fn duty(&mut self, swap_id: u64, before: &[Vec<Vec<u8>>], extra: &[(u64, &[u8])], also: &[([u8; 32], u32)]) -> (Ref, Vec<Ref>, Vec<u8>, Ref) {
        let job_id = self.swap(swap_id).job_id;
        let job = self.job(job_id);
        let anchor = if job.anchored_at != 0 {
            Ref { hash: job.anchor.hash, height: job.anchor.height, epoch_time: job.anchor.epoch_time }
        } else {
            let a = self.add(&[], None);
            self.anchor(job_id, a).unwrap();
            a
        };
        let mut blocks = vec![];
        for txs in before {
            blocks.push(self.add(txs, None));
        }
        let op = self.operator.insecure_clone();
        let o: ipow_protocol::accounts::Operator = self.ctx.get_account(&operator_pda(&op.pubkey())).unwrap();
        let payload = sha256(&[b"iPoW job", &tag_of(swap_id)]);
        let mut inputs = vec![(o.chain_head_txid, o.chain_head_vout)];
        inputs.extend_from_slice(also);
        let ret = op_return(&payload);
        let mut outputs: Vec<(u64, &[u8])> = vec![(546, &COIN_SCRIPT), (0, &ret)];
        outputs.extend_from_slice(extra);
        let tagged = tx(&inputs, &outputs);
        let proof_block = self.add(&[tagged.clone()], None);
        let mut path = vec![proof_block];
        for _ in 0..5 {
            path.push(self.add(&[], None));
        }
        let tip = self.tip;
        // From the proof back to the anchor, through every block between,
        // whoever mined them (in a tunnel, the other leg's blocks too).
        let _ = &blocks;
        let mut to_proof = vec![proof_block];
        while to_proof.last().unwrap().hash != anchor.hash {
            let parent = self.parents[&to_proof.last().unwrap().hash];
            to_proof.push(parent);
        }
        let walk_to_proof = self.walk(proof_block, anchor, &to_proof);
        let to_tip: Vec<Ref> = path.iter().rev().cloned().collect();
        let walk_to_tip = self.walk(tip, proof_block, &to_tip);
        let (siblings, index) = self.proof_of(&proof_block, &tagged);
        let txid = sha256d(&tagged);
        let ix = self.ix(
            ipow_protocol::ID,
            ipow_protocol::client::accounts::ProveJob {
                job: job_pda(job_id),
                application: pr(&[b"application", config().as_ref()]),
                operator: operator_pda(&op.pubkey()),
                proof_node: node_pda(&proof_block),
                walk_to_proof,
                walk_to_tip,
                used_tx: pr(&[b"used_tx", &txid]),
                owner: op.pubkey(),
                system_program: SYSTEM,
            },
            ipow_protocol::client::args::ProveJob {
                proof: ipow_protocol::types::Proof {
                    proof_block: proof_block.p(),
                    tip: tip.p(),
                    raw_tx: tagged.clone(),
                    txid,
                    siblings,
                    tx_index: index,
                    head_index: 0,
                    tag_index: 1,
                },
            },
        );
        self.send(ix, &[&op]).unwrap();
        (anchor, blocks, tagged, proof_block)
    }

    fn after_lock(&mut self, job_id: u64) {
        let end = self.job(job_id).lock_end;
        self.set_now(end + 1);
    }

    fn slash_missed(&mut self, job_id: u64) {
        let job = self.job(job_id);
        let deadline = job.opened_at.max(job.last_bid_at + 60).min(job.opened_at + 15 * 60) + 30 * 48 * 60;
        self.set_now(deadline + 1);
        let g = self.guardian.insecure_clone();
        let salt = [5u8; 32];
        let note = sha256(&[g.pubkey().as_ref(), &job_id.to_le_bytes(), &[0u8; 32], &salt]);
        let ix = self.ix(
            ipow_protocol::ID,
            ipow_protocol::client::accounts::SealNote { record: pr(&[b"note", &note]), guardian: g.pubkey(), system_program: SYSTEM },
            ipow_protocol::client::args::SealNote { note },
        );
        self.send(ix, &[&g]).unwrap();
        let ix = self.ix(
            ipow_protocol::ID,
            ipow_protocol::client::accounts::ReportMissedDuty {
                job: job_pda(job_id),
                operator: operator_pda(&job.operator),
                note_record: pr(&[b"note", &note]),
                application_credit: pr(&[b"credit", job.application.as_ref()]),
                guardian_credit: pr(&[b"credit", g.pubkey().as_ref()]),
                payer_credit: pr(&[b"credit", job.payer.as_ref()]),
                guardian: g.pubkey(),
                system_program: SYSTEM,
            },
            ipow_protocol::client::args::ReportMissedDuty { note, salt },
        );
        self.send(ix, &[&g]).unwrap();
    }

    fn complete_sell(&mut self, swap_id: u64, raw: &[u8]) -> Result<(), String> {
        let s = self.swap(swap_id);
        let job = self.job(s.job_id);
        let caller = self.stranger.insecure_clone();
        let ix = self.ix(
            conversion::ID,
            conversion::client::accounts::CompleteSell {
                swap: swap_pda(swap_id),
                job: job_pda(s.job_id),
                operator: job.operator,
                mint: None,
                escrow: None,
                to: None,
                token_program: None,
            },
            conversion::client::args::CompleteSell { raw_tx: raw.to_vec() },
        );
        self.send(ix, &[&caller])
    }

    fn refund_sell(&mut self, swap_id: u64, raw: &[u8]) -> Result<(), String> {
        let s = self.swap(swap_id);
        let caller = self.stranger.insecure_clone();
        let ix = self.ix(
            conversion::ID,
            conversion::client::accounts::RefundSell {
                swap: swap_pda(swap_id),
                job: job_pda(s.job_id),
                user: s.user,
                config: config(),
                credit: pr(&[b"credit", config().as_ref()]),
                protocol: pr(&[b"protocol"]),
                protocol_vault: pr(&[b"vault"]),
                protocol_program: ipow_protocol::ID,
                mint: None,
                escrow: None,
                to: None,
                token_program: None,
            },
            conversion::client::args::RefundSell { raw_tx: raw.to_vec() },
        );
        self.send(ix, &[&caller])
    }

    fn complete_buy(&mut self, swap_id: u64, receipt: &[u8], payment: &[u8]) -> Result<(), String> {
        let s = self.swap(swap_id);
        let caller = self.stranger.insecure_clone();
        let ix = self.ix(
            conversion::ID,
            conversion::client::accounts::CompleteBuy { swap: swap_pda(swap_id), job: job_pda(s.job_id), user: s.user, mint: None, escrow: None, to: None, token_program: None },
            conversion::client::args::CompleteBuy { receipt_raw: receipt.to_vec(), payment_raw: payment.to_vec(), vout: 0 },
        );
        self.send(ix, &[&caller])
    }

    #[allow(clippy::too_many_arguments)]
    fn prove_my_payment(&mut self, swap_id: u64, payment: &[u8], pay_block: Ref, low: (Ref, Vec<Ref>), high: (Ref, Vec<Ref>), top: Ref) -> Result<(), String> {
        let s = self.swap(swap_id);
        let job = self.job(s.job_id);
        let anchor = Ref { hash: job.anchor.hash, height: job.anchor.height, epoch_time: job.anchor.epoch_time };
        let walk_low = self.walk(pay_block, low.0, &low.1);
        let walk_high = self.walk(high.0, pay_block, &high.1);
        let (siblings, index) = self.proof_of(&pay_block, payment);
        let caller = self.stranger.insecure_clone();
        let ix = self.ix(
            conversion::ID,
            conversion::client::accounts::ProveMyPayment {
                swap: swap_pda(swap_id),
                job: job_pda(s.job_id),
                anchor_node: node_pda(&anchor),
                pay_node: node_pda(&pay_block),
                walk_low,
                walk_high,
                user: s.user,
                mint: None,
                escrow: None,
                to: None,
                token_program: None,
            },
            conversion::client::args::ProveMyPayment {
                payment_raw: payment.to_vec(),
                vout: 0,
                pay_block: pay_block.c(),
                siblings,
                tx_index: index,
                top: top.c(),
                parent_epoch_time: 0,
            },
        );
        self.send(ix, &[&caller])
    }

    fn reclaim(&mut self, swap_id: u64) -> Result<(), String> {
        let s = self.swap(swap_id);
        let job = self.job(s.job_id);
        let caller = self.stranger.insecure_clone();
        let ix = self.ix(
            conversion::ID,
            conversion::client::accounts::Reclaim { swap: swap_pda(swap_id), job: job_pda(s.job_id), operator: job.operator, mint: None, escrow: None, to: None, token_program: None },
            conversion::client::args::Reclaim {},
        );
        self.send(ix, &[&caller])
    }

    fn compensate(&mut self, swap_id: u64) -> Result<(), String> {
        let s = self.swap(swap_id);
        let caller = self.stranger.insecure_clone();
        let ix = self.ix(
            conversion::ID,
            conversion::client::accounts::Compensate {
                swap: swap_pda(swap_id),
                job: job_pda(s.job_id),
                user: s.user,
                config: config(),
                credit: pr(&[b"credit", config().as_ref()]),
                protocol: pr(&[b"protocol"]),
                protocol_vault: pr(&[b"vault"]),
                protocol_program: ipow_protocol::ID,
            },
            conversion::client::args::Compensate {},
        );
        self.send(ix, &[&caller])
    }

    /// A buy of 1 SOL for 0.05 BTC, won, anchored at a new block, and funded.
    /// Returns (swap, job, the anchor's parent).
    fn funded(&mut self) -> (u64, u64, Ref) {
        let swap_id = self.buy(SOL, 5_000_000).unwrap();
        let job_id = self.win(swap_id);
        let parent = self.tip;
        let anchor = self.add(&[], None);
        self.anchor(job_id, anchor).unwrap();
        let op = self.operator.insecure_clone();
        let mut script = OPERATOR_SCRIPT.to_vec();
        script[2] = swap_id as u8;
        self.fund(swap_id, &script, &op).unwrap();
        (swap_id, job_id, parent)
    }
}

fn payment_to(script: &[u8], seed: u8) -> Vec<u8> {
    tx(&[([seed; 32], 0)], &[(5_000_000, script)])
}

fn operator_script(swap_id: u64) -> Vec<u8> {
    let mut script = OPERATOR_SCRIPT.to_vec();
    script[2] = swap_id as u8;
    script
}

// ---------------------------------------------------------------------
// Sell
// ---------------------------------------------------------------------

#[test]
fn pays_the_operator_once_its_payment_is_proven_and_the_lock_has_ended() {
    let mut w = World::new();
    let id = w.sell(SOL, 5_000_000);
    w.win(id);
    let (_, _, tagged, _) = w.duty(id, &[], &[(5_000_000, &USER_SCRIPT)], &[]);
    expect_err(w.complete_sell(id, &tagged), "LockNotEnded");
    let job_id = w.swap(id).job_id;
    w.after_lock(job_id);
    let before = w.balance(&w.operator.pubkey());
    w.complete_sell(id, &tagged).unwrap();
    assert_eq!(w.balance(&w.operator.pubkey()), before + SOL);
}

#[test]
fn refunds_the_user_when_the_proven_transaction_pays_too_little() {
    let mut w = World::new();
    let id = w.sell(SOL, 5_000_000);
    w.win(id);
    let (_, _, tagged, _) = w.duty(id, &[], &[(4_999_999, &USER_SCRIPT)], &[]);
    w.after_lock(w.swap(id).job_id);
    expect_err(w.complete_sell(id, &tagged), "NotPaid");
    let before = w.balance(&w.user.pubkey());
    w.refund_sell(id, &tagged).unwrap();
    assert_eq!(w.balance(&w.user.pubkey()), before + SOL);
}

#[test]
fn refunds_after_a_missed_deadline_and_passes_on_the_escrow_share() {
    let mut w = World::new();
    let id = w.sell(SOL, 5_000_000);
    let job_id = w.win(id);
    expect_err(w.refund_sell(id, &[]), "NotRefundable");
    w.slash_missed(job_id);
    let escrow = w.job(job_id).escrow;
    let before = w.balance(&w.user.pubkey());
    w.refund_sell(id, &[]).unwrap();
    assert_eq!(w.balance(&w.user.pubkey()), before + SOL + escrow * 8 / 10);
    // Once only.
    let before = w.balance(&w.user.pubkey());
    w.compensate(id).unwrap();
    assert_eq!(w.balance(&w.user.pubkey()), before);
}

// ---------------------------------------------------------------------
// Buy
// ---------------------------------------------------------------------

#[test]
fn gives_the_user_the_coin_once_the_receipt_spends_the_payment() {
    let mut w = World::new();
    let (id, _, _) = w.funded();
    let paid = payment_to(&operator_script(id), 7);
    let (_, _, receipt, _) = w.duty(id, &[], &[], &[(sha256d(&paid), 0)]);
    let before = w.balance(&w.user.pubkey());
    w.complete_buy(id, &receipt, &paid).unwrap();
    assert_eq!(w.balance(&w.user.pubkey()), before + SOL);
    expect_err(w.complete_buy(id, &receipt, &paid), "WrongState");
}

#[test]
fn gives_the_user_the_coin_on_their_own_proof_below_the_close_and_refuses_another_branch() {
    let mut w = World::new();
    let (id, _, _) = w.funded();
    let paid = payment_to(&operator_script(id), 7);
    let mut before: Vec<Vec<Vec<u8>>> = vec![vec![paid.clone()]];
    before.extend((0..12).map(|_| vec![]));
    let (anchor, blocks, _, proof_block) = w.duty(id, &before, &[], &[]);

    // A made-up branch from the anchor, with its own payment: refused.
    let forged = payment_to(&operator_script(id), 8);
    let forged_block = w.add(&[forged.clone()], Some(anchor));
    let mut on = forged_block;
    let mut path = vec![];
    for _ in 0..5 {
        on = w.add(&[], Some(on));
        path.push(on);
    }
    let mut high = path.iter().rev().cloned().collect::<Vec<_>>();
    high.push(forged_block);
    expect_err(
        w.prove_my_payment(id, &forged, forged_block, (anchor, vec![forged_block, anchor]), (on, high), on),
        "NotLinked",
    );

    // The real payment, below the close.
    let pay_block = blocks[0];
    let mut high: Vec<Ref> = blocks.iter().rev().cloned().collect();
    high.insert(0, proof_block);
    let user_before = w.balance(&w.user.pubkey());
    w.prove_my_payment(id, &paid, pay_block, (anchor, vec![pay_block, anchor]), (proof_block, high), pay_block).unwrap();
    assert_eq!(w.balance(&w.user.pubkey()), user_before + SOL);
}

#[test]
fn lets_the_user_prove_on_the_anchors_parent_once_the_operator_failed() {
    let mut w = World::new();
    let (id, job_id, parent) = w.funded();
    // Bitcoin dropped the anchor: the real chain grows from its parent.
    let first = w.add(&[], Some(parent));
    let paid = payment_to(&operator_script(id), 7);
    let pay_block = w.add(&[paid.clone()], Some(first));
    let mut on = pay_block;
    let mut above = vec![];
    for _ in 0..5 {
        on = w.add(&[], Some(on));
        above.push(on);
    }
    let mut high: Vec<Ref> = above.iter().rev().cloned().collect();
    high.push(pay_block);
    let low = (parent, vec![pay_block, first, parent]);
    // Not while the operator can still prove.
    expect_err(w.prove_my_payment(id, &paid, pay_block, low.clone(), (on, high.clone()), on), "NotLinked");
    w.slash_missed(job_id);
    let before = w.balance(&w.user.pubkey());
    w.prove_my_payment(id, &paid, pay_block, low, (on, high), on).unwrap();
    assert_eq!(w.balance(&w.user.pubkey()), before + SOL);
    let escrow = w.job(job_id).escrow;
    let before = w.balance(&w.user.pubkey());
    w.compensate(id).unwrap();
    assert_eq!(w.balance(&w.user.pubkey()), before + escrow * 8 / 10);
}

#[test]
fn gives_the_operator_the_coin_back_after_its_close() {
    let mut w = World::new();
    let (id, job_id, _) = w.funded();
    let before: Vec<Vec<Vec<u8>>> = (0..13).map(|_| vec![]).collect();
    w.duty(id, &before, &[], &[]);
    expect_err(w.reclaim(id), "NotReclaimable");
    w.after_lock(job_id);
    let op_before = w.balance(&w.operator.pubkey());
    w.reclaim(id).unwrap();
    assert_eq!(w.balance(&w.operator.pubkey()), op_before + SOL);
}

#[test]
fn lets_only_the_operator_fund_after_its_anchor_with_a_new_script() {
    let mut w = World::new();
    let id = w.buy(SOL, 5_000_000).unwrap();
    let job_id = w.win(id);
    let op = w.operator.insecure_clone();
    let stranger = w.stranger.insecure_clone();
    // Before an anchor the job's anchor is empty: its node does not exist.
    assert!(w.fund(id, &OPERATOR_SCRIPT, &op).is_err());
    let anchor = w.add(&[], None);
    w.anchor(job_id, anchor).unwrap();
    expect_err(w.fund(id, &OPERATOR_SCRIPT, &stranger), "NotOperator");
    w.fund(id, &OPERATOR_SCRIPT, &op).unwrap();

    // A second swap cannot reuse the script.
    let id2 = w.buy(SOL, 5_000_000).unwrap();
    let job2 = w.win(id2);
    let anchor2 = w.add(&[], None);
    w.anchor(job2, anchor2).unwrap();
    assert!(w.fund(id2, &OPERATOR_SCRIPT, &op).is_err(), "script already named");

    // An anchor mined 40 minutes ago: the payment blocks are mostly over.
    let id3 = w.buy(SOL, 5_000_000).unwrap();
    let job3 = w.win(id3);
    let t = w.now();
    w.set_now(t - 40 * 60);
    let old = w.add(&[], None);
    w.set_now(t);
    w.anchor(job3, old).unwrap();
    expect_err(w.fund(id3, &operator_script(99), &op), "AnchorTooOld");
}

#[test]
fn refuses_a_swap_above_the_size_limit() {
    let mut w = World::new();
    expect_err(w.buy(SOL, 10_000_001), "TooLarge");
}

// ---------------------------------------------------------------------
// Tokens
// ---------------------------------------------------------------------

#[test]
fn sells_a_token() {
    use anchor_spl::associated_token::get_associated_token_address_with_program_id as ata;
    let mut w = World::new();
    let token_program = anchor_spl::token::ID;
    let payer = w.stranger.insecure_clone();
    let mint = litesvm_token::CreateMint::new(&mut w.ctx.svm, &payer).decimals(6).send().unwrap();
    let user = w.user.insecure_clone();
    let op = w.operator.insecure_clone();
    let user_ata = litesvm_token::CreateAssociatedTokenAccount::new(&mut w.ctx.svm, &payer, &mint).owner(&user.pubkey()).send().unwrap();
    let op_ata = litesvm_token::CreateAssociatedTokenAccount::new(&mut w.ctx.svm, &payer, &mint).owner(&op.pubkey()).send().unwrap();
    litesvm_token::MintTo::new(&mut w.ctx.svm, &payer, &mint, &user_ata, 1_000).send().unwrap();

    let id = w.swap_count() + 1;
    let (swap, job, tag_record) = w.open_accounts(id);
    let escrow = ata(&swap, &mint, &token_program);
    let ix = w.ix(
        conversion::ID,
        conversion::client::accounts::Sell {
            config: config(),
            swap,
            user: user.pubkey(),
            protocol: pr(&[b"protocol"]),
            application: pr(&[b"application", config().as_ref()]),
            job,
            tag_record,
            protocol_vault: pr(&[b"vault"]),
            protocol_program: ipow_protocol::ID,
            system_program: SYSTEM,
            mint: Some(mint),
            escrow: Some(escrow),
            from: Some(user_ata),
            token_program: Some(token_program),
            associated_token_program: Some(anchor_spl::associated_token::ID),
        },
        conversion::client::args::Sell { amount: 1_000, sats: 5_000_000, script: USER_SCRIPT.to_vec(), confirmations: 6, paid: FEES },
    );
    w.send(ix, &[&user]).unwrap();
    assert_eq!(w.swap(id).amount, 1_000);

    w.win(id);
    let (_, _, tagged, _) = w.duty(id, &[], &[(5_000_000, &USER_SCRIPT)], &[]);
    w.after_lock(w.swap(id).job_id);
    let s = w.swap(id);
    let job_account = w.job(s.job_id);
    let caller = w.stranger.insecure_clone();
    let ix = w.ix(
        conversion::ID,
        conversion::client::accounts::CompleteSell {
            swap,
            job: job_pda(s.job_id),
            operator: job_account.operator,
            mint: Some(mint),
            escrow: Some(escrow),
            to: Some(op_ata),
            token_program: Some(token_program),
        },
        conversion::client::args::CompleteSell { raw_tx: tagged },
    );
    w.send(ix, &[&caller]).unwrap();
    let account: anchor_spl::token::spl_token::state::Account = litesvm_token::get_spl_account(&w.ctx.svm, &op_ata).unwrap();
    assert_eq!(account.amount, 1_000);
}

// ---------------------------------------------------------------------
// Found by the review
// ---------------------------------------------------------------------

#[test]
fn cannot_mark_a_swap_compensated_without_the_credit_record() {
    let mut w = World::new();
    let id = w.sell(SOL, 5_000_000);
    let job_id = w.win(id);
    w.slash_missed(job_id);
    // Another account in place of this application's credit record.
    let s = w.swap(id);
    let caller = w.stranger.insecure_clone();
    let ix = w.ix(
        conversion::ID,
        conversion::client::accounts::Compensate {
            swap: swap_pda(id),
            job: job_pda(s.job_id),
            user: s.user,
            config: config(),
            credit: pr(&[b"credit", caller.pubkey().as_ref()]),
            protocol: pr(&[b"protocol"]),
            protocol_vault: pr(&[b"vault"]),
            protocol_program: ipow_protocol::ID,
        },
        conversion::client::args::Compensate {},
    );
    expect_err(w.send(ix, &[&caller]), "ConstraintSeeds");
    // The real one pays the full share.
    let escrow = w.job(job_id).escrow;
    let before = w.balance(&w.user.pubkey());
    w.compensate(id).unwrap();
    assert_eq!(w.balance(&w.user.pubkey()), before + escrow * 8 / 10);
}

#[test]
fn cancels_a_swap_whose_operator_did_not_fund_in_time() {
    let mut w = World::new();
    let id = w.buy(SOL, 5_000_000).unwrap();
    w.win(id);
    let cancel = |w: &mut World| {
        let s = w.swap(id);
        let caller = w.stranger.insecure_clone();
        let ix = w.ix(conversion::ID, conversion::client::accounts::Cancel { swap: swap_pda(id), job: job_pda(s.job_id) }, conversion::client::args::Cancel {});
        w.send(ix, &[&caller])
    };
    expect_err(cancel(&mut w), "FundingTimeNotOver");
    let t = w.now();
    w.set_now(t + 31 * 60);
    cancel(&mut w).unwrap();
    assert!(matches!(w.swap(id).state, conversion::types::SwapState::Cancelled));
}

#[test]
fn gives_the_operator_the_coin_back_36_hours_after_its_deadline_when_nothing_was_proven() {
    let mut w = World::new();
    let (id, job_id, _) = w.funded();
    expect_err(w.reclaim(id), "NotReclaimable");
    let job = w.job(job_id);
    let auction_end = (job.opened_at + 15 * 60).min(job.last_bid_at + 60);
    w.set_now(auction_end + 30 * 48 * 60 + 36 * HOUR);
    let before = w.balance(&w.operator.pubkey());
    w.reclaim(id).unwrap();
    assert_eq!(w.balance(&w.operator.pubkey()), before + SOL);
}

// ---------------------------------------------------------------------
// Tunnels between programmable networks (T1, T2)
// ---------------------------------------------------------------------

impl World {
    /// A sell paying `script`, whose payment counts only in Bitcoin blocks `from` to `to`.
    fn sell_in_window(&mut self, script: &[u8], from: u32, to: u32) -> Result<u64, String> {
        let id = self.swap_count() + 1;
        let (swap, job, tag_record) = self.open_accounts(id);
        let user = self.user.insecure_clone();
        let ix = self.ix(
            conversion::ID,
            conversion::client::accounts::Sell {
                config: config(),
                swap,
                user: user.pubkey(),
                protocol: pr(&[b"protocol"]),
                application: pr(&[b"application", config().as_ref()]),
                job,
                tag_record,
                protocol_vault: pr(&[b"vault"]),
                protocol_program: ipow_protocol::ID,
                system_program: SYSTEM,
                mint: None,
                escrow: None,
                from: None,
                token_program: None,
                associated_token_program: None,
            },
            conversion::client::args::SellInWindow { amount: SOL, sats: 5_000_000, script: script.to_vec(), pay_from: from, pay_to: to, confirmations: 6, paid: FEES },
        );
        self.send(ix, &[&user])?;
        Ok(id)
    }

    /// A buy opened by `opener` for `recipient`.
    fn buy_for(&mut self, opener: &Keypair, recipient: Pubkey) -> Result<u64, String> {
        let id = self.swap_count() + 1;
        let (swap, job, tag_record) = self.open_accounts(id);
        let ix = self.ix(
            conversion::ID,
            conversion::client::accounts::Buy {
                config: config(),
                swap,
                user: opener.pubkey(),
                protocol: pr(&[b"protocol"]),
                application: pr(&[b"application", config().as_ref()]),
                job,
                tag_record,
                protocol_vault: pr(&[b"vault"]),
                protocol_program: ipow_protocol::ID,
                system_program: SYSTEM,
                mint: None,
            },
            conversion::client::args::BuyFor { recipient, amount: SOL, sats: 5_000_000, confirmations: 6, paid: FEES },
        );
        self.send(ix, &[opener])?;
        Ok(id)
    }

    fn complete_buy_at(&mut self, swap_id: u64, receipt: &[u8], payment: &[u8], vout: u32) -> Result<(), String> {
        let s = self.swap(swap_id);
        let caller = self.stranger.insecure_clone();
        let ix = self.ix(
            conversion::ID,
            conversion::client::accounts::CompleteBuy { swap: swap_pda(swap_id), job: job_pda(s.job_id), user: s.user, mint: None, escrow: None, to: None, token_program: None },
            conversion::client::args::CompleteBuy { receipt_raw: receipt.to_vec(), payment_raw: payment.to_vec(), vout },
        );
        self.send(ix, &[&caller])
    }
}

#[test]
fn completes_a_sell_whose_payment_is_mined_inside_its_window() {
    let mut w = World::new();
    let h = w.tip.height;
    // The duty anchors at h + 1 and its transaction lands at h + 2.
    let id = w.sell_in_window(&USER_SCRIPT, h + 2, h + 13).unwrap();
    w.win(id);
    let (_, _, tagged, proof_block) = w.duty(id, &[], &[(5_000_000, &USER_SCRIPT)], &[]);
    assert_eq!(proof_block.height, h + 2);
    w.after_lock(w.swap(id).job_id);
    let op = w.operator.pubkey();
    let before = w.balance(&op);
    w.complete_sell(id, &tagged).unwrap();
    assert_eq!(w.balance(&op), before + SOL);
}

#[test]
fn refunds_the_user_when_the_payment_is_mined_outside_the_window() {
    let mut w = World::new();
    let h = w.tip.height;
    let id = w.sell_in_window(&USER_SCRIPT, h + 2, h + 3).unwrap();
    w.win(id);
    // Three blocks before the transaction: it lands at h + 5, after the window.
    let (_, _, tagged, _) = w.duty(id, &[vec![], vec![], vec![]], &[(5_000_000, &USER_SCRIPT)], &[]);
    w.after_lock(w.swap(id).job_id);
    expect_err(w.complete_sell(id, &tagged), "PaidOutsideWindow");
    let before = w.balance(&w.user.pubkey());
    w.refund_sell(id, &[]).unwrap();
    assert_eq!(w.balance(&w.user.pubkey()), before + SOL);
}

#[test]
fn counts_a_payment_in_the_windows_first_or_last_block_and_not_one_block_either_side() {
    // The duty's transaction lands at h + 2 in each case.
    for (from, to, inside) in [(2, 2, true), (1, 2, true), (2, 9, true), (3, 9, false), (1, 1, false)] {
        let mut w = World::new();
        let h = w.tip.height;
        let id = w.sell_in_window(&USER_SCRIPT, h + from, h + to).unwrap();
        w.win(id);
        let (_, _, tagged, proof_block) = w.duty(id, &[], &[(5_000_000, &USER_SCRIPT)], &[]);
        assert_eq!(proof_block.height, h + 2);
        w.after_lock(w.swap(id).job_id);
        if inside {
            expect_err(w.refund_sell(id, &tagged), "NotRefundable");
            let op = w.operator.pubkey();
            let before = w.balance(&op);
            w.complete_sell(id, &tagged).unwrap();
            assert_eq!(w.balance(&op), before + SOL);
        } else {
            expect_err(w.complete_sell(id, &tagged), "PaidOutsideWindow");
            let before = w.balance(&w.user.pubkey());
            w.refund_sell(id, &[]).unwrap();
            assert_eq!(w.balance(&w.user.pubkey()), before + SOL);
        }
    }
}

#[test]
fn does_not_refund_a_sell_paid_in_full_inside_its_window() {
    let mut w = World::new();
    let h = w.tip.height;
    let id = w.sell_in_window(&USER_SCRIPT, h + 2, h + 13).unwrap();
    w.win(id);
    let (_, _, tagged, _) = w.duty(id, &[], &[(5_000_000, &USER_SCRIPT)], &[]);
    expect_err(w.refund_sell(id, &tagged), "NotRefundable");
    w.after_lock(w.swap(id).job_id);
    expect_err(w.refund_sell(id, &tagged), "NotRefundable");
    assert!(w.refund_sell(id, &[]).is_err());
}

#[test]
fn refuses_a_window_that_starts_at_zero_or_ends_before_it_starts() {
    let mut w = World::new();
    expect_err(w.sell_in_window(&USER_SCRIPT, 0, 10), "BadWindow");
    expect_err(w.sell_in_window(&USER_SCRIPT, 10, 9), "BadWindow");
}

#[test]
fn opens_a_buy_for_a_recipient_the_opener_paying_the_fees() {
    let mut w = World::new();
    let stranger = w.stranger.insecure_clone();
    expect_err(w.buy_for(&stranger, Pubkey::default()), "ZeroRecipient");
    let user = w.user.pubkey();
    let id = w.buy_for(&stranger, user).unwrap();
    assert_eq!(w.swap(id).user, user);
}

#[test]
fn links_a_buy_and_a_sell_with_one_bitcoin_payment_both_complete() {
    let mut w = World::new();
    // The buy: the operator opens it for the user, wins, anchors and locks 1 SOL at its new script.
    let op = w.operator.insecure_clone();
    let user = w.user.pubkey();
    let buy_id = w.buy_for(&op, user).unwrap();
    let buy_job = w.win(buy_id);
    let anchor = w.add(&[], None);
    w.anchor(buy_job, anchor).unwrap();
    let script = operator_script(buy_id);
    w.fund(buy_id, &script, &op).unwrap();

    // The sell pays the buy's script within the buy's payment blocks.
    let sell_id = w.sell_in_window(&script, anchor.height + 1, anchor.height + 12).unwrap();
    w.win(sell_id);
    let (_, _, sold, proof_block) = w.duty(sell_id, &[], &[(5_000_000, &script)], &[]);
    assert!(proof_block.height >= anchor.height + 1 && proof_block.height <= anchor.height + 12);

    // The buy's receipt spends that payment: output 2 of the sell's transaction.
    let (_, _, receipt, _) = w.duty(buy_id, &[], &[], &[(sha256d(&sold), 2)]);
    let before = w.balance(&user);
    w.complete_buy_at(buy_id, &receipt, &sold, 2).unwrap();
    assert_eq!(w.balance(&user), before + SOL);

    // The sell's operator is paid once its lock ends.
    w.after_lock(w.swap(sell_id).job_id);
    let before = w.balance(&op.pubkey());
    w.complete_sell(sell_id, &sold).unwrap();
    assert_eq!(w.balance(&op.pubkey()), before + SOL);
}
