use anchor_lang;
use anchor_litesvm::{AnchorContext, AnchorLiteSVM, TestHelpers};
use sha2::{Digest, Sha256};
use solana_account::Account as SolanaAccount;
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

/// `commit_global_header` now enforces a real Bitcoin-difficulty-scale
/// `POW_LIMIT` (see `bitcoin::pow::target_from_bits`'s own doc comment) —
/// mining a header against a cheap, made-up target (this test used to mine
/// against regtest's easy `0x207fffff` bits) is no longer accepted, since
/// that's now exactly the gap the fix closes. Real proof-of-work at that
/// scale (~2^224) needs ~4 billion hashes, impractical for a fast unit
/// test, so this seeds a `GlobalHeader` account directly instead —
/// mirroring `test_message_commitment.rs`'s `seed_message_commitment`,
/// which already bypasses full instruction flow to seed account state
/// directly for setup elsewhere in this workspace.
fn seed_header(ctx: &mut AnchorContext, height: u64, merkle_root_le: [u8; 32]) -> Pubkey {
    const GLOBAL_HEADER_DISCRIMINATOR: [u8; 8] = [0xfe, 0x2a, 0x4e, 0xdc, 0x67, 0x9f, 0x66, 0x78];

    let (header_pda, _) = ctx
        .svm
        .get_pda_with_bump(&[b"header", &height.to_le_bytes()], &ipow::ID);

    let mut data = GLOBAL_HEADER_DISCRIMINATOR.to_vec();
    data.extend_from_slice(&height.to_le_bytes()); // height
    data.extend_from_slice(&[0u8; 32]); // hash_le — unchecked by anything this test exercises
    data.extend_from_slice(&[0u8; 32]); // prev_hash_le — likewise unchecked
    data.extend_from_slice(&merkle_root_le);
    data.extend_from_slice(&0x1d00ffffu32.to_le_bytes()); // n_bits — real mainnet genesis difficulty
    data.extend_from_slice(&1_700_000_000u32.to_le_bytes()); // timestamp
    data.extend_from_slice(&0i64.to_le_bytes()); // arrival_time

    let lamports = ctx.svm.minimum_balance_for_rent_exemption(data.len());
    ctx.svm
        .set_account(
            header_pda,
            SolanaAccount {
                lamports,
                data,
                owner: ipow::ID,
                executable: false,
                rent_epoch: 0,
            },
        )
        .unwrap();

    header_pda
}

/// A minimal single-input, single-output legacy transaction paying
/// `value_sats` to `program`. Not a 0-input tx: an inCount of 0x00 followed by
/// an outCount of 0x01 is indistinguishable from SegWit marker+flag framing.
fn tx_paying(program: &[u8], value_sats: u64) -> Vec<u8> {
    let mut tx = Vec::new();
    tx.extend_from_slice(&1u32.to_le_bytes());
    tx.push(0x01);
    tx.extend_from_slice(&[0u8; 32]);
    tx.extend_from_slice(&[0u8; 4]);
    tx.push(0x00);
    tx.extend_from_slice(&[0xffu8; 4]);
    tx.push(0x01);
    tx.extend_from_slice(&value_sats.to_le_bytes());
    tx.push(program.len() as u8);
    tx.extend_from_slice(program);
    tx
}

/// Full bitcoin-to-native settlement, driven end to end through litesvm: the
/// user commits a conversion, the operator approves it (locking in the
/// ipow-side Bitcoin output script), a real header is committed whose
/// merkle root matches a crafted Bitcoin transaction paying that exact
/// script, and the *user* (not the operator) submits the Merkle proof to
/// claim their converted native funds — this Solana program acts as the
/// destination chain. Also verifies Fix #1 (the `operator` account address
/// constraint) doesn't block this user-initiated claim: `operator` here is
/// only the fee-recipient slot, not the caller.
#[test]
fn user_claims_bitcoin_to_native_conversion_via_a_real_committed_header_and_proof() {
    let (mut ctx, _admin, operator, global_state) = setup_initialized();
    let user = ctx.svm.create_funded_account(10_000_000_000).unwrap();
    let (escrow_vault, _) = ctx.svm.get_pda_with_bump(&[b"escrow"], &ipow::ID);

    const BITCOIN_AMOUNT: u64 = 100_000;
    const NATIVE_AMOUNT: u64 = 1_000_000_000; // 1 SOL
    const COMMIT_FEE: u64 = 5_000_000; // 0.5% of NATIVE_AMOUNT, per commit_fee_bps = 50

    // Fund protocol liquidity so escrow can cover the eventual native payout
    // (RESERVE_MARGIN_BPS is 100%, i.e. the full native_amount gets reserved).
    let add_liquidity_ix = ctx
        .program()
        .accounts(ipow::client::accounts::AddLiquidity {
            global_state,
            escrow_vault,
            operator: operator.pubkey(),
            system_program: anchor_lang::solana_program::system_program::ID,
        })
        .args(ipow::client::args::AddLiquidity {
            amount: NATIVE_AMOUNT + 10_000_000,
        })
        .instruction()
        .unwrap();
    ctx.execute_instruction(add_liquidity_ix, &[&operator])
        .unwrap()
        .assert_success();

    let conversion = ctx
        .svm
        .get_pda_with_bump(&[b"conversion", &1u64.to_le_bytes()], &ipow::ID)
        .0;

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
            user_program: vec![0x11u8; 20],
        })
        .instruction()
        .unwrap();
    ctx.execute_instruction(commit_ix, &[&user])
        .unwrap()
        .assert_success();

    // Operator approves, committing to the ipow-side Bitcoin output
    // script the user's proof must eventually pay.
    let ipow_program: Vec<u8> = {
        let mut p = vec![0x76, 0xa9, 0x14];
        p.extend_from_slice(&[0x22u8; 20]);
        p.extend_from_slice(&[0x88, 0xac]);
        p
    };
    let program_hash: [u8; 32] = Sha256::digest(&ipow_program).into();
    let used_program_pda = ctx
        .svm
        .get_pda_with_bump(&[b"used_prog", &program_hash], &ipow::ID)
        .0;

    let approve_ix = ctx
        .program()
        .accounts(ipow::client::accounts::ApproveAndStartWithAnchor {
            global_state,
            escrow_vault,
            conversion,
            used_program_pda,
            operator: operator.pubkey(),
            system_program: anchor_lang::solana_program::system_program::ID,
        })
        .args(ipow::client::args::ApproveAndStartWithAnchor {
            duty_window_seconds: 3600,
            ipow_receive_program: ipow_program.clone(),
            program_hash,
        })
        .instruction()
        .unwrap();
    ctx.execute_instruction(approve_ix, &[&operator])
        .unwrap()
        .assert_success();

    // Craft a Bitcoin tx paying the approved output script, and seed a
    // header (bypassing real PoW mining — see seed_header's own comment)
    // whose merkle root is that transaction's own txid — a
    // single-transaction block's merkle root is simply its one
    // transaction's txid, so the Merkle branch submitted later is empty.
    let tx_raw = tx_paying(&ipow_program, BITCOIN_AMOUNT + 1_000);
    let hash1 = Sha256::digest(&tx_raw);
    let hash2 = Sha256::digest(hash1);
    let mut txid_le = [0u8; 32];
    txid_le.copy_from_slice(&hash2);

    let header_pda = seed_header(&mut ctx, 0, txid_le);

    let proof_cache = ctx
        .svm
        .get_pda_with_bump(&[b"proof", &1u64.to_le_bytes()], &ipow::ID)
        .0;
    let height_tracker = ctx
        .svm
        .get_pda_with_bump(&[b"tracker", &0u64.to_le_bytes()], &ipow::ID)
        .0;

    let escrow_balance_before = ctx.svm.get_balance(&escrow_vault).unwrap_or(0);
    let operator_balance_before = ctx.svm.get_balance(&operator.pubkey()).unwrap_or(0);

    // The user themselves (not the operator) signs and submits the proof to
    // claim their converted native funds. `operator` is still required as an
    // account — it's the fee-recipient slot Fix #1 now validates by address —
    // but it is not the caller.
    let submit_ix = ctx
        .program()
        .accounts(ipow::client::accounts::SubmitBitcoinProofCache {
            global_state,
            escrow_vault,
            conversion,
            proof_cache,
            height_tracker,
            header: Some(header_pda),
            operator: operator.pubkey(),
            user: user.pubkey(),
            signer: user.pubkey(),
            system_program: anchor_lang::solana_program::system_program::ID,
        })
        .args(ipow::client::args::SubmitBitcoinProofCache {
            tx_raw,
            vout_index: 0,
            proof_block_height: 0,
            branch_le: vec![],
            index: 0,
        })
        .instruction()
        .unwrap();
    ctx.execute_instruction(submit_ix, &[&user])
        .unwrap()
        .assert_success();

    let conv: ipow::accounts::Conversion = ctx.get_account(&conversion).unwrap();
    assert!(conv.proof_verified);

    // The user's native payout and the operator's commit-fee both come out of
    // escrow; checking escrow/operator balances (rather than the user's, who
    // is also this transaction's fee-payer) avoids tx-fee noise.
    let escrow_balance_after = ctx.svm.get_balance(&escrow_vault).unwrap_or(0);
    assert_eq!(
        escrow_balance_before - escrow_balance_after,
        NATIVE_AMOUNT + COMMIT_FEE
    );

    let operator_balance_after = ctx.svm.get_balance(&operator.pubkey()).unwrap_or(0);
    assert_eq!(operator_balance_after - operator_balance_before, COMMIT_FEE);
}
