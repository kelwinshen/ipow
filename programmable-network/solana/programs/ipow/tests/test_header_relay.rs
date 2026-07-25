use anchor_lang;
use anchor_litesvm::{AnchorContext, AnchorLiteSVM, TestHelpers};
use sha2::{Digest, Sha256};
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

/// Real Bitcoin mainnet genesis block header (height 0), byte-identical to the
/// fixture independently verified for the Solidity/Ethereum test suite: decodes
/// to exactly 80 bytes, its double-SHA256 hash satisfies its own encoded PoW
/// target, and matches the well-known genesis hash
/// 000000000019d6689c085ae165831e934ff763ae46a2a6c172b3f1b60a8ce26f. Decoded from
/// hex here (not hand-transcribed as a byte array) to avoid transcription errors.
fn genesis_header_80() -> [u8; 80] {
    let hex_str = "0100000000000000000000000000000000000000000000000000000000000000000000003ba3edfd7a7b12b27ac72c3e67768f617fc81bc3888a51323a9fb8aa4b1e5e4a29ab5f49ffff001d1dac2b7c";
    let bytes = hex::decode(hex_str).unwrap();
    bytes.try_into().unwrap()
}

fn header_pda(ctx: &AnchorContext, height: u64) -> Pubkey {
    ctx.svm
        .get_pda_with_bump(&[b"header", &height.to_le_bytes()], &ipow::ID)
        .0
}

#[test]
fn commit_global_header_rejects_non_operator() {
    let (mut ctx, _admin, _operator, global_state) = setup_initialized();
    let stranger = ctx.svm.create_funded_account(10_000_000_000).unwrap();
    let header = header_pda(&ctx, 0);
    let dummy_tracker = Pubkey::new_unique();

    let ix = ctx
        .program()
        .accounts(ipow::client::accounts::CommitGlobalHeader {
            global_state,
            header,
            prev_height_tracker: dummy_tracker,
            prev_epoch_start_header: None,
            prev_epoch_end_header: None,
            operator: stranger.pubkey(),
            system_program: anchor_lang::solana_program::system_program::ID,
        })
        .args(ipow::client::args::CommitGlobalHeader {
            header_80: genesis_header_80(),
            height: 0,
        })
        .instruction()
        .unwrap();

    let result = ctx.execute_instruction(ix, &[&stranger]).unwrap();
    assert!(!result.is_success());
}

#[test]
fn commit_global_header_rejects_low_work() {
    let (mut ctx, _admin, operator, global_state) = setup_initialized();
    let header = header_pda(&ctx, 0);
    let dummy_tracker = Pubkey::new_unique();

    // Flip the last nonce byte of the real genesis header — breaks the PoW
    // solution while keeping the header well-formed (still 80 bytes, same
    // declared difficulty bits), so LowWork is the only possible revert reason.
    let mut tampered = genesis_header_80();
    tampered[79] ^= 0xff;

    let ix = ctx
        .program()
        .accounts(ipow::client::accounts::CommitGlobalHeader {
            global_state,
            header,
            prev_height_tracker: dummy_tracker,
            prev_epoch_start_header: None,
            prev_epoch_end_header: None,
            operator: operator.pubkey(),
            system_program: anchor_lang::solana_program::system_program::ID,
        })
        .args(ipow::client::args::CommitGlobalHeader {
            header_80: tampered,
            height: 0,
        })
        .instruction()
        .unwrap();

    let result = ctx.execute_instruction(ix, &[&operator]).unwrap();
    assert!(!result.is_success());
}

#[test]
fn commit_global_header_accepts_real_genesis_header() {
    let (mut ctx, _admin, operator, global_state) = setup_initialized();
    let header = header_pda(&ctx, 0);
    let dummy_tracker = Pubkey::new_unique();

    let ix = ctx
        .program()
        .accounts(ipow::client::accounts::CommitGlobalHeader {
            global_state,
            header,
            prev_height_tracker: dummy_tracker,
            prev_epoch_start_header: None,
            prev_epoch_end_header: None,
            operator: operator.pubkey(),
            system_program: anchor_lang::solana_program::system_program::ID,
        })
        .args(ipow::client::args::CommitGlobalHeader {
            header_80: genesis_header_80(),
            height: 0,
        })
        .instruction()
        .unwrap();

    ctx.execute_instruction(ix, &[&operator])
        .unwrap()
        .assert_success();

    let stored: ipow::accounts::GlobalHeader = ctx.get_account(&header).unwrap();
    assert_eq!(stored.height, 0);
    assert_eq!(stored.n_bits, 0x1d00ffff);
    assert_eq!(stored.timestamp, 1231006505);

    let state: ipow::accounts::GlobalState = ctx.get_account(&global_state).unwrap();
    assert_eq!(state.global_tip_height, 0);
}

#[test]
fn operator_open_tunnel_opens_a_conversion_anchored_to_a_real_header() {
    let (mut ctx, _admin, operator, global_state) = setup_initialized();

    // A real header must exist at the tip before a window can be anchored.
    let header = header_pda(&ctx, 0);
    let dummy_tracker = Pubkey::new_unique();
    let commit_header_ix = ctx
        .program()
        .accounts(ipow::client::accounts::CommitGlobalHeader {
            global_state,
            header,
            prev_height_tracker: dummy_tracker,
            prev_epoch_start_header: None,
            prev_epoch_end_header: None,
            operator: operator.pubkey(),
            system_program: anchor_lang::solana_program::system_program::ID,
        })
        .args(ipow::client::args::CommitGlobalHeader {
            header_80: genesis_header_80(),
            height: 0,
        })
        .instruction()
        .unwrap();
    ctx.execute_instruction(commit_header_ix, &[&operator])
        .unwrap()
        .assert_success();

    // Fund the protocol so it can reserve the full native payout (100%
    // RESERVE_MARGIN_BPS). Liquidity availability is also net of a rent-exempt
    // minimum buffer on the escrow account, so fund a bit more than the exact
    // reserve amount, not exactly `native_amount`.
    let (escrow_vault, _) = ctx.svm.get_pda_with_bump(&[b"escrow"], &ipow::ID);
    let native_amount = 1_000_000_000u64;
    let add_liquidity_ix = ctx
        .program()
        .accounts(ipow::client::accounts::AddLiquidity {
            global_state,
            escrow_vault,
            operator: operator.pubkey(),
            system_program: anchor_lang::solana_program::system_program::ID,
        })
        .args(ipow::client::args::AddLiquidity {
            amount: native_amount + 10_000_000,
        })
        .instruction()
        .unwrap();
    ctx.execute_instruction(add_liquidity_ix, &[&operator])
        .unwrap()
        .assert_success();

    // Register the destination network.
    const REMOTE_NETWORK_ID: u64 = 777;
    let (network_config, _) = ctx
        .svm
        .get_pda_with_bump(&[b"network", &REMOTE_NETWORK_ID.to_le_bytes()], &ipow::ID);
    let add_network_ix = ctx
        .program()
        .accounts(ipow::client::accounts::AddNetwork {
            global_state,
            network_config,
            admin: operator.pubkey(),
            system_program: anchor_lang::solana_program::system_program::ID,
        })
        .args(ipow::client::args::AddNetwork {
            network_id: REMOTE_NETWORK_ID,
            min_addr_len: 20,
            max_addr_len: 32,
        })
        .instruction()
        .unwrap();
    ctx.execute_instruction(add_network_ix, &[&operator])
        .unwrap()
        .assert_success();

    let ipow_program: Vec<u8> = {
        let mut p = vec![0x76, 0xa9, 0x14];
        p.extend_from_slice(&[0x22u8; 20]);
        p.extend_from_slice(&[0x88, 0xac]);
        p
    };
    let program_hash: [u8; 32] = Sha256::digest(&ipow_program).into();

    let next_tx_id: u64 = {
        let state: ipow::accounts::GlobalState = ctx.get_account(&global_state).unwrap();
        state.next_tx_id
    };
    let conversion = ctx
        .svm
        .get_pda_with_bump(&[b"conversion", &next_tx_id.to_le_bytes()], &ipow::ID)
        .0;
    let used_program_pda = ctx
        .svm
        .get_pda_with_bump(&[b"used_prog", &program_hash], &ipow::ID)
        .0;

    let dest_address = Pubkey::new_unique();
    let bitcoin_amount = 100_000u64;

    let ix = ctx
        .program()
        .accounts(ipow::client::accounts::OperatorOpenTunnel {
            global_state,
            escrow_vault,
            conversion,
            used_program_pda,
            network_config,
            operator: operator.pubkey(),
            system_program: anchor_lang::solana_program::system_program::ID,
        })
        .args(ipow::client::args::OperatorOpenTunnel {
            bitcoin_amount,
            native_amount,
            network_id: REMOTE_NETWORK_ID,
            dest_address,
            network_address: vec![0x11; 20],
            duty_window_seconds: 3600,
            ipow_receive_program: ipow_program,
            locked_anchor_height: 0,
            program_hash,
        })
        .instruction()
        .unwrap();

    ctx.execute_instruction(ix, &[&operator])
        .unwrap()
        .assert_success();

    let conv: ipow::accounts::Conversion = ctx.get_account(&conversion).unwrap();
    assert_eq!(conv.user, dest_address);
    assert!(!conv.is_native_to_bitcoin);
    assert_eq!(conv.native_amount, native_amount);
    assert_eq!(conv.reserved_native, native_amount); // 100% RESERVE_MARGIN_BPS
    assert!(conv.window_started);
    assert_eq!(conv.window_start_height, 0);

    let state: ipow::accounts::GlobalState = ctx.get_account(&global_state).unwrap();
    assert_eq!(state.total_reserved_native, native_amount);
    assert_eq!(state.active_open_conversions, 1);
}
