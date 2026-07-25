use anchor_lang;
use anchor_litesvm::{AnchorContext, AnchorLiteSVM, TestHelpers};
use solana_keypair::Keypair;
use solana_program::pubkey::Pubkey;
use solana_signer::Signer;

anchor_lang::declare_program!(ipow);

fn new_ctx() -> AnchorContext {
    AnchorLiteSVM::build_with_program(ipow::ID, include_bytes!("../../../target/deploy/ipow.so"))
}

/// Funds an operator keypair and runs `initialize`, returning the context, the
/// admin (deployer) keypair, the operator keypair, and the global_state PDA.
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

const REMOTE_NETWORK_ID: u64 = 777;

#[test]
fn add_network_registers_a_new_network() {
    let (mut ctx, _admin, operator, global_state) = setup_initialized();

    let (network_config, _) = ctx
        .svm
        .get_pda_with_bump(&[b"network", &REMOTE_NETWORK_ID.to_le_bytes()], &ipow::ID);

    let ix = ctx
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

    ctx.execute_instruction(ix, &[&operator])
        .unwrap()
        .assert_success();

    let cfg: ipow::accounts::SupportedNetwork = ctx.get_account(&network_config).unwrap();
    assert_eq!(cfg.network_id, REMOTE_NETWORK_ID);
    assert_eq!(cfg.min_addr_len, 20);
    assert_eq!(cfg.max_addr_len, 32);
    assert!(cfg.is_active);
}

#[test]
fn add_network_rejects_non_operator() {
    let (mut ctx, _admin, _operator, global_state) = setup_initialized();
    let stranger = ctx.svm.create_funded_account(10_000_000_000).unwrap();

    let (network_config, _) = ctx
        .svm
        .get_pda_with_bump(&[b"network", &REMOTE_NETWORK_ID.to_le_bytes()], &ipow::ID);

    let ix = ctx
        .program()
        .accounts(ipow::client::accounts::AddNetwork {
            global_state,
            network_config,
            admin: stranger.pubkey(),
            system_program: anchor_lang::solana_program::system_program::ID,
        })
        .args(ipow::client::args::AddNetwork {
            network_id: REMOTE_NETWORK_ID,
            min_addr_len: 20,
            max_addr_len: 32,
        })
        .instruction()
        .unwrap();

    let result = ctx.execute_instruction(ix, &[&stranger]).unwrap();
    assert!(!result.is_success());
}

#[test]
fn update_network_changes_active_flag() {
    let (mut ctx, _admin, operator, global_state) = setup_initialized();

    let (network_config, _) = ctx
        .svm
        .get_pda_with_bump(&[b"network", &REMOTE_NETWORK_ID.to_le_bytes()], &ipow::ID);

    let add_ix = ctx
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
    ctx.execute_instruction(add_ix, &[&operator])
        .unwrap()
        .assert_success();

    let update_ix = ctx
        .program()
        .accounts(ipow::client::accounts::UpdateNetwork {
            global_state,
            network_config,
            admin: operator.pubkey(),
        })
        .args(ipow::client::args::UpdateNetwork {
            _network_id: REMOTE_NETWORK_ID,
            min_addr_len: 20,
            max_addr_len: 40,
            is_active: false,
        })
        .instruction()
        .unwrap();
    ctx.execute_instruction(update_ix, &[&operator])
        .unwrap()
        .assert_success();

    let cfg: ipow::accounts::SupportedNetwork = ctx.get_account(&network_config).unwrap();
    assert_eq!(cfg.max_addr_len, 40);
    assert!(!cfg.is_active);
}

#[test]
fn add_liquidity_increases_escrow_balance() {
    let (mut ctx, _admin, operator, global_state) = setup_initialized();
    let (escrow_vault, _) = ctx.svm.get_pda_with_bump(&[b"escrow"], &ipow::ID);

    // The escrow PDA holds no account at all until first funded, so `get_balance`
    // returns `None` rather than `Some(0)`.
    let balance_before = ctx.svm.get_balance(&escrow_vault).unwrap_or(0);

    let ix = ctx
        .program()
        .accounts(ipow::client::accounts::AddLiquidity {
            global_state,
            escrow_vault,
            operator: operator.pubkey(),
            system_program: anchor_lang::solana_program::system_program::ID,
        })
        .args(ipow::client::args::AddLiquidity {
            amount: 1_000_000_000,
        })
        .instruction()
        .unwrap();

    ctx.execute_instruction(ix, &[&operator])
        .unwrap()
        .assert_success();

    let balance_after = ctx.svm.get_balance(&escrow_vault).unwrap();
    assert_eq!(balance_after - balance_before, 1_000_000_000);
}

#[test]
fn remove_liquidity_withdraws_previously_added_funds() {
    let (mut ctx, _admin, operator, global_state) = setup_initialized();
    let (escrow_vault, _) = ctx.svm.get_pda_with_bump(&[b"escrow"], &ipow::ID);

    let add_ix = ctx
        .program()
        .accounts(ipow::client::accounts::AddLiquidity {
            global_state,
            escrow_vault,
            operator: operator.pubkey(),
            system_program: anchor_lang::solana_program::system_program::ID,
        })
        .args(ipow::client::args::AddLiquidity {
            amount: 1_000_000_000,
        })
        .instruction()
        .unwrap();
    ctx.execute_instruction(add_ix, &[&operator])
        .unwrap()
        .assert_success();

    // Check the escrow's balance decrease, not the operator's balance increase:
    // the operator is also this transaction's fee payer, so its net balance
    // change is `amount - tx_fee`, not exactly `amount`.
    let escrow_balance_before = ctx.svm.get_balance(&escrow_vault).unwrap();

    let remove_ix = ctx
        .program()
        .accounts(ipow::client::accounts::RemoveLiquidity {
            global_state,
            escrow_vault,
            operator: operator.pubkey(),
            system_program: anchor_lang::solana_program::system_program::ID,
        })
        .args(ipow::client::args::RemoveLiquidity {
            amount: 400_000_000,
        })
        .instruction()
        .unwrap();
    ctx.execute_instruction(remove_ix, &[&operator])
        .unwrap()
        .assert_success();

    let escrow_balance_after = ctx.svm.get_balance(&escrow_vault).unwrap();
    assert_eq!(escrow_balance_before - escrow_balance_after, 400_000_000);
}

#[test]
fn remove_liquidity_rejects_amount_exceeding_available() {
    let (mut ctx, _admin, operator, global_state) = setup_initialized();
    let (escrow_vault, _) = ctx.svm.get_pda_with_bump(&[b"escrow"], &ipow::ID);

    let add_ix = ctx
        .program()
        .accounts(ipow::client::accounts::AddLiquidity {
            global_state,
            escrow_vault,
            operator: operator.pubkey(),
            system_program: anchor_lang::solana_program::system_program::ID,
        })
        .args(ipow::client::args::AddLiquidity { amount: 1_000_000 })
        .instruction()
        .unwrap();
    ctx.execute_instruction(add_ix, &[&operator])
        .unwrap()
        .assert_success();

    let remove_ix = ctx
        .program()
        .accounts(ipow::client::accounts::RemoveLiquidity {
            global_state,
            escrow_vault,
            operator: operator.pubkey(),
            system_program: anchor_lang::solana_program::system_program::ID,
        })
        .args(ipow::client::args::RemoveLiquidity { amount: 2_000_000 })
        .instruction()
        .unwrap();

    let result = ctx.execute_instruction(remove_ix, &[&operator]).unwrap();
    assert!(!result.is_success());
}
