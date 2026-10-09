//! Tests of the Solana protocol, first piece: bond, applications, jobs, fees
//! and the auction. They mirror `test/protocol/iPoWProtocol.test.ts` on Ethereum.
//! Spec: docs/design/ipow-protocol.md, sections 3 to 5.

use anchor_lang;
use anchor_lang::solana_program::instruction::{AccountMeta, Instruction};
use anchor_litesvm::{AnchorContext, AnchorLiteSVM, TestHelpers};
use solana_keypair::Keypair;
use solana_program::pubkey::Pubkey;
use solana_signer::Signer;

anchor_lang::declare_program!(ipow_protocol);

const SOL: u64 = 1_000_000_000;
const MINUTE: i64 = 60;
const HOUR: i64 = 3600;
const DAY: i64 = 24 * HOUR;
const T0: i64 = 1_800_000_000;
const SYSTEM: Pubkey = anchor_lang::solana_program::system_program::ID;

fn pda(seeds: &[&[u8]]) -> Pubkey {
    Pubkey::find_program_address(seeds, &ipow_protocol::ID).0
}

fn protocol_pda() -> Pubkey {
    pda(&[b"protocol"])
}
fn vault_pda() -> Pubkey {
    pda(&[b"vault"])
}
fn operator_pda(owner: &Pubkey) -> Pubkey {
    pda(&[b"operator", owner.as_ref()])
}
fn application_pda(key: &Pubkey) -> Pubkey {
    pda(&[b"application", key.as_ref()])
}
fn job_pda(id: u64) -> Pubkey {
    pda(&[b"job", &id.to_le_bytes()])
}
fn tag_pda(app: &Pubkey, tag: &[u8; 32]) -> Pubkey {
    pda(&[b"tag", app.as_ref(), tag])
}
fn credit_pda(owner: &Pubkey) -> Pubkey {
    pda(&[b"credit", owner.as_ref()])
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

struct Env {
    ctx: AnchorContext,
    application: Keypair,
    user: Keypair,
    a: Keypair,
    b: Keypair,
    stranger: Keypair,
}

impl Env {
    fn new() -> Self {
        let mut ctx = AnchorLiteSVM::build_with_program(
            ipow_protocol::ID,
            include_bytes!("../../../../target/deploy/ipow_protocol.so"),
        );
        let application = ctx.svm.create_funded_account(1_000 * SOL).unwrap();
        let user = ctx.svm.create_funded_account(1_000 * SOL).unwrap();
        let a = ctx.svm.create_funded_account(1_000 * SOL).unwrap();
        let b = ctx.svm.create_funded_account(1_000 * SOL).unwrap();
        let stranger = ctx.svm.create_funded_account(1_000 * SOL).unwrap();
        let mut env = Env { ctx, application, user, a, b, stranger };
        env.set_now(T0);
        let payer = env.a.insecure_clone();
        let ix = env
            .ctx
            .program()
            .accounts(ipow_protocol::client::accounts::InitializeProtocol {
                protocol: protocol_pda(),
                vault: vault_pda(),
                payer: payer.pubkey(),
                system_program: SYSTEM,
            })
            .args(ipow_protocol::client::args::InitializeProtocol {})
            .instruction()
            .unwrap();
        env.send(ix, &payer).unwrap();
        let app = env.application.insecure_clone();
        env.register(&app, vec![]).unwrap();
        env
    }

    fn set_now(&mut self, t: i64) {
        let mut clock: solana_clock::Clock = self.ctx.svm.get_sysvar();
        clock.unix_timestamp = t;
        self.ctx.svm.set_sysvar(&clock);
    }

    fn now(&self) -> i64 {
        let clock: solana_clock::Clock = self.ctx.svm.get_sysvar();
        clock.unix_timestamp
    }

    fn send(&mut self, ix: Instruction, signer: &Keypair) -> Result<(), String> {
        self.send_many(ix, &[signer])
    }

    fn send_many(&mut self, ix: Instruction, signers: &[&Keypair]) -> Result<(), String> {
        self.ctx.svm.expire_blockhash();
        let result = self
            .ctx
            .execute_instructions(vec![compute_budget(1_400_000), ix], signers)
            .unwrap();
        if result.is_success() {
            Ok(())
        } else {
            Err(result.logs().join("\n"))
        }
    }

    fn register(&mut self, key: &Keypair, periods: Vec<u32>) -> Result<(), String> {
        let ix = self
            .ctx
            .program()
            .accounts(ipow_protocol::client::accounts::RegisterApplication {
                application: application_pda(&key.pubkey()),
                key: key.pubkey(),
                funder: key.pubkey(),
                system_program: SYSTEM,
            })
            .args(ipow_protocol::client::args::RegisterApplication { challenge_periods: periods })
            .instruction()
            .unwrap();
        self.send(ix, key)
    }

    fn lock_bond(&mut self, who: &Keypair, amount: u64) -> Result<(), String> {
        let ix = self
            .ctx
            .program()
            .accounts(ipow_protocol::client::accounts::LockBond {
                protocol: protocol_pda(),
                operator: operator_pda(&who.pubkey()),
                vault: vault_pda(),
                owner: who.pubkey(),
                system_program: SYSTEM,
            })
            .args(ipow_protocol::client::args::LockBond { amount })
            .instruction()
            .unwrap();
        self.send(ix, who)
    }

    fn withdraw_bond(&mut self, who: &Keypair, amount: u64) -> Result<(), String> {
        let ix = self
            .ctx
            .program()
            .accounts(ipow_protocol::client::accounts::WithdrawBond {
                protocol: protocol_pda(),
                operator: operator_pda(&who.pubkey()),
                vault: vault_pda(),
                owner: who.pubkey(),
            })
            .args(ipow_protocol::client::args::WithdrawBond { amount })
            .instruction()
            .unwrap();
        self.send(ix, who)
    }

    /// (window + 20 signatures) x 5,000 lamports x 1.5. For 30 blocks that
    /// is 375,000 lamports, 0.000375 SOL.
    fn fee(&self, confirmations: u16) -> u64 {
        let window = 24 + confirmations as u64;
        (window + 20) * 5_000 * 3 / 2
    }

    fn job_count(&self) -> u64 {
        let p: ipow_protocol::accounts::Protocol = self.ctx.get_account(&protocol_pda()).unwrap();
        p.job_count
    }

    #[allow(clippy::too_many_arguments)]
    fn open_job_as(
        &mut self,
        app: &Keypair,
        tag: [u8; 32],
        escrow: u64,
        bps: u16,
        confirmations: u16,
        claim_kind: u16,
        paid: u64,
    ) -> Result<u64, String> {
        let id = self.job_count() + 1;
        let ix = self
            .ctx
            .program()
            .accounts(ipow_protocol::client::accounts::OpenJob {
                protocol: protocol_pda(),
                application: application_pda(&app.pubkey()),
                job: job_pda(id),
                tag_record: tag_pda(&app.pubkey(), &tag),
                vault: vault_pda(),
                key: app.pubkey(),
                funder: app.pubkey(),
                system_program: SYSTEM,
            })
            .args(ipow_protocol::client::args::OpenJob {
                tag,
                escrow,
                escrow_fee_bps: bps,
                confirmations,
                claim_kind,
                payer: self.user.pubkey(),
                paid,
            })
            .instruction()
            .unwrap();
        self.send(ix, app)?;
        Ok(id)
    }

    /// A job of 1 SOL escrow with 6 confirmations, sent exactly its fees.
    fn open_job(&mut self, tag: u8) -> u64 {
        let app = self.application.insecure_clone();
        let paid = self.fee(6) + SOL / 200;
        self.open_job_as(&app, [tag; 32], SOL, 50, 6, 0, paid).unwrap()
    }

    fn bid(&mut self, who: &Keypair, job_id: u64, amount: u64) -> Result<(), String> {
        let job = self.job(job_id);
        let previous = if job.has_operator && job.operator != who.pubkey() {
            Some(operator_pda(&job.operator))
        } else {
            None
        };
        let ix = self
            .ctx
            .program()
            .accounts(ipow_protocol::client::accounts::Bid {
                job: job_pda(job_id),
                operator: operator_pda(&who.pubkey()),
                previous,
                owner: who.pubkey(),
            })
            .args(ipow_protocol::client::args::Bid { amount })
            .instruction()
            .unwrap();
        self.send(ix, who)
    }

    fn expire(&mut self, job_id: u64) -> Result<(), String> {
        let payer = self.job(job_id).payer;
        let caller = self.stranger.insecure_clone();
        let ix = self
            .ctx
            .program()
            .accounts(ipow_protocol::client::accounts::Expire {
                job: job_pda(job_id),
                payer_credit: credit_pda(&payer),
                funder: caller.pubkey(),
                system_program: SYSTEM,
            })
            .args(ipow_protocol::client::args::Expire {})
            .instruction()
            .unwrap();
        self.send(ix, &caller)
    }

    fn withdraw_credit(&mut self, who: &Keypair) -> Result<(), String> {
        let ix = self
            .ctx
            .program()
            .accounts(ipow_protocol::client::accounts::WithdrawCredit {
                protocol: protocol_pda(),
                credit: credit_pda(&who.pubkey()),
                vault: vault_pda(),
                owner: who.pubkey(),
            })
            .args(ipow_protocol::client::args::WithdrawCredit {})
            .instruction()
            .unwrap();
        self.send(ix, who)
    }

    fn job(&self, id: u64) -> ipow_protocol::accounts::Job {
        self.ctx.get_account(&job_pda(id)).unwrap()
    }

    fn operator(&self, owner: &Pubkey) -> (u64, u64) {
        let o: ipow_protocol::accounts::Operator = self.ctx.get_account(&operator_pda(owner)).unwrap();
        (o.bond, o.locked)
    }

    fn credit(&self, owner: &Pubkey) -> u64 {
        match self.ctx.get_account::<ipow_protocol::accounts::Credit>(&credit_pda(owner)) {
            Ok(c) => c.amount,
            Err(_) => 0,
        }
    }

    fn vault_held(&self) -> u64 {
        let lamports = self.ctx.svm.get_account(&vault_pda()).unwrap().lamports;
        lamports - self.ctx.svm.minimum_balance_for_rent_exemption(0)
    }
}

fn expect_err<T: std::fmt::Debug>(result: Result<T, String>, needle: &str) {
    match result {
        Ok(v) => panic!("expected {needle}, got success {v:?}"),
        Err(logs) => assert!(logs.contains(needle), "expected {needle}, got:\n{logs}"),
    }
}

fn code(name: &str) -> String {
    format!("Error Code: {name}")
}

// ---------------------------------------------------------------------
// Operators (D12, D31, D43)
// ---------------------------------------------------------------------

#[test]
fn makes_anyone_an_operator_by_locking_a_bond_and_lets_it_withdraw_free_bond() {
    let mut env = Env::new();
    let a = env.a.insecure_clone();
    env.lock_bond(&a, 1).unwrap();
    assert_eq!(env.operator(&a.pubkey()), (1, 0));
    env.lock_bond(&a, 3 * SOL).unwrap();
    env.withdraw_bond(&a, 2 * SOL).unwrap();
    assert_eq!(env.operator(&a.pubkey()), (SOL + 1, 0));
    expect_err(env.withdraw_bond(&a, SOL + 2), &code("BondNotFree"));
    expect_err(env.lock_bond(&a, 0), &code("ZeroAmount"));
}

#[test]
fn keeps_bond_that_is_locked_for_a_job() {
    let mut env = Env::new();
    let a = env.a.insecure_clone();
    env.lock_bond(&a, 3 * SOL).unwrap();
    let job = env.open_job(1);
    env.bid(&a, job, 2 * SOL).unwrap();
    assert_eq!(env.operator(&a.pubkey()), (3 * SOL, 2 * SOL));
    expect_err(env.withdraw_bond(&a, SOL + 1), &code("BondNotFree"));
    env.withdraw_bond(&a, SOL).unwrap();
}

// ---------------------------------------------------------------------
// Applications (D17, D28, D63, D78)
// ---------------------------------------------------------------------

#[test]
fn lets_anyone_register_once_with_periods_between_36_hours_and_7_days() {
    let mut env = Env::new();
    let s = env.stranger.insecure_clone();
    expect_err(env.register(&s, vec![36 * 3600 - 1]), &code("ChallengePeriodOutOfRange"));
    expect_err(env.register(&s, vec![7 * 86400 + 1]), &code("ChallengePeriodOutOfRange"));
    expect_err(env.register(&s, vec![7 * 86400; 33]), &code("TooManyClaimKinds"));
    env.register(&s, vec![36 * 3600, 7 * 86400]).unwrap();
    let app: ipow_protocol::accounts::Application = env.ctx.get_account(&application_pda(&s.pubkey())).unwrap();
    assert_eq!(app.challenge_periods, vec![36 * 3600, 7 * 86400]);
    expect_err(env.register(&s, vec![]), "already in use");
}

// ---------------------------------------------------------------------
// Opening a job, fees (D19, D20, D24, D25, D55, D56, D58, D77, D79, D80)
// ---------------------------------------------------------------------

#[test]
fn opens_a_job_with_the_fees_calculated_on_chain() {
    let mut env = Env::new();
    let job_id = env.open_job(1);
    let job = env.job(job_id);
    assert_eq!(job.escrow, SOL);
    assert_eq!(job.escrow_fee, SOL / 200);
    assert_eq!(job.commitment_fee, env.fee(6));
    assert_eq!(job.payer, env.user.pubkey());
    assert_eq!(job.application, env.application.pubkey());
    assert_eq!(job.opened_at, T0);
    assert_eq!(env.vault_held(), env.fee(6) + SOL / 200);
}

#[test]
fn charges_more_for_a_longer_window() {
    let mut env = Env::new();
    assert!(env.fee(76) > env.fee(6));
    let app = env.application.insecure_clone();
    assert_eq!(env.fee(6), 375_000);
    let paid = env.fee(76) + SOL / 200;
    let id = env.open_job_as(&app, [2; 32], SOL, 50, 76, 0, paid).unwrap();
    assert_eq!(env.job(id).commitment_fee, env.fee(76));
    expect_err(env.open_job_as(&app, [3; 32], SOL, 50, 77, 0, paid), &code("ConfirmationsOutOfRange"));
    expect_err(env.open_job_as(&app, [4; 32], SOL, 50, 5, 0, paid), &code("ConfirmationsOutOfRange"));
}

#[test]
fn keeps_what_was_paid_above_the_fees_for_the_operator() {
    let mut env = Env::new();
    let app = env.application.insecure_clone();
    let paid = env.fee(6) + SOL / 200 + SOL / 100;
    let id = env.open_job_as(&app, [1; 32], SOL, 50, 6, 0, paid).unwrap();
    assert_eq!(env.job(id).commitment_fee, env.fee(6) + SOL / 100);
    assert_eq!(env.job(id).escrow_fee, SOL / 200);
}

#[test]
fn rejects_a_job_that_does_not_pay_its_fees_or_has_too_little_escrow() {
    let mut env = Env::new();
    let app = env.application.insecure_clone();
    let fee = env.fee(6);
    expect_err(env.open_job_as(&app, [1; 32], SOL, 50, 6, 0, fee), &code("FeesNotPaid"));
    expect_err(env.open_job_as(&app, [1; 32], 0, 50, 6, 0, fee * 2), &code("EscrowTooLow"));
    expect_err(env.open_job_as(&app, [1; 32], 5 * fee - 1, 0, 6, 0, fee), &code("EscrowTooLow"));
    env.open_job_as(&app, [1; 32], 5 * fee, 0, 6, 0, fee).unwrap();
    expect_err(env.open_job_as(&app, [2; 32], SOL, 10_001, 6, 0, 2 * SOL), &code("EscrowFeeOutOfRange"));
    env.open_job_as(&app, [3; 32], SOL, 10_000, 6, 0, SOL + fee).unwrap();
}

#[test]
fn lets_an_application_use_a_tag_once_and_only_its_registered_claim_kinds() {
    let mut env = Env::new();
    let app = env.application.insecure_clone();
    let paid = env.fee(6) + SOL / 200;
    env.open_job_as(&app, [1; 32], SOL, 50, 6, 0, paid).unwrap();
    expect_err(env.open_job_as(&app, [1; 32], SOL, 50, 6, 0, paid), "already in use");
    expect_err(env.open_job_as(&app, [2; 32], SOL, 50, 6, 1, paid), &code("UnknownClaimKind"));

    // Another application may use the same tag.
    let s = env.stranger.insecure_clone();
    env.register(&s, vec![7 * 86400]).unwrap();
    env.open_job_as(&s, [1; 32], SOL, 50, 6, 1, paid).unwrap();
}

#[test]
fn rejects_a_job_from_an_address_that_is_not_registered() {
    let mut env = Env::new();
    let s = env.stranger.insecure_clone();
    let paid = env.fee(6) + SOL / 200;
    expect_err(env.open_job_as(&s, [1; 32], SOL, 50, 6, 0, paid), "AccountNotInitialized");
}

// ---------------------------------------------------------------------
// Auction (D29, D32, D33, D37, D76, D82)
// ---------------------------------------------------------------------

fn with_bonds() -> (Env, Keypair, Keypair) {
    let mut env = Env::new();
    let a = env.a.insecure_clone();
    let b = env.b.insecure_clone();
    env.lock_bond(&a, 3 * SOL).unwrap();
    env.lock_bond(&b, 3 * SOL).unwrap();
    (env, a, b)
}

#[test]
fn accepts_a_bid_of_x_and_rejects_less_or_more_than_the_free_bond() {
    let (mut env, a, _) = with_bonds();
    let job = env.open_job(1);
    expect_err(env.bid(&a, job, SOL - 1), &code("BidTooLow"));
    expect_err(env.bid(&a, job, 3 * SOL + 1), &code("BondNotFree"));
    env.bid(&a, job, SOL).unwrap();
    assert_eq!(env.operator(&a.pubkey()), (3 * SOL, SOL));
}

#[test]
fn frees_the_bond_of_the_operator_that_is_outbid_and_needs_a_step_of_one_tenth_percent() {
    let (mut env, a, b) = with_bonds();
    let job = env.open_job(1);
    env.bid(&a, job, SOL).unwrap();
    expect_err(env.bid(&b, job, SOL + SOL / 1000 - 1), &code("BidTooLow"));
    env.bid(&b, job, SOL + SOL / 1000).unwrap();
    assert_eq!(env.operator(&a.pubkey()), (3 * SOL, 0));
    assert_eq!(env.operator(&b.pubkey()), (3 * SOL, SOL + SOL / 1000));
    assert_eq!(env.job(job).operator, b.pubkey());
    // The escrow fee stays on x.
    assert_eq!(env.job(job).escrow_fee, SOL / 200);
}

#[test]
fn lets_an_operator_raise_its_own_bid() {
    let (mut env, a, _) = with_bonds();
    let job = env.open_job(1);
    env.bid(&a, job, 2 * SOL).unwrap();
    env.bid(&a, job, 3 * SOL).unwrap();
    assert_eq!(env.operator(&a.pubkey()), (3 * SOL, 3 * SOL));
}

#[test]
fn locks_the_winner_in_one_minute_after_the_last_bid_and_closes_after_15_minutes() {
    let (mut env, a, b) = with_bonds();
    let job = env.open_job(1);
    env.set_now(T0 + 3 * MINUTE);
    env.bid(&a, job, SOL).unwrap();
    env.set_now(T0 + 4 * MINUTE - 1);
    env.bid(&b, job, 2 * SOL).unwrap();
    env.set_now(T0 + 5 * MINUTE - 1);
    expect_err(env.bid(&a, job, 3 * SOL), &code("AuctionClosed"));

    let late = env.open_job(2);
    env.set_now(env.now() + 15 * MINUTE - 10);
    env.bid(&a, late, SOL).unwrap();
    env.set_now(env.job(late).opened_at + 15 * MINUTE);
    expect_err(env.bid(&b, late, 2 * SOL), &code("AuctionClosed"));
}

// ---------------------------------------------------------------------
// A job nobody takes (D61)
// ---------------------------------------------------------------------

#[test]
fn expires_after_15_minutes_and_returns_all_that_was_paid() {
    let mut env = Env::new();
    let app = env.application.insecure_clone();
    let paid = env.fee(6) + SOL / 200 + SOL / 100;
    let job = env.open_job_as(&app, [1; 32], SOL, 50, 6, 0, paid).unwrap();
    env.set_now(T0 + 15 * MINUTE - 1);
    expect_err(env.expire(job), &code("AuctionOpen"));
    env.set_now(T0 + 15 * MINUTE);
    env.expire(job).unwrap();
    assert_eq!(env.credit(&env.user.pubkey()), paid);
    expect_err(env.expire(job), &code("FeesAlreadyReturned"));

    let user = env.user.insecure_clone();
    let before = env.ctx.svm.get_account(&user.pubkey()).unwrap().lamports;
    env.withdraw_credit(&user).unwrap();
    let after = env.ctx.svm.get_account(&user.pubkey()).unwrap().lamports;
    assert_eq!(after - before + 5_000 * 1, paid + 5_000 - 5_000 + 5_000 - 5_000 + 0 * 0 + (before - before));
    assert_eq!(env.credit(&user.pubkey()), 0);
    assert_eq!(env.vault_held(), 0);
}

#[test]
fn does_not_expire_a_job_that_has_a_winner() {
    let (mut env, a, _) = with_bonds();
    let job = env.open_job(1);
    env.bid(&a, job, SOL).unwrap();
    env.set_now(T0 + 15 * MINUTE);
    expect_err(env.expire(job), &code("JobHasBid"));
}

#[test]
fn holds_exactly_the_bonds_the_fees_of_open_jobs_and_the_credits() {
    let (mut env, a, b) = with_bonds();
    let first = env.open_job(1);
    let second = env.open_job(2);
    let fees = |env: &Env, id: u64| {
        let j = env.job(id);
        if j.fees_returned { 0 } else { j.commitment_fee + j.escrow_fee }
    };
    let check = |env: &Env| {
        let expected = env.operator(&a.pubkey()).0
            + env.operator(&b.pubkey()).0
            + fees(env, first)
            + fees(env, second)
            + env.credit(&env.user.pubkey());
        assert_eq!(env.vault_held(), expected);
    };
    check(&env);
    env.bid(&a, first, SOL).unwrap();
    env.bid(&b, first, 2 * SOL).unwrap();
    env.withdraw_bond(&a, 3 * SOL).unwrap();
    check(&env);
    env.set_now(T0 + 15 * MINUTE);
    env.expire(second).unwrap();
    check(&env);
    let user = env.user.insecure_clone();
    env.withdraw_credit(&user).unwrap();
    check(&env);
    let _ = (DAY, AccountMeta::new(Pubkey::default(), false));
}

#[test]
fn rejects_a_job_with_no_payer() {
    let mut env = Env::new();
    env.user = Keypair::new();
    let app = env.application.insecure_clone();
    let paid = env.fee(6) + SOL / 200;
    let id = env.job_count() + 1;
    let tag = [1u8; 32];
    let ix = env
        .ctx
        .program()
        .accounts(ipow_protocol::client::accounts::OpenJob {
            protocol: protocol_pda(),
            application: application_pda(&app.pubkey()),
            job: job_pda(id),
            tag_record: tag_pda(&app.pubkey(), &tag),
            vault: vault_pda(),
            key: app.pubkey(),
            funder: app.pubkey(),
            system_program: SYSTEM,
        })
        .args(ipow_protocol::client::args::OpenJob {
            tag,
            escrow: SOL,
            escrow_fee_bps: 50,
            confirmations: 6,
            claim_kind: 0,
            payer: Pubkey::default(),
            paid,
        })
        .instruction()
        .unwrap();
    expect_err(env.send(ix, &app), &code("ZeroAddress"));
}
