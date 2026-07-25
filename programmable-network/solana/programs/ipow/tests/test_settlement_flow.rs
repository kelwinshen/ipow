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

/// Mirrors `bitcoin::pow::target_from_bits` from the program crate. Duplicated
/// here (not imported) because `declare_program!` only generates IDL-derived
/// client bindings, not access to the program crate's internal Rust
/// functions — mining a header against a made-up transaction still needs the
/// exact same target math the on-chain program uses to validate it.
fn target_from_bits(bits: u32) -> [u8; 32] {
    let mut target = [0u8; 32];
    let exp = (bits >> 24) as usize;
    let mantissa = bits & 0x007FFFFF;

    if exp > 3 {
        let shift = exp - 3;
        if shift < 32 {
            target[32 - shift - 3] = (mantissa >> 16) as u8;
            target[32 - shift - 2] = (mantissa >> 8) as u8;
            target[32 - shift - 1] = mantissa as u8;
        }
    } else {
        let shift = 3 - exp;
        let shifted_mant = mantissa >> (8 * shift);
        target[29] = (shifted_mant >> 16) as u8;
        target[30] = (shifted_mant >> 8) as u8;
        target[31] = shifted_mant as u8;
    }
    target
}

fn hash_meets_target(hash_le: &[u8; 32], target_be: &[u8; 32]) -> bool {
    let mut hash_be = [0u8; 32];
    for i in 0..32 {
        hash_be[i] = hash_le[31 - i];
    }
    hash_be <= *target_be
}

/// Builds an 80-byte header carrying `merkle_root_le`, mining the nonce
/// against an intentionally-easy target (regtest's well-known `0x207fffff`
/// powLimit bits) so this completes in a handful of iterations rather than
/// requiring real proof-of-work. `commit_global_header` doesn't check
/// `prev_hash_le` against anything, so it's left zeroed.
fn mine_header(merkle_root_le: [u8; 32]) -> [u8; 80] {
    const N_BITS: u32 = 0x207fffff;
    let target = target_from_bits(N_BITS);

    let mut header = [0u8; 80];
    header[0..4].copy_from_slice(&1u32.to_le_bytes());
    header[36..68].copy_from_slice(&merkle_root_le);
    header[68..72].copy_from_slice(&1_700_000_000u32.to_le_bytes());
    header[72..76].copy_from_slice(&N_BITS.to_le_bytes());

    for nonce in 0u32..10_000 {
        header[76..80].copy_from_slice(&nonce.to_le_bytes());
        let hash1 = Sha256::digest(header);
        let hash2 = Sha256::digest(hash1);
        let mut hash_le = [0u8; 32];
        hash_le.copy_from_slice(&hash2);
        if hash_meets_target(&hash_le, &target) {
            return header;
        }
    }
    panic!("failed to mine a header satisfying the easy test target");
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

    // Craft a Bitcoin tx paying the approved output script, and mine a header
    // whose merkle root is that transaction's own txid — a single-transaction
    // block's merkle root is simply its one transaction's txid, so the
    // Merkle branch submitted later is empty.
    let tx_raw = tx_paying(&ipow_program, BITCOIN_AMOUNT + 1_000);
    let hash1 = Sha256::digest(&tx_raw);
    let hash2 = Sha256::digest(hash1);
    let mut txid_le = [0u8; 32];
    txid_le.copy_from_slice(&hash2);

    let header_80 = mine_header(txid_le);
    let header_pda = ctx
        .svm
        .get_pda_with_bump(&[b"header", &0u64.to_le_bytes()], &ipow::ID)
        .0;
    let dummy_tracker = Pubkey::new_unique();

    let commit_header_ix = ctx
        .program()
        .accounts(ipow::client::accounts::CommitGlobalHeader {
            global_state,
            header: header_pda,
            prev_height_tracker: dummy_tracker,
            prev_epoch_start_header: None,
            prev_epoch_end_header: None,
            operator: operator.pubkey(),
            system_program: anchor_lang::solana_program::system_program::ID,
        })
        .args(ipow::client::args::CommitGlobalHeader {
            header_80,
            height: 0,
        })
        .instruction()
        .unwrap();
    ctx.execute_instruction(commit_header_ix, &[&operator])
        .unwrap()
        .assert_success();

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
