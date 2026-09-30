//! Tests of the protocol's vault on Solana: the same rules as
//! `test/iPoWVault.test.ts` on Ethereum, with the two networks' parts
//! swapped. Spec: docs/design/ipow-protocol.md, section 11. The Bitcoin blocks
//! are mined by the test at an easy difficulty, which only the test build of
//! the light client accepts.

use anchor_lang;
use anchor_lang::solana_program::instruction::{AccountMeta, Instruction};
use anchor_lang::AccountDeserialize;
use anchor_litesvm::{AnchorContext, AnchorLiteSVM, TestHelpers};
use sha2::{Digest, Sha256};
use solana_keypair::Keypair;
use solana_program::pubkey::Pubkey;
use solana_signer::Signer;

anchor_lang::declare_program!(ipow_vault);
anchor_lang::declare_program!(ipow_protocol);
anchor_lang::declare_program!(ipow_light_client);

const SOL: u64 = 1_000_000_000;
const HOUR: i64 = 3600;
const DAY: i64 = 24 * HOUR;
const WEEK: i64 = 7 * DAY;
const T0: i64 = 1_800_000_000;
const EASY: u32 = 0x207f_ffff;
const SYSTEM: Pubkey = anchor_lang::solana_program::system_program::ID;
const TOKEN: Pubkey = anchor_spl::token::ID;
const FEES: u64 = SOL / 10;
const DEPOSIT: u64 = 50_000_000;
const MIN_CERTIFYING_ESCROW: u64 = 20 * SOL;
const ETHEREUM_VAULT: [u8; 20] = [0x11; 20];
const PEER_OPERATOR: [u8; 20] = [0xca; 20];
const ETH_USER: [u8; 20] = [0xbb; 20];
const GWEI_PER_ETH: u64 = 1_000_000_000;
const COIN_SCRIPT: [u8; 22] = {
    let mut s = [0x11u8; 22];
    s[0] = 0x00;
    s[1] = 0x14;
    s
};
const ETHEREUM: u8 = 1;
const SOLANA: u8 = 2;

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
// Records (section 11.5): amounts in gwei, big-endian
// ---------------------------------------------------------------------

fn lock_rec(id: u64, amount: u64, recipient: &Pubkey, fee: u64) -> Vec<u8> {
    [&[1u8][..], &id.to_be_bytes(), &amount.to_be_bytes(), recipient.as_ref(), &fee.to_be_bytes()].concat()
}
fn request_rec(id: u64, amount: u64, to: &[u8; 20], fee: u64) -> Vec<u8> {
    [&[2u8][..], &id.to_be_bytes(), &amount.to_be_bytes(), to, &fee.to_be_bytes()].concat()
}
fn cancel_rec(id: u64) -> Vec<u8> {
    [&[3u8][..], &id.to_be_bytes()].concat()
}
fn bond_rec(net: u8, amount: u64) -> Vec<u8> {
    [&[4u8, net][..], &amount.to_be_bytes()].concat()
}
fn exit_rec() -> Vec<u8> {
    vec![5u8]
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
fn vt(seeds: &[&[u8]]) -> Pubkey {
    Pubkey::find_program_address(seeds, &ipow_vault::ID).0
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
    fn v(&self) -> ipow_vault::types::BlockRef {
        ipow_vault::types::BlockRef { hash: self.hash, height: self.height, epoch_time: self.epoch_time }
    }
    fn l(&self) -> ipow_light_client::types::BlockRef {
        ipow_light_client::types::BlockRef { hash: self.hash, height: self.height, epoch_time: self.epoch_time }
    }
}

fn node_pda(r: &Ref) -> Pubkey {
    lc(&[b"node", &r.hash, &r.height.to_le_bytes(), &r.epoch_time.to_le_bytes()])
}
fn real_pda(r: &Ref) -> Pubkey {
    vt(&[b"real", &r.hash, &r.height.to_le_bytes(), &r.epoch_time.to_le_bytes()])
}
fn job_pda(id: u64) -> Pubkey {
    pr(&[b"job", &id.to_le_bytes()])
}
fn operator_pda(o: &Pubkey) -> Pubkey {
    pr(&[b"operator", o.as_ref()])
}
fn config() -> Pubkey {
    vt(&[b"config"])
}
fn mint() -> Pubkey {
    vt(&[b"veth"])
}
fn holding() -> Pubkey {
    vt(&[b"holding"])
}
fn chain_pda(o: &Pubkey) -> Pubkey {
    vt(&[b"chain", o.as_ref()])
}
fn claim_pda(id: u64) -> Pubkey {
    vt(&[b"claim", &id.to_le_bytes()])
}
fn stake_pda(id: u64, who: &Pubkey) -> Pubkey {
    vt(&[b"stake", &id.to_le_bytes(), who.as_ref()])
}
fn credit_pda(who: &Pubkey) -> Pubkey {
    vt(&[b"credit", who.as_ref()])
}
fn request_pda(id: u64) -> Pubkey {
    vt(&[b"request", &id.to_le_bytes()])
}
fn lock_pda(id: u64) -> Pubkey {
    vt(&[b"lock", &id.to_le_bytes()])
}
fn ata(owner: &Pubkey) -> Pubkey {
    anchor_spl::associated_token::get_associated_token_address(owner, &mint())
}
fn checkpoint_tag(n: u64) -> [u8; 32] {
    sha256(&[b"iPoW checkpoint", ipow_vault::ID.as_ref(), &n.to_le_bytes()])
}
fn pair_commitment(peer: &[u8; 20], operator: &Pubkey) -> [u8; 32] {
    sha256(&[b"iPoW pair", &ETHEREUM_VAULT, peer, ipow_vault::ID.as_ref(), operator.as_ref()])
}
fn payload(batch: &[u8]) -> [u8; 32] {
    sha256(&[b"iPoW vault", batch])
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

/// A message on an operator's pair chain.
#[derive(Clone)]
struct Msg {
    tx: Vec<u8>,
    block: Ref,
    batch: Vec<u8>,
}

/// An operator's pair chain: its coin, its registration, the messages it
/// wrote, in order.
struct Pair {
    who: Keypair,
    coin: ([u8; 32], u32),
    registration: Msg,
    msgs: Vec<Msg>,
}

struct World {
    ctx: AnchorContext,
    user: Keypair,
    operator: Keypair,
    guardian: Keypair,
    stranger: Keypair,
    tip: Ref,
    /// The main chain, lowest first.
    main: Vec<Ref>,
    blocks: std::collections::HashMap<[u8; 32], Vec<[u8; 32]>>,
    salt: u32,
    walks: u64,
}

impl World {
    fn new() -> Self {
        let ctx = AnchorLiteSVM::build_with_programs(&[
            (ipow_light_client::ID, include_bytes!("../../../target/deploy-test/ipow_light_client.so")),
            (ipow_protocol::ID, include_bytes!("../../../target/deploy/ipow_protocol.so")),
            (ipow_vault::ID, include_bytes!("../../../target/deploy-test/ipow_vault.so")),
        ]);
        let mut w = World {
            ctx,
            user: Keypair::new(),
            operator: Keypair::new(),
            guardian: Keypair::new(),
            stranger: Keypair::new(),
            tip: Ref { hash: [0; 32], height: 0, epoch_time: 0 },
            main: vec![],
            blocks: Default::default(),
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
                ipow_vault::ID,
                ipow_vault::client::accounts::Initialize {
                    config: config(),
                    mint: mint(),
                    holding: holding(),
                    application: pr(&[b"application", config().as_ref()]),
                    payer: payer.pubkey(),
                    program_data: SYSTEM,
                    protocol_program: ipow_protocol::ID,
                    token_program: TOKEN,
                    system_program: SYSTEM,
                },
                ipow_vault::client::args::Initialize { ethereum_vault: ETHEREUM_VAULT, deposit: DEPOSIT, min_certifying_escrow: MIN_CERTIFYING_ESCROW },
            ),
            &[&payer],
        )
        .unwrap();
        w.start_chain();

        // The operator's protocol bond and chain head, for checkpoint jobs.
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
                ipow_protocol::client::args::LockBond { amount: 100 * SOL },
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

    fn later(&mut self, s: i64) {
        let t = self.now();
        self.set_now(t + s);
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
        let nodes: Vec<Ref> = headers.iter().enumerate().map(|(i, h)| Ref { hash: sha256d(h), height: i as u32, epoch_time }).collect();
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
        for n in &nodes {
            ix.accounts.push(AccountMeta::new(node_pda(n), false));
        }
        self.send(ix, &[&payer]).unwrap();
        self.main = nodes;
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
        if parent.is_none() {
            self.tip = r;
            self.main.push(r);
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

    /// A walk along the main chain from `high` down to `low`.
    fn main_walk(&mut self, high: Ref, low: Ref) -> Pubkey {
        let hi = self.main.iter().position(|r| *r == high).unwrap();
        let lo = self.main.iter().position(|r| *r == low).unwrap();
        let path: Vec<Ref> = self.main[lo..=hi].iter().rev().cloned().collect();
        self.walk(high, low, &path)
    }

    fn job(&self, id: u64) -> ipow_protocol::accounts::Job {
        self.ctx.get_account(&job_pda(id)).unwrap()
    }
    fn job_count(&self) -> u64 {
        let p: ipow_protocol::accounts::Protocol = self.ctx.get_account(&pr(&[b"protocol"])).unwrap();
        p.job_count
    }
    fn vault_config(&self) -> ipow_vault::accounts::Config {
        self.ctx.get_account(&config()).unwrap()
    }
    fn chain(&self, o: &Pubkey) -> ipow_vault::accounts::Chain {
        self.ctx.get_account(&chain_pda(o)).unwrap()
    }
    fn claim(&self, id: u64) -> ipow_vault::accounts::Claim {
        self.ctx.get_account(&claim_pda(id)).unwrap()
    }
    fn credit(&self, who: &Pubkey) -> (u64, u64) {
        match self.ctx.svm.get_account(&credit_pda(who)) {
            Some(a) if !a.data.is_empty() => {
                let c = ipow_vault::accounts::Credit::try_deserialize(&mut &a.data[..]).unwrap();
                (c.lamports, c.veth)
            }
            _ => (0, 0),
        }
    }
    fn tokens(&self, account: &Pubkey) -> u64 {
        let a = self.ctx.svm.get_account(account).unwrap();
        anchor_spl::token::TokenAccount::try_deserialize(&mut &a.data[..]).unwrap().amount
    }
    fn supply(&self) -> u64 {
        let a = self.ctx.svm.get_account(&mint()).unwrap();
        anchor_spl::token::Mint::try_deserialize(&mut &a.data[..]).unwrap().supply
    }

    /// An associated vETH account for `owner`.
    fn veth_account(&mut self, owner: &Pubkey) -> Pubkey {
        let payer = self.stranger.insecure_clone();
        litesvm_token::CreateAssociatedTokenAccount::new(&mut self.ctx.svm, &payer, &mint()).owner(owner).send().unwrap()
    }

    /// D108, D116: a checkpoint job opened through the vault, proven by the
    /// operator on top of the chain, its lock ended. Its proof block is real.
    fn checkpoint(&mut self) -> Ref {
        let n = self.vault_config().checkpoint_count + 1;
        let job_id = self.job_count() + 1;
        let funder = self.stranger.insecure_clone();
        let ix = self.ix(
            ipow_vault::ID,
            ipow_vault::client::accounts::OpenCheckpoint {
                config: config(),
                protocol: pr(&[b"protocol"]),
                application: pr(&[b"application", config().as_ref()]),
                job: job_pda(job_id),
                tag_record: pr(&[b"tag", config().as_ref(), &checkpoint_tag(n)]),
                protocol_vault: pr(&[b"vault"]),
                funder: funder.pubkey(),
                protocol_program: ipow_protocol::ID,
                system_program: SYSTEM,
            },
            ipow_vault::client::args::OpenCheckpoint { confirmations: 6, paid: SOL },
        );
        self.send(ix, &[&funder]).unwrap();
        assert_eq!(self.job(job_id).escrow, MIN_CERTIFYING_ESCROW);
        let proof_block = self.prove_job(job_id, checkpoint_tag(n), config());
        expect_err(self.record_real_from_job(job_id), "NotCertified");
        let end = self.job(job_id).lock_end;
        self.set_now(end + 1);
        self.record_real_from_job(job_id).unwrap();
        proof_block
    }

    /// The operator wins job `job_id`, anchors, and proves it.
    fn prove_job(&mut self, job_id: u64, tag: [u8; 32], app_key: Pubkey) -> Ref {
        let op = self.operator.insecure_clone();
        let job = self.job(job_id);
        let ix = self.ix(
            ipow_protocol::ID,
            ipow_protocol::client::accounts::Bid { job: job_pda(job_id), operator: operator_pda(&op.pubkey()), previous: None, owner: op.pubkey() },
            ipow_protocol::client::args::Bid { amount: job.escrow },
        );
        self.send(ix, &[&op]).unwrap();
        self.later(61);
        let anchor = self.add(&[], None);
        let ix = self.ix(
            ipow_protocol::ID,
            ipow_protocol::client::accounts::AnchorJob { job: job_pda(job_id), node: node_pda(&anchor), owner: op.pubkey() },
            ipow_protocol::client::args::AnchorJob { anchor: anchor.p() },
        );
        self.send(ix, &[&op]).unwrap();
        let o: ipow_protocol::accounts::Operator = self.ctx.get_account(&operator_pda(&op.pubkey())).unwrap();
        let ret = op_return(&sha256(&[b"iPoW job", &tag]));
        let tagged = tx(&[(o.chain_head_txid, o.chain_head_vout)], &[(546, &COIN_SCRIPT), (0, &ret)]);
        let proof_block = self.add(&[tagged.clone()], None);
        let mut path = vec![proof_block];
        for _ in 0..5 {
            path.push(self.add(&[], None));
        }
        let tip = self.tip;
        let walk_to_proof = self.walk(proof_block, anchor, &[proof_block, anchor]);
        let to_tip: Vec<Ref> = path.iter().rev().cloned().collect();
        let walk_to_tip = self.walk(tip, proof_block, &to_tip);
        let (siblings, index) = self.proof_of(&proof_block, &tagged);
        let txid = sha256d(&tagged);
        let ix = self.ix(
            ipow_protocol::ID,
            ipow_protocol::client::accounts::ProveJob {
                job: job_pda(job_id),
                application: pr(&[b"application", app_key.as_ref()]),
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
                    raw_tx: tagged,
                    txid,
                    siblings,
                    tx_index: index,
                    head_index: 0,
                    tag_index: 1,
                },
            },
        );
        self.send(ix, &[&op]).unwrap();
        proof_block
    }

    fn record_real_from_job(&mut self, job_id: u64) -> Result<(), String> {
        let job = self.job(job_id);
        let pb = Ref { hash: job.proof_block.hash, height: job.proof_block.height, epoch_time: job.proof_block.epoch_time };
        let payer = self.stranger.insecure_clone();
        let ix = self.ix(
            ipow_vault::ID,
            ipow_vault::client::accounts::RecordRealFromJob { config: config(), job: job_pda(job_id), real: real_pda(&pb), payer: payer.pubkey(), system_program: SYSTEM },
            ipow_vault::client::args::RecordRealFromJob { job_id },
        );
        self.send(ix, &[&payer])
    }

    /// Writes the registration of `who`'s pair chain on Bitcoin.
    fn write_registration(&mut self, who: &Keypair) -> Pair {
        let commitment = pair_commitment(&PEER_OPERATOR, &who.pubkey());
        let t = tx(&[(sha256(&[b"pair funding", who.pubkey().as_ref()]), 0)], &[(546, &COIN_SCRIPT), (0, &op_return(&commitment))]);
        let block = self.add(&[t.clone()], None);
        Pair { who: who.insecure_clone(), coin: (sha256d(&t), 0), registration: Msg { tx: t, block, batch: vec![] }, msgs: vec![] }
    }

    /// Writes a message on the pair chain, mined on `on` or on the tip.
    fn write(&mut self, pair: &mut Pair, batch: Vec<u8>, on: Option<Ref>) -> Msg {
        let t = tx(&[pair.coin], &[(546, &COIN_SCRIPT), (0, &op_return(&payload(&batch)))]);
        let block = self.add(&[t.clone()], on);
        pair.coin = (sha256d(&t), 0);
        let m = Msg { tx: t, block, batch };
        pair.msgs.push(m.clone());
        m
    }

    fn btc(&mut self, m: &Msg, real: Ref) -> (ipow_vault::types::Btc, Pubkey) {
        let walk = if m.block == real { SYSTEM } else { self.main_walk(real, m.block) };
        let (siblings, index) = self.proof_of(&m.block, &m.tx);
        (ipow_vault::types::Btc { block: m.block.v(), raw_tx: m.tx.clone(), siblings, tx_index: index, real: real.v() }, walk)
    }

    fn register(&mut self, pair: &Pair, real: Ref) -> Result<(), String> {
        let (btc, walk) = self.btc(&pair.registration.clone(), real);
        let who = pair.who.insecure_clone();
        let ix = self.ix(
            ipow_vault::ID,
            ipow_vault::client::accounts::RegisterChain {
                config: config(),
                chain: chain_pda(&who.pubkey()),
                real: real_pda(&real),
                walk,
                node: node_pda(&pair.registration.block),
                operator: who.pubkey(),
                system_program: SYSTEM,
            },
            ipow_vault::client::args::RegisterChain { peer_operator: PEER_OPERATOR, btc, coin_index: 0, tag_index: 1 },
        );
        self.send(ix, &[&who])
    }

    fn add_deposits(&mut self, who: &Keypair, amount: u64) {
        let ix = self.ix(
            ipow_vault::ID,
            ipow_vault::client::accounts::AddDeposits { chain: chain_pda(&who.pubkey()), config: config(), operator: who.pubkey(), system_program: SYSTEM },
            ipow_vault::client::args::AddDeposits { amount },
        );
        self.send(ix, &[who]).unwrap();
    }

    fn add_bond(&mut self, who: &Keypair, amount: u64) -> Result<(), String> {
        let from = ata(&who.pubkey());
        let ix = self.ix(
            ipow_vault::ID,
            ipow_vault::client::accounts::AddBond { chain: chain_pda(&who.pubkey()), mint: mint(), from, holding: holding(), operator: who.pubkey(), token_program: TOKEN },
            ipow_vault::client::args::AddBond { amount },
        );
        self.send(ix, &[who])
    }

    /// Submits `m` of `operator`'s chain from `from`, with the accounts its
    /// home records need.
    fn submit(&mut self, operator: &Pubkey, m: &Msg, real: Ref, from: &Keypair, extra: &[Pubkey]) -> Result<(), String> {
        let (btc, walk) = self.btc(m, real);
        let id = self.vault_config().claim_count + 1;
        let mut ix = self.ix(
            ipow_vault::ID,
            ipow_vault::client::accounts::SubmitMessage {
                config: config(),
                chain: chain_pda(operator),
                real: real_pda(&real),
                walk,
                node: node_pda(&m.block),
                claim: claim_pda(id),
                stake: stake_pda(id, operator),
                operator_credit: credit_pda(operator),
                submitter_credit: credit_pda(&from.pubkey()),
                mint: mint(),
                holding: holding(),
                submitter: from.pubkey(),
                buffer: None,
                token_program: TOKEN,
                system_program: SYSTEM,
            },
            ipow_vault::client::args::SubmitMessage { operator: *operator, btc, input_index: 0, tag_index: 1, batch: m.batch.clone() },
        );
        for e in extra {
            ix.accounts.push(AccountMeta::new(*e, false));
        }
        self.send(ix, &[from])
    }

    fn side(&mut self, claim_id: u64, who: &Keypair, object: bool) -> Result<(), String> {
        let ix = if object {
            self.ix(
                ipow_vault::ID,
                ipow_vault::client::accounts::Object { claim: claim_pda(claim_id), stake: stake_pda(claim_id, &who.pubkey()), config: config(), who: who.pubkey(), system_program: SYSTEM },
                ipow_vault::client::args::Object { claim_id },
            )
        } else {
            self.ix(
                ipow_vault::ID,
                ipow_vault::client::accounts::Answer { claim: claim_pda(claim_id), stake: stake_pda(claim_id, &who.pubkey()), config: config(), who: who.pubkey(), system_program: SYSTEM },
                ipow_vault::client::args::Answer { claim_id },
            )
        };
        self.send(ix, &[who])
    }

    fn decide(&mut self, claim_id: u64) -> Result<(), String> {
        let operator = self.claim(claim_id).operator;
        let ix = self.ix(
            ipow_vault::ID,
            ipow_vault::client::accounts::Decide { claim: claim_pda(claim_id), chain: chain_pda(&operator), config: config() },
            ipow_vault::client::args::Decide { claim_id },
        );
        let payer = self.stranger.insecure_clone();
        self.send(ix, &[&payer])
    }

    fn collect(&mut self, claim_id: u64, who: &Keypair) -> Result<(), String> {
        let ix = self.ix(
            ipow_vault::ID,
            ipow_vault::client::accounts::Collect { claim: claim_pda(claim_id), stake: stake_pda(claim_id, &who.pubkey()), credit: credit_pda(&who.pubkey()), who: who.pubkey(), system_program: SYSTEM },
            ipow_vault::client::args::Collect { claim_id },
        );
        self.send(ix, &[who])
    }

    fn issue(&mut self, claim_id: u64, lock_id: u64, to: Pubkey) -> Result<(), String> {
        let payer = self.stranger.insecure_clone();
        let ix = self.ix(
            ipow_vault::ID,
            ipow_vault::client::accounts::Issue {
                claim: claim_pda(claim_id),
                mark: lock_pda(lock_id),
                config: config(),
                mint: mint(),
                holding: holding(),
                to,
                payer: payer.pubkey(),
                token_program: TOKEN,
                system_program: SYSTEM,
            },
            ipow_vault::client::args::Issue { claim_id, lock_id },
        );
        self.send(ix, &[&payer])
    }

    fn give_up(&mut self, claim_id: u64, lock_id: u64, who: &Keypair) -> Result<(), String> {
        let ix = self.ix(
            ipow_vault::ID,
            ipow_vault::client::accounts::GiveUp { claim: claim_pda(claim_id), mark: lock_pda(lock_id), recipient: who.pubkey(), system_program: SYSTEM },
            ipow_vault::client::args::GiveUp { claim_id, lock_id },
        );
        self.send(ix, &[who])
    }

    fn make_request(&mut self, who: &Keypair, amount: u64, fee: u64) -> Result<u64, String> {
        let id = self.vault_config().request_count + 1;
        let ix = self.ix(
            ipow_vault::ID,
            ipow_vault::client::accounts::MakeRequest {
                config: config(),
                request: request_pda(id),
                mint: mint(),
                from: ata(&who.pubkey()),
                holding: holding(),
                user: who.pubkey(),
                token_program: TOKEN,
                system_program: SYSTEM,
            },
            ipow_vault::client::args::MakeRequest { amount, to: ETH_USER, fee },
        );
        self.send(ix, &[who])?;
        Ok(id)
    }

    fn withdraw_credit(&mut self, who: &Keypair, to: Option<Pubkey>) -> Result<(), String> {
        let ix = self.ix(
            ipow_vault::ID,
            ipow_vault::client::accounts::WithdrawCredit {
                credit: credit_pda(&who.pubkey()),
                config: config(),
                mint: mint(),
                holding: holding(),
                to,
                owner: who.pubkey(),
                token_program: TOKEN,
            },
            ipow_vault::client::args::WithdrawCredit {},
        );
        self.send(ix, &[who])
    }

    /// The operator's chain registered, its messages `batches` written below
    /// a checkpoint, and lamports for 10 deposits. Returns the pair and the
    /// real block.
    fn ready(&mut self, batches: Vec<Vec<u8>>) -> (Pair, Ref) {
        let op = self.operator.insecure_clone();
        let mut pair = self.write_registration(&op);
        for b in batches {
            self.write(&mut pair, b, None);
        }
        let real = self.checkpoint();
        self.register(&pair, real).unwrap();
        self.add_deposits(&op, 10 * DEPOSIT);
        (pair, real)
    }

    /// Submits message `i` of the operator's chain as the guardian.
    fn sub(&mut self, pair: &Pair, i: usize, real: Ref, extra: &[Pubkey]) -> Result<(), String> {
        let g = self.guardian.insecure_clone();
        let op = pair.who.pubkey();
        self.submit(&op, &pair.msgs[i].clone(), real, &g, extra)
    }

    /// Makes the operator's ETH bond on Ethereum count here: message 0 must
    /// be a BOND for Ethereum. Returns its claim.
    fn peer_bond(&mut self, pair: &Pair, real: Ref) -> u64 {
        self.sub(pair, 0, real, &[]).unwrap();
        let id = self.vault_config().claim_count;
        self.later(WEEK);
        self.decide(id).unwrap();
        id
    }
}

// ---------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------

#[test]
fn issues_veth_for_a_lock_after_7_days_once_and_never_for_a_given_up_lock() {
    let mut w = World::new();
    let user = w.user.insecure_clone();
    let stranger = w.stranger.insecure_clone();
    let (pair, real) = w.ready(vec![
        bond_rec(ETHEREUM, 10 * GWEI_PER_ETH),
        [lock_rec(1, GWEI_PER_ETH, &user.pubkey(), 0), lock_rec(2, GWEI_PER_ETH, &stranger.pubkey(), 0)].concat(),
    ]);
    w.peer_bond(&pair, real);
    assert_eq!(w.chain(&w.operator.pubkey()).peer_bond, 10 * GWEI_PER_ETH);
    w.sub(&pair, 1, real, &[]).unwrap();
    let id = w.vault_config().claim_count;
    let to = w.veth_account(&user.pubkey());
    expect_err(w.issue(id, 1, to), "NotAccepted");
    // Not before the claim is accepted: it may carry a false LOCK record.
    expect_err(w.give_up(id, 2, &stranger), "NotAccepted");
    w.later(WEEK);
    w.decide(id).unwrap();
    // The recipient of lock #2 gives it up.
    w.give_up(id, 2, &stranger).unwrap();
    expect_err(w.give_up(id, 1, &stranger), "WrongAccount");
    // Not to someone else's account.
    let guardian = w.guardian.pubkey();
    let other = w.veth_account(&guardian);
    expect_err(w.issue(id, 1, other), "WrongAccount");
    w.issue(id, 1, to).unwrap();
    assert_eq!(w.tokens(&to), GWEI_PER_ETH);
    assert!(w.issue(id, 1, to).is_err());
    let to2 = w.veth_account(&stranger.pubkey());
    assert!(w.issue(id, 2, to2).is_err());
}

#[test]
fn refuses_a_forged_message_and_keeps_the_order() {
    let mut w = World::new();
    let op = w.operator.insecure_clone();
    let g = w.guardian.insecure_clone();
    let mut pair = w.write_registration(&op);
    let reg = pair.registration.block;
    w.write(&mut pair, bond_rec(ETHEREUM, 1), None);
    w.write(&mut pair, bond_rec(ETHEREUM, 1), None);
    let real = w.checkpoint();
    w.register(&pair, real).unwrap();
    // Out of order.
    expect_err(w.sub(&pair, 1, real, &[]), "WrongCoin");
    // Mallory's own block off the main chain, with a message "from" the
    // operator: not below any real block.
    let mut fake = Pair { who: op.insecure_clone(), coin: pair.coin, registration: pair.registration.clone(), msgs: vec![] };
    fake.coin = (sha256d(&pair.registration.tx), 0);
    let forged = w.write(&mut fake, exit_rec(), Some(reg));
    let (siblings, index) = w.proof_of(&forged.block, &forged.tx);
    // The only walks from the real block go down the main chain; one to
    // the forged block's parent does not reach the forged block.
    let walk = w.main_walk(real, reg);
    let ix = w.ix(
        ipow_vault::ID,
        ipow_vault::client::accounts::SubmitMessage {
            config: config(),
            chain: chain_pda(&op.pubkey()),
            real: real_pda(&real),
            walk,
            node: node_pda(&forged.block),
            claim: claim_pda(1),
            stake: stake_pda(1, &op.pubkey()),
            operator_credit: credit_pda(&op.pubkey()),
            submitter_credit: credit_pda(&g.pubkey()),
            mint: mint(),
            holding: holding(),
            submitter: g.pubkey(),
            buffer: None,
            token_program: TOKEN,
            system_program: SYSTEM,
        },
        ipow_vault::client::args::SubmitMessage {
            operator: op.pubkey(),
            btc: ipow_vault::types::Btc { block: forged.block.v(), raw_tx: forged.tx.clone(), siblings, tx_index: index, real: real.v() },
            input_index: 0,
            tag_index: 1,
            batch: forged.batch.clone(),
        },
    );
    expect_err(w.send(ix, &[&g]), "NotReal");
    w.sub(&pair, 0, real, &[]).unwrap();
    expect_err(w.sub(&pair, 0, real, &[]), "WrongCoin");
    w.sub(&pair, 1, real, &[]).unwrap();
    assert_eq!(w.chain(&op.pubkey()).messages, 2);
}

#[test]
fn opens_no_claim_past_80_percent_of_the_bond_on_ethereum() {
    let mut w = World::new();
    let user = w.user.insecure_clone();
    let (pair, real) = w.ready(vec![
        lock_rec(1, GWEI_PER_ETH, &user.pubkey(), 0),
        bond_rec(ETHEREUM, 10 * GWEI_PER_ETH),
        lock_rec(2, 8 * GWEI_PER_ETH + 1, &user.pubkey(), 0),
        lock_rec(3, 8 * GWEI_PER_ETH, &user.pubkey(), 0),
    ]);
    // No bond counts yet: no claim.
    w.sub(&pair, 0, real, &[]).unwrap();
    assert_eq!(w.vault_config().claim_count, 0);
    w.sub(&pair, 1, real, &[]).unwrap();
    w.later(WEEK);
    w.decide(1).unwrap();
    w.sub(&pair, 2, real, &[]).unwrap();
    assert_eq!(w.vault_config().claim_count, 1);
    w.sub(&pair, 3, real, &[]).unwrap();
    assert_eq!(w.vault_config().claim_count, 2);
}

#[test]
fn an_objection_that_stands_refuses_the_claim_and_every_other_one_of_the_chain() {
    let mut w = World::new();
    let user = w.user.insecure_clone();
    let g = w.guardian.insecure_clone();
    let (pair, real) = w.ready(vec![
        bond_rec(ETHEREUM, 10 * GWEI_PER_ETH),
        lock_rec(1, GWEI_PER_ETH, &user.pubkey(), 0),
        lock_rec(2, GWEI_PER_ETH, &user.pubkey(), 0),
    ]);
    w.peer_bond(&pair, real);
    w.sub(&pair, 1, real, &[]).unwrap(); // claim 2
    w.sub(&pair, 2, real, &[]).unwrap(); // claim 3
    w.side(2, &g, true).unwrap();
    expect_err(w.side(2, &g, true), "AlreadyHeld");
    w.later(WEEK);
    w.decide(2).unwrap();
    w.decide(3).unwrap();
    assert!(!w.claim(2).accepted);
    assert!(!w.claim(3).accepted);
    let to = w.veth_account(&user.pubkey());
    expect_err(w.issue(2, 1, to), "NotAccepted");
    // The objector collects its deposit and the operator's.
    w.collect(2, &g).unwrap();
    assert_eq!(w.credit(&g.pubkey()).0, 2 * DEPOSIT);
    expect_err(w.collect(2, &g), "NothingToCollect");
    let op = w.operator.insecure_clone();
    expect_err(w.collect(2, &op), "NothingToCollect");
    let before = w.ctx.svm.get_balance(&g.pubkey()).unwrap();
    w.withdraw_credit(&g, None).unwrap();
    // Less the transaction's fee, which the guardian paid.
    let after = w.ctx.svm.get_balance(&g.pubkey()).unwrap();
    assert!(after > before + 2 * DEPOSIT - 10_000 && after <= before + 2 * DEPOSIT);
}

#[test]
fn an_answer_restarts_the_7_days_and_the_answering_side_wins() {
    let mut w = World::new();
    let user = w.user.insecure_clone();
    let g = w.guardian.insecure_clone();
    let (pair, real) = w.ready(vec![bond_rec(ETHEREUM, 10 * GWEI_PER_ETH), lock_rec(1, GWEI_PER_ETH, &user.pubkey(), 0)]);
    w.peer_bond(&pair, real);
    w.sub(&pair, 1, real, &[]).unwrap();
    w.later(6 * DAY);
    w.side(2, &g, true).unwrap();
    w.later(6 * DAY);
    w.side(2, &user, false).unwrap();
    w.later(6 * DAY);
    expect_err(w.decide(2), "WindowNotOver");
    w.later(DAY);
    expect_err(w.side(2, &g, true), "WindowOver");
    w.decide(2).unwrap();
    assert!(w.claim(2).accepted);
    // Two answers share the objection.
    w.collect(2, &user).unwrap();
    assert_eq!(w.credit(&user.pubkey()).0, DEPOSIT + DEPOSIT / 2);
}

/// The user holds `amount` vETH: lock #1 issued to it. Messages 0 and 1 of
/// the chain are the BOND for Ethereum and that LOCK.
fn with_veth(w: &mut World, amount: u64, more: Vec<Vec<u8>>) -> (Pair, Ref, Pubkey) {
    let user = w.user.insecure_clone();
    let mut batches = vec![bond_rec(ETHEREUM, 100 * GWEI_PER_ETH), lock_rec(1, amount, &user.pubkey(), 0)];
    batches.extend(more);
    let (pair, real) = w.ready(batches);
    w.peer_bond(&pair, real);
    w.sub(&pair, 1, real, &[]).unwrap();
    let id = w.vault_config().claim_count;
    w.later(WEEK);
    w.decide(id).unwrap();
    let to = w.veth_account(&user.pubkey());
    w.issue(id, 1, to).unwrap();
    (pair, real, to)
}

#[test]
fn a_true_request_earns_its_fee_once_and_a_false_one_slashes_the_veth_bond() {
    let mut w = World::new();
    let op = w.operator.insecure_clone();
    let user = w.user.insecure_clone();
    let g = w.guardian.insecure_clone();
    let (pair, real, user_veth) = with_veth(
        &mut w,
        10 * GWEI_PER_ETH,
        vec![
            bond_rec(SOLANA, 3 * GWEI_PER_ETH),
            request_rec(1, GWEI_PER_ETH, &ETH_USER, 5),
            request_rec(1, GWEI_PER_ETH, &ETH_USER, 5),
            request_rec(2, 2, &ETH_USER, 0), // wrong amount
        ],
    );
    // The operator's vETH bond: 4 vETH from the user.
    let op_veth = w.veth_account(&op.pubkey());
    token_transfer(&mut w, &user, user_veth, op_veth, 4 * GWEI_PER_ETH);
    w.add_bond(&op, 4 * GWEI_PER_ETH).unwrap();
    w.sub(&pair, 2, real, &[]).unwrap();
    assert_eq!(w.chain(&op.pubkey()).stated, 3 * GWEI_PER_ETH);

    let r1 = w.make_request(&user, GWEI_PER_ETH, 5).unwrap();
    let r2 = w.make_request(&user, 1, 0).unwrap();
    w.sub(&pair, 3, real, &[request_pda(r1)]).unwrap();
    assert_eq!(w.credit(&op.pubkey()).1, 5);
    w.sub(&pair, 4, real, &[request_pda(r1)]).unwrap();
    assert_eq!(w.credit(&op.pubkey()).1, 5);

    let supply = w.supply();
    w.sub(&pair, 5, real, &[request_pda(r2)]).unwrap();
    let c = w.chain(&op.pubkey());
    assert!(c.slashed);
    assert_eq!(c.bond, 0);
    // 20% to the guardian, 80% burned.
    assert_eq!(w.credit(&g.pubkey()).1, 4 * GWEI_PER_ETH / 5);
    assert_eq!(w.supply(), supply - 4 * GWEI_PER_ETH * 4 / 5);
    let g_veth = w.veth_account(&g.pubkey());
    w.withdraw_credit(&g, Some(g_veth)).unwrap();
    assert_eq!(w.tokens(&g_veth), 4 * GWEI_PER_ETH / 5);
}

fn token_transfer(w: &mut World, owner: &Keypair, from: Pubkey, to: Pubkey, amount: u64) {
    let mut data = vec![3u8];
    data.extend_from_slice(&amount.to_le_bytes());
    let ix = Instruction {
        program_id: TOKEN,
        accounts: vec![AccountMeta::new(from, false), AccountMeta::new(to, false), AccountMeta::new_readonly(owner.pubkey(), true)],
        data,
    };
    w.send(ix, &[owner]).unwrap();
}

#[test]
fn a_cancel_is_true_only_for_a_lock_given_up_here() {
    let mut w = World::new();
    let op = w.operator.insecure_clone();
    let user = w.user.insecure_clone();
    let (pair, real) = w.ready(vec![
        bond_rec(ETHEREUM, 10 * GWEI_PER_ETH),
        lock_rec(1, GWEI_PER_ETH, &user.pubkey(), 0),
        cancel_rec(1),
        cancel_rec(2),
    ]);
    w.peer_bond(&pair, real);
    w.sub(&pair, 1, real, &[]).unwrap();
    w.later(WEEK);
    w.decide(2).unwrap();
    w.give_up(2, 1, &user).unwrap();
    w.sub(&pair, 2, real, &[lock_pda(1)]).unwrap();
    assert!(!w.chain(&op.pubkey()).slashed);
    w.sub(&pair, 3, real, &[lock_pda(2)]).unwrap();
    assert!(w.chain(&op.pubkey()).slashed);
}

#[test]
fn keeps_a_stated_bond_locked_and_frees_it_after_exit() {
    let mut w = World::new();
    let op = w.operator.insecure_clone();
    let user = w.user.insecure_clone();
    let (pair, real, user_veth) = with_veth(
        &mut w,
        10 * GWEI_PER_ETH,
        vec![bond_rec(SOLANA, 2 * GWEI_PER_ETH), exit_rec(), bond_rec(SOLANA, 1)],
    );
    let op_veth = w.veth_account(&op.pubkey());
    token_transfer(&mut w, &user, user_veth, op_veth, 3 * GWEI_PER_ETH);
    w.add_bond(&op, 3 * GWEI_PER_ETH).unwrap();
    w.sub(&pair, 2, real, &[]).unwrap();
    let withdraw = |w: &mut World, amount: u64| {
        let ix = w.ix(
            ipow_vault::ID,
            ipow_vault::client::accounts::WithdrawBond {
                chain: chain_pda(&op.pubkey()),
                config: config(),
                mint: mint(),
                holding: holding(),
                to: op_veth,
                operator: op.pubkey(),
                token_program: TOKEN,
            },
            ipow_vault::client::args::WithdrawBond { amount },
        );
        w.send(ix, &[&op])
    };
    expect_err(withdraw(&mut w, GWEI_PER_ETH + 1), "BondNotFree");
    withdraw(&mut w, GWEI_PER_ETH).unwrap();
    w.sub(&pair, 3, real, &[]).unwrap();
    expect_err(w.sub(&pair, 4, real, &[]), "ChainEnded");
    withdraw(&mut w, 2 * GWEI_PER_ETH).unwrap();
    assert_eq!(w.tokens(&op_veth), 3 * GWEI_PER_ETH);
}

#[test]
fn slashes_a_bond_for_solana_above_the_bond_and_a_batch_that_does_not_parse() {
    let mut w = World::new();
    let op = w.operator.insecure_clone();
    let (pair, real) = w.ready(vec![bond_rec(SOLANA, 1)]);
    w.sub(&pair, 0, real, &[]).unwrap();
    assert!(w.chain(&op.pubkey()).slashed);

    let mut w = World::new();
    let op = w.operator.insecure_clone();
    let (pair, real) = w.ready(vec![vec![9, 1]]);
    w.sub(&pair, 0, real, &[]).unwrap();
    assert!(w.chain(&op.pubkey()).slashed);
}

#[test]
fn a_bond_for_ethereum_is_carried_again_when_its_claim_could_not_open() {
    let mut w = World::new();
    let op = w.operator.insecure_clone();
    let user = w.user.insecure_clone();
    let (pair, real) = w.ready(vec![
        [bond_rec(ETHEREUM, 10 * GWEI_PER_ETH), lock_rec(1, GWEI_PER_ETH, &user.pubkey(), 0)].concat(),
        bond_rec(ETHEREUM, 10 * GWEI_PER_ETH),
        bond_rec(ETHEREUM, 20 * GWEI_PER_ETH),
    ]);
    w.sub(&pair, 0, real, &[]).unwrap();
    assert_eq!(w.vault_config().claim_count, 0);
    w.sub(&pair, 1, real, &[]).unwrap();
    assert_eq!(w.vault_config().claim_count, 1);
    // A later one is not acted on here, and not slashed here.
    w.sub(&pair, 2, real, &[]).unwrap();
    assert_eq!(w.vault_config().claim_count, 1);
    w.later(WEEK);
    w.decide(1).unwrap();
    let c = w.chain(&op.pubkey());
    assert!(!c.slashed);
    assert_eq!(c.peer_bond, 10 * GWEI_PER_ETH);
}

#[test]
fn certifies_no_job_below_the_minimum_escrow() {
    let mut w = World::new();
    // Mallory's own application opens a cheap job, and her operator proves it.
    let mallory = w.stranger.insecure_clone();
    let ix = w.ix(
        ipow_protocol::ID,
        ipow_protocol::client::accounts::RegisterApplication {
            application: pr(&[b"application", mallory.pubkey().as_ref()]),
            key: mallory.pubkey(),
            funder: mallory.pubkey(),
            system_program: SYSTEM,
        },
        ipow_protocol::client::args::RegisterApplication { challenge_periods: vec![] },
    );
    w.send(ix, &[&mallory]).unwrap();
    let tag = [7u8; 32];
    let job_id = w.job_count() + 1;
    let ix = w.ix(
        ipow_protocol::ID,
        ipow_protocol::client::accounts::OpenJob {
            protocol: pr(&[b"protocol"]),
            application: pr(&[b"application", mallory.pubkey().as_ref()]),
            job: job_pda(job_id),
            tag_record: pr(&[b"tag", mallory.pubkey().as_ref(), &tag]),
            vault: pr(&[b"vault"]),
            key: mallory.pubkey(),
            funder: mallory.pubkey(),
            system_program: SYSTEM,
        },
        ipow_protocol::client::args::OpenJob {
            tag,
            escrow: SOL / 10,
            escrow_fee_bps: 50,
            confirmations: 6,
            claim_kind: 0,
            payer: mallory.pubkey(),
            paid: FEES,
        },
    );
    w.send(ix, &[&mallory]).unwrap();
    w.prove_job(job_id, tag, mallory.pubkey());
    let end = w.job(job_id).lock_end;
    w.set_now(end + 1);
    expect_err(w.record_real_from_job(job_id), "EscrowTooLow");
}

fn buffer_pda(owner: &Pubkey) -> Pubkey {
    vt(&[b"buffer", owner.as_ref()])
}

#[test]
fn a_large_message_goes_through_a_buffer_and_one_past_the_limits_cannot_be_uploaded() {
    let mut w = World::new();
    let op = w.operator.insecure_clone();
    let g = w.guardian.insecure_clone();
    let user = w.user.insecure_clone();
    let locks: Vec<u8> = (1..=10).flat_map(|i| lock_rec(i, GWEI_PER_ETH, &user.pubkey(), 0)).collect();
    let (pair, real) = w.ready(vec![bond_rec(ETHEREUM, 100 * GWEI_PER_ETH), locks]);
    w.peer_bond(&pair, real);

    // 570 bytes of records do not fit in one Solana transaction with a
    // proof from a full block. The test Solana does not enforce the size,
    // so the buffer's path is shown directly.
    let m = pair.msgs[1].clone();

    let open = w.ix(
        ipow_vault::ID,
        ipow_vault::client::accounts::OpenBuffer { buffer: buffer_pda(&g.pubkey()), owner: g.pubkey(), system_program: SYSTEM },
        ipow_vault::client::args::OpenBuffer {},
    );
    w.send(open, &[&g]).unwrap();
    let write = |w: &mut World, raw: bool, data: Vec<u8>| {
        let ix = w.ix(
            ipow_vault::ID,
            ipow_vault::client::accounts::WriteBuffer { buffer: buffer_pda(&g.pubkey()), owner: g.pubkey() },
            ipow_vault::client::args::WriteBuffer { raw, data },
        );
        w.send(ix, &[&g])
    };
    write(&mut w, true, m.tx.clone()).unwrap();
    for chunk in m.batch.chunks(300) {
        write(&mut w, false, chunk.to_vec()).unwrap();
    }
    let (mut btc, walk) = w.btc(&m, real);
    btc.raw_tx = vec![];
    let id = w.vault_config().claim_count + 1;
    let ix = w.ix(
        ipow_vault::ID,
        ipow_vault::client::accounts::SubmitMessage {
            config: config(),
            chain: chain_pda(&op.pubkey()),
            real: real_pda(&real),
            walk,
            node: node_pda(&m.block),
            claim: claim_pda(id),
            stake: stake_pda(id, &op.pubkey()),
            operator_credit: credit_pda(&op.pubkey()),
            submitter_credit: credit_pda(&g.pubkey()),
            mint: mint(),
            holding: holding(),
            submitter: g.pubkey(),
            buffer: Some(buffer_pda(&g.pubkey())),
            token_program: TOKEN,
            system_program: SYSTEM,
        },
        ipow_vault::client::args::SubmitMessage { operator: op.pubkey(), btc, input_index: 0, tag_index: 1, batch: vec![] },
    );
    w.send(ix, &[&g]).unwrap();
    assert_eq!(w.claim(id).records.len(), 570);

    // Past the limits of section 11.3, a buffer takes nothing more.
    expect_err(write(&mut w, false, vec![0; 2048 - 570 + 1]), "TooLarge");
    expect_err(write(&mut w, true, vec![0; 1024]), "TooLarge");
    let close = w.ix(
        ipow_vault::ID,
        ipow_vault::client::accounts::CloseBuffer { buffer: buffer_pda(&g.pubkey()), owner: g.pubkey() },
        ipow_vault::client::args::CloseBuffer {},
    );
    w.send(close, &[&g]).unwrap();
    assert!(w.ctx.svm.get_account(&buffer_pda(&g.pubkey())).map_or(true, |a| a.lamports == 0));
}

#[test]
fn burns_the_whole_bond_when_the_operator_submits_its_own_lie_and_refuses_its_open_claim() {
    let mut w = World::new();
    let op = w.operator.insecure_clone();
    let user = w.user.insecure_clone();
    let (pair, real, user_veth) = with_veth(
        &mut w,
        10 * GWEI_PER_ETH,
        vec![lock_rec(2, GWEI_PER_ETH, &user.pubkey(), 0), request_rec(9, 1, &ETH_USER, 0)],
    );
    let op_veth = w.veth_account(&op.pubkey());
    token_transfer(&mut w, &user, user_veth, op_veth, 2 * GWEI_PER_ETH);
    w.add_bond(&op, 2 * GWEI_PER_ETH).unwrap();
    w.sub(&pair, 2, real, &[]).unwrap();
    let open = w.vault_config().claim_count;
    let supply = w.supply();
    // No request #9 exists: the operator submits its own false message.
    w.submit(&op.pubkey(), &pair.msgs[3].clone(), real, &op, &[request_pda(9)]).unwrap();
    assert!(w.chain(&op.pubkey()).slashed);
    assert_eq!(w.supply(), supply - 2 * GWEI_PER_ETH);
    assert_eq!(w.credit(&op.pubkey()).1, 0);
    // Its open claim is refused with no objection; nobody wins its deposit.
    w.later(WEEK);
    w.decide(open).unwrap();
    assert!(!w.claim(open).accepted);
    expect_err(w.collect(open, &op), "NothingToCollect");
}

#[test]
fn takes_the_same_bond_for_solana_again_as_true_and_another_amount_as_false() {
    let mut w = World::new();
    let op = w.operator.insecure_clone();
    let user = w.user.insecure_clone();
    let (pair, real, user_veth) = with_veth(
        &mut w,
        10 * GWEI_PER_ETH,
        vec![bond_rec(SOLANA, GWEI_PER_ETH), bond_rec(SOLANA, GWEI_PER_ETH), bond_rec(SOLANA, 2 * GWEI_PER_ETH)],
    );
    let op_veth = w.veth_account(&op.pubkey());
    token_transfer(&mut w, &user, user_veth, op_veth, 3 * GWEI_PER_ETH);
    w.add_bond(&op, 3 * GWEI_PER_ETH).unwrap();
    w.sub(&pair, 2, real, &[]).unwrap();
    w.sub(&pair, 3, real, &[]).unwrap();
    assert!(!w.chain(&op.pubkey()).slashed);
    assert_eq!(w.chain(&op.pubkey()).stated, GWEI_PER_ETH);
    w.sub(&pair, 4, real, &[]).unwrap();
    assert!(w.chain(&op.pubkey()).slashed);
}
