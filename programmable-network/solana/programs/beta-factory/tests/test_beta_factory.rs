//! BETA v2 factory (`docs/DESIGN_V2.md` §6) — end-to-end against litesvm.
//! Anchors are real (legacy-serialized) Bitcoin transactions proven against
//! seeded `ipow` headers, exactly as the program would see them on-chain.

use anchor_lang;
use anchor_litesvm::{AnchorContext, AnchorLiteSVM, Program as LiteSvmProgram, ProgramTestExt, TestHelpers};
use sha2::{Digest, Sha256};
use solana_account::Account as SolanaAccount;
use solana_clock::Clock;
use solana_keypair::Keypair;
use solana_program::pubkey::Pubkey;
use solana_signer::Signer;

anchor_lang::declare_program!(ipow);
anchor_lang::declare_program!(beta_factory);

const SOL: u64 = 1_000_000_000;
/// litesvm charges 5000 lamports per signature.
const FEE: u64 = 5000;
const UNIT: u64 = 1_000_000_000;
const KIND_MINT: u8 = 1;
const KIND_RELEASE: u8 = 2;
const KIND_VETO: u8 = 3;
const KIND_CANCEL: u8 = 4;
const KIND_ATTEST: u8 = 5;
const KIND_CLEAR: u8 = 6;
const KIND_ALIVE: u8 = 7;
const T_CHALLENGE: i64 = 7 * 86_400;
const GENESIS_TXID: [u8; 32] = [7u8; 32];
const OP_ID: [u8; 32] = [1u8; 32];
const AUD_ID: [u8; 32] = [2u8; 32];
/// Every test registers this one composition (DESIGN_V2 §8): a local SOL
/// leg (verified directly, exactly like §6's) plus one remote leg judged
/// on the Bitcoin statement bus exactly like §6/§7's single Ethereum leg
/// — the two-component equivalent of what the pre-§8 factory always did.
/// `component_index` for that one remote leg is always 0.
const COMPOSITION_ID: u64 = 1;

fn new_ctx() -> AnchorContext {
    let mut ctx = AnchorLiteSVM::build_with_program(
        beta_factory::ID,
        include_bytes!("../../../target/deploy/beta_factory.so"),
    );
    ctx.deploy_program(ipow::ID, include_bytes!("../../../target/deploy/ipow.so"));
    ctx
}

fn now(ctx: &AnchorContext) -> i64 {
    let clock: Clock = ctx.svm.get_sysvar();
    clock.unix_timestamp
}

fn advance_clock(ctx: &mut AnchorContext, seconds: i64) {
    let mut clock: Clock = ctx.svm.get_sysvar();
    clock.unix_timestamp += seconds;
    ctx.svm.set_sysvar(&clock);
}

fn disc(name: &str) -> [u8; 8] {
    let h = Sha256::digest(format!("account:{name}").as_bytes());
    let mut d = [0u8; 8];
    d.copy_from_slice(&h[..8]);
    d
}

/// Seeds an `ipow` `GlobalHeader` directly (real PoW is impractical in a
/// unit test — same bypass `ipow-conversion`'s tests use).
fn seed_header(ctx: &mut AnchorContext, height: u64, merkle_root_le: [u8; 32], timestamp: u32) -> Pubkey {
    let (header_pda, _) = ctx.svm.get_pda_with_bump(&[b"header", &height.to_le_bytes()], &ipow::ID);
    let mut data = disc("GlobalHeader").to_vec();
    data.extend_from_slice(&height.to_le_bytes());
    data.extend_from_slice(&[0u8; 32]);
    data.extend_from_slice(&[0u8; 32]);
    data.extend_from_slice(&merkle_root_le);
    data.extend_from_slice(&0x1d00ffffu32.to_le_bytes());
    data.extend_from_slice(&timestamp.to_le_bytes());
    data.extend_from_slice(&0i64.to_le_bytes());
    let lamports = ctx.svm.minimum_balance_for_rent_exemption(data.len());
    ctx.svm
        .set_account(header_pda, SolanaAccount { lamports, data, owner: ipow::ID, executable: false, rent_epoch: 0 })
        .unwrap();
    header_pda
}

/// Seeds `ipow`'s `GlobalState` with a given relay tip (the rate-window clock).
fn seed_ipow_global_state(ctx: &mut AnchorContext, tip: u64) -> Pubkey {
    let (pda, _) = ctx.svm.get_pda_with_bump(&[b"global_state"], &ipow::ID);
    let mut data = disc("GlobalState").to_vec();
    data.extend_from_slice(&[0u8; 32]); // operator
    data.extend_from_slice(&0u16.to_le_bytes()); // commit_fee_bps
    data.extend_from_slice(&0u64.to_le_bytes()); // next_tx_id
    data.extend_from_slice(&tip.to_le_bytes()); // global_tip_height
    data.extend_from_slice(&[0u8; 8 * 5]); // min_anchor_height .. total_reserved_native
    data.push(0); // escrow_bump
    let lamports = ctx.svm.minimum_balance_for_rent_exemption(data.len());
    ctx.svm
        .set_account(pda, SolanaAccount { lamports, data, owner: ipow::ID, executable: false, rent_epoch: 0 })
        .unwrap();
    pda
}

fn set_tip(ctx: &mut AnchorContext, pda: Pubkey, tip: u64) {
    let mut account = ctx.svm.get_account(&pda).unwrap();
    account.data[50..58].copy_from_slice(&tip.to_le_bytes());
    ctx.svm.set_account(pda, account).unwrap();
}

fn dsha256(b: &[u8]) -> [u8; 32] {
    let h1 = Sha256::digest(b);
    let h2 = Sha256::digest(h1);
    let mut out = [0u8; 32];
    out.copy_from_slice(&h2);
    out
}

fn sha256(b: &[u8]) -> [u8; 32] {
    let mut out = [0u8; 32];
    out.copy_from_slice(&Sha256::digest(b));
    out
}

/// A statement-chain anchor: spends `(prev_txid, prev_vout)`, output 0 is
/// the new chain head, output 1 is `OP_RETURN ver|kind|hash` (or an
/// arbitrary script when `payload` is None, to model a stalled anchor).
fn anchor_tx(prev_txid_le: [u8; 32], prev_vout: u32, payload: Option<(u8, [u8; 32])>, salt: u8) -> Vec<u8> {
    let mut tx = Vec::new();
    tx.extend_from_slice(&1u32.to_le_bytes());
    tx.push(0x01);
    tx.extend_from_slice(&prev_txid_le);
    tx.extend_from_slice(&prev_vout.to_le_bytes());
    tx.push(0x01);
    tx.push(salt); // 1-byte scriptSig: makes otherwise-identical anchors distinct
    tx.extend_from_slice(&[0xffu8; 4]);
    tx.push(0x02);
    tx.extend_from_slice(&1000u64.to_le_bytes());
    tx.push(22);
    tx.push(0x00);
    tx.push(0x14);
    tx.extend_from_slice(&[0x11u8; 20]);
    tx.extend_from_slice(&0u64.to_le_bytes());
    match payload {
        Some((kind, hash)) => {
            tx.push(36);
            tx.push(0x6a);
            tx.push(34);
            tx.push(1);
            tx.push(kind);
            tx.extend_from_slice(&hash);
        }
        None => {
            tx.push(3);
            tx.extend_from_slice(&[0x6a, 0x01, 0x00]);
        }
    }
    tx
}

/// `component_index` is always 0 in these tests: every registered
/// composition has exactly one remote leg (§8's generalization of the
/// old single Ethereum leg).
fn stmt_mint(lock_id: u64, sol_user: Pubkey, nonce: u64, units: u64, deadline: i64) -> Vec<u8> {
    let mut v = vec![KIND_MINT];
    v.extend_from_slice(&COMPOSITION_ID.to_be_bytes());
    v.push(0); // component_index
    v.extend_from_slice(&lock_id.to_be_bytes());
    v.extend_from_slice(sol_user.as_ref());
    v.extend_from_slice(&nonce.to_be_bytes());
    v.extend_from_slice(&units.to_be_bytes());
    v.extend_from_slice(&(deadline as u64).to_be_bytes());
    v
}
fn stmt_release(lock_id: u64, burn_id: u64, to_eth: [u8; 20], units: u64) -> Vec<u8> {
    let mut v = vec![KIND_RELEASE];
    v.extend_from_slice(&lock_id.to_be_bytes());
    v.extend_from_slice(&burn_id.to_be_bytes());
    v.extend_from_slice(&to_eth);
    v.extend_from_slice(&units.to_be_bytes());
    v
}
fn stmt_veto(target_party_id: [u8; 32], target_txid_le: [u8; 32]) -> Vec<u8> {
    let mut v = vec![KIND_VETO];
    v.extend_from_slice(&target_party_id);
    v.extend_from_slice(&target_txid_le);
    v
}
fn stmt_target(kind: u8, target: [u8; 32]) -> Vec<u8> {
    let mut v = vec![kind];
    v.extend_from_slice(&target);
    v
}
fn stmt_cancel(lock_id: u64) -> Vec<u8> {
    let mut v = vec![KIND_CANCEL];
    v.extend_from_slice(&lock_id.to_be_bytes());
    v
}

fn params() -> beta_factory::types::FactoryParams {
    beta_factory::types::FactoryParams {
        sol_per_unit: SOL,
        eth_gwei_per_unit: 1_000_000_000,
        window_blocks: 6,
        mint_cap_units_per_window: 100,
        t_skip_secs: 3600,
        t_challenge_secs: T_CHALLENGE,
        unbond_delay_secs: 60,
        min_operator_bond: 5 * SOL,
        min_auditor_bond: SOL,
        comp_lamports_per_unit: SOL,
        veto_slash_lamports: SOL / 2,
        veto_reward_lamports: SOL / 10,
        bounty_bps: 1000,
    }
}

struct Setup {
    ctx: AnchorContext,
    governance: Keypair,
    config: Pubkey,
    vault: Pubkey,
    mint_authority: Pubkey,
    bond_escrow: Pubkey,
    insurance: Pubkey,
    reward_pool: Pubkey,
    beta_mint: Pubkey,
    ipow_global_state: Pubkey,
    next_height: u64,
}

fn setup() -> Setup {
    let mut ctx = new_ctx();
    let mut clock: Clock = ctx.svm.get_sysvar();
    clock.unix_timestamp = 1_800_000_000;
    ctx.svm.set_sysvar(&clock);
    let governance = ctx.svm.create_funded_account(100 * SOL).unwrap();
    let ipow_global_state = seed_ipow_global_state(&mut ctx, 1000);
    let (config, _) = ctx.svm.get_pda_with_bump(&[b"config"], &beta_factory::ID);
    let (vault, _) = ctx.svm.get_pda_with_bump(&[b"vault"], &beta_factory::ID);
    let (mint_authority, _) = ctx.svm.get_pda_with_bump(&[b"mint_authority"], &beta_factory::ID);
    let (bond_escrow, _) = ctx.svm.get_pda_with_bump(&[b"bond_escrow"], &beta_factory::ID);
    let (insurance, _) = ctx.svm.get_pda_with_bump(&[b"insurance"], &beta_factory::ID);
    let (reward_pool, _) = ctx.svm.get_pda_with_bump(&[b"rewards"], &beta_factory::ID);
    let beta_mint_kp = Keypair::new();
    let ix = LiteSvmProgram::new(beta_factory::ID)
        .accounts(beta_factory::client::accounts::Initialize {
            config,
            vault,
            mint_authority,
            bond_escrow,
            insurance,
            beta_mint: beta_mint_kp.pubkey(),
            admin: governance.pubkey(),
            token_program: litesvm_token::spl_token::ID,
            system_program: anchor_lang::solana_program::system_program::ID,
        })
        .args(beta_factory::client::args::Initialize { governance: governance.pubkey(), params: params() })
        .instruction()
        .unwrap();
    ctx.execute_instruction(ix, &[&governance, &beta_mint_kp]).unwrap().assert_success();

    let register_comp_ix = LiteSvmProgram::new(beta_factory::ID)
        .accounts(beta_factory::client::accounts::RegisterComposition {
            config,
            composition: composition_pda(COMPOSITION_ID),
            governance: governance.pubkey(),
            system_program: anchor_lang::solana_program::system_program::ID,
        })
        .args(beta_factory::client::args::RegisterComposition {
            id: COMPOSITION_ID,
            components: vec![
                beta_factory::types::Component { network_id: 0, token_id: [0u8; 32], amount_per_unit: SOL },
                beta_factory::types::Component { network_id: 1, token_id: [0u8; 32], amount_per_unit: SOL },
            ],
        })
        .instruction()
        .unwrap();
    ctx.execute_instruction(register_comp_ix, &[&governance]).unwrap().assert_success();

    Setup {
        ctx,
        governance,
        config,
        vault,
        mint_authority,
        bond_escrow,
        insurance,
        reward_pool,
        beta_mint: beta_mint_kp.pubkey(),
        ipow_global_state,
        next_height: 2000,
    }
}

fn party_pda(party_id: [u8; 32]) -> Pubkey {
    Pubkey::find_program_address(&[b"party", &party_id], &beta_factory::ID).0
}
fn anchor_pda(txid_le: [u8; 32]) -> Pubkey {
    Pubkey::find_program_address(&[b"anchor", &txid_le], &beta_factory::ID).0
}
fn pending_pda(user: Pubkey, nonce: u64) -> Pubkey {
    Pubkey::find_program_address(&[b"pending", user.as_ref(), &nonce.to_le_bytes()], &beta_factory::ID).0
}
fn burn_pda(burn_id: u64) -> Pubkey {
    Pubkey::find_program_address(&[b"burn", &burn_id.to_le_bytes()], &beta_factory::ID).0
}
fn composition_pda(id: u64) -> Pubkey {
    Pubkey::find_program_address(&[b"composition", &id.to_le_bytes()], &beta_factory::ID).0
}
fn token_vault_authority_pda() -> Pubkey {
    Pubkey::find_program_address(&[b"token_vault_authority"], &beta_factory::ID).0
}
fn ata(owner: Pubkey, mint: Pubkey) -> Pubkey {
    spl_associated_token_account_interface::address::get_associated_token_address(&owner, &mint)
}

fn register(s: &mut Setup, party_id: [u8; 32], kind: beta_factory::types::PartyKind, owner: &Keypair, bond: u64, with_gov: bool) -> Result<(), String> {
    let ix = LiteSvmProgram::new(beta_factory::ID)
        .accounts(beta_factory::client::accounts::RegisterParty {
            config: s.config,
            party: party_pda(party_id),
            bond_escrow: s.bond_escrow,
            owner: owner.pubkey(),
            governance: if with_gov { Some(s.governance.pubkey()) } else { None },
            system_program: anchor_lang::solana_program::system_program::ID,
        })
        .args(beta_factory::client::args::RegisterParty { party_id, kind, anchor_txid_le: GENESIS_TXID, anchor_vout: 0, bond })
        .instruction()
        .unwrap();
    let signers: Vec<&Keypair> = if with_gov { vec![owner, &s.governance] } else { vec![owner] };
    let r = s.ctx.execute_instruction(ix, &signers).unwrap();
    if r.is_success() { Ok(()) } else { Err(format!("{:?}", r.logs())) }
}

fn register_operator(s: &mut Setup) -> Keypair {
    let op = s.ctx.svm.create_funded_account(50 * SOL).unwrap();
    register(s, OP_ID, beta_factory::types::PartyKind::Operator, &op, 10 * SOL, true).unwrap();
    op
}
fn register_auditor(s: &mut Setup) -> Keypair {
    let a = s.ctx.svm.create_funded_account(50 * SOL).unwrap();
    register(s, AUD_ID, beta_factory::types::PartyKind::Auditor, &a, 2 * SOL, false).unwrap();
    a
}

fn fees_pda() -> Pubkey {
    Pubkey::find_program_address(&[b"fees"], &beta_factory::ID).0
}

fn lock_sol(s: &mut Setup, user: &Keypair, nonce: u64, units: u64, deadline: i64) {
    lock_sol_with_fee(s, user, nonce, units, deadline, 0);
}

fn lock_sol_with_fee(s: &mut Setup, user: &Keypair, nonce: u64, units: u64, deadline: i64, attest_fee: u64) {
    let ix = LiteSvmProgram::new(beta_factory::ID)
        .accounts(beta_factory::client::accounts::LockSol {
            config: s.config,
            pending: pending_pda(user.pubkey(), nonce),
            composition: composition_pda(COMPOSITION_ID),
            vault: s.vault,
            fees: fees_pda(),
            beta_mint: s.beta_mint,
            user_beta: ata(user.pubkey(), s.beta_mint),
            user: user.pubkey(),
            token_program: litesvm_token::spl_token::ID,
            associated_token_program: spl_associated_token_account_interface::program::ID,
            system_program: anchor_lang::solana_program::system_program::ID,
            token_vault_authority: token_vault_authority_pda(),
            local_spl_0_mint: None, local_spl_0_user: None, local_spl_0_vault: None,
            local_spl_1_mint: None, local_spl_1_user: None, local_spl_1_vault: None,
            local_spl_2_mint: None, local_spl_2_user: None, local_spl_2_vault: None,
        })
        .args(beta_factory::client::args::LockSol { nonce, composition_id: COMPOSITION_ID, units, deadline, attest_fee })
        .instruction()
        .unwrap();
    s.ctx.execute_instruction(ix, &[user]).unwrap().assert_success();
}

/// `lock_id` is the one remote (component 0) leg's lock id — the same
/// thing `eth_lock_id` always meant, just named for its now-generic role.
fn approve_pending(s: &mut Setup, user: &Keypair, nonce: u64, lock_id: u64) {
    let ix = LiteSvmProgram::new(beta_factory::ID)
        .accounts(beta_factory::client::accounts::ApprovePending { pending: pending_pda(user.pubkey(), nonce), user: user.pubkey() })
        .args(beta_factory::client::args::ApprovePending { nonce, remote_lock_id: vec![lock_id] })
        .instruction()
        .unwrap();
    s.ctx.execute_instruction(ix, &[user]).unwrap().assert_success();
}

#[derive(Default)]
struct Extra {
    pending: Option<Pubkey>,
    pending_user: Option<Pubkey>,
    burn: Option<Pubkey>,
    target_party: Option<Pubkey>,
    target_anchor: Option<Pubkey>,
    prior_party: Option<Pubkey>,
}

/// Builds the anchor tx on `party`'s chain, seeds a header containing it,
/// and calls `process_anchor`. Returns `(txid, result)`.
fn process(
    s: &mut Setup,
    party_id: [u8; 32],
    party_owner: Pubkey,
    prev: ([u8; 32], u32),
    statement: &[u8],
    submitter: &Keypair,
    extra: Extra,
    salt: u8,
) -> ([u8; 32], anchor_litesvm::TransactionResult) {
    s.ctx.svm.expire_blockhash();
    let kind = statement[0];
    let tx_raw = anchor_tx(prev.0, prev.1, Some((kind, sha256(statement))), salt);
    let txid = dsha256(&tx_raw);
    let height = s.next_height;
    s.next_height += 1;
    let ts = now(&s.ctx) as u32;
    let header = seed_header(&mut s.ctx, height, txid, ts);
    let ix = LiteSvmProgram::new(beta_factory::ID)
        .accounts(beta_factory::client::accounts::ProcessAnchor {
            config: s.config,
            party: party_pda(party_id),
            party_owner,
            processed: anchor_pda(txid),
            header,
            bond_escrow: s.bond_escrow,
            insurance: s.insurance,
            reward_pool: s.reward_pool,
            fees: fees_pda(),
            submitter: submitter.pubkey(),
            system_program: anchor_lang::solana_program::system_program::ID,
            pending: extra.pending,
            pending_user: extra.pending_user,
            burn: extra.burn,
            target_party: extra.target_party,
            target_anchor: extra.target_anchor,
            prior_party: extra.prior_party,
        })
        .args(beta_factory::client::args::ProcessAnchor {
            txid_le: txid,
            statement: statement.to_vec(),
            tx_raw,
            proof_block_height: height,
            branch_le: vec![],
            index: 0,
        })
        .instruction()
        .unwrap();
    let r = s.ctx.execute_instruction(ix, &[submitter]).unwrap();
    (txid, r)
}

/// `txid` is the anchor that settled component 0, the one remote leg
/// every test composition has — `remote_0` maps to it, `remote_1`/`remote_2`
/// stay `None` since these compositions never have more than one remote.
fn exercise(s: &mut Setup, txid: [u8; 32], party_id: [u8; 32], user: Pubkey, nonce: u64, submitter: &Keypair) -> anchor_litesvm::TransactionResult {
    s.ctx.svm.expire_blockhash();
    let ix = LiteSvmProgram::new(beta_factory::ID)
        .accounts(beta_factory::client::accounts::ExerciseMint {
            config: s.config,
            pending: pending_pda(user, nonce),
            composition: composition_pda(COMPOSITION_ID),
            party: Some(party_pda(party_id)),
            user,
            user_beta: ata(user, s.beta_mint),
            beta_mint: s.beta_mint,
            mint_authority: s.mint_authority,
            fees: fees_pda(),
            token_program: litesvm_token::spl_token::ID,
            system_program: anchor_lang::solana_program::system_program::ID,
            remote_0: Some(anchor_pda(txid)),
            remote_1: None,
            remote_2: None,
            remote_3: None,
            remote_4: None,
            remote_5: None,
            remote_6: None,
        })
        .args(beta_factory::client::args::ExerciseMint {})
        .instruction()
        .unwrap();
    s.ctx.execute_instruction(ix, &[submitter]).unwrap()
}

fn settle(s: &mut Setup, txid: [u8; 32], attester: Option<Pubkey>, pending: Option<Pubkey>, submitter: &Keypair) -> anchor_litesvm::TransactionResult {
    s.ctx.svm.expire_blockhash();
    let ix = LiteSvmProgram::new(beta_factory::ID)
        .accounts(beta_factory::client::accounts::SettleMint { config: s.config, processed: anchor_pda(txid), bond_escrow: s.bond_escrow, insurance: s.insurance, system_program: anchor_lang::solana_program::system_program::ID, attester, pending })
        .args(beta_factory::client::args::SettleMint { txid_le: txid })
        .instruction().unwrap();
    s.ctx.execute_instruction(ix, &[submitter]).unwrap()
}

/// Attests `target` from `party` (its own chain from `prev`), enabling immediate exercise.
fn attest(s: &mut Setup, party_id: [u8; 32], owner: &Keypair, prev: ([u8; 32], u32), target: [u8; 32], pending: Pubkey, salt: u8) -> ([u8; 32], anchor_litesvm::TransactionResult) {
    process(s, party_id, owner.pubkey(), prev, &stmt_target(KIND_ATTEST, target), owner, Extra { target_anchor: Some(anchor_pda(target)), pending: Some(pending), ..Default::default() }, salt)
}

fn burn_redeem(s: &mut Setup, burner: &Keypair, units: u64, to_eth: [u8; 20]) -> u64 {
    let cfg: beta_factory::accounts::FactoryConfig = s.ctx.get_account(&s.config).unwrap();
    let burn_id = cfg.next_burn_id;
    let ix = LiteSvmProgram::new(beta_factory::ID)
        .accounts(beta_factory::client::accounts::BurnRedeem {
            config: s.config,
            burn: burn_pda(burn_id),
            vault: s.vault,
            beta_mint: s.beta_mint,
            burner_beta: ata(burner.pubkey(), s.beta_mint),
            burner: burner.pubkey(),
            token_program: litesvm_token::spl_token::ID,
            system_program: anchor_lang::solana_program::system_program::ID,
        })
        .args(beta_factory::client::args::BurnRedeem { units, to_eth })
        .instruction()
        .unwrap();
    s.ctx.execute_instruction(ix, &[burner]).unwrap().assert_success();
    burn_id
}

fn beta_balance(s: &AnchorContext, owner: Pubkey, mint: Pubkey) -> u64 {
    token_balance(s, owner, mint)
}
fn token_balance(s: &AnchorContext, owner: Pubkey, mint: Pubkey) -> u64 {
    let acc: litesvm_token::spl_token::state::Account = litesvm_token::get_spl_account(&s.svm, &ata(owner, mint)).unwrap();
    acc.amount
}

fn party(s: &AnchorContext, id: [u8; 32]) -> beta_factory::accounts::Party {
    s.get_account(&party_pda(id)).unwrap()
}
fn processed(s: &AnchorContext, txid: [u8; 32]) -> beta_factory::accounts::ProcessedAnchor {
    s.get_account(&anchor_pda(txid)).unwrap()
}
fn config(s: &Setup) -> beta_factory::accounts::FactoryConfig {
    s.ctx.get_account(&s.config).unwrap()
}

/// Full honest mint: lock SOL, approve for Ethereum lock 7, operator anchors MINT, process, exercise.
fn honest_mint(s: &mut Setup, op: &Keypair, user: &Keypair, nonce: u64, units: u64, eth_lock_id: u64, prev: ([u8; 32], u32), salt: u8) -> [u8; 32] {
    let deadline = now(&s.ctx) + 86_400;
    lock_sol(s, user, nonce, units, deadline);
    approve_pending(s, user, nonce, eth_lock_id);
    let stmt = stmt_mint(eth_lock_id, user.pubkey(), nonce, units, deadline);
    let (txid, r) = process(s, OP_ID, op.pubkey(), prev, &stmt, op, Extra { pending: Some(pending_pda(user.pubkey(), nonce)), ..Default::default() }, salt);
    r.assert_success();
    txid
}

/// v3 fast path: operator anchors MINT, then ATTESTs it from its own chain (escrow from its bond).
/// Returns (mint txid, attest txid); the operator chain head is now the attest txid.
fn honest_mint_attested(s: &mut Setup, op: &Keypair, user: &Keypair, nonce: u64, units: u64, eth_lock_id: u64, prev: ([u8; 32], u32), salt: u8) -> ([u8; 32], [u8; 32]) {
    let mtx = honest_mint(s, op, user, nonce, units, eth_lock_id, prev, salt);
    let (atx, r) = attest(s, OP_ID, op, (mtx, 0), mtx, pending_pda(user.pubkey(), nonce), salt.wrapping_add(100));
    r.assert_success();
    (mtx, atx)
}

// ---------------------------------------------------------------- tests

#[test]
fn mint_happy_path_queues_then_exercises() {
    let mut s = setup();
    let op = register_operator(&mut s);
    let user = s.ctx.svm.create_funded_account(20 * SOL).unwrap();
    let deadline = now(&s.ctx) + 86_400;
    lock_sol(&mut s, &user, 1, 2, deadline);
    approve_pending(&mut s, &user, 1, 7);
    assert_eq!(config(&s).pending_lamports, 2 * SOL);

    let stmt = stmt_mint(7, user.pubkey(), 1, 2, deadline);
    let (txid, r) = process(&mut s, OP_ID, op.pubkey(), (GENESIS_TXID, 0), &stmt, &op, Extra { pending: Some(pending_pda(user.pubkey(), 1)), ..Default::default() }, 1);
    r.assert_success();

    let pa = processed(&s.ctx, txid);
    assert_eq!(pa.kind, KIND_MINT);
    assert!(matches!(pa.status, beta_factory::types::AnchorStatus::Queued));
    assert_eq!(pa.units, 2);
    let p = party(&s.ctx, OP_ID);
    assert_eq!(p.anchor_txid_le, txid);
    assert_eq!(p.anchor_vout, 0);
    assert_eq!(p.seq, 1);
    let pend: beta_factory::accounts::Pending = s.ctx.get_account(&pending_pda(user.pubkey(), 1)).unwrap();
    assert_eq!(pend.queued_by, OP_ID);

    // v3: neither attested nor past the window → not yet.
    exercise(&mut s, txid, OP_ID, user.pubkey(), 1, &op).assert_anchor_error("NotAttested");
    let (atx, r) = attest(&mut s, OP_ID, &op, (txid, 0), txid, pending_pda(user.pubkey(), 1), 2);
    r.assert_success();
    let pa2 = processed(&s.ctx, txid);
    assert_eq!(pa2.attested_by, OP_ID);
    assert_eq!(pa2.escrow, 2 * SOL);
    assert_eq!(party(&s.ctx, OP_ID).bond, 10 * SOL - 2 * SOL);
    let _ = atx;
    exercise(&mut s, txid, OP_ID, user.pubkey(), 1, &op).assert_success();
    assert_eq!(beta_balance(&s.ctx, user.pubkey(), s.beta_mint), 2 * UNIT);
    let c = config(&s);
    assert_eq!(c.pending_lamports, 0);
    assert_eq!(c.reserve_lamports, 2 * SOL);
    assert_eq!(c.eth_claims_units, 2);
    assert!(s.ctx.svm.get_account(&pending_pda(user.pubkey(), 1)).is_none());
    assert!(matches!(processed(&s.ctx, txid).status, beta_factory::types::AnchorStatus::Exercised));
    // A second exercise of the same anchor is refused.
    exercise(&mut s, txid, OP_ID, user.pubkey(), 1, &op).assert_anchor_error("AccountNotInitialized");
}

#[test]
fn false_mint_statement_slashes_operator_and_compensates_named_user() {
    let mut s = setup();
    let op = register_operator(&mut s);
    let victim = s.ctx.svm.create_funded_account(SOL).unwrap();
    let submitter = s.ctx.svm.create_funded_account(5 * SOL).unwrap();
    let before_victim = s.ctx.svm.get_balance(&victim.pubkey()).unwrap();
    let before_sub = s.ctx.svm.get_balance(&submitter.pubkey()).unwrap();

    // No pending lock exists for (victim, nonce 9) — a lie about Solana.
    let stmt = stmt_mint(7, victim.pubkey(), 9, 3, now(&s.ctx) + 1000);
    let (txid, r) = process(&mut s, OP_ID, op.pubkey(), (GENESIS_TXID, 0), &stmt, &submitter, Extra { pending_user: Some(victim.pubkey()), ..Default::default() }, 1);
    r.assert_success();

    assert!(matches!(processed(&s.ctx, txid).status, beta_factory::types::AnchorStatus::Slashed));
    let p = party(&s.ctx, OP_ID);
    assert!(p.dead);
    assert_eq!(p.bond, 10 * SOL - 3 * SOL);
    let after_victim = s.ctx.svm.get_balance(&victim.pubkey()).unwrap();
    assert_eq!(after_victim - before_victim, 3 * SOL * 9 / 10);
    let after_sub = s.ctx.svm.get_balance(&submitter.pubkey()).unwrap();
    // Submitter paid rent for the ProcessedAnchor account, so compare net of that.
    let rent = s.ctx.svm.get_balance(&anchor_pda(txid)).unwrap();
    assert_eq!(after_sub + rent + FEE - before_sub, 3 * SOL / 10);
    // Nothing to exercise.
    exercise(&mut s, txid, OP_ID, victim.pubkey(), 9, &submitter).assert_anchor_error("AccountNotInitialized");
}

#[test]
fn mint_with_wrong_lock_id_is_a_lie_even_though_pending_exists() {
    let mut s = setup();
    let op = register_operator(&mut s);
    let user = s.ctx.svm.create_funded_account(20 * SOL).unwrap();
    let deadline = now(&s.ctx) + 86_400;
    lock_sol(&mut s, &user, 1, 1, deadline);
    approve_pending(&mut s, &user, 1, 7);
    let stmt = stmt_mint(8, user.pubkey(), 1, 1, deadline); // user approved 7, not 8
    let (txid, r) = process(&mut s, OP_ID, op.pubkey(), (GENESIS_TXID, 0), &stmt, &op, Extra { pending: Some(pending_pda(user.pubkey(), 1)), pending_user: Some(user.pubkey()), ..Default::default() }, 1);
    r.assert_success();
    assert!(matches!(processed(&s.ctx, txid).status, beta_factory::types::AnchorStatus::Slashed));
    assert!(party(&s.ctx, OP_ID).dead);
    // The user's SOL is untouched and still expirable after the deadline.
    let pend: beta_factory::accounts::Pending = s.ctx.get_account(&pending_pda(user.pubkey(), 1)).unwrap();
    assert_eq!(pend.queued_by, [0u8; 32]);
}

#[test]
fn anchor_off_the_statement_chain_is_rejected() {
    let mut s = setup();
    let op = register_operator(&mut s);
    let user = s.ctx.svm.create_funded_account(20 * SOL).unwrap();
    let deadline = now(&s.ctx) + 86_400;
    lock_sol(&mut s, &user, 1, 1, deadline);
    approve_pending(&mut s, &user, 1, 7);
    let stmt = stmt_mint(7, user.pubkey(), 1, 1, deadline);
    // Spends some other outpoint — not the operator's registered head.
    let (_, r) = process(&mut s, OP_ID, op.pubkey(), ([9u8; 32], 0), &stmt, &op, Extra { pending: Some(pending_pda(user.pubkey(), 1)), ..Default::default() }, 1);
    r.assert_anchor_error("NotOnStatementChain");
    assert_eq!(party(&s.ctx, OP_ID).seq, 0);
}

#[test]
fn statement_hash_mismatch_and_kind_mismatch_are_rejected() {
    let mut s = setup();
    let op = register_operator(&mut s);
    let user = s.ctx.svm.create_funded_account(20 * SOL).unwrap();
    let deadline = now(&s.ctx) + 86_400;
    lock_sol(&mut s, &user, 1, 1, deadline);
    approve_pending(&mut s, &user, 1, 7);
    let stmt = stmt_mint(7, user.pubkey(), 1, 1, deadline);
    let other = stmt_mint(7, user.pubkey(), 1, 2, deadline);

    // Anchor commits to `other`, submitter supplies `stmt`.
    let tx_raw = anchor_tx(GENESIS_TXID, 0, Some((KIND_MINT, sha256(&other))), 1);
    let txid = dsha256(&tx_raw);
    let ts = now(&s.ctx) as u32;
    let header = seed_header(&mut s.ctx, 5000, txid, ts);
    let ix = LiteSvmProgram::new(beta_factory::ID)
        .accounts(beta_factory::client::accounts::ProcessAnchor {
            config: s.config, party: party_pda(OP_ID), party_owner: op.pubkey(), processed: anchor_pda(txid), header,
            bond_escrow: s.bond_escrow, insurance: s.insurance, reward_pool: s.reward_pool, fees: fees_pda(), submitter: op.pubkey(),
            system_program: anchor_lang::solana_program::system_program::ID,
            pending: Some(pending_pda(user.pubkey(), 1)), pending_user: None, burn: None, target_party: None, target_anchor: None, prior_party: None,
        })
        .args(beta_factory::client::args::ProcessAnchor { txid_le: txid, statement: stmt.clone(), tx_raw: tx_raw.clone(), proof_block_height: 5000, branch_le: vec![], index: 0 })
        .instruction().unwrap();
    s.ctx.execute_instruction(ix, &[&op]).unwrap().assert_anchor_error("StatementHashMismatch");

    // Anchor says RELEASE, statement is a MINT.
    let tx_raw2 = anchor_tx(GENESIS_TXID, 0, Some((KIND_RELEASE, sha256(&stmt))), 2);
    let txid2 = dsha256(&tx_raw2);
    let header2 = seed_header(&mut s.ctx, 5001, txid2, ts);
    let ix = LiteSvmProgram::new(beta_factory::ID)
        .accounts(beta_factory::client::accounts::ProcessAnchor {
            config: s.config, party: party_pda(OP_ID), party_owner: op.pubkey(), processed: anchor_pda(txid2), header: header2,
            bond_escrow: s.bond_escrow, insurance: s.insurance, reward_pool: s.reward_pool, fees: fees_pda(), submitter: op.pubkey(),
            system_program: anchor_lang::solana_program::system_program::ID,
            pending: Some(pending_pda(user.pubkey(), 1)), pending_user: None, burn: None, target_party: None, target_anchor: None, prior_party: None,
        })
        .args(beta_factory::client::args::ProcessAnchor { txid_le: txid2, statement: stmt, tx_raw: tx_raw2, proof_block_height: 5001, branch_le: vec![], index: 0 })
        .instruction().unwrap();
    s.ctx.execute_instruction(ix, &[&op]).unwrap().assert_anchor_error("KindMismatch");
}

#[test]
fn wrong_txid_or_wrong_header_is_rejected() {
    let mut s = setup();
    let op = register_operator(&mut s);
    let stmt = stmt_cancel(1);
    let tx_raw = anchor_tx(GENESIS_TXID, 0, Some((KIND_CANCEL, sha256(&stmt))), 1);
    let txid = dsha256(&tx_raw);
    let ts = now(&s.ctx) as u32;
    // Header whose merkle root is NOT this tx.
    let header = seed_header(&mut s.ctx, 6000, [0xaa; 32], ts);
    let mk = |s: &Setup, txid_arg: [u8; 32], header: Pubkey| {
        LiteSvmProgram::new(beta_factory::ID)
            .accounts(beta_factory::client::accounts::ProcessAnchor {
                config: s.config, party: party_pda(OP_ID), party_owner: op.pubkey(), processed: anchor_pda(txid_arg), header,
                bond_escrow: s.bond_escrow, insurance: s.insurance, reward_pool: s.reward_pool, fees: fees_pda(), submitter: op.pubkey(),
                system_program: anchor_lang::solana_program::system_program::ID,
                pending: None, pending_user: None, burn: None, target_party: None, target_anchor: None, prior_party: None,
            })
            .args(beta_factory::client::args::ProcessAnchor { txid_le: txid_arg, statement: stmt.clone(), tx_raw: tx_raw.clone(), proof_block_height: 6000, branch_le: vec![], index: 0 })
            .instruction().unwrap()
    };
    s.ctx.execute_instruction(mk(&s, txid, header), &[&op]).unwrap().assert_anchor_error("InvalidMerkleBranch");
    s.ctx.execute_instruction(mk(&s, [0xbb; 32], header), &[&op]).unwrap().assert_anchor_error("TxidMismatch");
}

#[test]
fn same_anchor_cannot_be_processed_twice_and_chain_must_stay_in_order() {
    let mut s = setup();
    let op = register_operator(&mut s);
    let stmt = stmt_cancel(1);
    let (txid, r) = process(&mut s, OP_ID, op.pubkey(), (GENESIS_TXID, 0), &stmt, &op, Extra::default(), 1);
    r.assert_success();
    assert!(matches!(processed(&s.ctx, txid).status, beta_factory::types::AnchorStatus::Exercised));
    // Re-submitting the same anchor: PDA already exists.
    s.ctx.svm.expire_blockhash();
    let (_, r) = process(&mut s, OP_ID, op.pubkey(), (GENESIS_TXID, 0), &stmt, &op, Extra::default(), 1);
    assert!(!r.is_success());
    // Next anchor must spend (txid, 0); spending genesis again is off-chain.
    let (_, r) = process(&mut s, OP_ID, op.pubkey(), (GENESIS_TXID, 0), &stmt_cancel(2), &op, Extra::default(), 2);
    r.assert_anchor_error("NotOnStatementChain");
    let (_, r) = process(&mut s, OP_ID, op.pubkey(), (txid, 0), &stmt_cancel(2), &op, Extra::default(), 3);
    r.assert_success();
    assert_eq!(party(&s.ctx, OP_ID).seq, 2);
}

#[test]
fn redeem_then_true_release_marks_burn_claimed_and_second_release_is_a_lie() {
    let mut s = setup();
    let op = register_operator(&mut s);
    let user = s.ctx.svm.create_funded_account(20 * SOL).unwrap();
    let (mtx, txid) = honest_mint_attested(&mut s, &op, &user, 1, 2, 7, (GENESIS_TXID, 0), 1);
    exercise(&mut s, mtx, OP_ID, user.pubkey(), 1, &op).assert_success();

    let before = s.ctx.svm.get_balance(&user.pubkey()).unwrap();
    let to_eth = [0xeeu8; 20];
    let burn_id = burn_redeem(&mut s, &user, 2, to_eth);
    assert_eq!(beta_balance(&s.ctx, user.pubkey(), s.beta_mint), 0);
    let after = s.ctx.svm.get_balance(&user.pubkey()).unwrap();
    let burn_rent = s.ctx.svm.get_balance(&burn_pda(burn_id)).unwrap();
    assert_eq!(after + burn_rent + FEE - before, 2 * SOL);
    let c = config(&s);
    assert_eq!(c.reserve_lamports, 0);
    assert_eq!(c.eth_claims_units, 0);

    let rel = stmt_release(7, burn_id, to_eth, 2);
    let (rtx, r) = process(&mut s, OP_ID, op.pubkey(), (txid, 0), &rel, &op, Extra { burn: Some(burn_pda(burn_id)), ..Default::default() }, 2);
    r.assert_success();
    assert!(matches!(processed(&s.ctx, rtx).status, beta_factory::types::AnchorStatus::Exercised));
    let b: beta_factory::accounts::Burn = s.ctx.get_account(&burn_pda(burn_id)).unwrap();
    assert!(b.claimed);
    assert!(!party(&s.ctx, OP_ID).dead);

    // Releasing the same burn again is a false statement about Solana.
    let ins_before = s.ctx.svm.get_balance(&s.insurance).unwrap_or(0);
    let (rtx2, r) = process(&mut s, OP_ID, op.pubkey(), (rtx, 0), &rel, &op, Extra { burn: Some(burn_pda(burn_id)), ..Default::default() }, 3);
    r.assert_success();
    assert!(matches!(processed(&s.ctx, rtx2).status, beta_factory::types::AnchorStatus::Slashed));
    assert!(party(&s.ctx, OP_ID).dead);
    let ins_after = s.ctx.svm.get_balance(&s.insurance).unwrap_or(0);
    assert_eq!(ins_after - ins_before, 2 * SOL * 9 / 10);
}

#[test]
fn release_for_nonexistent_burn_slashes_to_insurance() {
    let mut s = setup();
    let op = register_operator(&mut s);
    let rel = stmt_release(7, 42, [0xeeu8; 20], 1);
    let (rtx, r) = process(&mut s, OP_ID, op.pubkey(), (GENESIS_TXID, 0), &rel, &op, Extra::default(), 1);
    r.assert_success();
    assert!(matches!(processed(&s.ctx, rtx).status, beta_factory::types::AnchorStatus::Slashed));
    assert_eq!(party(&s.ctx, OP_ID).bond, 9 * SOL);
    assert_eq!(s.ctx.svm.get_balance(&s.insurance).unwrap_or(0), SOL * 9 / 10);
}

#[test]
fn veto_on_true_release_slashes_auditor_and_veto_on_false_release_is_rewarded() {
    let mut s = setup();
    let op = register_operator(&mut s);
    let aud = register_auditor(&mut s);
    let user = s.ctx.svm.create_funded_account(20 * SOL).unwrap();
    let (mtx, atx) = honest_mint_attested(&mut s, &op, &user, 1, 1, 7, (GENESIS_TXID, 0), 1);
    exercise(&mut s, mtx, OP_ID, user.pubkey(), 1, &op).assert_success();
    let to_eth = [0xeeu8; 20];
    let burn_id = burn_redeem(&mut s, &user, 1, to_eth);

    // True release, then a false veto against it.
    let (rtx, r) = process(&mut s, OP_ID, op.pubkey(), (atx, 0), &stmt_release(7, burn_id, to_eth, 1), &op, Extra { burn: Some(burn_pda(burn_id)), ..Default::default() }, 2);
    r.assert_success();
    let (vtx, r) = process(&mut s, AUD_ID, aud.pubkey(), (GENESIS_TXID, 0), &stmt_veto(OP_ID, rtx), &aud, Extra { target_party: Some(party_pda(OP_ID)), target_anchor: Some(anchor_pda(rtx)), ..Default::default() }, 3);
    r.assert_success();
    assert!(matches!(processed(&s.ctx, vtx).status, beta_factory::types::AnchorStatus::Slashed));
    let a = party(&s.ctx, AUD_ID);
    assert!(a.dead);
    assert_eq!(a.bond, 2 * SOL - SOL / 2);

    // A fresh auditor vetoes a *false* release and gets rewarded from insurance.
    let aud2 = s.ctx.svm.create_funded_account(50 * SOL).unwrap();
    let aud2_id = [3u8; 32];
    register(&mut s, aud2_id, beta_factory::types::PartyKind::Auditor, &aud2, 2 * SOL, false).unwrap();
    let (ftx, r) = process(&mut s, OP_ID, op.pubkey(), (rtx, 0), &stmt_release(7, 99, to_eth, 1), &op, Extra::default(), 4);
    r.assert_success();
    assert!(matches!(processed(&s.ctx, ftx).status, beta_factory::types::AnchorStatus::Slashed));
    let ins = s.ctx.svm.get_balance(&s.insurance).unwrap_or(0);
    assert!(ins > 0);
    fund_rewards(&mut s, &op, SOL);
    let aud2_before = s.ctx.svm.get_balance(&aud2.pubkey()).unwrap();
    let (vtx2, r) = process(&mut s, aud2_id, aud2.pubkey(), (GENESIS_TXID, 0), &stmt_veto(OP_ID, ftx), &aud2, Extra { target_party: Some(party_pda(OP_ID)), target_anchor: Some(anchor_pda(ftx)), ..Default::default() }, 5);
    r.assert_success();
    assert!(matches!(processed(&s.ctx, vtx2).status, beta_factory::types::AnchorStatus::Exercised));
    let aud2_after = s.ctx.svm.get_balance(&aud2.pubkey()).unwrap();
    let rent = s.ctx.svm.get_balance(&anchor_pda(vtx2)).unwrap();
    assert_eq!(aud2_after + rent + FEE - aud2_before, SOL / 10);
    // Insurance was untouched by the reward.
    assert_eq!(s.ctx.svm.get_balance(&s.insurance).unwrap_or(0), ins);
}

fn fund_rewards(s: &mut Setup, funder: &Keypair, amount: u64) {
    let ix = LiteSvmProgram::new(beta_factory::ID)
        .accounts(beta_factory::client::accounts::FundRewards { reward_pool: s.reward_pool, funder: funder.pubkey(), system_program: anchor_lang::solana_program::system_program::ID })
        .args(beta_factory::client::args::FundRewards { amount })
        .instruction().unwrap();
    s.ctx.execute_instruction(ix, &[funder]).unwrap().assert_success();
}

#[test]
fn attest_fee_is_paid_to_the_attester_immediately_not_deferred_to_settle() {
    let mut s = setup();
    let op = register_operator(&mut s);
    let aud = register_auditor(&mut s);
    let user = s.ctx.svm.create_funded_account(20 * SOL).unwrap();
    let deadline = now(&s.ctx) + 86_400;
    lock_sol_with_fee(&mut s, &user, 1, 1, deadline, SOL / 100); // 0.01 SOL fee
    approve_pending(&mut s, &user, 1, 7);
    let stmt = stmt_mint(7, user.pubkey(), 1, 1, deadline);
    let (mtx, r) = process(&mut s, OP_ID, op.pubkey(), (GENESIS_TXID, 0), &stmt, &op, Extra { pending: Some(pending_pda(user.pubkey(), 1)), ..Default::default() }, 1);
    r.assert_success();

    let aud_before = s.ctx.svm.get_balance(&aud.pubkey()).unwrap();
    let (atx, r) = attest(&mut s, AUD_ID, &aud, (GENESIS_TXID, 0), mtx, pending_pda(user.pubkey(), 1), 2);
    r.assert_success();
    // Paid instantly — before exercise, before settle, before anyone knows the eventual
    // verdict. Net of the tx fee and the rent aud itself paid to create its own attest anchor.
    let rent = s.ctx.svm.get_balance(&anchor_pda(atx)).unwrap();
    assert_eq!(s.ctx.svm.get_balance(&aud.pubkey()).unwrap() + rent + FEE - aud_before, SOL / 100);
    let p: beta_factory::accounts::Pending = s.ctx.get_account(&pending_pda(user.pubkey(), 1)).unwrap();
    assert_eq!(p.attest_fee, 0); // spent, can't be paid twice

    // A redundant second attest does not pay again.
    let aud2 = s.ctx.svm.create_funded_account(50 * SOL).unwrap();
    let aud2_id = [7u8; 32];
    register(&mut s, aud2_id, beta_factory::types::PartyKind::Auditor, &aud2, 2 * SOL, false).unwrap();
    let before2 = s.ctx.svm.get_balance(&aud2.pubkey()).unwrap();
    let (_, r) = attest(&mut s, aud2_id, &aud2, (GENESIS_TXID, 0), mtx, pending_pda(user.pubkey(), 1), 3);
    r.assert_success();
    assert!(s.ctx.svm.get_balance(&aud2.pubkey()).unwrap() <= before2); // paid rent for the anchor account, never a fee

    exercise(&mut s, mtx, OP_ID, user.pubkey(), 1, &op).assert_success();
}

#[test]
fn attest_fee_is_refunded_to_the_user_when_nobody_accelerates_the_mint() {
    let mut s = setup();
    let op = register_operator(&mut s);
    let user = s.ctx.svm.create_funded_account(20 * SOL).unwrap();
    let deadline = now(&s.ctx) + 86_400;
    let fee = SOL / 100;
    lock_sol_with_fee(&mut s, &user, 1, 1, deadline, fee);
    approve_pending(&mut s, &user, 1, 7);
    let stmt = stmt_mint(7, user.pubkey(), 1, 1, deadline);
    let (mtx, r) = process(&mut s, OP_ID, op.pubkey(), (GENESIS_TXID, 0), &stmt, &op, Extra { pending: Some(pending_pda(user.pubkey(), 1)), ..Default::default() }, 1);
    r.assert_success();

    // Free path: nobody ever attests.
    advance_clock(&mut s.ctx, T_CHALLENGE + 1);
    let before = s.ctx.svm.get_balance(&user.pubkey()).unwrap();
    exercise(&mut s, mtx, OP_ID, user.pubkey(), 1, &op).assert_success();
    // Fee refunded plus the closed Pending account's own rent — at minimum the fee itself came back.
    assert!(s.ctx.svm.get_balance(&user.pubkey()).unwrap() >= before + fee);
    assert_eq!(beta_balance(&s.ctx, user.pubkey(), s.beta_mint), UNIT); // minted anyway, for free
}

#[test]
fn a_mint_queued_by_a_retired_operator_can_be_re_anchored_by_a_live_one_or_cancelled() {
    let mut s = setup();
    let op = register_operator(&mut s);
    let user = s.ctx.svm.create_funded_account(20 * SOL).unwrap();
    // Queue a mint under op, then retire op with a lie.
    let mtx = honest_mint(&mut s, &op, &user, 1, 1, 7, (GENESIS_TXID, 0), 1);
    let (_, r) = process(&mut s, OP_ID, op.pubkey(), (mtx, 0), &stmt_release(7, 42, [0xeeu8; 20], 1), &op, Extra::default(), 2);
    r.assert_success();
    assert!(party(&s.ctx, OP_ID).dead);
    advance_clock(&mut s.ctx, T_CHALLENGE + 1);
    exercise(&mut s, mtx, OP_ID, user.pubkey(), 1, &op).assert_anchor_error("PartyDead");

    // A second operator re-anchors the same MINT. Without proving the prior party is dead → lie.
    let op2 = s.ctx.svm.create_funded_account(50 * SOL).unwrap();
    let op2_id = [9u8; 32];
    register(&mut s, op2_id, beta_factory::types::PartyKind::Operator, &op2, 10 * SOL, true).unwrap();
    let deadline = { let p: beta_factory::accounts::Pending = s.ctx.get_account(&pending_pda(user.pubkey(), 1)).unwrap(); p.deadline };
    advance_clock(&mut s.ctx, -(T_CHALLENGE + 1)); // back inside the original deadline for the re-anchor
    let stmt = stmt_mint(7, user.pubkey(), 1, 1, deadline);
    let (t2, r) = process(&mut s, op2_id, op2.pubkey(), (GENESIS_TXID, 0), &stmt, &op2, Extra { pending: Some(pending_pda(user.pubkey(), 1)), pending_user: Some(user.pubkey()), prior_party: Some(party_pda(OP_ID)), ..Default::default() }, 3);
    r.assert_success();
    assert!(matches!(processed(&s.ctx, t2).status, beta_factory::types::AnchorStatus::Queued));
    let p: beta_factory::accounts::Pending = s.ctx.get_account(&pending_pda(user.pubkey(), 1)).unwrap();
    assert_eq!(p.queued_by, op2_id);
    // The old anchor can no longer exercise the slot; the new one can (after attesting).
    let (_, r) = attest(&mut s, op2_id, &op2, (t2, 0), t2, pending_pda(user.pubkey(), 1), 30);
    r.assert_success();
    exercise(&mut s, mtx, OP_ID, user.pubkey(), 1, &op).assert_anchor_error("PartyDead");
    exercise(&mut s, t2, op2_id, user.pubkey(), 1, &op2).assert_success();
    assert_eq!(beta_balance(&s.ctx, user.pubkey(), s.beta_mint), UNIT);

    // Cancel path: a second user queued under the dead op, nobody re-anchors, deadline passes.
    let u2 = s.ctx.svm.create_funded_account(20 * SOL).unwrap();
    let d2 = now(&s.ctx) + 100;
    lock_sol(&mut s, &u2, 1, 1, d2);
    approve_pending(&mut s, &u2, 1, 8);
    // Queue it under op2, then kill op2 with a lie.
    let head = { let p = party(&s.ctx, op2_id); p.anchor_txid_le };
    let (t3, r) = process(&mut s, op2_id, op2.pubkey(), (head, 0), &stmt_mint(8, u2.pubkey(), 1, 1, d2), &op2, Extra { pending: Some(pending_pda(u2.pubkey(), 1)), ..Default::default() }, 4);
    r.assert_success();
    let (_, r) = process(&mut s, op2_id, op2.pubkey(), (t3, 0), &stmt_release(7, 99, [0xeeu8; 20], 1), &op2, Extra::default(), 5);
    r.assert_success();
    assert!(party(&s.ctx, op2_id).dead);
    let mk = |s: &Setup, prior: Option<Pubkey>| LiteSvmProgram::new(beta_factory::ID)
        .accounts(beta_factory::client::accounts::ExpirePending { config: s.config, pending: pending_pda(u2.pubkey(), 1), composition: composition_pda(COMPOSITION_ID), vault: s.vault, user: u2.pubkey(), system_program: anchor_lang::solana_program::system_program::ID, token_program: litesvm_token::spl_token::ID, token_vault_authority: token_vault_authority_pda(), local_spl_0_mint: None, local_spl_0_user: None, local_spl_0_vault: None, local_spl_1_mint: None, local_spl_1_user: None, local_spl_1_vault: None, local_spl_2_mint: None, local_spl_2_user: None, local_spl_2_vault: None, prior_party: prior })
        .args(beta_factory::client::args::ExpirePending { nonce: 1 })
        .instruction().unwrap();
    advance_clock(&mut s.ctx, 101);
    s.ctx.svm.expire_blockhash();
    s.ctx.execute_instruction(mk(&s, None), &[&op]).unwrap().assert_anchor_error("PendingQueued");
    s.ctx.svm.expire_blockhash();
    let before = s.ctx.svm.get_balance(&u2.pubkey()).unwrap();
    s.ctx.execute_instruction(mk(&s, Some(party_pda(op2_id))), &[&op]).unwrap().assert_success();
    assert!(s.ctx.svm.get_balance(&u2.pubkey()).unwrap() > before + SOL - 1);
}

#[test]
fn dead_veto_pauses_operator_until_an_alive_statement_clears_it() {
    let mut s = setup();
    let op = register_operator(&mut s);
    let aud = register_auditor(&mut s);
    let user = s.ctx.svm.create_funded_account(20 * SOL).unwrap();
    let (mtx, _) = honest_mint_attested(&mut s, &op, &user, 1, 1, 7, (GENESIS_TXID, 0), 1);
    let (vtx, r) = process(&mut s, AUD_ID, aud.pubkey(), (GENESIS_TXID, 0), &stmt_veto(OP_ID, [0u8; 32]), &aud, Extra { target_party: Some(party_pda(OP_ID)), ..Default::default() }, 2);
    r.assert_success();
    assert_eq!(party(&s.ctx, OP_ID).paused_until, i64::MAX);
    exercise(&mut s, mtx, OP_ID, user.pubkey(), 1, &op).assert_anchor_error("Paused");
    // No clock lifts it.
    advance_clock(&mut s.ctx, 30 * 86_400);
    exercise(&mut s, mtx, OP_ID, user.pubkey(), 1, &op).assert_anchor_error("Paused");
    // An ALIVE statement (judged on Ethereum) does.
    let (_, r) = process(&mut s, AUD_ID, aud.pubkey(), (vtx, 0), &stmt_target(KIND_ALIVE, OP_ID), &aud, Extra { target_party: Some(party_pda(OP_ID)), ..Default::default() }, 3);
    r.assert_success();
    assert_eq!(party(&s.ctx, OP_ID).paused_until, 0);
    exercise(&mut s, mtx, OP_ID, user.pubkey(), 1, &op).assert_success();
}

#[test]
fn veto_on_queued_mint_holds_it_until_a_clear_lifts_it() {
    let mut s = setup();
    let op = register_operator(&mut s);
    let aud = register_auditor(&mut s);
    let user = s.ctx.svm.create_funded_account(20 * SOL).unwrap();
    let (mtx, _) = honest_mint_attested(&mut s, &op, &user, 1, 1, 7, (GENESIS_TXID, 0), 1);
    let (vtx, r) = process(&mut s, AUD_ID, aud.pubkey(), (GENESIS_TXID, 0), &stmt_veto(OP_ID, mtx), &aud, Extra { target_party: Some(party_pda(OP_ID)), target_anchor: Some(anchor_pda(mtx)), ..Default::default() }, 2);
    r.assert_success();
    exercise(&mut s, mtx, OP_ID, user.pubkey(), 1, &op).assert_anchor_error("Held");
    advance_clock(&mut s.ctx, 30 * 86_400);
    exercise(&mut s, mtx, OP_ID, user.pubkey(), 1, &op).assert_anchor_error("Held");
    let (_, r) = process(&mut s, AUD_ID, aud.pubkey(), (vtx, 0), &stmt_target(KIND_CLEAR, mtx), &aud, Extra { target_anchor: Some(anchor_pda(mtx)), ..Default::default() }, 3);
    r.assert_success();
    exercise(&mut s, mtx, OP_ID, user.pubkey(), 1, &op).assert_success();
}

#[test]
fn unattested_mint_executes_only_after_the_challenge_window_and_settles_clean() {
    let mut s = setup();
    let op = register_operator(&mut s);
    let u1 = s.ctx.svm.create_funded_account(20 * SOL).unwrap();
    let t1 = honest_mint(&mut s, &op, &u1, 1, 1, 7, (GENESIS_TXID, 0), 1);
    exercise(&mut s, t1, OP_ID, u1.pubkey(), 1, &op).assert_anchor_error("NotAttested");
    // The queued lock cannot be expired out from under the mint.
    advance_clock(&mut s.ctx, 90_000);
    let ix = LiteSvmProgram::new(beta_factory::ID)
        .accounts(beta_factory::client::accounts::ExpirePending { config: s.config, pending: pending_pda(u1.pubkey(), 1), composition: composition_pda(COMPOSITION_ID), vault: s.vault, user: u1.pubkey(), system_program: anchor_lang::solana_program::system_program::ID, token_program: litesvm_token::spl_token::ID, token_vault_authority: token_vault_authority_pda(), local_spl_0_mint: None, local_spl_0_user: None, local_spl_0_vault: None, local_spl_1_mint: None, local_spl_1_user: None, local_spl_1_vault: None, local_spl_2_mint: None, local_spl_2_user: None, local_spl_2_vault: None, prior_party: None })
        .args(beta_factory::client::args::ExpirePending { nonce: 1 })
        .instruction().unwrap();
    s.ctx.execute_instruction(ix, &[&op]).unwrap().assert_anchor_error("PendingQueued");
    settle(&mut s, t1, None, None, &op).assert_anchor_error("ChallengeOpen");
    // Window closes: exercisable, and settle is a no-op success (no escrow, not held).
    advance_clock(&mut s.ctx, T_CHALLENGE);
    exercise(&mut s, t1, OP_ID, u1.pubkey(), 1, &op).assert_success();
    settle(&mut s, t1, None, None, &op).assert_success();
    assert!(processed(&s.ctx, t1).settled);
    assert_eq!(beta_balance(&s.ctx, u1.pubkey(), s.beta_mint), UNIT);
}

#[test]
fn attested_mint_escrow_returns_at_settle_when_unheld_and_is_forfeited_when_held() {
    let mut s = setup();
    let op = register_operator(&mut s);
    let aud = register_auditor(&mut s);
    let user = s.ctx.svm.create_funded_account(20 * SOL).unwrap();
    // Unheld: escrow comes back.
    let (m1, a1) = honest_mint_attested(&mut s, &op, &user, 1, 1, 7, (GENESIS_TXID, 0), 1);
    exercise(&mut s, m1, OP_ID, user.pubkey(), 1, &op).assert_success();
    assert_eq!(party(&s.ctx, OP_ID).bond, 10 * SOL - SOL);
    advance_clock(&mut s.ctx, T_CHALLENGE + 1);
    settle(&mut s, m1, Some(party_pda(OP_ID)), None, &op).assert_success();
    assert_eq!(party(&s.ctx, OP_ID).bond, 10 * SOL);
    assert_eq!(processed(&s.ctx, m1).escrow, 0);
    // Held at settle: escrow → insurance (the unit stays; it is now backed by the escrow).
    let (m2, _a2) = honest_mint_attested(&mut s, &op, &user, 2, 1, 8, (a1, 0), 3);
    exercise(&mut s, m2, OP_ID, user.pubkey(), 2, &op).assert_success();
    let (_, r) = process(&mut s, AUD_ID, aud.pubkey(), (GENESIS_TXID, 0), &stmt_veto(OP_ID, m2), &aud, Extra { target_party: Some(party_pda(OP_ID)), target_anchor: Some(anchor_pda(m2)), ..Default::default() }, 5);
    r.assert_success();
    advance_clock(&mut s.ctx, T_CHALLENGE + 1);
    let ins_before = s.ctx.svm.get_balance(&s.insurance).unwrap_or(0);
    settle(&mut s, m2, Some(party_pda(OP_ID)), None, &op).assert_success();
    assert_eq!(s.ctx.svm.get_balance(&s.insurance).unwrap_or(0) - ins_before, SOL);
    assert_eq!(party(&s.ctx, OP_ID).bond, 10 * SOL - SOL);
    assert!(matches!(processed(&s.ctx, m2).status, beta_factory::types::AnchorStatus::Exercised));
}

#[test]
fn held_unattested_mint_is_cancelled_at_settle_and_the_user_can_expire() {
    let mut s = setup();
    let op = register_operator(&mut s);
    let aud = register_auditor(&mut s);
    let user = s.ctx.svm.create_funded_account(20 * SOL).unwrap();
    let mtx = honest_mint(&mut s, &op, &user, 1, 1, 7, (GENESIS_TXID, 0), 1);
    let (_, r) = process(&mut s, AUD_ID, aud.pubkey(), (GENESIS_TXID, 0), &stmt_veto(OP_ID, mtx), &aud, Extra { target_party: Some(party_pda(OP_ID)), target_anchor: Some(anchor_pda(mtx)), ..Default::default() }, 2);
    r.assert_success();
    advance_clock(&mut s.ctx, T_CHALLENGE + 1);
    exercise(&mut s, mtx, OP_ID, user.pubkey(), 1, &op).assert_anchor_error("Held");
    settle(&mut s, mtx, None, Some(pending_pda(user.pubkey(), 1)), &op).assert_success();
    assert!(matches!(processed(&s.ctx, mtx).status, beta_factory::types::AnchorStatus::Cancelled));
    let p: beta_factory::accounts::Pending = s.ctx.get_account(&pending_pda(user.pubkey(), 1)).unwrap();
    assert_eq!(p.queued_by, [0u8; 32]);
    let ix = LiteSvmProgram::new(beta_factory::ID)
        .accounts(beta_factory::client::accounts::ExpirePending { config: s.config, pending: pending_pda(user.pubkey(), 1), composition: composition_pda(COMPOSITION_ID), vault: s.vault, user: user.pubkey(), system_program: anchor_lang::solana_program::system_program::ID, token_program: litesvm_token::spl_token::ID, token_vault_authority: token_vault_authority_pda(), local_spl_0_mint: None, local_spl_0_user: None, local_spl_0_vault: None, local_spl_1_mint: None, local_spl_1_user: None, local_spl_1_vault: None, local_spl_2_mint: None, local_spl_2_user: None, local_spl_2_vault: None, prior_party: None })
        .args(beta_factory::client::args::ExpirePending { nonce: 1 })
        .instruction().unwrap();
    let before = s.ctx.svm.get_balance(&user.pubkey()).unwrap();
    s.ctx.execute_instruction(ix, &[&op]).unwrap().assert_success();
    assert!(s.ctx.svm.get_balance(&user.pubkey()).unwrap() >= before + SOL);
}

#[test]
fn a_duplicate_attest_is_a_no_op_that_still_advances_the_chain() {
    let mut s = setup();
    let op = register_operator(&mut s);
    let aud = register_auditor(&mut s);
    let user = s.ctx.svm.create_funded_account(20 * SOL).unwrap();
    let (mtx, _) = honest_mint_attested(&mut s, &op, &user, 1, 1, 7, (GENESIS_TXID, 0), 1);
    let bond_before = party(&s.ctx, AUD_ID).bond;
    let (a2, r) = attest(&mut s, AUD_ID, &aud, (GENESIS_TXID, 0), mtx, pending_pda(user.pubkey(), 1), 2);
    r.assert_success();
    assert!(matches!(processed(&s.ctx, a2).status, beta_factory::types::AnchorStatus::Exercised));
    assert_eq!(party(&s.ctx, AUD_ID).bond, bond_before); // no second escrow
    assert_eq!(processed(&s.ctx, mtx).attested_by, OP_ID); // first attester stands
    assert_eq!(party(&s.ctx, AUD_ID).anchor_txid_le, a2); // chain advanced
}

#[test]
fn attesting_a_release_solana_found_false_is_slashed() {
    let mut s = setup();
    let op = register_operator(&mut s);
    let aud = register_auditor(&mut s);
    let (rtx, r) = process(&mut s, OP_ID, op.pubkey(), (GENESIS_TXID, 0), &stmt_release(7, 42, [0xeeu8; 20], 1), &op, Extra::default(), 1);
    r.assert_success();
    assert!(matches!(processed(&s.ctx, rtx).status, beta_factory::types::AnchorStatus::Slashed));
    let (atx, r) = process(&mut s, AUD_ID, aud.pubkey(), (GENESIS_TXID, 0), &stmt_target(KIND_ATTEST, rtx), &aud, Extra { target_anchor: Some(anchor_pda(rtx)), ..Default::default() }, 2);
    r.assert_success();
    assert!(matches!(processed(&s.ctx, atx).status, beta_factory::types::AnchorStatus::Slashed));
    assert!(party(&s.ctx, AUD_ID).dead);
}

#[test]
fn expire_pending_refunds_after_deadline_only() {
    let mut s = setup();
    let user = s.ctx.svm.create_funded_account(20 * SOL).unwrap();
    let deadline = now(&s.ctx) + 100;
    lock_sol(&mut s, &user, 5, 3, deadline);
    let mk = |s: &Setup| LiteSvmProgram::new(beta_factory::ID)
        .accounts(beta_factory::client::accounts::ExpirePending { config: s.config, pending: pending_pda(user.pubkey(), 5), composition: composition_pda(COMPOSITION_ID), vault: s.vault, user: user.pubkey(), system_program: anchor_lang::solana_program::system_program::ID, token_program: litesvm_token::spl_token::ID, token_vault_authority: token_vault_authority_pda(), local_spl_0_mint: None, local_spl_0_user: None, local_spl_0_vault: None, local_spl_1_mint: None, local_spl_1_user: None, local_spl_1_vault: None, local_spl_2_mint: None, local_spl_2_user: None, local_spl_2_vault: None, prior_party: None })
        .args(beta_factory::client::args::ExpirePending { nonce: 5 })
        .instruction().unwrap();
    s.ctx.execute_instruction(mk(&s), &[&user]).unwrap().assert_anchor_error("PendingNotExpired");
    advance_clock(&mut s.ctx, 101);
    s.ctx.svm.expire_blockhash();
    let before = s.ctx.svm.get_balance(&user.pubkey()).unwrap();
    let rent = s.ctx.svm.get_balance(&pending_pda(user.pubkey(), 5)).unwrap();
    s.ctx.execute_instruction(mk(&s), &[&user]).unwrap().assert_success();
    let after = s.ctx.svm.get_balance(&user.pubkey()).unwrap();
    assert_eq!(after + FEE - before, 3 * SOL + rent);
    assert_eq!(config(&s).pending_lamports, 0);
}

#[test]
fn stalled_anchor_can_be_skipped_after_t_skip_and_a_skipped_release_is_audited_later() {
    let mut s = setup();
    let op = register_operator(&mut s);
    // Anchor carrying a RELEASE hash whose preimage nobody reveals.
    let rel = stmt_release(7, 77, [0xeeu8; 20], 1);
    let tx_raw = anchor_tx(GENESIS_TXID, 0, Some((KIND_RELEASE, sha256(&rel))), 1);
    let txid = dsha256(&tx_raw);
    let ts = now(&s.ctx) as u32;
    let header = seed_header(&mut s.ctx, 7000, txid, ts);
    let mk = |s: &Setup| LiteSvmProgram::new(beta_factory::ID)
        .accounts(beta_factory::client::accounts::SkipAnchor { config: s.config, party: party_pda(OP_ID), processed: anchor_pda(txid), header, submitter: op.pubkey(), system_program: anchor_lang::solana_program::system_program::ID })
        .args(beta_factory::client::args::SkipAnchor { txid_le: txid, tx_raw: tx_raw.clone(), proof_block_height: 7000, branch_le: vec![], index: 0 })
        .instruction().unwrap();
    s.ctx.execute_instruction(mk(&s), &[&op]).unwrap().assert_anchor_error("SkipNotReady");
    advance_clock(&mut s.ctx, 3601);
    s.ctx.svm.expire_blockhash();
    s.ctx.execute_instruction(mk(&s), &[&op]).unwrap().assert_success();
    let pa = processed(&s.ctx, txid);
    assert!(matches!(pa.status, beta_factory::types::AnchorStatus::Skipped));
    assert_eq!(pa.kind, KIND_RELEASE);
    assert_eq!(party(&s.ctx, OP_ID).anchor_txid_le, txid);

    // Preimage surfaces: burn 77 never existed → slashed.
    let ix = LiteSvmProgram::new(beta_factory::ID)
        .accounts(beta_factory::client::accounts::AuditSkippedRelease { config: s.config, party: party_pda(OP_ID), processed: anchor_pda(txid), bond_escrow: s.bond_escrow, insurance: s.insurance, submitter: op.pubkey(), system_program: anchor_lang::solana_program::system_program::ID, burn: None })
        .args(beta_factory::client::args::AuditSkippedRelease { txid_le: txid, statement: rel })
        .instruction().unwrap();
    s.ctx.execute_instruction(ix, &[&op]).unwrap().assert_success();
    assert!(matches!(processed(&s.ctx, txid).status, beta_factory::types::AnchorStatus::Slashed));
    assert!(party(&s.ctx, OP_ID).dead);
}

#[test]
fn operator_registration_needs_governance_and_minimum_bond() {
    let mut s = setup();
    let op = s.ctx.svm.create_funded_account(50 * SOL).unwrap();
    assert!(register(&mut s, OP_ID, beta_factory::types::PartyKind::Operator, &op, 10 * SOL, false).is_err());
    let op2 = s.ctx.svm.create_funded_account(50 * SOL).unwrap();
    assert!(register(&mut s, [5u8; 32], beta_factory::types::PartyKind::Operator, &op2, SOL, true).is_err());
    assert!(register(&mut s, [6u8; 32], beta_factory::types::PartyKind::Operator, &op2, 5 * SOL, true).is_ok());
    let aud = s.ctx.svm.create_funded_account(50 * SOL).unwrap();
    assert!(register(&mut s, AUD_ID, beta_factory::types::PartyKind::Auditor, &aud, SOL / 2, false).is_err());
    assert!(register(&mut s, AUD_ID, beta_factory::types::PartyKind::Auditor, &aud, SOL, false).is_ok());
    assert_eq!(s.ctx.svm.get_balance(&s.bond_escrow).unwrap(), 6 * SOL);
}

#[test]
fn unbond_returns_the_bond_after_the_delay_and_retires_the_party() {
    let mut s = setup();
    let aud = register_auditor(&mut s);
    let req = LiteSvmProgram::new(beta_factory::ID)
        .accounts(beta_factory::client::accounts::RequestUnbond { party: party_pda(AUD_ID), owner: aud.pubkey() })
        .args(beta_factory::client::args::RequestUnbond {})
        .instruction().unwrap();
    s.ctx.execute_instruction(req, &[&aud]).unwrap().assert_success();
    let mk = |s: &Setup| LiteSvmProgram::new(beta_factory::ID)
        .accounts(beta_factory::client::accounts::WithdrawBond { config: s.config, party: party_pda(AUD_ID), bond_escrow: s.bond_escrow, owner: aud.pubkey(), system_program: anchor_lang::solana_program::system_program::ID })
        .args(beta_factory::client::args::WithdrawBond {})
        .instruction().unwrap();
    s.ctx.execute_instruction(mk(&s), &[&aud]).unwrap().assert_anchor_error("UnbondNotReady");
    advance_clock(&mut s.ctx, 61);
    s.ctx.svm.expire_blockhash();
    let before = s.ctx.svm.get_balance(&aud.pubkey()).unwrap();
    s.ctx.execute_instruction(mk(&s), &[&aud]).unwrap().assert_success();
    assert_eq!(s.ctx.svm.get_balance(&aud.pubkey()).unwrap() + FEE - before, 2 * SOL);
    let a = party(&s.ctx, AUD_ID);
    assert!(a.dead);
    assert_eq!(a.bond, 0);
    // Retired party's vetoes are refused.
    let op = register_operator(&mut s);
    let (_, r) = process(&mut s, AUD_ID, aud.pubkey(), (GENESIS_TXID, 0), &stmt_veto(OP_ID, [0u8; 32]), &aud, Extra { target_party: Some(party_pda(OP_ID)), ..Default::default() }, 1);
    r.assert_anchor_error("PartyDead");
    let _ = op;
}

/// A MINT statement for an arbitrary composition/component, not just the
/// shared single-remote `COMPOSITION_ID` — used to prove §8's actual
/// multi-network gating (more than one remote leg, on different chains).
fn stmt_mint_component(composition_id: u64, component_index: u8, lock_id: u64, sol_user: Pubkey, nonce: u64, units: u64, deadline: i64) -> Vec<u8> {
    let mut v = vec![KIND_MINT];
    v.extend_from_slice(&composition_id.to_be_bytes());
    v.push(component_index);
    v.extend_from_slice(&lock_id.to_be_bytes());
    v.extend_from_slice(sol_user.as_ref());
    v.extend_from_slice(&nonce.to_be_bytes());
    v.extend_from_slice(&units.to_be_bytes());
    v.extend_from_slice(&(deadline as u64).to_be_bytes());
    v
}

fn register_composition(s: &mut Setup, id: u64, components: Vec<beta_factory::types::Component>) {
    let ix = LiteSvmProgram::new(beta_factory::ID)
        .accounts(beta_factory::client::accounts::RegisterComposition {
            config: s.config,
            composition: composition_pda(id),
            governance: s.governance.pubkey(),
            system_program: anchor_lang::solana_program::system_program::ID,
        })
        .args(beta_factory::client::args::RegisterComposition { id, components })
        .instruction()
        .unwrap();
    s.ctx.execute_instruction(ix, &[&s.governance.insecure_clone()]).unwrap().assert_success();
}

/// `spl_locals[i]` is `(mint, user_ata, vault_ata)` for the composition's
/// i-th non-native local (Solana) leg, in composition order — up to 3,
/// matching `local_spl_0/1/2`. Pass `&[]` when every local leg is native.
fn lock_sol_composition(s: &mut Setup, user: &Keypair, nonce: u64, composition_id: u64, units: u64, deadline: i64, spl_locals: &[(Pubkey, Pubkey, Pubkey)]) {
    let slot = |i: usize, pick: fn(&(Pubkey, Pubkey, Pubkey)) -> Pubkey| spl_locals.get(i).map(pick);
    let ix = LiteSvmProgram::new(beta_factory::ID)
        .accounts(beta_factory::client::accounts::LockSol {
            config: s.config,
            pending: pending_pda(user.pubkey(), nonce),
            composition: composition_pda(composition_id),
            vault: s.vault,
            fees: fees_pda(),
            beta_mint: s.beta_mint,
            user_beta: ata(user.pubkey(), s.beta_mint),
            user: user.pubkey(),
            token_program: litesvm_token::spl_token::ID,
            associated_token_program: spl_associated_token_account_interface::program::ID,
            system_program: anchor_lang::solana_program::system_program::ID,
            token_vault_authority: token_vault_authority_pda(),
            local_spl_0_mint: slot(0, |t| t.0), local_spl_0_user: slot(0, |t| t.1), local_spl_0_vault: slot(0, |t| t.2),
            local_spl_1_mint: slot(1, |t| t.0), local_spl_1_user: slot(1, |t| t.1), local_spl_1_vault: slot(1, |t| t.2),
            local_spl_2_mint: slot(2, |t| t.0), local_spl_2_user: slot(2, |t| t.1), local_spl_2_vault: slot(2, |t| t.2),
        })
        .args(beta_factory::client::args::LockSol { nonce, composition_id, units, deadline, attest_fee: 0 })
        .instruction()
        .unwrap();
    s.ctx.execute_instruction(ix, &[user]).unwrap().assert_success();
}

fn approve_pending_multi(s: &mut Setup, user: &Keypair, nonce: u64, remote_lock_id: Vec<u64>) {
    let ix = LiteSvmProgram::new(beta_factory::ID)
        .accounts(beta_factory::client::accounts::ApprovePending { pending: pending_pda(user.pubkey(), nonce), user: user.pubkey() })
        .args(beta_factory::client::args::ApprovePending { nonce, remote_lock_id })
        .instruction()
        .unwrap();
    s.ctx.execute_instruction(ix, &[user]).unwrap().assert_success();
}

/// `remotes[i]` is the anchor txid confirming component `i + 1` (index 0
/// in `remotes` is remote component 0, i.e. composition component index
/// 1, since component 0 is always the local SOL leg) — `None` for a slot
/// this composition doesn't use.
fn exercise_multi(
    s: &mut Setup,
    composition_id: u64,
    party_id: [u8; 32],
    user: Pubkey,
    nonce: u64,
    remotes: [Option<[u8; 32]>; 7],
    submitter: &Keypair,
) -> anchor_litesvm::TransactionResult {
    s.ctx.svm.expire_blockhash();
    let ix = LiteSvmProgram::new(beta_factory::ID)
        .accounts(beta_factory::client::accounts::ExerciseMint {
            config: s.config,
            pending: pending_pda(user, nonce),
            composition: composition_pda(composition_id),
            party: Some(party_pda(party_id)),
            user,
            user_beta: ata(user, s.beta_mint),
            beta_mint: s.beta_mint,
            mint_authority: s.mint_authority,
            fees: fees_pda(),
            token_program: litesvm_token::spl_token::ID,
            system_program: anchor_lang::solana_program::system_program::ID,
            remote_0: remotes[0].map(anchor_pda),
            remote_1: remotes[1].map(anchor_pda),
            remote_2: remotes[2].map(anchor_pda),
            remote_3: remotes[3].map(anchor_pda),
            remote_4: remotes[4].map(anchor_pda),
            remote_5: remotes[5].map(anchor_pda),
            remote_6: remotes[6].map(anchor_pda),
        })
        .args(beta_factory::client::args::ExerciseMint {})
        .instruction()
        .unwrap();
    s.ctx.execute_instruction(ix, &[submitter]).unwrap()
}

#[test]
fn composition_with_three_networks_gates_on_every_remote_leg_independently() {
    let mut s = setup();
    let op = register_operator(&mut s);
    let user = s.ctx.svm.create_funded_account(20 * SOL).unwrap();

    // A composition spanning three distinct networks (DESIGN_V2 §8.2):
    // the required local SOL leg, plus two *different* remote networks
    // (e.g. Ethereum and a second reserve chain) — not just two tokens on
    // the same one. `MAX_COMPONENTS` allows up to four.
    const NETWORK_ETH: u64 = 1;
    const NETWORK_OTHER: u64 = 2;
    const MULTI_COMPOSITION_ID: u64 = 2;
    register_composition(
        &mut s,
        MULTI_COMPOSITION_ID,
        vec![
            beta_factory::types::Component { network_id: 0, token_id: [0u8; 32], amount_per_unit: SOL },
            beta_factory::types::Component { network_id: NETWORK_ETH, token_id: [0u8; 32], amount_per_unit: SOL },
            beta_factory::types::Component { network_id: NETWORK_OTHER, token_id: [0u8; 32], amount_per_unit: SOL },
        ],
    );

    let deadline = now(&s.ctx) + 86_400;
    let nonce = 1;
    let units = 3;
    lock_sol_composition(&mut s, &user, nonce, MULTI_COMPOSITION_ID, units, deadline, &[]);
    approve_pending_multi(&mut s, &user, nonce, vec![70, 80]); // lock ids for the two remote legs
    assert_eq!(config(&s).pending_lamports, units * SOL);

    // Anchor and attest remote component 0 (the "Ethereum" leg) only.
    let stmt_a = stmt_mint_component(MULTI_COMPOSITION_ID, 0, 70, user.pubkey(), nonce, units, deadline);
    let (tx_a, r) = process(&mut s, OP_ID, op.pubkey(), (GENESIS_TXID, 0), &stmt_a, &op, Extra { pending: Some(pending_pda(user.pubkey(), nonce)), ..Default::default() }, 1);
    r.assert_success();
    let (_, r) = attest(&mut s, OP_ID, &op, (tx_a, 0), tx_a, pending_pda(user.pubkey(), nonce), 2);
    r.assert_success();

    // One leg ready, the other never anchored: exercise still refuses.
    exercise_multi(&mut s, MULTI_COMPOSITION_ID, OP_ID, user.pubkey(), nonce, [Some(tx_a), None, None, None, None, None, None], &op)
        .assert_anchor_error("MissingAccount");
    assert_eq!(beta_balance(&s.ctx, user.pubkey(), s.beta_mint), 0);

    // Now anchor and attest remote component 1 (the "second reserve
    // chain" leg) too — a genuinely different network from component 0.
    let head = { let p = party(&s.ctx, OP_ID); p.anchor_txid_le };
    let stmt_b = stmt_mint_component(MULTI_COMPOSITION_ID, 1, 80, user.pubkey(), nonce, units, deadline);
    let (tx_b, r) = process(&mut s, OP_ID, op.pubkey(), (head, 0), &stmt_b, &op, Extra { pending: Some(pending_pda(user.pubkey(), nonce)), ..Default::default() }, 3);
    r.assert_success();
    let (_, r) = attest(&mut s, OP_ID, &op, (tx_b, 0), tx_b, pending_pda(user.pubkey(), nonce), 4);
    r.assert_success();

    // Both remote legs ready on both networks: exercise now succeeds.
    exercise_multi(&mut s, MULTI_COMPOSITION_ID, OP_ID, user.pubkey(), nonce, [Some(tx_a), Some(tx_b), None, None, None, None, None], &op).assert_success();
    assert_eq!(beta_balance(&s.ctx, user.pubkey(), s.beta_mint), units * UNIT);
    let c = config(&s);
    assert_eq!(c.pending_lamports, 0);
    assert_eq!(c.reserve_lamports, units * SOL); // priced off the local leg only
    assert_eq!(c.eth_claims_units, units);
    assert!(matches!(processed(&s.ctx, tx_a).status, beta_factory::types::AnchorStatus::Exercised));
    assert!(matches!(processed(&s.ctx, tx_b).status, beta_factory::types::AnchorStatus::Exercised));
    assert!(s.ctx.svm.get_account(&pending_pda(user.pubkey(), nonce)).is_none());
}

#[test]
fn composition_spends_the_full_budget_on_seven_remote_networks() {
    let mut s = setup();
    let op = register_operator(&mut s);
    let user = s.ctx.svm.create_funded_account(20 * SOL).unwrap();

    // §8.12's flat budget maxed out the other way from the local test
    // below: 1 mandatory local leg + all 7 remaining slots spent on 7
    // *different* remote networks, one token each — proving exercise_mint
    // actually gates on more than the old hardcoded 3 remote slots.
    const MULTI7_COMPOSITION_ID: u64 = 5;
    let mut components = vec![beta_factory::types::Component { network_id: 0, token_id: [0u8; 32], amount_per_unit: SOL }];
    for net in 1..=7u64 {
        components.push(beta_factory::types::Component { network_id: net, token_id: [0u8; 32], amount_per_unit: SOL });
    }
    register_composition(&mut s, MULTI7_COMPOSITION_ID, components);

    let deadline = now(&s.ctx) + 86_400;
    let nonce = 1;
    let units = 1;
    lock_sol_composition(&mut s, &user, nonce, MULTI7_COMPOSITION_ID, units, deadline, &[]);
    let lock_ids: Vec<u64> = (0..7).map(|i| 100 + i).collect();
    approve_pending_multi(&mut s, &user, nonce, lock_ids.clone());

    let mut head = (GENESIS_TXID, 0u32);
    let mut txids = [[0u8; 32]; 7];
    for i in 0..7usize {
        let stmt = stmt_mint_component(MULTI7_COMPOSITION_ID, i as u8, lock_ids[i], user.pubkey(), nonce, units, deadline);
        let (tx, r) = process(&mut s, OP_ID, op.pubkey(), head, &stmt, &op, Extra { pending: Some(pending_pda(user.pubkey(), nonce)), ..Default::default() }, i as u8 + 1);
        r.assert_success();
        let (atx, r) = attest(&mut s, OP_ID, &op, (tx, 0), tx, pending_pda(user.pubkey(), nonce), i as u8 + 50);
        r.assert_success();
        txids[i] = tx;
        head = (atx, 0);
    }

    let remotes: [Option<[u8; 32]>; 7] = std::array::from_fn(|i| Some(txids[i]));
    exercise_multi(&mut s, MULTI7_COMPOSITION_ID, OP_ID, user.pubkey(), nonce, remotes, &op).assert_success();
    assert_eq!(beta_balance(&s.ctx, user.pubkey(), s.beta_mint), units * UNIT);
    for tx in txids {
        assert!(matches!(processed(&s.ctx, tx).status, beta_factory::types::AnchorStatus::Exercised));
    }
    assert!(s.ctx.svm.get_account(&pending_pda(user.pubkey(), nonce)).is_none());
}

#[test]
fn composition_spans_five_named_evm_networks() {
    let mut s = setup();
    let op = register_operator(&mut s);
    let user = s.ctx.svm.create_funded_account(20 * SOL).unwrap();

    // Real, verified EVM chain IDs for Base and Arbitrum; the other three
    // don't have a chain ID Claude could verify (Hyperliquid's HyperEVM
    // is believed to be 999 but unconfirmed here; Robinhood isn't known
    // to run a public chain at all; Tempo's isn't confirmed either) — so
    // those three use placeholder IDs above any real chain's range,
    // clearly not asserted as the real thing. The composition/exercise
    // mechanics being tested don't depend on the ID being real; only that
    // it's a distinct `network_id` judged on its own statement-chain leg.
    const NETWORK_BASE: u64 = 8453;
    const NETWORK_ARBITRUM: u64 = 42161;
    const NETWORK_HYPERLIQUID_PLACEHOLDER: u64 = 1_000_000_001;
    const NETWORK_ROBINHOOD_PLACEHOLDER: u64 = 1_000_000_002;
    const NETWORK_TEMPO_PLACEHOLDER: u64 = 1_000_000_003;
    let networks = [
        NETWORK_BASE,
        NETWORK_ARBITRUM,
        NETWORK_HYPERLIQUID_PLACEHOLDER,
        NETWORK_ROBINHOOD_PLACEHOLDER,
        NETWORK_TEMPO_PLACEHOLDER,
    ];

    const FIVE_NETWORK_COMPOSITION_ID: u64 = 6;
    let mut components = vec![beta_factory::types::Component { network_id: 0, token_id: [0u8; 32], amount_per_unit: SOL }];
    for net in networks {
        components.push(beta_factory::types::Component { network_id: net, token_id: [0u8; 32], amount_per_unit: SOL });
    }
    register_composition(&mut s, FIVE_NETWORK_COMPOSITION_ID, components);

    let deadline = now(&s.ctx) + 86_400;
    let nonce = 1;
    let units = 2;
    lock_sol_composition(&mut s, &user, nonce, FIVE_NETWORK_COMPOSITION_ID, units, deadline, &[]);
    assert_eq!(config(&s).pending_lamports, units * SOL);

    let lock_ids: Vec<u64> = (0..5).map(|i| 200 + i).collect();
    approve_pending_multi(&mut s, &user, nonce, lock_ids.clone());

    let mut head = (GENESIS_TXID, 0u32);
    let mut txids = [[0u8; 32]; 5];
    for i in 0..5usize {
        let stmt = stmt_mint_component(FIVE_NETWORK_COMPOSITION_ID, i as u8, lock_ids[i], user.pubkey(), nonce, units, deadline);
        let (tx, r) = process(&mut s, OP_ID, op.pubkey(), head, &stmt, &op, Extra { pending: Some(pending_pda(user.pubkey(), nonce)), ..Default::default() }, i as u8 + 1);
        r.assert_success();
        // Attest all but the last leg (Tempo) — proves gating still holds
        // with a mix of fast (ATTESTed) and unresolved legs, not just
        // all-attested like the 7-network test.
        if i < 4 {
            let (atx, r) = attest(&mut s, OP_ID, &op, (tx, 0), tx, pending_pda(user.pubkey(), nonce), i as u8 + 50);
            r.assert_success();
            head = (atx, 0);
        } else {
            head = (tx, 0);
        }
        txids[i] = tx;
    }

    let remotes: [Option<[u8; 32]>; 7] = std::array::from_fn(|i| if i < 5 { Some(txids[i]) } else { None });
    // Tempo's leg is neither attested nor past its challenge window yet.
    exercise_multi(&mut s, FIVE_NETWORK_COMPOSITION_ID, OP_ID, user.pubkey(), nonce, remotes, &op)
        .assert_anchor_error("NotAttested");
    assert_eq!(beta_balance(&s.ctx, user.pubkey(), s.beta_mint), 0);

    // Past the challenge window, the slow (unattested) Tempo leg is ready too.
    advance_clock(&mut s.ctx, T_CHALLENGE + 1);
    exercise_multi(&mut s, FIVE_NETWORK_COMPOSITION_ID, OP_ID, user.pubkey(), nonce, remotes, &op).assert_success();
    assert_eq!(beta_balance(&s.ctx, user.pubkey(), s.beta_mint), units * UNIT);
    for tx in txids {
        assert!(matches!(processed(&s.ctx, tx).status, beta_factory::types::AnchorStatus::Exercised));
    }
    assert!(s.ctx.svm.get_account(&pending_pda(user.pubkey(), nonce)).is_none());
}

#[test]
fn composition_can_lock_more_than_one_solana_token_with_no_operator_needed() {
    let mut s = setup();
    let user = s.ctx.svm.create_funded_account(20 * SOL).unwrap();

    // A second Solana-native asset alongside native SOL, both as *local*
    // legs of the same composition (DESIGN_V2 §8.10's full symmetric
    // revision: Solana may hold more than one token, the same as any
    // remote network can).
    let spl_mint = litesvm_token::CreateMint::new(&mut s.ctx.svm, &user).decimals(6).send().unwrap();
    let user_ata = litesvm_token::CreateAssociatedTokenAccount::new(&mut s.ctx.svm, &user, &spl_mint).send().unwrap();
    litesvm_token::MintTo::new(&mut s.ctx.svm, &user, &spl_mint, &user_ata, 1_000_000_000).send().unwrap();
    let vault_ata = ata(token_vault_authority_pda(), spl_mint);

    const LOCAL_MULTI_COMPOSITION_ID: u64 = 3;
    let spl_amount_per_unit: u64 = 1_000;
    register_composition(
        &mut s,
        LOCAL_MULTI_COMPOSITION_ID,
        vec![
            beta_factory::types::Component { network_id: 0, token_id: [0u8; 32], amount_per_unit: SOL },
            beta_factory::types::Component { network_id: 0, token_id: spl_mint.to_bytes(), amount_per_unit: spl_amount_per_unit },
        ],
    );

    let deadline = now(&s.ctx) + 86_400;
    let units = 5;
    let nonce = 1;
    let spl_before = token_balance(&s.ctx, user.pubkey(), spl_mint);
    lock_sol_composition(&mut s, &user, nonce, LOCAL_MULTI_COMPOSITION_ID, units, deadline, &[(spl_mint, user_ata, vault_ata)]);

    // Both local legs actually moved, atomically, in the one lock_sol call.
    assert_eq!(s.ctx.svm.get_balance(&s.vault).unwrap(), units * SOL);
    assert_eq!(token_balance(&s.ctx, token_vault_authority_pda(), spl_mint), units * spl_amount_per_unit);
    assert_eq!(token_balance(&s.ctx, user.pubkey(), spl_mint), spl_before - units * spl_amount_per_unit);
    assert_eq!(config(&s).pending_lamports, units * SOL);

    // No remote legs at all: nothing to approve for, and no operator is
    // ever involved — exercise succeeds immediately, permissionlessly,
    // with `party` and every `remote_N` left out entirely.
    approve_pending_multi(&mut s, &user, nonce, vec![]);
    let ix = LiteSvmProgram::new(beta_factory::ID)
        .accounts(beta_factory::client::accounts::ExerciseMint {
            config: s.config,
            pending: pending_pda(user.pubkey(), nonce),
            composition: composition_pda(LOCAL_MULTI_COMPOSITION_ID),
            party: None,
            user: user.pubkey(),
            user_beta: ata(user.pubkey(), s.beta_mint),
            beta_mint: s.beta_mint,
            mint_authority: s.mint_authority,
            fees: fees_pda(),
            token_program: litesvm_token::spl_token::ID,
            system_program: anchor_lang::solana_program::system_program::ID,
            remote_0: None,
            remote_1: None,
            remote_2: None,
            remote_3: None,
            remote_4: None,
            remote_5: None,
            remote_6: None,
        })
        .args(beta_factory::client::args::ExerciseMint {})
        .instruction()
        .unwrap();
    s.ctx.svm.expire_blockhash();
    s.ctx.execute_instruction(ix, &[&user]).unwrap().assert_success();

    assert_eq!(beta_balance(&s.ctx, user.pubkey(), s.beta_mint), units * UNIT);
    let c = config(&s);
    assert_eq!(c.pending_lamports, 0);
    assert_eq!(c.reserve_lamports, units * SOL);
    assert!(s.ctx.svm.get_account(&pending_pda(user.pubkey(), nonce)).is_none());

    // The SPL leg is still fully backed in the vault's own ATA — no
    // global tally tracks it, its balance is the reserve.
    assert_eq!(token_balance(&s.ctx, token_vault_authority_pda(), spl_mint), units * spl_amount_per_unit);
}

#[test]
fn expire_pending_refunds_both_native_and_spl_local_legs() {
    let mut s = setup();
    let user = s.ctx.svm.create_funded_account(20 * SOL).unwrap();

    let spl_mint = litesvm_token::CreateMint::new(&mut s.ctx.svm, &user).decimals(6).send().unwrap();
    let user_ata = litesvm_token::CreateAssociatedTokenAccount::new(&mut s.ctx.svm, &user, &spl_mint).send().unwrap();
    litesvm_token::MintTo::new(&mut s.ctx.svm, &user, &spl_mint, &user_ata, 1_000_000_000).send().unwrap();
    let vault_ata = ata(token_vault_authority_pda(), spl_mint);

    const COMPOSITION_ID_2: u64 = 4;
    let spl_amount_per_unit: u64 = 2_000;
    register_composition(
        &mut s,
        COMPOSITION_ID_2,
        vec![
            beta_factory::types::Component { network_id: 0, token_id: [0u8; 32], amount_per_unit: SOL },
            beta_factory::types::Component { network_id: 0, token_id: spl_mint.to_bytes(), amount_per_unit: spl_amount_per_unit },
        ],
    );

    let deadline = now(&s.ctx) + 100;
    let units = 3;
    let nonce = 1;
    let sol_before = s.ctx.svm.get_balance(&user.pubkey()).unwrap();
    let spl_before = token_balance(&s.ctx, user.pubkey(), spl_mint);
    lock_sol_composition(&mut s, &user, nonce, COMPOSITION_ID_2, units, deadline, &[(spl_mint, user_ata, vault_ata)]);
    assert_eq!(token_balance(&s.ctx, token_vault_authority_pda(), spl_mint), units * spl_amount_per_unit);

    advance_clock(&mut s.ctx, 101);
    s.ctx.svm.expire_blockhash();
    let ix = LiteSvmProgram::new(beta_factory::ID)
        .accounts(beta_factory::client::accounts::ExpirePending {
            config: s.config,
            pending: pending_pda(user.pubkey(), nonce),
            composition: composition_pda(COMPOSITION_ID_2),
            vault: s.vault,
            user: user.pubkey(),
            token_program: litesvm_token::spl_token::ID,
            system_program: anchor_lang::solana_program::system_program::ID,
            prior_party: None,
            token_vault_authority: token_vault_authority_pda(),
            local_spl_0_mint: Some(spl_mint),
            local_spl_0_user: Some(user_ata),
            local_spl_0_vault: Some(vault_ata),
            local_spl_1_mint: None,
            local_spl_1_user: None,
            local_spl_1_vault: None,
            local_spl_2_mint: None,
            local_spl_2_user: None,
            local_spl_2_vault: None,
        })
        .args(beta_factory::client::args::ExpirePending { nonce })
        .instruction()
        .unwrap();
    s.ctx.execute_instruction(ix, &[&user]).unwrap().assert_success();

    // `sol_before` was captured before the lock, so the units*SOL round-
    // trips through the vault and back — net change is just the small
    // fees/rent this test's several transactions spent, not a gain.
    let after = s.ctx.svm.get_balance(&user.pubkey()).unwrap();
    assert!(after + SOL / 100 >= sol_before);
    assert!(after < sol_before);
    assert_eq!(token_balance(&s.ctx, user.pubkey(), spl_mint), spl_before);
    assert_eq!(token_balance(&s.ctx, token_vault_authority_pda(), spl_mint), 0);
    assert_eq!(s.ctx.svm.get_balance(&s.vault).unwrap_or(0), 0);
    assert_eq!(config(&s).pending_lamports, 0);
    assert!(s.ctx.svm.get_account(&pending_pda(user.pubkey(), nonce)).is_none());
}

#[test]
fn dead_operator_anchors_are_still_processed_but_never_exercised() {
    let mut s = setup();
    let op = register_operator(&mut s);
    let user = s.ctx.svm.create_funded_account(20 * SOL).unwrap();
    // Lie first → dead.
    let (ltx, r) = process(&mut s, OP_ID, op.pubkey(), (GENESIS_TXID, 0), &stmt_release(7, 42, [0xeeu8; 20], 1), &op, Extra::default(), 1);
    r.assert_success();
    assert!(party(&s.ctx, OP_ID).dead);
    // A subsequent true MINT is judged and queued (so its audit trail exists) but exercise is refused.
    let mtx = honest_mint(&mut s, &op, &user, 1, 1, 7, (ltx, 0), 2);
    assert!(matches!(processed(&s.ctx, mtx).status, beta_factory::types::AnchorStatus::Queued));
    advance_clock(&mut s.ctx, T_CHALLENGE + 1);
    exercise(&mut s, mtx, OP_ID, user.pubkey(), 1, &op).assert_anchor_error("PartyDead");
}
