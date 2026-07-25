use anchor_lang;
use anchor_litesvm::{AnchorContext, AnchorLiteSVM, TestHelpers};
use solana_keypair::Keypair;
use solana_program::pubkey::Pubkey;
use solana_signer::Signer;

anchor_lang::declare_program!(ipow);

fn new_ctx() -> AnchorContext {
    AnchorLiteSVM::build_with_program(ipow::ID, include_bytes!("../../../target/deploy/ipow.so"))
}

fn setup_initialized() -> (AnchorContext, Keypair, Keypair, Pubkey) {
    let mut ctx = new_ctx();
    let admin = ctx.svm.create_funded_account(10_000_000_000).unwrap();
    let operator = ctx.svm.create_funded_account(10_000_000_000).unwrap();

    let (global_state, _) = ctx.svm.get_pda_with_bump(&[b"global_state"], &ipow::ID);
    let (escrow_vault, _) = ctx.svm.get_pda_with_bump(&[b"escrow"], &ipow::ID);

    let ix = ctx
        .program()
        .accounts(ipow::client::accounts::Initialize {
            global_state,
            escrow_vault,
            admin: admin.pubkey(),
            system_program: anchor_lang::solana_program::system_program::ID,
        })
        .args(ipow::client::args::Initialize {
            operator: operator.pubkey(),
            commit_fee_bps: 50,
        })
        .instruction()
        .unwrap();

    ctx.execute_instruction(ix, &[&admin])
        .unwrap()
        .assert_success();

    (ctx, admin, operator, global_state)
}

fn conversion_pda(ctx: &AnchorContext, next_tx_id: u64) -> Pubkey {
    ctx.svm
        .get_pda_with_bump(&[b"conversion", &next_tx_id.to_le_bytes()], &ipow::ID)
        .0
}

const BITCOIN_AMOUNT: u64 = 100_000;
const NATIVE_AMOUNT: u64 = 1_000_000_000; // 1 SOL
const COMMIT_FEE: u64 = 5_000_000; // 0.5% of NATIVE_AMOUNT
fn btc_program() -> Vec<u8> {
    let mut p = vec![0x76, 0xa9, 0x14];
    p.extend_from_slice(&[0x11u8; 20]);
    p.extend_from_slice(&[0x88, 0xac]);
    p
}

#[test]
fn commit_bitcoin_to_native_rejects_zero_amount() {
    let (mut ctx, _admin, _operator, global_state) = setup_initialized();
    let user = ctx.svm.create_funded_account(10_000_000_000).unwrap();
    let conversion = conversion_pda(&ctx, 1);
    let (escrow_vault, _) = ctx.svm.get_pda_with_bump(&[b"escrow"], &ipow::ID);

    let ix = ctx
        .program()
        .accounts(ipow::client::accounts::CommitBitcoinToNative {
            global_state,
            escrow_vault,
            conversion,
            user: user.pubkey(),
            system_program: anchor_lang::solana_program::system_program::ID,
        })
        .args(ipow::client::args::CommitBitcoinToNative {
            bitcoin_amount: 0,
            native_amount: NATIVE_AMOUNT,
            network_id: 0,
            network_address: vec![],
            user_program: btc_program(),
        })
        .instruction()
        .unwrap();

    let result = ctx.execute_instruction(ix, &[&user]).unwrap();
    assert!(!result.is_success());
}

#[test]
fn commit_bitcoin_to_native_rejects_bad_program_length() {
    let (mut ctx, _admin, _operator, global_state) = setup_initialized();
    let user = ctx.svm.create_funded_account(10_000_000_000).unwrap();
    let conversion = conversion_pda(&ctx, 1);
    let (escrow_vault, _) = ctx.svm.get_pda_with_bump(&[b"escrow"], &ipow::ID);

    let ix = ctx
        .program()
        .accounts(ipow::client::accounts::CommitBitcoinToNative {
            global_state,
            escrow_vault,
            conversion,
            user: user.pubkey(),
            system_program: anchor_lang::solana_program::system_program::ID,
        })
        .args(ipow::client::args::CommitBitcoinToNative {
            bitcoin_amount: BITCOIN_AMOUNT,
            native_amount: NATIVE_AMOUNT,
            network_id: 0,
            network_address: vec![],
            user_program: vec![], // empty — invalid
        })
        .instruction()
        .unwrap();

    let result = ctx.execute_instruction(ix, &[&user]).unwrap();
    assert!(!result.is_success());
}

#[test]
fn commit_bitcoin_to_native_rejects_nonzero_network_id() {
    let (mut ctx, _admin, _operator, global_state) = setup_initialized();
    let user = ctx.svm.create_funded_account(10_000_000_000).unwrap();
    let conversion = conversion_pda(&ctx, 1);
    let (escrow_vault, _) = ctx.svm.get_pda_with_bump(&[b"escrow"], &ipow::ID);

    let ix = ctx
        .program()
        .accounts(ipow::client::accounts::CommitBitcoinToNative {
            global_state,
            escrow_vault,
            conversion,
            user: user.pubkey(),
            system_program: anchor_lang::solana_program::system_program::ID,
        })
        .args(ipow::client::args::CommitBitcoinToNative {
            bitcoin_amount: BITCOIN_AMOUNT,
            native_amount: NATIVE_AMOUNT,
            network_id: 777, // must be 0 for this instruction
            network_address: vec![],
            user_program: btc_program(),
        })
        .instruction()
        .unwrap();

    let result = ctx.execute_instruction(ix, &[&user]).unwrap();
    assert!(!result.is_success());
}

#[test]
fn commit_bitcoin_to_native_succeeds_and_holds_commit_fee() {
    let (mut ctx, _admin, _operator, global_state) = setup_initialized();
    let user = ctx.svm.create_funded_account(10_000_000_000).unwrap();
    let conversion = conversion_pda(&ctx, 1);
    let (escrow_vault, _) = ctx.svm.get_pda_with_bump(&[b"escrow"], &ipow::ID);

    let ix = ctx
        .program()
        .accounts(ipow::client::accounts::CommitBitcoinToNative {
            global_state,
            escrow_vault,
            conversion,
            user: user.pubkey(),
            system_program: anchor_lang::solana_program::system_program::ID,
        })
        .args(ipow::client::args::CommitBitcoinToNative {
            bitcoin_amount: BITCOIN_AMOUNT,
            native_amount: NATIVE_AMOUNT,
            network_id: 0,
            network_address: vec![],
            user_program: btc_program(),
        })
        .instruction()
        .unwrap();

    ctx.execute_instruction(ix, &[&user])
        .unwrap()
        .assert_success();

    let state: ipow::accounts::GlobalState = ctx.get_account(&global_state).unwrap();
    assert_eq!(state.total_held_commit_fees, COMMIT_FEE);
    assert_eq!(state.next_tx_id, 2);

    let conv: ipow::accounts::Conversion = ctx.get_account(&conversion).unwrap();
    assert_eq!(conv.tx_id, 1);
    assert_eq!(conv.user, user.pubkey());
    assert!(!conv.is_native_to_bitcoin);
    assert_eq!(conv.commit_fee, COMMIT_FEE);
}

#[test]
fn commit_native_to_bitcoin_succeeds_for_direct_bitcoin_settlement() {
    let (mut ctx, _admin, _operator, global_state) = setup_initialized();
    let user = ctx.svm.create_funded_account(10_000_000_000).unwrap();
    let conversion = conversion_pda(&ctx, 1);
    let (escrow_vault, _) = ctx.svm.get_pda_with_bump(&[b"escrow"], &ipow::ID);

    let ix = ctx
        .program()
        .accounts(ipow::client::accounts::CommitNativeToBitcoin {
            global_state,
            escrow_vault,
            conversion,
            network_config: None,
            user: user.pubkey(),
            system_program: anchor_lang::solana_program::system_program::ID,
        })
        .args(ipow::client::args::CommitNativeToBitcoin {
            native_amount: NATIVE_AMOUNT,
            bitcoin_amount: BITCOIN_AMOUNT,
            network_id: 0,
            network_address: vec![],
            user_program: btc_program(),
        })
        .instruction()
        .unwrap();

    ctx.execute_instruction(ix, &[&user])
        .unwrap()
        .assert_success();

    let conv: ipow::accounts::Conversion = ctx.get_account(&conversion).unwrap();
    assert!(conv.is_native_to_bitcoin);
    assert_eq!(conv.native_amount, NATIVE_AMOUNT);
    assert_eq!(conv.bitcoin_amount, BITCOIN_AMOUNT);
}

#[test]
fn commit_native_to_bitcoin_rejects_network_address_for_direct_bitcoin() {
    let (mut ctx, _admin, _operator, global_state) = setup_initialized();
    let user = ctx.svm.create_funded_account(10_000_000_000).unwrap();
    let conversion = conversion_pda(&ctx, 1);
    let (escrow_vault, _) = ctx.svm.get_pda_with_bump(&[b"escrow"], &ipow::ID);

    let ix = ctx
        .program()
        .accounts(ipow::client::accounts::CommitNativeToBitcoin {
            global_state,
            escrow_vault,
            conversion,
            network_config: None,
            user: user.pubkey(),
            system_program: anchor_lang::solana_program::system_program::ID,
        })
        .args(ipow::client::args::CommitNativeToBitcoin {
            native_amount: NATIVE_AMOUNT,
            bitcoin_amount: BITCOIN_AMOUNT,
            network_id: 0,
            network_address: vec![0x11; 20], // must be empty for network_id 0
            user_program: btc_program(),
        })
        .instruction()
        .unwrap();

    let result = ctx.execute_instruction(ix, &[&user]).unwrap();
    assert!(!result.is_success());
}

#[test]
fn refund_if_not_approved_returns_commit_fee_to_user() {
    let (mut ctx, _admin, _operator, global_state) = setup_initialized();
    let user = ctx.svm.create_funded_account(10_000_000_000).unwrap();
    let conversion = conversion_pda(&ctx, 1);
    let (escrow_vault, _) = ctx.svm.get_pda_with_bump(&[b"escrow"], &ipow::ID);

    let commit_ix = ctx
        .program()
        .accounts(ipow::client::accounts::CommitBitcoinToNative {
            global_state,
            escrow_vault,
            conversion,
            user: user.pubkey(),
            system_program: anchor_lang::solana_program::system_program::ID,
        })
        .args(ipow::client::args::CommitBitcoinToNative {
            bitcoin_amount: BITCOIN_AMOUNT,
            native_amount: NATIVE_AMOUNT,
            network_id: 0,
            network_address: vec![],
            user_program: btc_program(),
        })
        .instruction()
        .unwrap();
    ctx.execute_instruction(commit_ix, &[&user])
        .unwrap()
        .assert_success();

    let escrow_balance_before = ctx.svm.get_balance(&escrow_vault).unwrap();

    let refund_ix = ctx
        .program()
        .accounts(ipow::client::accounts::RefundIfNotApproved {
            global_state,
            escrow_vault,
            conversion,
            user: user.pubkey(),
            system_program: anchor_lang::solana_program::system_program::ID,
        })
        .args(ipow::client::args::RefundIfNotApproved {})
        .instruction()
        .unwrap();
    ctx.execute_instruction(refund_ix, &[&user])
        .unwrap()
        .assert_success();

    // A fully-drained escrow (0 lamports left) is purged from state entirely on
    // Solana, so `get_balance` returns `None` rather than `Some(0)` here.
    let escrow_balance_after = ctx.svm.get_balance(&escrow_vault).unwrap_or(0);
    assert_eq!(escrow_balance_before - escrow_balance_after, COMMIT_FEE);

    let conv: ipow::accounts::Conversion = ctx.get_account(&conversion).unwrap();
    assert_eq!(conv.commit_fee, 0);

    let state: ipow::accounts::GlobalState = ctx.get_account(&global_state).unwrap();
    assert_eq!(state.total_held_commit_fees, 0);
}

#[test]
fn refund_if_not_approved_rejects_double_refund() {
    let (mut ctx, _admin, _operator, global_state) = setup_initialized();
    let user = ctx.svm.create_funded_account(10_000_000_000).unwrap();
    let conversion = conversion_pda(&ctx, 1);
    let (escrow_vault, _) = ctx.svm.get_pda_with_bump(&[b"escrow"], &ipow::ID);

    let commit_ix = ctx
        .program()
        .accounts(ipow::client::accounts::CommitBitcoinToNative {
            global_state,
            escrow_vault,
            conversion,
            user: user.pubkey(),
            system_program: anchor_lang::solana_program::system_program::ID,
        })
        .args(ipow::client::args::CommitBitcoinToNative {
            bitcoin_amount: BITCOIN_AMOUNT,
            native_amount: NATIVE_AMOUNT,
            network_id: 0,
            network_address: vec![],
            user_program: btc_program(),
        })
        .instruction()
        .unwrap();
    ctx.execute_instruction(commit_ix, &[&user])
        .unwrap()
        .assert_success();

    fn refund_accounts(
        global_state: Pubkey,
        escrow_vault: Pubkey,
        conversion: Pubkey,
        user: Pubkey,
    ) -> ipow::client::accounts::RefundIfNotApproved {
        ipow::client::accounts::RefundIfNotApproved {
            global_state,
            escrow_vault,
            conversion,
            user,
            system_program: anchor_lang::solana_program::system_program::ID,
        }
    }

    let refund_ix = ctx
        .program()
        .accounts(refund_accounts(
            global_state,
            escrow_vault,
            conversion,
            user.pubkey(),
        ))
        .args(ipow::client::args::RefundIfNotApproved {})
        .instruction()
        .unwrap();
    ctx.execute_instruction(refund_ix, &[&user])
        .unwrap()
        .assert_success();

    let second_refund_ix = ctx
        .program()
        .accounts(refund_accounts(
            global_state,
            escrow_vault,
            conversion,
            user.pubkey(),
        ))
        .args(ipow::client::args::RefundIfNotApproved {})
        .instruction()
        .unwrap();
    let result = ctx.execute_instruction(second_refund_ix, &[&user]).unwrap();
    assert!(!result.is_success());
}

#[test]
fn submit_bitcoin_proof_cache_rejects_non_operator_supplied_as_operator_account() {
    // Regression test for the fix adding `address = global_state.operator` to
    // the `operator` account: before that fix, this account was an unchecked
    // `UncheckedAccount` accepted from the caller as-is, so a native-to-bitcoin
    // proof submission would pay `conversion.native_amount` to whatever pubkey
    // was supplied here instead of the protocol's real operator.
    let (mut ctx, _admin, operator, global_state) = setup_initialized();
    let user = ctx.svm.create_funded_account(10_000_000_000).unwrap();
    let stranger = ctx.svm.create_funded_account(10_000_000_000).unwrap();
    let conversion = conversion_pda(&ctx, 1);
    let (escrow_vault, _) = ctx.svm.get_pda_with_bump(&[b"escrow"], &ipow::ID);

    let commit_ix = ctx
        .program()
        .accounts(ipow::client::accounts::CommitNativeToBitcoin {
            global_state,
            escrow_vault,
            conversion,
            network_config: None,
            user: user.pubkey(),
            system_program: anchor_lang::solana_program::system_program::ID,
        })
        .args(ipow::client::args::CommitNativeToBitcoin {
            native_amount: NATIVE_AMOUNT,
            bitcoin_amount: BITCOIN_AMOUNT,
            network_id: 0,
            network_address: vec![],
            user_program: btc_program(),
        })
        .instruction()
        .unwrap();
    ctx.execute_instruction(commit_ix, &[&user])
        .unwrap()
        .assert_success();

    let (proof_cache, _) = ctx
        .svm
        .get_pda_with_bump(&[b"proof", &1u64.to_le_bytes()], &ipow::ID);
    let (height_tracker, _) = ctx
        .svm
        .get_pda_with_bump(&[b"tracker", &0u64.to_le_bytes()], &ipow::ID);

    let submit_ix = ctx
        .program()
        .accounts(ipow::client::accounts::SubmitBitcoinProofCache {
            global_state,
            escrow_vault,
            conversion,
            proof_cache,
            height_tracker,
            header: None,
            operator: stranger.pubkey(), // should be `operator.pubkey()`
            user: user.pubkey(),
            signer: operator.pubkey(),
            system_program: anchor_lang::solana_program::system_program::ID,
        })
        .args(ipow::client::args::SubmitBitcoinProofCache {
            tx_raw: vec![],
            vout_index: 0,
            proof_block_height: 0,
            branch_le: vec![],
            index: 0,
        })
        .instruction()
        .unwrap();

    let result = ctx.execute_instruction(submit_ix, &[&operator]).unwrap();
    result.assert_anchor_error("Unauthorized");
}
