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
/// What a transaction costs its signer.
const TX_FEE: u64 = 5_000;
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
// Records (section 11.9): amounts in record units, big-endian
// ---------------------------------------------------------------------

/// The time of the locks on Ethereum in the tests' records: late enough
/// that an attest earns the whole fast fee, unless a test moves the clock
/// there (D124).
const LOCKED_AT: i64 = T0 + 60 * DAY;
/// The time of the burns here: the test moves the clock there before one.
const BURNED_AT: i64 = T0 + 50 * DAY;
/// An Ethereum address in 32 bytes.
const ETH_USER32: [u8; 32] = {
    let mut a = [0u8; 32];
    let mut i = 12;
    while i < 32 {
        a[i] = 0xbb;
        i += 1;
    }
    a
};

#[allow(clippy::too_many_arguments)]
fn lock_rec(home: u8, asset: u32, id: u64, amount: u64, recipient: &[u8; 32], fee: u64, fast_fee: u64, at: i64) -> Vec<u8> {
    [
        &[1u8, home][..],
        &asset.to_be_bytes(),
        &id.to_be_bytes(),
        &amount.to_be_bytes(),
        recipient,
        &fee.to_be_bytes(),
        &fast_fee.to_be_bytes(),
        &at.to_be_bytes(),
    ]
    .concat()
}
#[allow(clippy::too_many_arguments)]
fn request_rec(net: u8, asset: u32, id: u64, amount: u64, to: &[u8; 32], fee: u64, fast_fee: u64, at: i64) -> Vec<u8> {
    [&[2u8, net][..], &asset.to_be_bytes(), &id.to_be_bytes(), &amount.to_be_bytes(), to, &fee.to_be_bytes(), &fast_fee.to_be_bytes(), &at.to_be_bytes()].concat()
}
fn cancel_rec(net: u8, id: u64) -> Vec<u8> {
    [&[3u8, net][..], &id.to_be_bytes()].concat()
}
fn bond_rec(net: u8, home: u8, asset: u32, amount: u64) -> Vec<u8> {
    [&[4u8, net, home][..], &asset.to_be_bytes(), &amount.to_be_bytes()].concat()
}
fn asset_rec(home: u8, asset: u32, token: &[u8; 32], decimals: u8) -> Vec<u8> {
    [&[6u8, home][..], &asset.to_be_bytes(), token, &[decimals]].concat()
}
fn exit_rec() -> Vec<u8> {
    vec![5u8]
}
/// A lock of ETH on Ethereum for vETH to `recipient` here.
fn eth_lock(id: u64, amount: u64, recipient: &Pubkey, fee: u64, fast_fee: u64) -> Vec<u8> {
    lock_rec(ETHEREUM, 0, id, amount, &recipient.to_bytes(), fee, fast_fee, LOCKED_AT)
}
/// A burn here of vETH for ETH to ETH_USER, made at BURNED_AT.
fn veth_burn(id: u64, amount: u64, fee: u64, fast_fee: u64) -> Vec<u8> {
    request_rec(SOLANA, 0, id, amount, &ETH_USER32, fee, fast_fee, BURNED_AT)
}
/// vETH's ASSET record: ETH, asset 0 of Ethereum, with 9 decimals.
fn veth_asset() -> Vec<u8> {
    asset_rec(ETHEREUM, 0, &[0u8; 32], 9)
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
/// The receipt here of Ethereum's asset `n`, and the vault's account of it.
fn receipt(n: u32) -> Pubkey {
    vt(&[b"receipt", &n.to_le_bytes()])
}
fn holding(n: u32) -> Pubkey {
    vt(&[b"holding", &n.to_le_bytes()])
}
/// vETH.
fn mint() -> Pubkey {
    receipt(0)
}
fn asset_pda(n: u32) -> Pubkey {
    vt(&[b"asset", &n.to_le_bytes()])
}
fn home_lock_pda(id: u64) -> Pubkey {
    vt(&[b"home_lock", &id.to_le_bytes()])
}
fn paid_pda(id: u64) -> Pubkey {
    vt(&[b"paid", &id.to_le_bytes()])
}
fn fast_pay_pda(id: u64, record: &[u8]) -> Pubkey {
    vt(&[b"fast_pay", &id.to_le_bytes(), &sha256(&[record])])
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
fn credit_pda(who: &Pubkey, home: u8, asset: u32) -> Pubkey {
    vt(&[b"credit", who.as_ref(), &[home], &asset.to_le_bytes()])
}
fn request_pda(id: u64) -> Pubkey {
    vt(&[b"request", &id.to_le_bytes()])
}
fn lock_pda(id: u64) -> Pubkey {
    vt(&[b"lock", &id.to_le_bytes()])
}
fn fast_pda(id: u64) -> Pubkey {
    vt(&[b"fast", &id.to_le_bytes()])
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
                    sol: asset_pda(0),
                    application: pr(&[b"application", config().as_ref()]),
                    payer: payer.pubkey(),
                    program_data: SYSTEM,
                    protocol_program: ipow_protocol::ID,
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
    /// What `who` can withdraw in an asset.
    fn credit(&self, who: &Pubkey, home: u8, asset: u32) -> u64 {
        match self.ctx.svm.get_account(&credit_pda(who, home, asset)) {
            Some(a) if !a.data.is_empty() => ipow_vault::accounts::Credit::try_deserialize(&mut &a.data[..]).unwrap().amount,
            _ => 0,
        }
    }
    fn home_asset(&self, n: u32) -> ipow_vault::accounts::HomeAsset {
        self.ctx.get_account(&asset_pda(n)).unwrap()
    }
    fn lamports(&self, who: &Pubkey) -> u64 {
        self.ctx.svm.get_account(who).map_or(0, |a| a.lamports)
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

    /// Submits `m` of `operator`'s chain from `from`, with the accounts its
    /// records need, in the order of the batch.
    fn submit(&mut self, operator: &Pubkey, m: &Msg, real: Ref, from: &Keypair, extra: &[Pubkey]) -> Result<(), String> {
        let (btc, walk) = self.btc(m, real);
        let id = self.vault_config().claim_count + 1;
        let index = self.chain(operator).messages;
        let mut ix = self.ix(
            ipow_vault::ID,
            ipow_vault::client::accounts::SubmitMessage {
                config: config(),
                chain: chain_pda(operator),
                real: real_pda(&real),
                walk,
                node: node_pda(&m.block),
                message: message_pda(operator, index),
                claim: claim_pda(id),
                stake: stake_pda(id, operator),
                submitter: from.pubkey(),
                buffer: None,
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
            ipow_vault::client::accounts::Collect {
                claim: claim_pda(claim_id),
                stake: stake_pda(claim_id, &who.pubkey()),
                credit: credit_pda(&who.pubkey(), SOLANA, 0),
                who: who.pubkey(),
                system_program: SYSTEM,
            },
            ipow_vault::client::args::Collect { claim_id },
        );
        self.send(ix, &[who])
    }

    fn make_receipt(&mut self, claim_id: u64, asset: u32, record: Vec<u8>) -> Result<(), String> {
        let payer = self.stranger.insecure_clone();
        let ix = self.ix(
            ipow_vault::ID,
            ipow_vault::client::accounts::MakeReceipt {
                config: config(),
                claim: claim_pda(claim_id),
                mint: receipt(asset),
                holding: holding(asset),
                payer: payer.pubkey(),
                token_program: TOKEN,
                system_program: SYSTEM,
            },
            ipow_vault::client::args::MakeReceipt { claim_id, asset, record },
        );
        self.send(ix, &[&payer])
    }

    fn issue(&mut self, claim_id: u64, record: &[u8], to: Pubkey) -> Result<(), String> {
        let payer = self.stranger.insecure_clone();
        let (lock_id, asset) = (u64::from_be_bytes(record[6..14].try_into().unwrap()), u32::from_be_bytes(record[2..6].try_into().unwrap()));
        let ix = self.ix(
            ipow_vault::ID,
            ipow_vault::client::accounts::Issue {
                claim: claim_pda(claim_id),
                mark: lock_pda(lock_id),
                config: config(),
                mint: receipt(asset),
                holding: holding(asset),
                to,
                payer: payer.pubkey(),
                token_program: TOKEN,
                system_program: SYSTEM,
            },
            ipow_vault::client::args::Issue { claim_id, lock_id, asset, record: record.to_vec() },
        );
        self.send(ix, &[&payer])
    }

    fn give_up(&mut self, claim_id: u64, record: &[u8], who: &Keypair) -> Result<(), String> {
        let lock_id = u64::from_be_bytes(record[6..14].try_into().unwrap());
        let ix = self.ix(
            ipow_vault::ID,
            ipow_vault::client::accounts::GiveUp { claim: claim_pda(claim_id), mark: lock_pda(lock_id), recipient: who.pubkey(), system_program: SYSTEM },
            ipow_vault::client::args::GiveUp { claim_id, lock_id, record: record.to_vec() },
        );
        self.send(ix, &[who])
    }

    /// Burns vETH for ETH to ETH_USER; returns the burn's number.
    fn burn(&mut self, who: &Keypair, amount: u64, fee: u64, fast_fee: u64) -> Result<u64, String> {
        let id = self.vault_config().request_count + 1;
        let ix = self.ix(
            ipow_vault::ID,
            ipow_vault::client::accounts::MakeRequest {
                config: config(),
                request: request_pda(id),
                mint: receipt(0),
                from: ata(&who.pubkey()),
                holding: holding(0),
                user: who.pubkey(),
                token_program: TOKEN,
                system_program: SYSTEM,
            },
            ipow_vault::client::args::MakeRequest { asset: 0, amount, to: ETH_USER32, fee, fast_fee },
        );
        self.send(ix, &[who])?;
        Ok(id)
    }

    fn take_request_fee(&mut self, who: &Keypair, id: u64) -> Result<(), String> {
        let to = ata(&who.pubkey());
        let ix = self.ix(
            ipow_vault::ID,
            ipow_vault::client::accounts::TakeRequestFee {
                config: config(),
                request: request_pda(id),
                mint: receipt(0),
                holding: holding(0),
                to,
                operator: who.pubkey(),
                token_program: TOKEN,
            },
            ipow_vault::client::args::TakeRequestFee { request_id: id },
        );
        self.send(ix, &[who])
    }

    /// Locks `amount` lamports of SOL for its receipt on Ethereum, to
    /// ETH_USER; returns the lock's number.
    fn lock_sol(&mut self, who: &Keypair, amount: u64, fee: u64, fast_fee: u64) -> Result<u64, String> {
        let id = self.vault_config().lock_count + 1;
        let ix = self.ix(
            ipow_vault::ID,
            ipow_vault::client::accounts::Lock {
                config: config(),
                home_asset: asset_pda(0),
                lock: home_lock_pda(id),
                from: None,
                tokens: None,
                mint: None,
                token_program: None,
                user: who.pubkey(),
                system_program: SYSTEM,
            },
            ipow_vault::client::args::Lock { asset: 0, recipient: ETH_USER32, amount, fee, fast_fee },
        );
        self.send(ix, &[who])?;
        Ok(id)
    }

    /// The LOCK record of lock `id` here.
    fn sol_lock_rec(&self, id: u64) -> Vec<u8> {
        let l: ipow_vault::accounts::HomeLock = self.ctx.get_account(&home_lock_pda(id)).unwrap();
        lock_rec(SOLANA, l.asset, id, l.amount, &l.recipient, l.fee, l.fast_fee, l.locked_at)
    }

    fn take_lock_fee(&mut self, who: &Keypair, id: u64) -> Result<(), String> {
        let ix = self.ix(
            ipow_vault::ID,
            ipow_vault::client::accounts::TakeLockFee {
                config: config(),
                lock: home_lock_pda(id),
                asset: asset_pda(0),
                to: None,
                tokens: None,
                mint: None,
                token_program: None,
                operator: who.pubkey(),
            },
            ipow_vault::client::args::TakeLockFee { lock_id: id },
        );
        self.send(ix, &[who])
    }

    fn return_lock(&mut self, claim_id: u64, lock_id: u64, owner: &Pubkey) -> Result<(), String> {
        let payer = self.stranger.insecure_clone();
        let ix = self.ix(
            ipow_vault::ID,
            ipow_vault::client::accounts::ReturnLock {
                config: config(),
                claim: claim_pda(claim_id),
                lock: home_lock_pda(lock_id),
                asset: asset_pda(0),
                owner: *owner,
                to: None,
                tokens: None,
                mint: None,
                token_program: None,
            },
            ipow_vault::client::args::ReturnLock { claim_id, lock_id, record: cancel_rec(ETHEREUM, lock_id) },
        );
        self.send(ix, &[&payer])
    }

    /// Pays a burn on Ethereum of vSOL at once, in SOL, to its address.
    fn fast_pay(&mut self, attester: &Keypair, record: &[u8]) -> Result<(), String> {
        let id = u64::from_be_bytes(record[6..14].try_into().unwrap());
        let to = Pubkey::new_from_array(record[22..54].try_into().unwrap());
        let ix = self.ix(
            ipow_vault::ID,
            ipow_vault::client::accounts::FastPay {
                home_asset: asset_pda(0),
                fast_pay: fast_pay_pda(id, record),
                paid: paid_pda(id),
                to,
                from: None,
                mint: None,
                token_program: None,
                attester: attester.pubkey(),
                system_program: SYSTEM,
            },
            ipow_vault::client::args::FastPay { request_id: id, asset: 0, record_hash: sha256(&[record]), record: record.to_vec() },
        );
        self.send(ix, &[attester])
    }

    /// Pays a burn on Ethereum of vSOL carried by an accepted claim, in SOL.
    fn pay_request(&mut self, claim_id: u64, record: &[u8], attester: Option<Pubkey>) -> Result<(), String> {
        let id = u64::from_be_bytes(record[6..14].try_into().unwrap());
        let to = Pubkey::new_from_array(record[22..54].try_into().unwrap());
        let payer = self.stranger.insecure_clone();
        let ix = self.ix(
            ipow_vault::ID,
            ipow_vault::client::accounts::PayRequest {
                config: config(),
                claim: claim_pda(claim_id),
                home_asset: asset_pda(0),
                paid: paid_pda(id),
                fast_pay: fast_pay_pda(id, record),
                to,
                attester,
                tokens: None,
                mint: None,
                token_program: None,
                payer: payer.pubkey(),
                system_program: SYSTEM,
            },
            ipow_vault::client::args::PayRequest { claim_id, request_id: id, asset: 0, record: record.to_vec() },
        );
        self.send(ix, &[&payer])
    }

    fn add_bond_receipt(&mut self, who: &Keypair, amount: u64) -> Result<(), String> {
        let ix = self.ix(
            ipow_vault::ID,
            ipow_vault::client::accounts::AddBondReceipt {
                chain: chain_pda(&who.pubkey()),
                mint: receipt(0),
                from: ata(&who.pubkey()),
                holding: holding(0),
                operator: who.pubkey(),
                token_program: TOKEN,
            },
            ipow_vault::client::args::AddBondReceipt { asset: 0, amount },
        );
        self.send(ix, &[who])
    }

    fn add_bond_sol(&mut self, who: &Keypair, amount: u64) -> Result<(), String> {
        let ix = self.ix(
            ipow_vault::ID,
            ipow_vault::client::accounts::AddBondHome {
                chain: chain_pda(&who.pubkey()),
                config: config(),
                home_asset: asset_pda(0),
                from: None,
                tokens: None,
                mint: None,
                token_program: None,
                operator: who.pubkey(),
                system_program: SYSTEM,
            },
            ipow_vault::client::args::AddBondHome { asset: 0, amount },
        );
        self.send(ix, &[who])
    }

    fn withdraw_bond_receipt(&mut self, who: &Keypair, amount: u64) -> Result<(), String> {
        let ix = self.ix(
            ipow_vault::ID,
            ipow_vault::client::accounts::WithdrawBondReceipt {
                chain: chain_pda(&who.pubkey()),
                config: config(),
                mint: receipt(0),
                holding: holding(0),
                to: ata(&who.pubkey()),
                operator: who.pubkey(),
                token_program: TOKEN,
            },
            ipow_vault::client::args::WithdrawBondReceipt { asset: 0, amount },
        );
        self.send(ix, &[who])
    }

    /// Settles a slashed chain's bond in SOL (`home` Solana) or vETH.
    fn settle_slash(&mut self, operator: &Pubkey, home: u8, slasher: &Pubkey) -> Result<(), String> {
        let payer = self.stranger.insecure_clone();
        let sol = home == SOLANA;
        let ix = self.ix(
            ipow_vault::ID,
            ipow_vault::client::accounts::SettleSlash {
                chain: chain_pda(operator),
                config: config(),
                home_asset: sol.then(|| asset_pda(0)),
                mint: (!sol).then(|| receipt(0)),
                holding: (!sol).then(|| holding(0)),
                slasher_credit: credit_pda(slasher, home, 0),
                payer: payer.pubkey(),
                token_program: (!sol).then_some(TOKEN),
                system_program: SYSTEM,
            },
            ipow_vault::client::args::SettleSlash { home, asset: 0 },
        );
        self.send(ix, &[&payer])
    }

    fn withdraw_credit_receipt(&mut self, who: &Keypair, to: Pubkey) -> Result<(), String> {
        let ix = self.ix(
            ipow_vault::ID,
            ipow_vault::client::accounts::WithdrawCreditReceipt {
                credit: credit_pda(&who.pubkey(), ETHEREUM, 0),
                config: config(),
                mint: receipt(0),
                holding: holding(0),
                to,
                owner: who.pubkey(),
                token_program: TOKEN,
            },
            ipow_vault::client::args::WithdrawCreditReceipt { asset: 0 },
        );
        self.send(ix, &[who])
    }

    fn withdraw_credit_sol(&mut self, who: &Keypair) -> Result<(), String> {
        let ix = self.ix(
            ipow_vault::ID,
            ipow_vault::client::accounts::WithdrawCreditHome {
                credit: credit_pda(&who.pubkey(), SOLANA, 0),
                config: config(),
                home_asset: asset_pda(0),
                to: None,
                tokens: None,
                mint: None,
                token_program: None,
                owner: who.pubkey(),
            },
            ipow_vault::client::args::WithdrawCreditHome { asset: 0 },
        );
        self.send(ix, &[who])
    }

    /// Attests a lock on Ethereum stated by `record`; returns the attest's
    /// number.
    fn attest(&mut self, who: &Keypair, record: &[u8]) -> Result<u64, String> {
        let n = self.vault_config().attest_count + 1;
        let lock_id = u64::from_be_bytes(record[6..14].try_into().unwrap());
        let recipient = Pubkey::new_from_array(record[22..54].try_into().unwrap());
        let to = ata(&recipient);
        if self.ctx.svm.get_account(&to).is_none() {
            self.veth_account(&recipient);
        }
        let ix = self.ix(
            ipow_vault::ID,
            ipow_vault::client::accounts::AttestLock {
                config: config(),
                chain: chain_pda(&who.pubkey()),
                mark: lock_pda(lock_id),
                fast: fast_pda(n),
                mint: receipt(0),
                holding: holding(0),
                from: ata(&who.pubkey()),
                to,
                attester: who.pubkey(),
                token_program: TOKEN,
                system_program: SYSTEM,
            },
            ipow_vault::client::args::AttestLock { asset: 0, record: record.to_vec() },
        );
        self.send(ix, &[who])?;
        Ok(n)
    }

    fn fast(&self, n: u64) -> Option<ipow_vault::accounts::FastLock> {
        match self.ctx.svm.get_account(&fast_pda(n)) {
            Some(a) if !a.data.is_empty() => Some(ipow_vault::accounts::FastLock::try_deserialize(&mut &a.data[..]).unwrap()),
            _ => None,
        }
    }

    fn link_fast(&mut self, who: &Keypair, claim_id: u64, attest: u64, linked: Option<u64>) -> Result<(), String> {
        let ix = self.ix(
            ipow_vault::ID,
            ipow_vault::client::accounts::LinkFast { fast: fast_pda(attest), claim: claim_pda(claim_id), linked: linked.map(claim_pda), attester: who.pubkey() },
            ipow_vault::client::args::LinkFast { claim_id, attest },
        );
        self.send(ix, &[who])
    }

    fn burn_fast(&mut self, caller: &Keypair, attest: u64, linked: Option<u64>) -> Result<(), String> {
        let ix = self.ix(
            ipow_vault::ID,
            ipow_vault::client::accounts::BurnFast {
                config: config(),
                fast: fast_pda(attest),
                linked: linked.map(claim_pda),
                mint: receipt(0),
                holding: holding(0),
                caller_credit: credit_pda(&caller.pubkey(), ETHEREUM, 0),
                caller: caller.pubkey(),
                token_program: TOKEN,
                system_program: SYSTEM,
            },
            ipow_vault::client::args::BurnFast { attest },
        );
        self.send(ix, &[caller])
    }

    fn settle_fast(&mut self, caller: &Keypair, claim_id: u64, attest: u64, attester: &Pubkey, to: Option<Pubkey>) -> Result<(), String> {
        let ix = self.ix(
            ipow_vault::ID,
            ipow_vault::client::accounts::SettleFast {
                config: config(),
                claim: claim_pda(claim_id),
                fast: fast_pda(attest),
                mark: lock_pda(self.fast(attest).map_or(0, |f| f.lock_id)),
                attester: *attester,
                mint: receipt(0),
                holding: holding(0),
                prev: self.fast(attest).and_then(|f| (f.prev != 0).then(|| fast_pda(f.prev))),
                to,
                attester_credit: credit_pda(attester, ETHEREUM, 0),
                caller_credit: credit_pda(&caller.pubkey(), ETHEREUM, 0),
                caller: caller.pubkey(),
                token_program: TOKEN,
                system_program: SYSTEM,
            },
            ipow_vault::client::args::SettleFast { claim_id, attest },
        );
        self.send(ix, &[caller])
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

    /// Lets 7 days pass and decides the last claim.
    fn accept_last(&mut self) -> u64 {
        let id = self.vault_config().claim_count;
        self.later(WEEK);
        self.decide(id).unwrap();
        id
    }
}

fn message_pda(operator: &Pubkey, index: u64) -> Pubkey {
    vt(&[b"message", operator.as_ref(), &index.to_le_bytes()])
}

fn buffer_pda(owner: &Pubkey) -> Pubkey {
    vt(&[b"buffer", owner.as_ref()])
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

/// The operator's first message, accepted: vETH's ASSET record and its bonds
/// on Ethereum, 100 ETH and 100 vSOL, counted here; vETH made. Then
/// `more`, from message 1.
fn setup(w: &mut World, more: Vec<Vec<u8>>) -> (Pair, Ref) {
    let first = [veth_asset(), bond_rec(ETHEREUM, ETHEREUM, 0, 100 * GWEI_PER_ETH), bond_rec(ETHEREUM, SOLANA, 0, 100 * SOL)].concat();
    let mut batches = vec![first];
    batches.extend(more);
    let (pair, real) = w.ready(batches);
    w.sub(&pair, 0, real, &[]).unwrap();
    w.accept_last();
    w.make_receipt(1, 0, veth_asset()).unwrap();
    (pair, real)
}

/// `setup`, then lock #1 of `amount` ETH to the user issued as vETH in
/// message 1, claim 2. Then `more`, from message 2.
fn with_veth(w: &mut World, amount: u64, more: Vec<Vec<u8>>) -> (Pair, Ref, Pubkey) {
    let user = w.user.pubkey();
    let lock = eth_lock(1, amount, &user, 0, 0);
    let mut batches = vec![lock.clone()];
    batches.extend(more);
    let (pair, real) = setup(w, batches);
    w.sub(&pair, 1, real, &[]).unwrap();
    w.accept_last();
    let to = w.veth_account(&user);
    w.issue(2, &lock, to).unwrap();
    (pair, real, to)
}

// ---------------------------------------------------------------------
// Receipts of Ethereum's assets
// ---------------------------------------------------------------------

#[test]
fn makes_vETH_from_an_asset_record_and_issues_it_for_a_lock_once_never_for_a_given_up_lock() {
    let mut w = World::new();
    let user = w.user.insecure_clone();
    let stranger = w.stranger.insecure_clone();
    let to_user = eth_lock(1, GWEI_PER_ETH, &user.pubkey(), 0, 3);
    let to_stranger = eth_lock(2, GWEI_PER_ETH, &stranger.pubkey(), 0, 0);
    let (pair, real) = setup(&mut w, vec![[to_user.clone(), to_stranger.clone()].concat()]);
    assert!(w.make_receipt(1, 0, veth_asset()).is_err());
    let c = w.chain(&w.operator.pubkey());
    let eth = c.positions.iter().find(|p| p.home == ETHEREUM && p.asset == 0).unwrap();
    assert_eq!(eth.peer_bond, 100 * GWEI_PER_ETH);
    w.sub(&pair, 1, real, &[]).unwrap();
    let id = w.vault_config().claim_count;
    assert_eq!(w.claim(id).assets[0].value, 2 * GWEI_PER_ETH + 3);
    let to = w.veth_account(&user.pubkey());
    expect_err(w.issue(id, &to_user, to), "NotAccepted");
    // Not before the claim is accepted: it may carry a false LOCK record.
    expect_err(w.give_up(id, &to_stranger, &stranger), "NotAccepted");
    w.later(WEEK);
    w.decide(id).unwrap();
    w.give_up(id, &to_stranger, &stranger).unwrap();
    expect_err(w.give_up(id, &to_user, &stranger), "WrongAccount");
    let guardian = w.guardian.pubkey();
    let other = w.veth_account(&guardian);
    expect_err(w.issue(id, &to_user, other), "WrongAccount");
    w.issue(id, &to_user, to).unwrap();
    // The fast fee nobody earned goes to the recipient.
    assert_eq!(w.tokens(&to), GWEI_PER_ETH + 3);
    assert!(w.issue(id, &to_user, to).is_err());
    let to2 = w.veth_account(&stranger.pubkey());
    assert!(w.issue(id, &to_stranger, to2).is_err());
}

#[test]
fn refuses_a_forged_message_and_keeps_the_order() {
    let mut w = World::new();
    let op = w.operator.insecure_clone();
    let g = w.guardian.insecure_clone();
    let mut pair = w.write_registration(&op);
    let reg = pair.registration.block;
    w.write(&mut pair, bond_rec(ETHEREUM, ETHEREUM, 0, 1), None);
    w.write(&mut pair, bond_rec(ETHEREUM, ETHEREUM, 0, 1), None);
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
    let walk = w.main_walk(real, reg);
    let ix = w.ix(
        ipow_vault::ID,
        ipow_vault::client::accounts::SubmitMessage {
            config: config(),
            chain: chain_pda(&op.pubkey()),
            real: real_pda(&real),
            walk,
            node: node_pda(&forged.block),
            message: message_pda(&op.pubkey(), 0),
            claim: claim_pda(1),
            stake: stake_pda(1, &op.pubkey()),
            submitter: g.pubkey(),
            buffer: None,
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
fn opens_no_claim_past_80_percent_of_the_bond_in_that_asset() {
    let mut w = World::new();
    let user = w.user.pubkey();
    // 100 ETH counted: 80 ETH may be open.
    let (pair, real) = setup(&mut w, vec![eth_lock(1, 80 * GWEI_PER_ETH + 1, &user, 0, 0), eth_lock(2, 80 * GWEI_PER_ETH, &user, 0, 0)]);
    let before = w.vault_config().claim_count;
    w.sub(&pair, 1, real, &[]).unwrap();
    assert_eq!(w.vault_config().claim_count, before);
    w.sub(&pair, 2, real, &[]).unwrap();
    assert_eq!(w.vault_config().claim_count, before + 1);
}

#[test]
fn an_objection_that_stands_refuses_the_claim_and_every_other_one_of_the_chain() {
    let mut w = World::new();
    let g = w.guardian.insecure_clone();
    let user = w.user.pubkey();
    let (pair, real) = setup(&mut w, vec![eth_lock(1, 1, &user, 0, 0), eth_lock(2, 1, &user, 0, 0), eth_lock(3, 1, &user, 0, 0)]);
    w.sub(&pair, 1, real, &[]).unwrap(); // claim 2
    w.sub(&pair, 2, real, &[]).unwrap(); // claim 3, opened before 2 is refused
    w.side(2, &g, true).unwrap();
    expect_err(w.side(2, &g, true), "AlreadyHeld");
    w.later(WEEK);
    w.decide(2).unwrap();
    assert!(!w.claim(2).accepted);
    w.collect(2, &g).unwrap();
    assert_eq!(w.credit(&g.pubkey(), SOLANA, 0), 2 * DEPOSIT);
    let before = w.lamports(&g.pubkey());
    w.withdraw_credit_sol(&g).unwrap();
    assert_eq!(w.lamports(&g.pubkey()), before + 2 * DEPOSIT - TX_FEE);
    w.decide(3).unwrap();
    assert!(!w.claim(3).accepted);
    w.sub(&pair, 3, real, &[]).unwrap();
    assert_eq!(w.vault_config().claim_count, 3);
}

#[test]
fn an_answer_restarts_the_7_days_and_the_answering_side_wins() {
    let mut w = World::new();
    let g = w.guardian.insecure_clone();
    let user = w.user.insecure_clone();
    let (pair, real) = setup(&mut w, vec![eth_lock(1, 1, &user.pubkey(), 0, 0)]);
    w.sub(&pair, 1, real, &[]).unwrap();
    w.later(6 * DAY);
    w.side(2, &g, true).unwrap();
    w.later(6 * DAY);
    w.side(2, &user, false).unwrap();
    w.later(6 * DAY);
    expect_err(w.decide(2), "WindowNotOver");
    w.later(DAY);
    w.decide(2).unwrap();
    assert!(w.claim(2).accepted);
    w.collect(2, &user).unwrap();
    assert_eq!(w.credit(&user.pubkey(), SOLANA, 0), DEPOSIT + DEPOSIT / 2);
    expect_err(w.collect(2, &g), "NothingToCollect");
}

// ---------------------------------------------------------------------
// Judging records about Solana (D109)
// ---------------------------------------------------------------------

#[test]
fn a_true_burn_earns_its_fee_once_and_a_false_one_slashes_every_bond() {
    let mut w = World::new();
    let op = w.operator.insecure_clone();
    let user = w.user.insecure_clone();
    let g = w.guardian.insecure_clone();
    let (pair, real, user_veth) = with_veth(
        &mut w,
        10 * GWEI_PER_ETH,
        vec![bond_rec(SOLANA, ETHEREUM, 0, 3 * GWEI_PER_ETH), veth_burn(1, GWEI_PER_ETH, 5, 0), veth_burn(1, GWEI_PER_ETH, 5, 0), veth_burn(2, 2, 0, 0)],
    );
    // The operator's vETH bond: 4 vETH from the user; and 2 SOL.
    let op_veth = w.veth_account(&op.pubkey());
    token_transfer(&mut w, &user, user_veth, op_veth, 4 * GWEI_PER_ETH);
    w.add_bond_receipt(&op, 4 * GWEI_PER_ETH).unwrap();
    w.add_bond_sol(&op, 2 * SOL).unwrap();
    w.sub(&pair, 2, real, &[]).unwrap();
    let c = w.chain(&op.pubkey());
    assert_eq!(c.positions.iter().find(|p| p.home == ETHEREUM && p.asset == 0).unwrap().stated, 3 * GWEI_PER_ETH);
    // A burn of 1 vETH with a fee of 5.
    w.set_now(BURNED_AT);
    w.burn(&user, GWEI_PER_ETH, 5, 0).unwrap();
    w.sub(&pair, 3, real, &[request_pda(1)]).unwrap();
    w.sub(&pair, 4, real, &[request_pda(1)]).unwrap();
    w.take_request_fee(&op, 1).unwrap();
    assert_eq!(w.tokens(&op_veth), 5);
    expect_err(w.take_request_fee(&op, 1), "NothingToCollect");
    // Burn #2 does not exist: false. Every bond of the chain is slashed.
    let supply = w.supply();
    w.sub(&pair, 5, real, &[request_pda(2)]).unwrap();
    let c = w.chain(&op.pubkey());
    assert!(c.slashed);
    assert_eq!(c.slasher, g.pubkey());
    w.settle_slash(&op.pubkey(), ETHEREUM, &g.pubkey()).unwrap();
    w.settle_slash(&op.pubkey(), SOLANA, &g.pubkey()).unwrap();
    expect_err(w.settle_slash(&op.pubkey(), SOLANA, &g.pubkey()), "NothingToCollect");
    // 80% of the vETH bond burned, 20% to the guardian; 80% of the SOL
    // bond backs vSOL, 20% to the guardian.
    assert_eq!(w.supply(), supply - 4 * GWEI_PER_ETH * 8 / 10);
    assert_eq!(w.credit(&g.pubkey(), ETHEREUM, 0), 4 * GWEI_PER_ETH / 5);
    assert_eq!(w.credit(&g.pubkey(), SOLANA, 0), 2 * SOL / 5);
    assert_eq!(w.home_asset(0).reserve, 2 * SOL * 8 / 10);
    let g_veth = w.veth_account(&g.pubkey());
    w.withdraw_credit_receipt(&g, g_veth).unwrap();
    assert_eq!(w.tokens(&g_veth), 4 * GWEI_PER_ETH / 5);
}

#[test]
fn a_cancel_is_true_only_for_a_lock_given_up_here() {
    let mut w = World::new();
    let op = w.operator.insecure_clone();
    let stranger = w.stranger.insecure_clone();
    let lock = eth_lock(2, GWEI_PER_ETH, &stranger.pubkey(), 0, 0);
    let (pair, real, _) = with_veth(&mut w, GWEI_PER_ETH, vec![lock.clone(), cancel_rec(SOLANA, 2), cancel_rec(SOLANA, 3)]);
    w.sub(&pair, 2, real, &[]).unwrap();
    w.accept_last();
    w.give_up(3, &lock, &stranger).unwrap();
    w.sub(&pair, 3, real, &[lock_pda(2)]).unwrap();
    assert!(!w.chain(&op.pubkey()).slashed);
    // Lock #3 was never given up here.
    w.sub(&pair, 4, real, &[lock_pda(3)]).unwrap();
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
        vec![bond_rec(SOLANA, ETHEREUM, 0, GWEI_PER_ETH), [bond_rec(ETHEREUM, ETHEREUM, 1, 5), exit_rec()].concat()],
    );
    let op_veth = w.veth_account(&op.pubkey());
    token_transfer(&mut w, &user, user_veth, op_veth, 2 * GWEI_PER_ETH);
    w.add_bond_receipt(&op, 2 * GWEI_PER_ETH).unwrap();
    w.sub(&pair, 2, real, &[]).unwrap();
    expect_err(w.withdraw_bond_receipt(&op, GWEI_PER_ETH + 1), "BondNotFree");
    w.withdraw_bond_receipt(&op, GWEI_PER_ETH).unwrap();
    // EXIT, with a BOND claim still open: the stated bond stays locked.
    w.sub(&pair, 3, real, &[]).unwrap();
    assert!(w.chain(&op.pubkey()).exited);
    expect_err(w.withdraw_bond_receipt(&op, 1), "BondNotFree");
    w.accept_last();
    w.withdraw_bond_receipt(&op, GWEI_PER_ETH).unwrap();
    assert_eq!(w.tokens(&op_veth), 2 * GWEI_PER_ETH);
}

#[test]
fn slashes_a_bond_above_the_bond_twice_in_a_batch_or_a_batch_that_does_not_parse() {
    for batch in [
        bond_rec(SOLANA, SOLANA, 0, 1),
        vec![9u8, 1],
        [bond_rec(SOLANA, ETHEREUM, 0, 1), bond_rec(SOLANA, ETHEREUM, 0, 1)].concat(),
        bond_rec(SOLANA, 7, 0, 1),
        bond_rec(SOLANA, ETHEREUM, 0, 0),
        [exit_rec(), bond_rec(ETHEREUM, ETHEREUM, 0, 1)].concat(),
    ] {
        let mut w = World::new();
        let op = w.operator.insecure_clone();
        let user = w.user.insecure_clone();
        let (pair, real, user_veth) = with_veth(&mut w, GWEI_PER_ETH, vec![batch]);
        let op_veth = w.veth_account(&op.pubkey());
        token_transfer(&mut w, &user, user_veth, op_veth, 1);
        w.add_bond_receipt(&op, 1).unwrap();
        w.sub(&pair, 2, real, &[]).unwrap();
        assert!(w.chain(&op.pubkey()).slashed);
    }
}

#[test]
fn takes_the_same_bond_for_solana_again_as_true_and_another_amount_as_false() {
    let mut w = World::new();
    let op = w.operator.insecure_clone();
    let (pair, real) = setup(&mut w, vec![bond_rec(SOLANA, SOLANA, 0, SOL), bond_rec(SOLANA, SOLANA, 0, SOL), bond_rec(SOLANA, SOLANA, 0, 2 * SOL)]);
    w.add_bond_sol(&op, 3 * SOL).unwrap();
    w.sub(&pair, 1, real, &[]).unwrap();
    w.sub(&pair, 2, real, &[]).unwrap();
    assert!(!w.chain(&op.pubkey()).slashed);
    w.sub(&pair, 3, real, &[]).unwrap();
    assert!(w.chain(&op.pubkey()).slashed);
}

#[test]
fn a_bond_for_ethereum_is_carried_again_when_its_claim_could_not_open() {
    let mut w = World::new();
    let op = w.operator.insecure_clone();
    let user = w.user.pubkey();
    let (pair, real) = w.ready(vec![
        // The LOCK passes the cover while no Ethereum bond counts: no claim.
        [bond_rec(ETHEREUM, ETHEREUM, 0, 10 * GWEI_PER_ETH), eth_lock(1, 1, &user, 0, 0)].concat(),
        bond_rec(ETHEREUM, ETHEREUM, 0, 10 * GWEI_PER_ETH),
        bond_rec(ETHEREUM, ETHEREUM, 0, 2),
    ]);
    w.sub(&pair, 0, real, &[]).unwrap();
    assert_eq!(w.vault_config().claim_count, 0);
    w.sub(&pair, 1, real, &[]).unwrap();
    assert_eq!(w.vault_config().claim_count, 1);
    // A later one is not acted on, and opens no claim.
    w.sub(&pair, 2, real, &[]).unwrap();
    assert_eq!(w.vault_config().claim_count, 1);
    w.accept_last();
    let c = w.chain(&op.pubkey());
    assert_eq!(c.positions.iter().find(|p| p.home == ETHEREUM && p.asset == 0).unwrap().peer_bond, 10 * GWEI_PER_ETH);
}

#[test]
fn burns_every_bond_to_the_backing_when_the_operator_submits_its_own_lie() {
    let mut w = World::new();
    let op = w.operator.insecure_clone();
    let (pair, real) = setup(&mut w, vec![lock_rec(SOLANA, 0, 9, 1, &ETH_USER32, 0, 0, 0)]);
    w.add_bond_sol(&op, SOL).unwrap();
    w.submit(&op.pubkey(), &pair.msgs[1].clone(), real, &op, &[home_lock_pda(9)]).unwrap();
    w.settle_slash(&op.pubkey(), SOLANA, &op.pubkey()).unwrap();
    assert_eq!(w.home_asset(0).reserve, SOL);
    assert_eq!(w.credit(&op.pubkey(), SOLANA, 0), 0);
}

// ---------------------------------------------------------------------
// SOL and tokens whose home is Solana (section 11.9)
// ---------------------------------------------------------------------

#[test]
fn locks_sol_for_its_receipt_and_a_true_lock_record_earns_its_fee_once() {
    let mut w = World::new();
    let op = w.operator.insecure_clone();
    let user = w.user.insecure_clone();
    let id = w.lock_sol(&user, 2 * SOL, 7, 3).unwrap();
    assert_eq!(w.home_asset(0).reserve, 2 * SOL + 3);
    let record = w.sol_lock_rec(id);
    let wrong = lock_rec(SOLANA, 0, id, 2 * SOL, &ETH_USER32, 7, 3, 0);
    let (pair, real) = setup(&mut w, vec![record.clone(), record, wrong]);
    w.sub(&pair, 1, real, &[home_lock_pda(id)]).unwrap();
    w.sub(&pair, 2, real, &[home_lock_pda(id)]).unwrap();
    let before = w.lamports(&op.pubkey());
    w.take_lock_fee(&op, id).unwrap();
    assert_eq!(w.lamports(&op.pubkey()), before + 7 - TX_FEE);
    expect_err(w.take_lock_fee(&op, id), "NothingToCollect");
    assert!(!w.chain(&op.pubkey()).slashed);
    // Another time: false.
    w.sub(&pair, 3, real, &[home_lock_pda(id)]).unwrap();
    assert!(w.chain(&op.pubkey()).slashed);
    // Not to an address that is not on Ethereum.
    let ix = w.ix(
        ipow_vault::ID,
        ipow_vault::client::accounts::Lock {
            config: config(),
            home_asset: asset_pda(0),
            lock: home_lock_pda(2),
            from: None,
            tokens: None,
            mint: None,
            token_program: None,
            user: user.pubkey(),
            system_program: SYSTEM,
        },
        ipow_vault::client::args::Lock { asset: 0, recipient: [7u8; 32], amount: 1, fee: 0, fast_fee: 0 },
    );
    expect_err(w.send(ix, &[&user]), "ZeroAddress");
}

#[test]
fn pays_a_burn_on_ethereum_in_sol_and_an_attester_its_share() {
    let mut w = World::new();
    let user = w.user.insecure_clone();
    let g = w.guardian.insecure_clone();
    w.lock_sol(&user, 3 * SOL, 0, 0).unwrap();
    let alice = Keypair::new().pubkey();
    // Burns of vSOL on Ethereum, made at BURNED_AT: one paid at once, one not.
    let fast = request_rec(ETHEREUM, 0, 7, SOL, &alice.to_bytes(), 0, 800, BURNED_AT);
    let slow = request_rec(ETHEREUM, 0, 8, SOL / 2, &alice.to_bytes(), 0, 5, BURNED_AT);
    let (pair, real) = setup(&mut w, vec![[fast.clone(), slow.clone()].concat()]);
    // Paid 3 days after the burn: 5 of the 8 days left (D124).
    w.set_now(BURNED_AT + 3 * DAY);
    w.fast_pay(&g, &fast).unwrap();
    assert_eq!(w.lamports(&alice), SOL);
    assert!(w.fast_pay(&g, &fast).is_err());
    w.sub(&pair, 1, real, &[asset_pda(0), asset_pda(0)]).unwrap();
    let id = w.vault_config().claim_count;
    assert_eq!(w.claim(id).assets[0].value, SOL + 800 + SOL / 2 + 5);
    expect_err(w.pay_request(id, &fast, Some(g.pubkey())), "NotAccepted");
    w.later(WEEK);
    w.decide(id).unwrap();
    let before = w.lamports(&g.pubkey());
    w.pay_request(id, &fast, Some(g.pubkey())).unwrap();
    assert_eq!(w.lamports(&g.pubkey()), before + SOL + 500);
    assert_eq!(w.lamports(&alice), SOL + 300);
    assert!(w.pay_request(id, &fast, Some(g.pubkey())).is_err());
    w.pay_request(id, &slow, None).unwrap();
    assert_eq!(w.lamports(&alice), SOL + 300 + SOL / 2 + 5);
    assert_eq!(w.home_asset(0).reserve, 3 * SOL - SOL - 800 - SOL / 2 - 5);
}

#[test]
fn returns_a_sol_lock_whose_cancel_was_accepted_with_its_fees() {
    let mut w = World::new();
    let user = w.user.insecure_clone();
    let id = w.lock_sol(&user, SOL, 4, 6).unwrap();
    let (pair, real) = setup(&mut w, vec![cancel_rec(ETHEREUM, id), cancel_rec(ETHEREUM, 99)]);
    w.sub(&pair, 1, real, &[home_lock_pda(id)]).unwrap();
    let claim = w.vault_config().claim_count;
    w.later(WEEK);
    w.decide(claim).unwrap();
    let before = w.lamports(&user.pubkey());
    w.return_lock(claim, id, &user.pubkey()).unwrap();
    assert_eq!(w.lamports(&user.pubkey()), before + SOL + 10);
    assert_eq!(w.home_asset(0).reserve, 0);
    expect_err(w.return_lock(claim, id, &user.pubkey()), "AlreadyDone");
    // A CANCEL of a lock that does not exist here is false.
    w.sub(&pair, 2, real, &[home_lock_pda(99)]).unwrap();
    assert!(w.chain(&w.operator.pubkey()).slashed);
}

#[test]
fn judges_an_asset_record_of_solana_by_its_mint_and_decimals() {
    let mut w = World::new();
    let (pair, real) = setup(&mut w, vec![asset_rec(SOLANA, 0, &[0u8; 32], 9), asset_rec(SOLANA, 0, &[0u8; 32], 8)]);
    w.sub(&pair, 1, real, &[asset_pda(0)]).unwrap();
    assert!(!w.chain(&w.operator.pubkey()).slashed);
    w.sub(&pair, 2, real, &[asset_pda(0)]).unwrap();
    assert!(w.chain(&w.operator.pubkey()).slashed);
}

#[test]
fn registers_a_token_once_and_locks_it() {
    let mut w = World::new();
    let user = w.user.insecure_clone();
    let payer = w.stranger.insecure_clone();
    let mint = litesvm_token::CreateMint::new(&mut w.ctx.svm, &payer).decimals(6).send().unwrap();
    let tokens = vt(&[b"home_tokens", mint.as_ref()]);
    let register = |w: &mut World| {
        let ix = w.ix(
            ipow_vault::ID,
            ipow_vault::client::accounts::RegisterAsset {
                config: config(),
                asset: asset_pda(w.vault_config().asset_count),
                mint,
                tokens,
                payer: payer.pubkey(),
                token_program: TOKEN,
                system_program: SYSTEM,
            },
            ipow_vault::client::args::RegisterAsset {},
        );
        w.send(ix, &[&payer])
    };
    register(&mut w).unwrap();
    assert!(register(&mut w).is_err());
    let a = w.home_asset(1);
    assert_eq!((a.mint, a.decimals, a.record_decimals, a.unit), (mint, 6, 6, 1));
    let from = litesvm_token::CreateAssociatedTokenAccount::new(&mut w.ctx.svm, &payer, &mint).owner(&user.pubkey()).send().unwrap();
    litesvm_token::MintTo::new(&mut w.ctx.svm, &payer, &mint, &from, 1_000).send().unwrap();
    let ix = w.ix(
        ipow_vault::ID,
        ipow_vault::client::accounts::Lock {
            config: config(),
            home_asset: asset_pda(1),
            lock: home_lock_pda(1),
            from: Some(from),
            tokens: Some(tokens),
            mint: Some(mint),
            token_program: Some(TOKEN),
            user: user.pubkey(),
            system_program: SYSTEM,
        },
        ipow_vault::client::args::Lock { asset: 1, recipient: ETH_USER32, amount: 900, fee: 50, fast_fee: 50 },
    );
    w.send(ix, &[&user]).unwrap();
    assert_eq!(w.tokens(&tokens), 1_000);
    assert_eq!(w.home_asset(1).reserve, 950);
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

#[test]
fn a_large_message_goes_through_a_buffer_and_one_past_the_limits_cannot_be_uploaded() {
    let mut w = World::new();
    let op = w.operator.insecure_clone();
    let g = w.guardian.insecure_clone();
    let user = w.user.pubkey();
    let locks: Vec<u8> = (1..=10).flat_map(|i| eth_lock(i, GWEI_PER_ETH, &user, 0, 0)).collect();
    let (pair, real) = setup(&mut w, vec![locks]);

    // 780 bytes of records do not fit in one Solana transaction with a
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
    let index = w.chain(&op.pubkey()).messages;
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
            message: message_pda(&op.pubkey(), index),
            claim: claim_pda(id),
            stake: stake_pda(id, &op.pubkey()),
            submitter: g.pubkey(),
            buffer: Some(buffer_pda(&g.pubkey())),
            system_program: SYSTEM,
        },
        ipow_vault::client::args::SubmitMessage { operator: op.pubkey(), btc, input_index: 0, tag_index: 1, batch: vec![] },
    );
    w.send(ix, &[&g]).unwrap();
    assert_eq!(w.claim(id).records.len(), 780);

    // Past the limits of D119, a buffer takes nothing more.
    expect_err(write(&mut w, false, vec![0; 2048 - 780 + 1]), "TooLarge");
    expect_err(write(&mut w, true, vec![0; 1024]), "TooLarge");
    let close = w.ix(
        ipow_vault::ID,
        ipow_vault::client::accounts::CloseBuffer { buffer: buffer_pda(&g.pubkey()), owner: g.pubkey() },
        ipow_vault::client::args::CloseBuffer {},
    );
    w.send(close, &[&g]).unwrap();
}

#[test]
fn publishes_each_batch_and_keeps_it_30_days() {
    let mut w = World::new();
    let op = w.operator.insecure_clone();
    let (pair, real) = w.ready(vec![bond_rec(ETHEREUM, ETHEREUM, 0, 7)]);
    w.sub(&pair, 0, real, &[]).unwrap();
    let m: ipow_vault::accounts::Message = w.ctx.get_account(&message_pda(&op.pubkey(), 0)).unwrap();
    assert_eq!(m.batch, pair.msgs[0].batch);
    assert_eq!(m.index, 0);
    // The guardian submitted the message, so it paid and gets the rent
    // back; anyone may close it, here the operator.
    let g = w.guardian.pubkey();
    assert_eq!(m.payer, g);
    let close = |w: &mut World, payer: Pubkey| {
        let ix = w.ix(
            ipow_vault::ID,
            ipow_vault::client::accounts::CloseMessage { message: message_pda(&op.pubkey(), 0), payer },
            ipow_vault::client::args::CloseMessage {},
        );
        w.send(ix, &[&op])
    };
    expect_err(close(&mut w, g), "WindowNotOver");
    w.later(30 * DAY);
    expect_err(close(&mut w, op.pubkey()), "ConstraintAddress");
    let before = w.lamports(&g);
    close(&mut w, g).unwrap();
    assert!(w.lamports(&g) > before);
}

// ---------------------------------------------------------------------
// Fast paths of locks on Ethereum (section 11.7, D122 to D126)
// ---------------------------------------------------------------------

/// vETH for the operator to attest with: `amount` from the user's lock #1.
fn attester_funds(w: &mut World, user_veth: Pubkey, amount: u64) {
    let user = w.user.insecure_clone();
    let op = w.operator.pubkey();
    let op_veth = w.veth_account(&op);
    token_transfer(w, &user, user_veth, op_veth, amount);
}

#[test]
fn an_attester_issues_a_receipt_at_once_and_is_repaid_with_its_share() {
    let mut w = World::new();
    let op = w.operator.insecure_clone();
    let g = w.guardian.insecure_clone();
    let alice = w.stranger.pubkey();
    let lock = lock_rec(ETHEREUM, 0, 5, GWEI_PER_ETH, &alice.to_bytes(), 0, 800, LOCKED_AT);
    let (pair, real, user_veth) = with_veth(&mut w, 10 * GWEI_PER_ETH, vec![lock.clone()]);
    attester_funds(&mut w, user_veth, 2 * GWEI_PER_ETH);
    let supply = w.supply();
    // Only an operator attests.
    w.veth_account(&g.pubkey());
    expect_err(w.attest(&g, &lock), "AccountNotInitialized");
    // Attested 3 days after the lock: 5 of the 8 days left (D124).
    w.set_now(LOCKED_AT + 3 * DAY);
    let n = w.attest(&op, &lock).unwrap();
    assert_eq!(w.tokens(&ata(&alice)), GWEI_PER_ETH);
    assert_eq!(w.supply(), supply + GWEI_PER_ETH);
    w.sub(&pair, 2, real, &[]).unwrap();
    let id = w.vault_config().claim_count;
    // Only the attester links, and only its own chain's claim.
    expect_err(w.link_fast(&g, id, n, None), "ConstraintAddress");
    w.link_fast(&op, id, n, None).unwrap();
    w.later(WEEK);
    expect_err(w.burn_fast(&g, n, Some(id)), "NotRefused");
    w.decide(id).unwrap();
    assert!(w.issue(id, &lock, ata(&alice)).is_err());
    expect_err(w.settle_fast(&g, id, n, &op.pubkey(), None), "WrongAccount");
    w.settle_fast(&g, id, n, &op.pubkey(), Some(ata(&alice))).unwrap();
    assert_eq!(w.credit(&op.pubkey(), ETHEREUM, 0), GWEI_PER_ETH * 5 / 4 + 500);
    assert_eq!(w.tokens(&ata(&alice)), GWEI_PER_ETH + 300);
    assert_eq!(w.supply(), supply + GWEI_PER_ETH + 800);
    assert!(w.fast(n).is_none());
}

#[test]
fn an_unlinked_attest_is_burned_after_7_days_and_a_late_true_record_pays_the_attester() {
    let mut w = World::new();
    let op = w.operator.insecure_clone();
    let g = w.guardian.insecure_clone();
    let alice = w.stranger.pubkey();
    let lock = eth_lock(5, GWEI_PER_ETH, &alice, 0, 0);
    let (pair, real, user_veth) = with_veth(&mut w, 10 * GWEI_PER_ETH, vec![lock.clone()]);
    attester_funds(&mut w, user_veth, 2 * GWEI_PER_ETH);
    let supply = w.supply();
    let n = w.attest(&op, &lock).unwrap();
    expect_err(w.burn_fast(&g, n, None), "WindowNotOver");
    w.later(WEEK);
    w.burn_fast(&g, n, None).unwrap();
    assert_eq!(w.supply(), supply);
    assert_eq!(w.credit(&g.pubkey(), ETHEREUM, 0), GWEI_PER_ETH / 4);
    expect_err(w.burn_fast(&g, n, None), "AlreadyDone");
    w.sub(&pair, 2, real, &[]).unwrap();
    let id = w.vault_config().claim_count;
    expect_err(w.link_fast(&op, id, n, None), "AlreadyDone");
    w.later(WEEK);
    w.decide(id).unwrap();
    w.settle_fast(&g, id, n, &op.pubkey(), Some(ata(&alice))).unwrap();
    assert_eq!(w.credit(&op.pubkey(), ETHEREUM, 0), GWEI_PER_ETH);
    assert_eq!(w.supply(), supply + GWEI_PER_ETH);
}

#[test]
fn settles_a_locks_attests_in_order_the_earliest_true_one_counts() {
    for wrong_first in [true, false] {
        let mut w = World::new();
        let op = w.operator.insecure_clone();
        let g = w.guardian.insecure_clone();
        let alice = w.stranger.pubkey();
        let lock = eth_lock(5, GWEI_PER_ETH, &alice, 0, 0);
        let wrong = lock_rec(ETHEREUM, 0, 5, 1, &g.pubkey().to_bytes(), 0, 0, LOCKED_AT);
        let (pair, real, user_veth) = with_veth(&mut w, 10 * GWEI_PER_ETH, vec![lock.clone()]);
        attester_funds(&mut w, user_veth, 4 * GWEI_PER_ETH);
        let supply = w.supply();
        let (first, second) = if wrong_first {
            let x = w.attest(&op, &wrong).unwrap();
            (x, w.attest(&op, &lock).unwrap())
        } else {
            let x = w.attest(&op, &lock).unwrap();
            (x, w.attest(&op, &wrong).unwrap())
        };
        // And a copy of the true one, later.
        let copy = w.attest(&op, &lock).unwrap();
        w.sub(&pair, 2, real, &[]).unwrap();
        let id = w.vault_config().claim_count;
        w.later(WEEK);
        w.decide(id).unwrap();
        expect_err(w.settle_fast(&g, id, second, &op.pubkey(), Some(ata(&alice))), "NotInOrder");
        for n in [first, second, copy] {
            w.settle_fast(&g, id, n, &op.pubkey(), Some(ata(&alice))).unwrap();
        }
        // Alice's receipt once from the true attest, plus the copy's, paid
        // for by its collateral: supply grew by the lock only.
        assert_eq!(w.supply(), supply + GWEI_PER_ETH);
        assert_eq!(w.credit(&op.pubkey(), ETHEREUM, 0), GWEI_PER_ETH * 5 / 4);
        assert_eq!(w.credit(&g.pubkey(), ETHEREUM, 0), 1 + GWEI_PER_ETH / 4);
        // No attest once the receipt counted.
        assert!(w.attest(&op, &lock).is_err());
    }
}

#[test]
fn a_lock_takes_attests_for_7_days_and_then_its_recipient_issues_the_true_receipt() {
    let mut w = World::new();
    let op = w.operator.insecure_clone();
    let g = w.guardian.insecure_clone();
    let alice = w.stranger.pubkey();
    let lock = eth_lock(5, GWEI_PER_ETH, &alice, 0, 4);
    let (pair, real, user_veth) = with_veth(&mut w, 10 * GWEI_PER_ETH, vec![lock.clone()]);
    attester_funds(&mut w, user_veth, 2 * GWEI_PER_ETH);
    w.sub(&pair, 2, real, &[]).unwrap();
    let id = w.vault_config().claim_count;
    w.later(WEEK);
    w.decide(id).unwrap();
    let wrong = lock_rec(ETHEREUM, 0, 5, 1, &g.pubkey().to_bytes(), 0, 0, LOCKED_AT);
    let n = w.attest(&op, &wrong).unwrap();
    let to = w.veth_account(&alice);
    w.settle_fast(&g, id, n, &op.pubkey(), Some(to)).unwrap();
    assert_eq!(w.tokens(&to), 0);
    expect_err(w.issue(id, &lock, to), "NotInOrder");
    w.later(WEEK);
    expect_err(w.attest(&op, &wrong), "WindowOver");
    w.issue(id, &lock, to).unwrap();
    assert_eq!(w.tokens(&to), GWEI_PER_ETH + 4);
}

#[test]
fn an_attest_whose_linked_claim_is_refused_can_be_burned() {
    let mut w = World::new();
    let op = w.operator.insecure_clone();
    let g = w.guardian.insecure_clone();
    let alice = w.stranger.pubkey();
    let lock = eth_lock(5, GWEI_PER_ETH, &alice, 0, 0);
    let (pair, real, user_veth) = with_veth(&mut w, 10 * GWEI_PER_ETH, vec![lock.clone()]);
    attester_funds(&mut w, user_veth, 2 * GWEI_PER_ETH);
    let n = w.attest(&op, &lock).unwrap();
    w.sub(&pair, 2, real, &[]).unwrap();
    let id = w.vault_config().claim_count;
    w.link_fast(&op, id, n, None).unwrap();
    w.side(id, &g, true).unwrap();
    w.later(WEEK);
    w.decide(id).unwrap();
    expect_err(w.burn_fast(&g, n, None), "WrongAccount");
    w.burn_fast(&g, n, Some(id)).unwrap();
    assert!(w.fast(n).unwrap().burned);
}

#[test]
fn a_burn_carries_its_fast_fee_and_time_and_the_record_must_state_both() {
    let mut w = World::new();
    let op = w.operator.insecure_clone();
    let user = w.user.insecure_clone();
    let (pair, real, user_veth) = with_veth(
        &mut w,
        10 * GWEI_PER_ETH,
        vec![bond_rec(SOLANA, ETHEREUM, 0, GWEI_PER_ETH), veth_burn(1, GWEI_PER_ETH, 0, 9), request_rec(SOLANA, 0, 1, GWEI_PER_ETH, &ETH_USER32, 0, 9, BURNED_AT + 1)],
    );
    let op_veth = w.veth_account(&op.pubkey());
    token_transfer(&mut w, &user, user_veth, op_veth, 2 * GWEI_PER_ETH);
    w.add_bond_receipt(&op, 2 * GWEI_PER_ETH).unwrap();
    let supply = w.supply();
    w.set_now(BURNED_AT);
    w.burn(&user, GWEI_PER_ETH, 0, 9).unwrap();
    assert_eq!(w.supply(), supply - GWEI_PER_ETH - 9);
    w.sub(&pair, 2, real, &[]).unwrap();
    w.sub(&pair, 3, real, &[request_pda(1)]).unwrap();
    assert!(!w.chain(&op.pubkey()).slashed);
    w.sub(&pair, 4, real, &[request_pda(1)]).unwrap();
    assert!(w.chain(&op.pubkey()).slashed);
}

// ---------------------------------------------------------------------
// Review cases of section 11.9
// ---------------------------------------------------------------------

#[test]
fn a_record_whose_fee_was_taken_or_whose_lock_returned_stays_true() {
    let mut w = World::new();
    let op = w.operator.insecure_clone();
    let user = w.user.insecure_clone();
    let id = w.lock_sol(&user, SOL, 7, 0).unwrap();
    let record = w.sol_lock_rec(id);
    let (pair, real) = setup(&mut w, vec![record.clone(), cancel_rec(ETHEREUM, id), record]);
    w.sub(&pair, 1, real, &[home_lock_pda(id)]).unwrap();
    w.take_lock_fee(&op, id).unwrap();
    w.sub(&pair, 2, real, &[home_lock_pda(id)]).unwrap();
    let claim = w.accept_last();
    w.return_lock(claim, id, &user.pubkey()).unwrap();
    // Carried again after its fee was taken and it was returned: true.
    w.sub(&pair, 3, real, &[home_lock_pda(id)]).unwrap();
    assert!(!w.chain(&op.pubkey()).slashed);
}

#[test]
fn pays_a_burn_once_whatever_account_is_left_out() {
    let mut w = World::new();
    let user = w.user.insecure_clone();
    let g = w.guardian.insecure_clone();
    w.lock_sol(&user, 3 * SOL, 0, 0).unwrap();
    let alice = Keypair::new().pubkey();
    let fast = request_rec(ETHEREUM, 0, 7, SOL, &alice.to_bytes(), 0, 0, BURNED_AT);
    let (pair, real) = setup(&mut w, vec![fast.clone()]);
    w.fast_pay(&g, &fast).unwrap();
    w.sub(&pair, 1, real, &[asset_pda(0)]).unwrap();
    let id = w.accept_last();
    // Naming no attester does not pay the address a second time.
    assert!(w.pay_request(id, &fast, None).is_err());
    let before = w.lamports(&g.pubkey());
    w.pay_request(id, &fast, Some(g.pubkey())).unwrap();
    assert_eq!(w.lamports(&g.pubkey()), before + SOL);
    assert_eq!(w.lamports(&alice), SOL);
}

#[test]
fn a_later_false_message_does_not_take_the_first_slashers_share() {
    let mut w = World::new();
    let op = w.operator.insecure_clone();
    let g = w.guardian.insecure_clone();
    let (pair, real) = setup(&mut w, vec![lock_rec(SOLANA, 0, 9, 1, &ETH_USER32, 0, 0, 0), lock_rec(SOLANA, 0, 9, 1, &ETH_USER32, 0, 0, 0)]);
    w.add_bond_sol(&op, SOL).unwrap();
    w.sub(&pair, 1, real, &[home_lock_pda(9)]).unwrap();
    w.submit(&op.pubkey(), &pair.msgs[2].clone(), real, &op, &[home_lock_pda(9)]).unwrap();
    assert_eq!(w.chain(&op.pubkey()).slasher, g.pubkey());
    w.settle_slash(&op.pubkey(), SOLANA, &g.pubkey()).unwrap();
    assert_eq!(w.credit(&g.pubkey(), SOLANA, 0), SOL / 5);
}

#[test]
fn a_message_naming_more_than_32_locks_burns_cancels_and_assets_is_false() {
    let mut w = World::new();
    let op = w.operator.insecure_clone();
    let user = w.user.pubkey();
    let batch: Vec<u8> = [
        (1..=17).flat_map(|i| eth_lock(i, 1, &user, 0, 0)).collect::<Vec<u8>>(),
        // ASSET records from Ethereum need no account here.
        (1..=16).flat_map(|i| asset_rec(ETHEREUM, i, &[1u8; 32], 6)).collect::<Vec<u8>>(),
    ]
    .concat();
    let (pair, real) = setup(&mut w, vec![batch]);
    w.sub(&pair, 1, real, &[]).unwrap();
    assert!(w.chain(&op.pubkey()).slashed);
}

#[test]
fn settles_a_wrong_attest_in_its_own_asset_and_leaves_another_assets_lock_to_its_issue() {
    let mut w = World::new();
    let op = w.operator.insecure_clone();
    let g = w.guardian.insecure_clone();
    let alice = w.stranger.pubkey();
    // Ethereum's asset 1, with a bond counted here; lock #5 of 7 of it.
    let asset1 = asset_rec(ETHEREUM, 1, &[5u8; 32], 6);
    let lock = lock_rec(ETHEREUM, 1, 5, 7 * GWEI_PER_ETH, &alice.to_bytes(), 0, 0, LOCKED_AT);
    let (pair, real, user_veth) =
        with_veth(&mut w, 10 * GWEI_PER_ETH, vec![[asset1.clone(), bond_rec(ETHEREUM, ETHEREUM, 1, 100 * GWEI_PER_ETH)].concat(), lock.clone()]);
    w.sub(&pair, 2, real, &[]).unwrap();
    let c = w.accept_last();
    w.make_receipt(c, 1, asset1).unwrap();
    attester_funds(&mut w, user_veth, 2 * GWEI_PER_ETH);
    // The attester names lock #5 as 1 vETH.
    let wrong = eth_lock(5, GWEI_PER_ETH, &alice, 0, 0);
    let n = w.attest(&op, &wrong).unwrap();
    let supply = w.supply();
    w.sub(&pair, 3, real, &[]).unwrap();
    let id = w.accept_last();
    w.later(WEEK);
    w.settle_fast(&g, id, n, &op.pubkey(), Some(ata(&alice))).unwrap();
    // The wrong vETH burned from its collateral, nothing minted for lock #5.
    assert_eq!(w.supply(), supply - GWEI_PER_ETH);
    assert_eq!(w.credit(&g.pubkey(), ETHEREUM, 0), GWEI_PER_ETH / 4);
    // The true receipt, by the slow issue.
    let payer = w.stranger.insecure_clone();
    let alice1 = litesvm_token::CreateAssociatedTokenAccount::new(&mut w.ctx.svm, &payer, &receipt(1)).owner(&alice).send().unwrap();
    w.issue(id, &lock, alice1).unwrap();
    assert_eq!(w.tokens(&alice1), 7 * GWEI_PER_ETH);
}
