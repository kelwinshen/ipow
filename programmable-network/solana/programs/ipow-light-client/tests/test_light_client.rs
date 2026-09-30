//! Tests of the Solana light client, mirroring `test/iPoWLightClient.test.ts`
//! on Ethereum. Spec: docs/design/ipow-protocol.md, section 2.
//!
//! The program under test is the test build (`--features test-limits`), in
//! `target/deploy-test/`. It keeps every rule, except that `initialize` may
//! lower the minimum difficulty so that a test can mine its own blocks. The
//! tests with real Bitcoin blocks set the real minimum.

mod fixtures;

use anchor_lang;

use anchor_lang::solana_program::instruction::{AccountMeta, Instruction};
use anchor_litesvm::{AnchorContext, AnchorLiteSVM, TestHelpers};
use sha2::{Digest, Sha256};
use solana_keypair::Keypair;
use solana_program::pubkey::Pubkey;
use solana_signer::Signer;

anchor_lang::declare_program!(ipow_light_client);
use crate::ipow_light_client::types::BlockRef;

const HOUR: i64 = 3600;
const WEEK: i64 = 7 * 24 * HOUR;
const E479: u32 = 965_664;
const E480: u32 = 967_680;
const EASY: u32 = 0x207f_ffff;

// ---------------------------------------------------------------------
// Bitcoin helpers, written independently of the program
// ---------------------------------------------------------------------

fn sha256d(data: &[u8]) -> [u8; 32] {
    let once = Sha256::digest(data);
    let twice = Sha256::digest(once);
    twice.into()
}

fn header(height: u32) -> [u8; 80] {
    let hex_str = fixtures::MAINNET_HEADERS
        .iter()
        .find(|(h, _)| *h == height)
        .unwrap_or_else(|| panic!("no fixture for {height}"))
        .1;
    hex::decode(hex_str).unwrap().try_into().unwrap()
}

fn time_of(h: &[u8; 80]) -> u32 {
    u32::from_le_bytes(h[68..72].try_into().unwrap())
}

fn bits_of(h: &[u8; 80]) -> u32 {
    u32::from_le_bytes(h[72..76].try_into().unwrap())
}

fn prev_of(h: &[u8; 80]) -> [u8; 32] {
    h[4..36].try_into().unwrap()
}

/// Target of a compact difficulty, big-endian.
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

/// Mines a header at an easy difficulty. Only the test build accepts it.
fn mine(prev: [u8; 32], time: u32, bits: u32, salt: u32) -> [u8; 80] {
    let mut h = [0u8; 80];
    h[0..4].copy_from_slice(&0x2000_0000u32.to_le_bytes());
    h[4..36].copy_from_slice(&prev);
    h[36..68].copy_from_slice(&sha256d(&salt.to_le_bytes()));
    h[68..72].copy_from_slice(&time.to_le_bytes());
    h[72..76].copy_from_slice(&bits.to_le_bytes());
    let target = target_be(bits);
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

// ---------------------------------------------------------------------
// Program helpers
// ---------------------------------------------------------------------

struct Env {
    ctx: AnchorContext,
    payer: Keypair,
    salt: u32,
    last_units: u64,
}

fn pda(seeds: &[&[u8]]) -> Pubkey {
    Pubkey::find_program_address(seeds, &crate::ipow_light_client::ID).0
}

fn config_pda() -> Pubkey {
    pda(&[b"config"])
}

fn days_pda() -> Pubkey {
    pda(&[b"days"])
}

fn node_pda(hash: &[u8; 32], height: u32, epoch_time: u32) -> Pubkey {
    pda(&[b"node", hash, &height.to_le_bytes(), &epoch_time.to_le_bytes()])
}

fn es_pda(hash: &[u8; 32], height: u32, first_time: u32) -> Pubkey {
    pda(&[b"epoch_start", hash, &height.to_le_bytes(), &first_time.to_le_bytes()])
}

fn compute_budget(units: u32) -> Instruction {
    let mut data = vec![2u8];
    data.extend_from_slice(&units.to_le_bytes());
    Instruction {
        program_id: "ComputeBudget111111111111111111111111111111".parse().unwrap(),
        accounts: vec![],
        data,
    }
}

fn pow_limit_be() -> [u8; 32] {
    let mut t = [0u8; 32];
    t[4] = 0xff;
    t[5] = 0xff;
    t
}

fn min_difficulty_be() -> [u8; 32] {
    let mut t = [0u8; 32];
    t[9] = 0x07;
    t[10] = 0xff;
    t[11] = 0xf8;
    t
}

impl Env {
    /// Real rules, with the lowest block number `min_height`.
    fn real(min_height: u32) -> Self {
        Self::new(min_height, min_difficulty_be(), pow_limit_be())
    }

    /// The minimum difficulty lowered so that a test can mine blocks.
    fn easy() -> Self {
        let t = target_be(EASY);
        Self::new(0, t, t)
    }

    fn new(min_height: u32, max_target: [u8; 32], pow_limit: [u8; 32]) -> Self {
        let ctx = AnchorLiteSVM::build_with_program(
            crate::ipow_light_client::ID,
            include_bytes!("../../../target/deploy-test/ipow_light_client.so"),
        );
        let mut env = Env { ctx, payer: Keypair::new(), salt: 0, last_units: 0 };
        env.payer = env.ctx.svm.create_funded_account(1_000_000_000_000).unwrap();
        let ix = env
            .ctx
            .program()
            .accounts(crate::ipow_light_client::client::accounts::Initialize {
                config: config_pda(),
                day_table: days_pda(),
                authority: env.payer.pubkey(),
                program_data: anchor_lang::solana_program::system_program::ID,
                system_program: anchor_lang::solana_program::system_program::ID,
            })
            .args(crate::ipow_light_client::client::args::Initialize { min_height, max_target, pow_limit })
            .instruction()
            .unwrap();
        env.send(ix).unwrap();
        env
    }

    fn set_now(&mut self, t: i64) {
        let mut clock: solana_clock::Clock = self.ctx.svm.get_sysvar();
        clock.unix_timestamp = t;
        self.ctx.svm.set_sysvar(&clock);
    }

    /// Sends one instruction with a large compute budget. Returns the
    /// program's logs on failure.
    fn send(&mut self, ix: Instruction) -> Result<(), String> {
        self.ctx.svm.expire_blockhash();
        let payer = self.payer.insecure_clone();
        let result = self
            .ctx
            .execute_instructions(vec![compute_budget(1_400_000), ix], &[&payer])
            .unwrap();
        self.last_units = result.compute_units();
        if result.is_success() {
            Ok(())
        } else {
            Err(result.logs().join("\n"))
        }
    }

    fn with_remaining(mut ix: Instruction, accounts: &[Pubkey]) -> Instruction {
        for a in accounts {
            ix.accounts.push(AccountMeta::new(*a, false));
        }
        ix
    }

    fn add_epoch_start(&mut self, headers: &[[u8; 80]], height: u32) -> Result<Pubkey, String> {
        let first_time = time_of(&headers[0]);
        let nodes: Vec<Pubkey> = headers
            .iter()
            .enumerate()
            .map(|(i, h)| node_pda(&sha256d(h), height + i as u32, first_time))
            .collect();
        let es = es_pda(&sha256d(&headers[0]), height, first_time);
        let ix = self
            .ctx
            .program()
            .accounts(crate::ipow_light_client::client::accounts::AddEpochStart {
                config: config_pda(),
                day_table: days_pda(),
                epoch_start: es,
                payer: self.payer.pubkey(),
                system_program: anchor_lang::solana_program::system_program::ID,
            })
            .args(crate::ipow_light_client::client::args::AddEpochStart { headers: headers.concat(), height })
            .instruction()
            .unwrap();
        self.send(Self::with_remaining(ix, &nodes))?;
        Ok(es)
    }

    fn jump(&mut self, h: [u8; 80], es: Pubkey, height: u32, epoch_time: u32) -> Result<Pubkey, String> {
        let node = node_pda(&sha256d(&h), height, epoch_time);
        let ix = self
            .ctx
            .program()
            .accounts(crate::ipow_light_client::client::accounts::Jump {
                config: config_pda(),
                day_table: days_pda(),
                epoch_start: es,
                node,
                payer: self.payer.pubkey(),
                system_program: anchor_lang::solana_program::system_program::ID,
            })
            .args(crate::ipow_light_client::client::args::Jump { header: h, height })
            .instruction()
            .unwrap();
        self.send(ix)?;
        Ok(node)
    }

    /// Streams headers on top of the block `parent`, stored at
    /// (parent_height, parent_epoch_time). The epoch time of the new blocks
    /// follows the rule of a new epoch.
    fn extend(&mut self, parent: Pubkey, parent_height: u32, parent_epoch_time: u32, headers: &[[u8; 80]]) -> Result<(), String> {
        let mut height = parent_height;
        let mut epoch_time = parent_epoch_time;
        let mut nodes = vec![];
        for h in headers {
            height += 1;
            if height % 2016 == 0 {
                epoch_time = time_of(h);
            }
            nodes.push(node_pda(&sha256d(h), height, epoch_time));
        }
        let ix = self
            .ctx
            .program()
            .accounts(crate::ipow_light_client::client::accounts::Extend {
                config: config_pda(),
                parent,
                payer: self.payer.pubkey(),
                system_program: anchor_lang::solana_program::system_program::ID,
            })
            .args(crate::ipow_light_client::client::args::Extend { headers: headers.concat() })
            .instruction()
            .unwrap();
        self.send(Self::with_remaining(ix, &nodes))
    }

    fn extend_back(&mut self, h: [u8; 80], child: Pubkey, child_height: u32, parent_epoch_time: u32, prev_epoch_time: u32) -> Result<Pubkey, String> {
        let parent = node_pda(&sha256d(&h), child_height - 1, parent_epoch_time);
        let ix = self
            .ctx
            .program()
            .accounts(crate::ipow_light_client::client::accounts::ExtendBack {
                config: config_pda(),
                child,
                parent,
                payer: self.payer.pubkey(),
                system_program: anchor_lang::solana_program::system_program::ID,
            })
            .args(crate::ipow_light_client::client::args::ExtendBack { header: h, prev_epoch_time })
            .instruction()
            .unwrap();
        self.send(ix)?;
        Ok(parent)
    }

    /// Walks from `descendant` down to `ancestor_height`, `per_step` blocks
    /// per transaction. `path` lists the node accounts from the descendant
    /// down. Returns the finished walk.
    fn walk(
        &mut self,
        walk_id: u64,
        descendant: BlockRef,
        ancestor: BlockRef,
        prev_epoch_time: u32,
        path: &[Pubkey],
        per_step: usize,
    ) -> Result<crate::ipow_light_client::accounts::Walk, String> {
        let walk = pda(&[b"walk", self.payer.pubkey().as_ref(), &walk_id.to_le_bytes()]);
        let ix = self
            .ctx
            .program()
            .accounts(crate::ipow_light_client::client::accounts::BeginWalk {
                walk,
                payer: self.payer.pubkey(),
                system_program: anchor_lang::solana_program::system_program::ID,
            })
            .args(crate::ipow_light_client::client::args::BeginWalk { walk_id, descendant, ancestor, prev_epoch_time })
            .instruction()
            .unwrap();
        self.send(ix)?;
        for chunk in path.chunks(per_step) {
            let ix = self
                .ctx
                .program()
                .accounts(crate::ipow_light_client::client::accounts::WalkStep { config: config_pda(), walk })
                .args(crate::ipow_light_client::client::args::WalkStep {})
                .instruction()
                .unwrap();
            self.send(Self::with_remaining(ix, chunk))?;
        }
        Ok(self.ctx.get_account(&walk).unwrap())
    }

    fn check_tx(&mut self, node: Pubkey, raw: Vec<u8>, siblings: Vec<[u8; 32]>, index: u64) -> Result<(), String> {
        let ix = self
            .ctx
            .program()
            .accounts(crate::ipow_light_client::client::accounts::CheckTx { node })
            .args(crate::ipow_light_client::client::args::CheckTx { raw_tx: raw, siblings, index })
            .instruction()
            .unwrap();
        self.send(ix)
    }

    fn node(&self, address: &Pubkey) -> crate::ipow_light_client::accounts::Node {
        self.ctx.get_account(address).unwrap()
    }

    fn next_salt(&mut self) -> u32 {
        self.salt += 1;
        self.salt
    }

    /// Mines `count` easy blocks in a row on `prev`.
    fn mine_chain(&mut self, prev: [u8; 32], first_time: u32, count: usize) -> Vec<[u8; 80]> {
        let mut out = vec![];
        let mut p = prev;
        for i in 0..count {
            let salt = self.next_salt();
            let h = mine(p, first_time + i as u32 * 600, EASY, salt);
            p = sha256d(&h);
            out.push(h);
        }
        out
    }
}

fn expect_err(result: Result<impl std::fmt::Debug, String>, code: &str) {
    match result {
        Ok(v) => panic!("expected {code}, got success {v:?}"),
        Err(logs) => assert!(
            logs.contains(&format!("Error Code: {code}")),
            "expected {code}, got:\n{logs}"
        ),
    }
}

fn range(from: u32, to: u32) -> Vec<[u8; 80]> {
    (from..=to).map(header).collect()
}

fn block_ref(h: &[u8; 80], height: u32, epoch_time: u32) -> BlockRef {
    BlockRef { hash: sha256d(h), height, epoch_time }
}

fn assert_same(a: &BlockRef, b: &BlockRef) {
    assert_eq!((a.hash, a.height, a.epoch_time), (b.hash, b.height, b.epoch_time));
}

fn es480() -> Vec<[u8; 80]> {
    range(E480, E480 + 5)
}

fn es480_time() -> u32 {
    time_of(&header(E480))
}

fn with_es480() -> (Env, Pubkey) {
    let mut env = Env::real(0);
    env.set_now(time_of(&header(E480 + 5)) as i64 + 600);
    let es = env.add_epoch_start(&es480(), E480).unwrap();
    (env, es)
}

// ---------------------------------------------------------------------
// Real Bitcoin blocks, real rules
// ---------------------------------------------------------------------

#[test]
fn records_an_epoch_start_and_stores_its_six_blocks() {
    let (env, es) = with_es480();
    let record: crate::ipow_light_client::accounts::EpochStart = env.ctx.get_account(&es).unwrap();
    assert_eq!(record.bits, bits_of(&header(E480)));
    assert_eq!(record.height, E480);
    assert_eq!(record.first_time, es480_time());
    for i in 0..6 {
        let node = env.node(&node_pda(&sha256d(&header(E480 + i)), E480 + i, es480_time()));
        assert_eq!(node.height, E480 + i);
        assert_eq!(node.prev_hash, prev_of(&header(E480 + i)));
    }
}

#[test]
fn rejects_an_epoch_start_at_a_height_that_does_not_start_an_epoch() {
    let mut env = Env::real(0);
    env.set_now(es480_time() as i64 + HOUR);
    expect_err(env.add_epoch_start(&es480(), E480 + 1), "InvalidHeight");
}

#[test]
fn rejects_an_epoch_start_below_the_lowest_block_number() {
    let mut env = Env::real(E480);
    env.set_now(time_of(&header(E479 + 5)) as i64 + 600);
    expect_err(env.add_epoch_start(&range(E479, E479 + 5), E479), "BelowMinHeight");
}

#[test]
fn rejects_blocks_that_do_not_name_each_other_or_mix_difficulties() {
    let mut env = Env::real(0);
    env.set_now(time_of(&header(E480 + 6)) as i64 + 600);
    let mut gap = range(E480, E480 + 4);
    gap.push(header(E480 + 6));
    expect_err(env.add_epoch_start(&gap, E480), "NotLinked");
    expect_err(env.add_epoch_start(&range(E480 - 1, E480 + 4), E480), "DifficultyMismatch");
}

#[test]
fn changes_nothing_when_an_epoch_start_is_recorded_again() {
    let (mut env, es) = with_es480();
    let before: crate::ipow_light_client::accounts::EpochStart = env.ctx.get_account(&es).unwrap();
    env.set_now(time_of(&header(E480 + 5)) as i64 + 6000);
    env.add_epoch_start(&es480(), E480).unwrap();
    let after: crate::ipow_light_client::accounts::EpochStart = env.ctx.get_account(&es).unwrap();
    assert_eq!(before.recorded_at, after.recorded_at);
}

#[test]
fn rejects_blocks_below_the_minimum_difficulty() {
    let mut env = Env::real(0);
    let genesis: [u8; 80] = hex::decode("0100000000000000000000000000000000000000000000000000000000000000000000003ba3edfd7a7b12b27ac72c3e67768f617fc81bc3888a51323a9fb8aa4b1e5e4a29ab5f49ffff001d1dac2b7c").unwrap().try_into().unwrap();
    env.set_now(time_of(&genesis) as i64 + HOUR);
    expect_err(env.add_epoch_start(&[genesis; 6], 0), "DifficultyTooLow");
}

#[test]
fn jumps_to_a_fresh_real_block_with_the_difficulty_of_its_epoch_start() {
    let (mut env, es) = with_es480();
    let anchor = header(E480 + 20);
    env.set_now(time_of(&anchor) as i64 + 300);
    let node = env.jump(anchor, es, E480 + 20, es480_time()).unwrap();
    let n = env.node(&node);
    assert_eq!(n.height, E480 + 20);
    assert_eq!(n.anchored_at, time_of(&anchor) as i64 + 300);
}

#[test]
fn rejects_an_anchor_older_than_two_hours_and_accepts_one_exactly_two_hours_old() {
    let (mut env, es) = with_es480();
    let anchor = header(E480 + 20);
    env.set_now(time_of(&anchor) as i64 + 2 * HOUR + 1);
    expect_err(env.jump(anchor, es, E480 + 20, es480_time()), "AnchorTooOld");
    env.set_now(time_of(&anchor) as i64 + 2 * HOUR);
    env.jump(anchor, es, E480 + 20, es480_time()).unwrap();
}

#[test]
fn rejects_an_anchor_with_another_difficulty_or_a_changed_nonce() {
    let (mut env, es) = with_es480();
    let other = header(E480 - 1);
    env.set_now(time_of(&other) as i64 + 300);
    expect_err(env.jump(other, es, E480, es480_time()), "DifficultyMismatch");

    let mut tampered = header(E480 + 20);
    tampered[79] ^= 0xff;
    env.set_now(time_of(&tampered) as i64 + 300);
    expect_err(env.jump(tampered, es, E480 + 20, es480_time()), "InsufficientWork");
}

#[test]
fn rejects_another_block_at_the_first_height_of_the_epoch() {
    let (mut env, es) = with_es480();
    let other = header(E480 + 1);
    env.set_now(time_of(&other) as i64 + 300);
    expect_err(env.jump(other, es, E480, es480_time()), "EpochTimeMismatch");
}

#[test]
fn streams_blocks_and_walks_back_to_the_anchor_in_steps() {
    let (mut env, es) = with_es480();
    let a = E480 + 10;
    env.set_now(time_of(&header(a)) as i64 + 300);
    let anchor = env.jump(header(a), es, a, es480_time()).unwrap();
    env.set_now(time_of(&header(E480 + 40)) as i64 + 300);
    let mut parent = anchor;
    for start in (a + 1..=a + 21).step_by(7) {
        env.extend(parent, start - 1, es480_time(), &range(start, start + 6)).unwrap();
        parent = node_pda(&sha256d(&header(start + 6)), start + 6, es480_time());
    }

    let path: Vec<Pubkey> = (a..=a + 21)
        .rev()
        .map(|h| node_pda(&sha256d(&header(h)), h, es480_time()))
        .collect();
    let walk = env
        .walk(1, block_ref(&header(a + 21), a + 21, es480_time()), block_ref(&header(a), a, es480_time()), 0, &path, 10)
        .unwrap();
    assert!(walk.finished);
    assert_same(&walk.last, &block_ref(&header(a), a, es480_time()));
}

#[test]
fn rejects_more_than_seven_headers_in_one_call() {
    let (mut env, es) = with_es480();
    let a = E480 + 10;
    env.set_now(time_of(&header(a)) as i64 + 300);
    let anchor = env.jump(header(a), es, a, es480_time()).unwrap();
    env.set_now(time_of(&header(a + 8)) as i64 + 300);
    expect_err(env.extend(anchor, a, es480_time(), &range(a + 1, a + 8)), "InvalidLength");
}

#[test]
fn rejects_a_gap_while_streaming() {
    let (mut env, es) = with_es480();
    let a = E480 + 10;
    env.set_now(time_of(&header(a)) as i64 + 300);
    let anchor = env.jump(header(a), es, a, es480_time()).unwrap();
    env.set_now(time_of(&header(E480 + 20)) as i64 + 300);
    expect_err(env.extend(anchor, a, es480_time(), &[header(a + 2)]), "NotLinked");
}

#[test]
fn does_not_walk_through_a_block_that_is_not_stored() {
    let (mut env, es) = with_es480();
    let a = E480 + 10;
    env.set_now(time_of(&header(a + 6)) as i64 + 300);
    env.jump(header(a), es, a, es480_time()).unwrap();
    env.jump(header(a + 6), es, a + 6, es480_time()).unwrap();
    let path: Vec<Pubkey> = (a..=a + 6)
        .rev()
        .map(|h| node_pda(&sha256d(&header(h)), h, es480_time()))
        .collect();
    expect_err(
        env.walk(1, block_ref(&header(a + 6), a + 6, es480_time()), block_ref(&header(a), a, es480_time()), 0, &path, 10),
        "UnknownBlock",
    );
}

#[test]
fn refuses_a_walk_longer_than_the_longest_window() {
    let (mut env, _) = with_es480();
    expect_err(
        env.walk(1, block_ref(&header(E480), E480 + 101, es480_time()), block_ref(&header(E480), E480, es480_time()), 0, &[], 10),
        "WalkTooLong",
    );
}

#[test]
fn streams_across_into_a_new_epoch_and_walks_back_with_the_old_epoch_time() {
    let mut env = Env::real(0);
    let last = E480 - 1;
    let es479_time = time_of(&header(E479));
    env.set_now(time_of(&header(E479 + 5)) as i64 + 600);
    let es = env.add_epoch_start(&range(E479, E479 + 5), E479).unwrap();
    env.set_now(time_of(&header(last)) as i64 + 60);
    let anchor = env.jump(header(last), es, last, es479_time).unwrap();
    env.set_now(time_of(&header(E480 + 8)) as i64 + 60);
    env.extend(anchor, last, es479_time, &range(E480, E480 + 6)).unwrap();
    let seventh = node_pda(&sha256d(&header(E480 + 6)), E480 + 6, es480_time());
    env.extend(seventh, E480 + 6, es480_time(), &[header(E480 + 7)]).unwrap();

    let first = env.node(&node_pda(&sha256d(&header(E480)), E480, es480_time()));
    assert_eq!(first.epoch_time, es480_time());
    assert_eq!(first.bits, bits_of(&header(E480)));

    let mut path: Vec<Pubkey> = (E480..=E480 + 7)
        .rev()
        .map(|h| node_pda(&sha256d(&header(h)), h, es480_time()))
        .collect();
    path.push(anchor);
    let walk = env
        .walk(1, block_ref(&header(E480 + 7), E480 + 7, es480_time()), block_ref(&header(last), last, es479_time), es479_time, &path, 5)
        .unwrap();
    assert!(walk.finished);
    assert_same(&walk.last, &block_ref(&header(last), last, es479_time));

    // With a wrong time for the older epoch the walk fails.
    expect_err(
        env.walk(2, block_ref(&header(E480 + 7), E480 + 7, es480_time()), block_ref(&header(last), last, es479_time), es479_time + 1, &path, 10),
        "WrongAccount",
    );
}

#[test]
fn stores_the_real_parent_of_an_anchor_and_refuses_one_below_the_lowest_number() {
    let (mut env, es) = with_es480();
    let a = E480 + 20;
    env.set_now(time_of(&header(a)) as i64 + 300);
    let anchor = env.jump(header(a), es, a, es480_time()).unwrap();
    let parent = env.extend_back(header(a - 1), anchor, a, es480_time(), 0).unwrap();
    assert_eq!(env.node(&parent).height, a - 1);
    expect_err(env.extend_back(header(a - 2), anchor, a, es480_time(), 0), "NotLinked");

    let mut low = Env::real(E480);
    low.set_now(time_of(&header(E480 + 5)) as i64 + 600);
    low.add_epoch_start(&es480(), E480).unwrap();
    let first = node_pda(&sha256d(&header(E480)), E480, es480_time());
    expect_err(
        low.extend_back(header(E480 - 1), first, E480, time_of(&header(E479)), time_of(&header(E479))),
        "BelowMinHeight",
    );
}

#[test]
fn finds_a_real_transaction_with_its_real_proof() {
    let (mut env, _) = with_es480();
    let node = node_pda(&sha256d(&header(fixtures::TX_HEIGHT)), fixtures::TX_HEIGHT, es480_time());
    let raw = hex::decode(fixtures::TX_RAW).unwrap();
    let siblings: Vec<[u8; 32]> = fixtures::TX_MERKLE
        .iter()
        .map(|s| {
            let mut b: [u8; 32] = hex::decode(s).unwrap().try_into().unwrap();
            b.reverse();
            b
        })
        .collect();
    env.check_tx(node, raw.clone(), siblings.clone(), fixtures::TX_INDEX).unwrap();
    expect_err(env.check_tx(node, raw.clone(), siblings.clone(), fixtures::TX_INDEX + 1), "NotInBlock");
    let mut changed = raw.clone();
    *changed.last_mut().unwrap() ^= 0xff;
    expect_err(env.check_tx(node, changed, siblings.clone(), fixtures::TX_INDEX), "NotInBlock");
    expect_err(env.check_tx(node, vec![0x11; 64], siblings, 0), "InvalidTransaction");
}

#[test]
fn counts_both_real_epoch_starts_two_weeks_apart() {
    let mut env = Env::real(0);
    env.set_now(time_of(&header(E479 + 5)) as i64 + 600);
    let es479 = env.add_epoch_start(&range(E479, E479 + 5), E479).unwrap();
    env.set_now(time_of(&header(E480 + 5)) as i64 + 600);
    env.add_epoch_start(&es480(), E480).unwrap();
    let anchor = header(E480 + 20);
    env.set_now(time_of(&anchor) as i64 + 300);
    // Epoch 480 is slightly harder than 479, so 479 still has more than half
    // of the highest difficulty: an anchor of each epoch can jump.
    let es480 = es_pda(&sha256d(&header(E480)), E480, es480_time());
    env.jump(anchor, es480, E480 + 20, es480_time()).unwrap();
    let old_anchor = header(E480 - 1);
    env.set_now(time_of(&old_anchor) as i64 + 300);
    env.jump(old_anchor, es479, E480 - 1, time_of(&header(E479))).unwrap();
}

// ---------------------------------------------------------------------
// Blocks mined by the test, easy difficulty
// ---------------------------------------------------------------------

const T0: u32 = 1_800_000_000;

fn easy_epoch_start(env: &mut Env, bits: u32, first_time: u32, height: u32) -> (Pubkey, Vec<[u8; 80]>) {
    let mut headers = vec![];
    let mut prev = [0u8; 32];
    for i in 0..6 {
        let salt = env.next_salt();
        let h = mine(prev, first_time + i * 600, bits, salt);
        prev = sha256d(&h);
        headers.push(h);
    }
    let es = env.add_epoch_start(&headers, height).unwrap();
    (es, headers)
}

#[test]
fn stops_counting_a_weak_epoch_start_when_a_harder_one_is_recorded() {
    let mut env = Env::easy();
    env.set_now(T0 as i64 + HOUR);
    let (weak, _) = easy_epoch_start(&mut env, EASY, T0, 0);
    let salt = env.next_salt();
    let before = mine([7u8; 32], T0 + HOUR as u32, EASY, salt);
    env.jump(before, weak, 10, T0).unwrap();

    // Just over twice as hard.
    easy_epoch_start(&mut env, 0x203f_ffff, T0, 2016);
    let salt = env.next_salt();
    let after = mine([8u8; 32], T0 + HOUR as u32, EASY, salt);
    expect_err(env.jump(after, weak, 11, T0), "EpochStartTooWeak");
}

#[test]
fn compares_only_epoch_starts_of_the_last_four_weeks() {
    let mut env = Env::easy();
    env.set_now(T0 as i64 + HOUR);
    easy_epoch_start(&mut env, 0x203f_ffff, T0, 0);
    let later = T0 + (4 * WEEK) as u32 + 24 * HOUR as u32;
    env.set_now(later as i64 + HOUR);
    let (weak, _) = easy_epoch_start(&mut env, EASY, later, 4032);
    let salt = env.next_salt();
    let anchor = mine([9u8; 32], later + HOUR as u32, EASY, salt);
    env.jump(anchor, weak, 4032 + 10, later).unwrap();
}

#[test]
fn keeps_two_branches_apart_and_walks_only_along_its_own() {
    let mut env = Env::easy();
    env.set_now(T0 as i64 + HOUR);
    let (es, _) = easy_epoch_start(&mut env, EASY, T0, 0);
    let salt = env.next_salt();
    let anchor_h = mine([1u8; 32], T0 + HOUR as u32, EASY, salt);
    let anchor = env.jump(anchor_h, es, 10, T0).unwrap();

    let a = env.mine_chain(sha256d(&anchor_h), T0 + HOUR as u32 + 600, 3);
    let b = env.mine_chain(sha256d(&anchor_h), T0 + HOUR as u32 + 600, 2);
    env.extend(anchor, 10, T0, &a).unwrap();
    env.extend(anchor, 10, T0, &b).unwrap();

    let path_a: Vec<Pubkey> = vec![
        node_pda(&sha256d(&a[2]), 13, T0),
        node_pda(&sha256d(&a[1]), 12, T0),
        node_pda(&sha256d(&a[0]), 11, T0),
        anchor,
    ];
    let walk = env.walk(1, block_ref(&a[2], 13, T0), block_ref(&anchor_h, 10, T0), 0, &path_a, 10).unwrap();
    assert!(walk.finished);
    assert_eq!(walk.last.hash, sha256d(&anchor_h));

    // A block of branch b is not on branch a's path.
    let wrong = vec![node_pda(&sha256d(&a[2]), 13, T0), node_pda(&sha256d(&b[1]), 12, T0)];
    expect_err(env.walk(2, block_ref(&a[2], 13, T0), block_ref(&b[0], 11, T0), 0, &wrong, 10), "WrongAccount");

    // A walk along branch a to the block of branch b at the same height does
    // not finish: the block reached is not the one stated.
    let along_a = vec![node_pda(&sha256d(&a[2]), 13, T0), node_pda(&sha256d(&a[1]), 12, T0), node_pda(&sha256d(&a[0]), 11, T0)];
    expect_err(env.walk(3, block_ref(&a[2], 13, T0), block_ref(&b[0], 11, T0), 0, &along_a, 10), "NotLinked");
}

#[test]
fn rejects_a_block_with_another_difficulty_inside_an_epoch() {
    let mut env = Env::easy();
    env.set_now(T0 as i64 + HOUR);
    let (es, _) = easy_epoch_start(&mut env, EASY, T0, 0);
    let salt = env.next_salt();
    let anchor_h = mine([1u8; 32], T0 + HOUR as u32, EASY, salt);
    let anchor = env.jump(anchor_h, es, 10, T0).unwrap();
    let salt = env.next_salt();
    let other = mine(sha256d(&anchor_h), T0 + HOUR as u32 + 600, 0x2040_0000, salt);
    expect_err(env.extend(anchor, 10, T0, &[other]), "DifficultyMismatch");
}

#[test]
fn adds_up_the_work_of_the_blocks_on_the_way() {
    let mut env = Env::easy();
    env.set_now(T0 as i64 + HOUR);
    let (es, _) = easy_epoch_start(&mut env, EASY, T0, 0);
    let salt = env.next_salt();
    let anchor_h = mine([1u8; 32], T0 + HOUR as u32, EASY, salt);
    let anchor = env.jump(anchor_h, es, 10, T0).unwrap();
    let chain = env.mine_chain(sha256d(&anchor_h), T0 + HOUR as u32 + 600, 3);
    env.extend(anchor, 10, T0, &chain).unwrap();

    let path: Vec<Pubkey> = vec![
        node_pda(&sha256d(&chain[2]), 13, T0),
        node_pda(&sha256d(&chain[1]), 12, T0),
        node_pda(&sha256d(&chain[0]), 11, T0),
        anchor,
    ];
    let walk = env.walk(1, block_ref(&chain[2], 13, T0), block_ref(&anchor_h, 10, T0), 0, &path, 10).unwrap();
    // The work of one block at 0x207fffff is 2; three blocks after the anchor.
    let mut expected = [0u8; 32];
    expected[31] = 6;
    assert_eq!(walk.work, expected);
}

#[test]
fn reports_the_compute_units_of_each_instruction() {
    let mut env = Env::real(0);
    env.set_now(time_of(&header(E480 + 5)) as i64 + 600);
    let es = env.add_epoch_start(&es480(), E480).unwrap();
    let epoch_start_units = env.last_units;
    let a = E480 + 10;
    env.set_now(time_of(&header(a)) as i64 + 60);
    let anchor = env.jump(header(a), es, a, es480_time()).unwrap();
    let jump_units = env.last_units;
    env.set_now(time_of(&header(a + 8)) as i64 + 60);
    env.extend(anchor, a, es480_time(), &range(a + 1, a + 7)).unwrap();
    let extend_units = env.last_units;
    let path: Vec<Pubkey> = (a..=a + 7).rev().map(|h| node_pda(&sha256d(&header(h)), h, es480_time())).collect();
    env.walk(1, block_ref(&header(a + 7), a + 7, es480_time()), block_ref(&header(a), a, es480_time()), 0, &path, 8).unwrap();
    let walk_units = env.last_units;
    println!(
        "compute units: epoch start {epoch_start_units}, jump {jump_units}, extend 7 blocks {extend_units}, walk step of 8 blocks {walk_units}"
    );
    // Every one fits the 1,400,000 limit of a transaction.
    for u in [epoch_start_units, jump_units, extend_units, walk_units] {
        assert!(u < 1_400_000);
    }
}

#[test]
fn stores_a_block_and_an_epoch_start_even_when_someone_sent_lamports_to_their_address_first() {
    let mut env = Env::real(0);
    env.set_now(time_of(&header(E480 + 5)) as i64 + 600);
    // An attacker pre-funds the address of every block of the epoch start,
    // and of the epoch start record.
    let griefer = env.ctx.svm.create_funded_account(10_000_000_000).unwrap();
    let mut targets: Vec<Pubkey> = (0..6)
        .map(|i| node_pda(&sha256d(&header(E480 + i)), E480 + i, es480_time()))
        .collect();
    targets.push(es_pda(&sha256d(&header(E480)), E480, es480_time()));
    for t in &targets {
        let ix = anchor_lang::solana_program::system_instruction::transfer(&griefer.pubkey(), t, 1_000_000);
        env.ctx.svm.expire_blockhash();
        env.ctx.execute_instruction(ix, &[&griefer]).unwrap().assert_success();
    }
    env.add_epoch_start(&es480(), E480).unwrap();
    let node = env.node(&targets[0]);
    assert_eq!(node.height, E480);
}

#[test]
fn closes_a_block_eight_weeks_after_it_was_stored_and_returns_its_rent_to_whoever_stored_it() {
    let (mut env, es) = with_es480();
    let node = node_pda(&sha256d(&header(E480)), E480, es480_time());
    let stored_at = env.node(&node).stored_at;
    let payer = env.payer.pubkey();
    let stranger = env.ctx.svm.create_funded_account(1_000_000_000).unwrap();
    let close = |env: &mut Env, payer: Pubkey| {
        let ix = env
            .ctx
            .program()
            .accounts(crate::ipow_light_client::client::accounts::CloseNode { node, payer })
            .args(crate::ipow_light_client::client::args::CloseNode {})
            .instruction()
            .unwrap();
        env.ctx.svm.expire_blockhash();
        let result = env.ctx.execute_instruction(ix, &[&stranger]).unwrap();
        if result.is_success() { Ok(()) } else { Err(result.logs().join("\n")) }
    };

    env.set_now(stored_at + 8 * WEEK - 1);
    expect_err(close(&mut env, payer), "TooEarlyToClose");
    env.set_now(stored_at + 8 * WEEK);
    // The rent can only go to whoever stored the block.
    expect_err(close(&mut env, stranger.pubkey()), "ConstraintHasOne");
    let rent = env.ctx.svm.get_account(&node).unwrap().lamports;
    let before = env.ctx.svm.get_account(&payer).unwrap().lamports;
    close(&mut env, payer).unwrap();
    assert_eq!(env.ctx.svm.get_account(&payer).unwrap().lamports, before + rent);
    assert!(env.ctx.svm.get_account(&node).map(|a| a.lamports == 0).unwrap_or(true));

    // The epoch start record too.
    let ix = env
        .ctx
        .program()
        .accounts(crate::ipow_light_client::client::accounts::CloseEpochStart { epoch_start: es, payer })
        .args(crate::ipow_light_client::client::args::CloseEpochStart {})
        .instruction()
        .unwrap();
    env.ctx.svm.expire_blockhash();
    assert!(env.ctx.execute_instruction(ix, &[&stranger]).unwrap().is_success());
}
