use anchor_lang;
use anchor_litesvm::{
    AnchorContext, AnchorLiteSVM, Program as LiteSvmProgram, ProgramTestExt, TestHelpers,
};
use sha2::{Digest, Sha256};
use solana_account::Account as SolanaAccount;
use solana_clock::Clock;
use solana_keypair::Keypair;
use solana_program::pubkey::Pubkey;
use solana_signer::Signer;

anchor_lang::declare_program!(ipow);
anchor_lang::declare_program!(ipow_conversion);

/// Deploys both `ipow` (header relay, unchanged) and `ipow_conversion`
/// (this feature) into the same litesvm context — matching the real
/// two-program deployment shape.
fn new_ctx() -> AnchorContext {
    let mut ctx = AnchorLiteSVM::build_with_program(
        ipow_conversion::ID,
        include_bytes!("../../../target/deploy/ipow_conversion.so"),
    );
    ctx.deploy_program(ipow::ID, include_bytes!("../../../target/deploy/ipow.so"));
    ctx
}

/// `commit_global_header` now enforces a real Bitcoin-difficulty-scale
/// `POW_LIMIT` (see `ipow::bitcoin::pow::target_from_bits`'s own doc
/// comment — this was a real, confirmed protocol gap this session found
/// and fixed: an unclamped target let an operator forge a fake header
/// chain for near-zero real work). Mining against it for real needs ~4
/// billion hashes, impractical for a fast unit test, so this seeds a
/// `GlobalHeader` account directly instead — mirroring `ipow-message-
/// relay`'s `test_message_commitment.rs::seed_message_commitment`, which
/// already bypasses full instruction flow to seed account state directly
/// for setup.
fn seed_header(ctx: &mut AnchorContext, height: u64, merkle_root_le: [u8; 32]) -> Pubkey {
    const GLOBAL_HEADER_DISCRIMINATOR: [u8; 8] = [0xfe, 0x2a, 0x4e, 0xdc, 0x67, 0x9f, 0x66, 0x78];

    let (header_pda, _) = ctx.svm.get_pda_with_bump(&[b"header", &height.to_le_bytes()], &ipow::ID);

    let mut data = GLOBAL_HEADER_DISCRIMINATOR.to_vec();
    data.extend_from_slice(&height.to_le_bytes());
    data.extend_from_slice(&[0u8; 32]); // hash_le — unchecked by anything exercised here
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
/// `value_sats` to `program`.
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

/// `declare_program!`'s generated client bindings don't expose a usable
/// path for a composite/nested `Accounts` field (`SplAccounts`, embedded in
/// `AddLiquidity`/`DepositConversion`/`SubmitProofCache`) — the IDL flattens
/// it into individual entries with no reusable named type, so there's no
/// `ipow_conversion::client::accounts::AddLiquiditySpl`-style path to reach
/// for. Building the account list by hand for those three instructions
/// (order/writable/signer flags read directly off the generated IDL) sidesteps
/// the problem entirely rather than guessing at generated names.
struct RawAccounts(Vec<solana_program::instruction::AccountMeta>);
impl anchor_lang::ToAccountMetas for RawAccounts {
    fn to_account_metas(&self, _is_signer: Option<bool>) -> Vec<solana_program::instruction::AccountMeta> {
        self.0.clone()
    }
}
fn meta(pubkey: Pubkey, writable: bool, signer: bool) -> solana_program::instruction::AccountMeta {
    if writable {
        solana_program::instruction::AccountMeta::new(pubkey, signer)
    } else {
        solana_program::instruction::AccountMeta::new_readonly(pubkey, signer)
    }
}

#[allow(clippy::too_many_arguments)]
fn submit_proof_cache_metas(
    conversion: Pubkey,
    pool: Pubkey,
    escrow_vault: Pubkey,
    stake_escrow: Pubkey,
    header_pda: Pubkey,
    operator: Pubkey,
    user: Pubkey,
    signer: Pubkey,
) -> Vec<solana_program::instruction::AccountMeta> {
    let mut metas = vec![
        meta(conversion, true, false),
        meta(pool, true, false),
        meta(escrow_vault, true, false),
        meta(Pubkey::new_unique(), true, false), // escrow_ata
        meta(pool, true, false),         // fee_pool (same PDA as pool for native)
        meta(escrow_vault, true, false), // fee_escrow (same PDA as escrow_vault for native)
        meta(stake_escrow, true, false),
        meta(header_pda, false, false),
        meta(operator, true, false),
        meta(Pubkey::new_unique(), true, false), // operator_token_account
        meta(user, true, false),
        meta(Pubkey::new_unique(), true, false), // user_token_account
    ];
    metas.extend(dummy_spl_metas());
    metas.push(meta(signer, true, true));
    metas.push(meta(anchor_lang::solana_program::system_program::ID, false, false));
    metas
}

/// Placeholder SPL account metas for the native-only tests in this file —
/// never touched by the program (native conversions skip the SPL branch
/// entirely), so any unique pubkeys work.
fn dummy_spl_metas() -> Vec<solana_program::instruction::AccountMeta> {
    vec![
        meta(Pubkey::new_unique(), false, false),
        meta(Pubkey::new_unique(), false, false),
        meta(Pubkey::new_unique(), false, false),
    ]
}

/// Like `dummy_spl_metas()`, but the `associated_token_program` slot is the
/// real program — a native-primary bundle's extra-token loop (`deposit_
/// conversion`/`submit_proof_cache`/`refund_no_proof_native_to_bitcoin`)
/// reuses the *primary* `spl.associated_token_program` typed field for its
/// own SPL ATA creation, so it can't be a placeholder once a real extra
/// token is involved, even though the primary slot itself stays native.
fn dummy_spl_metas_real_ata_program() -> Vec<solana_program::instruction::AccountMeta> {
    vec![
        meta(Pubkey::new_unique(), false, false),
        meta(Pubkey::new_unique(), false, false),
        meta(spl_associated_token_account_interface::program::ID, false, false),
    ]
}

/// The real `SplAccounts` triple (`mint`, `token_program`, `associated_
/// token_program`) — used wherever a claimant's bitcoin->native self-escrow
/// actually needs to move a real SPL/Token-2022 mint.
fn real_spl_metas(mint: Pubkey, token_program: Pubkey) -> Vec<solana_program::instruction::AccountMeta> {
    vec![
        meta(mint, false, false),
        meta(token_program, false, false),
        meta(spl_associated_token_account_interface::program::ID, false, false),
    ]
}

/// `propose_claim_conversion` now also carries the bitcoin->native claimant
/// self-escrow accounts (no more separate `add_liquidity`-funded pool) — see
/// its handler doc comment. `spl_metas` is `dummy_spl_metas()` for
/// native-mint/native->bitcoin claims, `real_spl_metas(...)` when a claimant
/// is actually self-escrowing a real SPL/Token-2022 mint.
#[allow(clippy::too_many_arguments)]
fn propose_claim_metas(
    stake_escrow: Pubkey,
    conversion: Pubkey,
    previous_claimant: Pubkey,
    previous_claimant_token_account: Pubkey,
    used_program_pda: Pubkey,
    pool: Pubkey,
    escrow_vault: Pubkey,
    escrow_ata: Pubkey,
    claimant_token_account: Pubkey,
    spl_metas: Vec<solana_program::instruction::AccountMeta>,
    claimant: Pubkey,
) -> Vec<solana_program::instruction::AccountMeta> {
    let mut metas = vec![
        meta(stake_escrow, true, false),
        meta(conversion, true, false),
        meta(previous_claimant, true, false),
        meta(previous_claimant_token_account, true, false),
        meta(used_program_pda, true, false),
        meta(pool, true, false),
        meta(escrow_vault, true, false),
        meta(escrow_ata, true, false),
        meta(claimant_token_account, true, false),
    ];
    metas.extend(spl_metas);
    metas.push(meta(claimant, true, true));
    metas.push(meta(anchor_lang::solana_program::system_program::ID, false, false));
    metas
}

fn advance_clock(ctx: &mut AnchorContext, seconds: i64) {
    let mut clock: Clock = ctx.svm.get_sysvar();
    clock.unix_timestamp += seconds;
    ctx.svm.set_sysvar(&clock);
}

/// Patches `ipow::state::GlobalState.global_tip_height` directly, bypassing
/// real header relay (mirrors `seed_header`'s own rationale) — needed for
/// the bundle refund test to push past `PROOF_BLOCKS_WINDOW` without
/// mining/relaying real headers. Field offset: 8 (discriminator) + 32
/// (operator) + 2 (commit_fee_bps) + 8 (next_tx_id) = 50.
fn bump_global_tip_height(ctx: &mut AnchorContext, ipow_global_state: Pubkey, height: u64) {
    let mut account = ctx.svm.get_account(&ipow_global_state).unwrap();
    account.data[50..58].copy_from_slice(&height.to_le_bytes());
    ctx.svm.set_account(ipow_global_state, account).unwrap();
}

/// `open_bundle_tunnel`'s account list, in the exact order its `Accounts`
/// struct declares — built by hand (like every instruction here that
/// embeds `SplAccounts`) since `declare_program!`'s generated client
/// bindings can't reach that composite field. `network_config: None` is
/// represented, per Anchor's own `Option<Account<'info, T>>` handling
/// (`anchor-lang`'s `accounts/option.rs`), as this program's own ID in
/// that slot — the sentinel Anchor checks for, not simple omission.
#[allow(clippy::too_many_arguments)]
fn open_bundle_tunnel_metas(
    conversion_global_state: Pubkey,
    conversion: Pubkey,
    network_config: Option<Pubkey>,
    used_program_pda: Pubkey,
    pool: Pubkey,
    escrow_vault: Pubkey,
    escrow_ata: Pubkey,
    opener_token_account: Pubkey,
    spl_metas: Vec<solana_program::instruction::AccountMeta>,
    ipow_global_state: Pubkey,
    opener: Pubkey,
) -> Vec<solana_program::instruction::AccountMeta> {
    let mut metas = vec![
        meta(conversion_global_state, true, false),
        meta(conversion, true, false),
        match network_config {
            Some(nc) => meta(nc, false, false),
            None => meta(ipow_conversion::ID, false, false),
        },
        meta(used_program_pda, true, false),
        meta(pool, true, false),
        meta(escrow_vault, true, false),
        meta(escrow_ata, true, false),
        meta(opener_token_account, true, false),
    ];
    metas.extend(spl_metas);
    metas.push(meta(ipow_global_state, false, false));
    metas.push(meta(opener, true, true));
    metas.push(meta(anchor_lang::solana_program::system_program::ID, false, false));
    metas
}

fn add_network(ctx: &mut AnchorContext, conversion_global_state: Pubkey, admin: &Keypair, network_id: u64) -> Pubkey {
    let (network_config, _) = ctx
        .svm
        .get_pda_with_bump(&[b"network", &network_id.to_le_bytes()], &ipow_conversion::ID);
    let ix = LiteSvmProgram::new(ipow_conversion::ID)
        .accounts(ipow_conversion::client::accounts::AddNetwork {
            conversion_global_state,
            network_config,
            admin: admin.pubkey(),
            system_program: anchor_lang::solana_program::system_program::ID,
        })
        .args(ipow_conversion::client::args::AddNetwork {
            network_id,
            min_addr_len: 1,
            max_addr_len: 32,
        })
        .instruction()
        .unwrap();
    ctx.execute_instruction(ix, &[admin]).unwrap().assert_success();
    network_config
}

fn setup_initialized() -> (AnchorContext, Keypair, Pubkey, Pubkey) {
    let mut ctx = new_ctx();
    let admin = ctx.svm.create_funded_account(10_000_000_000).unwrap();

    // ipow's own header-relay GlobalState — unrelated to this program's
    // conversion state, read cross-program by ipow-conversion.
    let (ipow_global_state, _) = ctx.svm.get_pda_with_bump(&[b"global_state"], &ipow::ID);
    let (ipow_escrow_vault, _) = ctx.svm.get_pda_with_bump(&[b"escrow"], &ipow::ID);
    let ix = LiteSvmProgram::new(ipow::ID)
        .accounts(ipow::client::accounts::Initialize {
            global_state: ipow_global_state,
            escrow_vault: ipow_escrow_vault,
            admin: admin.pubkey(),
            system_program: anchor_lang::solana_program::system_program::ID,
        })
        .args(ipow::client::args::Initialize {
            operator: admin.pubkey(),
            commit_fee_bps: 50,
        })
        .instruction()
        .unwrap();
    ctx.execute_instruction(ix, &[&admin]).unwrap().assert_success();

    // ipow-conversion's own global state.
    let (conversion_global_state, _) = ctx
        .svm
        .get_pda_with_bump(&[b"conversion_global_state"], &ipow_conversion::ID);
    let ix = LiteSvmProgram::new(ipow_conversion::ID)
        .accounts(ipow_conversion::client::accounts::Initialize {
            conversion_global_state,
            admin: admin.pubkey(),
            system_program: anchor_lang::solana_program::system_program::ID,
        })
        .args(ipow_conversion::client::args::Initialize {
            governance: admin.pubkey(),
            commit_fee_bps: 50,
        })
        .instruction()
        .unwrap();
    ctx.execute_instruction(ix, &[&admin]).unwrap().assert_success();

    (ctx, admin, ipow_global_state, conversion_global_state)
}

/// Full bitcoin->native conversion via the new permissionless auction: a
/// claimant wins by staking real SOL (no fixed operator gate at all), the
/// claim locks in after the (shortened, 1-minute) quiet period, and the
/// claimant proves a real Bitcoin payment via a mined header + Merkle proof
/// to release the user's payout from real, pre-funded pool liquidity.
#[test]
fn claimant_wins_auction_and_completes_bitcoin_to_native_conversion() {
    let (mut ctx, _admin, ipow_global_state, conversion_global_state) = setup_initialized();
    let user = ctx.svm.create_funded_account(10_000_000_000).unwrap();
    let claimant = ctx.svm.create_funded_account(10_000_000_000).unwrap();

    let (pool, _) = ctx
        .svm
        .get_pda_with_bump(&[b"pool", Pubkey::default().as_ref()], &ipow_conversion::ID);
    let (escrow_vault, _) = ctx
        .svm
        .get_pda_with_bump(&[b"escrow", Pubkey::default().as_ref()], &ipow_conversion::ID);
    let (stake_escrow, _) = ctx
        .svm
        .get_pda_with_bump(&[b"stake_escrow"], &ipow_conversion::ID);

    const BITCOIN_AMOUNT: u64 = 100_000;
    const NATIVE_AMOUNT: u64 = 1_000_000_000; // 1 SOL
    const REQUIRED_BOND: u64 = 500_000_000; // 0.5 SOL
    const COMMIT_FEE: u64 = 5_000_000; // 0.5% of NATIVE_AMOUNT, per commit_fee_bps = 50

    let (conversion, _) = ctx
        .svm
        .get_pda_with_bump(&[b"conversion", &1u64.to_le_bytes()], &ipow_conversion::ID);

    let commit_ix = LiteSvmProgram::new(ipow_conversion::ID)
        .accounts(ipow_conversion::client::accounts::CommitBitcoinToToken {
            conversion_global_state,
            fee_pool: pool,
            fee_escrow: escrow_vault,
            conversion,
            user: user.pubkey(),
            system_program: anchor_lang::solana_program::system_program::ID,
        })
        .args(ipow_conversion::client::args::CommitBitcoinToToken {
            bitcoin_amount: BITCOIN_AMOUNT,
            native_amount: NATIVE_AMOUNT,
            network_id: 0,
            network_address: vec![],
            user_program: vec![0x11u8; 20],
            token_mint: Pubkey::default(),
            extra_tokens: vec![],
        })
        .instruction()
        .unwrap();
    ctx.execute_instruction(commit_ix, &[&user]).unwrap().assert_success();

    // Claimant wins the auction — permissionless, no fixed operator gate.
    let ipow_program: Vec<u8> = {
        let mut p = vec![0x76, 0xa9, 0x14];
        p.extend_from_slice(&[0x22u8; 20]);
        p.extend_from_slice(&[0x88, 0xac]);
        p
    };
    let program_hash: [u8; 32] = Sha256::digest(&ipow_program).into();
    let (used_program_pda, _) = ctx
        .svm
        .get_pda_with_bump(&[b"used_prog", &program_hash], &ipow_conversion::ID);

    // Claimant self-escrows the full NATIVE_AMOUNT payout right here — no
    // separate governance-funded pool anymore.
    let propose_ix = LiteSvmProgram::new(ipow_conversion::ID)
        .accounts(RawAccounts(propose_claim_metas(
            stake_escrow,
            conversion,
            claimant.pubkey(),
            Pubkey::new_unique(), // previous_claimant_token_account, unused (first claim)
            used_program_pda,
            pool,
            escrow_vault,
            Pubkey::new_unique(), // escrow_ata, unused (native mint)
            Pubkey::new_unique(), // claimant_token_account, unused (native mint)
            dummy_spl_metas(),
            claimant.pubkey(),
        )))
        .args(ipow_conversion::client::args::ProposeClaimConversion {
            stake_amount: REQUIRED_BOND,
            proposed_rate_amount: NATIVE_AMOUNT,
            duty_window_seconds: 3600,
            ipow_receive_program: ipow_program.clone(),
            program_hash,
        })
        .instruction()
        .unwrap();
    ctx.execute_instruction(propose_ix, &[&claimant])
        .unwrap()
        .assert_success();

    // Quiet period (1 minute) must pass before the claim locks in.
    advance_clock(&mut ctx, 61);

    let finalize_ix = LiteSvmProgram::new(ipow_conversion::ID)
        .accounts(ipow_conversion::client::accounts::FinalizeClaimConversion {
            conversion,
            ipow_global_state,
        })
        .args(ipow_conversion::client::args::FinalizeClaimConversion {})
        .instruction()
        .unwrap();
    ctx.execute_instruction(finalize_ix, &[&claimant])
        .unwrap()
        .assert_success();

    let conv: ipow_conversion::accounts::Conversion = ctx.get_account(&conversion).unwrap();
    assert_eq!(conv.responsible_operator, claimant.pubkey());
    assert_eq!(conv.reserved_native, NATIVE_AMOUNT);

    // Craft a Bitcoin tx paying the claimed output script, and mine a
    // header whose merkle root is that transaction's own txid.
    let tx_raw = tx_paying(&ipow_program, BITCOIN_AMOUNT + 1_000);
    let hash1 = Sha256::digest(&tx_raw);
    let hash2 = Sha256::digest(hash1);
    let mut txid_le = [0u8; 32];
    txid_le.copy_from_slice(&hash2);

    let header_pda = seed_header(&mut ctx, 0, txid_le);

    let escrow_balance_before = ctx.svm.get_balance(&escrow_vault).unwrap_or(0);
    let claimant_balance_before = ctx.svm.get_balance(&claimant.pubkey()).unwrap_or(0);

    // The user themselves proves the real Bitcoin payment, releasing the
    // operator's already-reserved payout to the user.
    let submit_ix = LiteSvmProgram::new(ipow_conversion::ID)
        .accounts(RawAccounts(submit_proof_cache_metas(
            conversion,
            pool,
            escrow_vault,
            stake_escrow,
            header_pda,
            claimant.pubkey(),
            user.pubkey(),
            user.pubkey(),
        )))
        .args(ipow_conversion::client::args::SubmitProofCache {
            tx_raw,
            vout_index: 0,
            proof_block_height: 0,
            branch_le: vec![],
            index: 0,
        })
        .instruction()
        .unwrap();
    ctx.execute_instruction(submit_ix, &[&user]).unwrap().assert_success();

    let conv: ipow_conversion::accounts::Conversion = ctx.get_account(&conversion).unwrap();
    assert!(conv.proof_verified);

    // Check the escrow's own balance drop, not the user's — the user is
    // also this transaction's fee-payer, which would otherwise pollute the
    // assertion by the tx fee (same reasoning the original reference test
    // for this flow used). The escrow pays out both the user's native
    // payout and the operator's commit fee here.
    let escrow_balance_after = ctx.svm.get_balance(&escrow_vault).unwrap_or(0);
    assert_eq!(
        escrow_balance_before - escrow_balance_after,
        NATIVE_AMOUNT + COMMIT_FEE
    );

    // Claimant gets the commit fee for actually completing the duty — no
    // stake to return here, since `bitcoin->token` never posts one (the
    // self-escrowed collateral already carries the real exposure).
    let claimant_balance_after = ctx.svm.get_balance(&claimant.pubkey()).unwrap_or(0);
    assert!(claimant_balance_after > claimant_balance_before);
}

/// Full native->bitcoin conversion: the user deposits real native value
/// *before* the auction winner can be paid anything, and the winner only
/// gets paid by proving a real Bitcoin payment. No liquidity pool needed —
/// this direction releases the user's own deposit, not pooled capital.
#[test]
fn claimant_wins_auction_and_completes_native_to_bitcoin_conversion() {
    let (mut ctx, _admin, ipow_global_state, conversion_global_state) = setup_initialized();
    let user = ctx.svm.create_funded_account(10_000_000_000).unwrap();
    let claimant = ctx.svm.create_funded_account(10_000_000_000).unwrap();

    let (pool, _) = ctx
        .svm
        .get_pda_with_bump(&[b"pool", Pubkey::default().as_ref()], &ipow_conversion::ID);
    let (escrow_vault, _) = ctx
        .svm
        .get_pda_with_bump(&[b"escrow", Pubkey::default().as_ref()], &ipow_conversion::ID);
    let (stake_escrow, _) = ctx
        .svm
        .get_pda_with_bump(&[b"stake_escrow"], &ipow_conversion::ID);

    const BITCOIN_AMOUNT: u64 = 100_000;
    const NATIVE_AMOUNT: u64 = 1_000_000_000; // 1 SOL
    const REQUIRED_BOND: u64 = 500_000_000; // 0.5 SOL

    let (conversion, _) = ctx
        .svm
        .get_pda_with_bump(&[b"conversion", &1u64.to_le_bytes()], &ipow_conversion::ID);

    // User's own Bitcoin destination script, fixed at commit time (network_id
    // 0, direct Bitcoin) — this is what the auction winner must pay.
    let user_bitcoin_program: Vec<u8> = {
        let mut p = vec![0x76, 0xa9, 0x14];
        p.extend_from_slice(&[0x33u8; 20]);
        p.extend_from_slice(&[0x88, 0xac]);
        p
    };

    let commit_ix = LiteSvmProgram::new(ipow_conversion::ID)
        .accounts(ipow_conversion::client::accounts::CommitTokenToBitcoin {
            conversion_global_state,
            fee_pool: pool,
            fee_escrow: escrow_vault,
            conversion,
            network_config: None,
            user: user.pubkey(),
            system_program: anchor_lang::solana_program::system_program::ID,
        })
        .args(ipow_conversion::client::args::CommitTokenToBitcoin {
            native_amount: NATIVE_AMOUNT,
            bitcoin_amount: BITCOIN_AMOUNT,
            network_id: 0,
            network_address: vec![],
            user_program: user_bitcoin_program.clone(),
            required_bond: REQUIRED_BOND,
            token_mint: Pubkey::default(),
            extra_tokens: vec![],
        })
        .instruction()
        .unwrap();
    ctx.execute_instruction(commit_ix, &[&user]).unwrap().assert_success();

    // Claimant wins the auction — no receive-program of their own needed
    // here (the user already fixed the destination script at commit time).
    let (used_program_pda, _) = ctx
        .svm
        .get_pda_with_bump(&[b"used_prog", &[0u8; 32]], &ipow_conversion::ID);
    // native->bitcoin never self-escrows anything here (only bitcoin->native
    // does) — these accounts are present but unused.
    let propose_ix = LiteSvmProgram::new(ipow_conversion::ID)
        .accounts(RawAccounts(propose_claim_metas(
            stake_escrow,
            conversion,
            claimant.pubkey(),
            Pubkey::new_unique(),
            used_program_pda,
            pool,
            escrow_vault,
            Pubkey::new_unique(),
            Pubkey::new_unique(),
            dummy_spl_metas(),
            claimant.pubkey(),
        )))
        .args(ipow_conversion::client::args::ProposeClaimConversion {
            stake_amount: REQUIRED_BOND,
            proposed_rate_amount: BITCOIN_AMOUNT,
            duty_window_seconds: 3600,
            ipow_receive_program: vec![],
            program_hash: [0u8; 32],
        })
        .instruction()
        .unwrap();
    ctx.execute_instruction(propose_ix, &[&claimant])
        .unwrap()
        .assert_success();

    advance_clock(&mut ctx, 61);

    let finalize_ix = LiteSvmProgram::new(ipow_conversion::ID)
        .accounts(ipow_conversion::client::accounts::FinalizeClaimConversion {
            conversion,
            ipow_global_state,
        })
        .args(ipow_conversion::client::args::FinalizeClaimConversion {})
        .instruction()
        .unwrap();
    ctx.execute_instruction(finalize_ix, &[&claimant])
        .unwrap()
        .assert_success();

    // User deposits real value only now that a real claimant is locked in.
    let deposit_ix = {
            let mut deposit_metas = vec![
                meta(ipow_global_state, false, false),
                meta(conversion, true, false),
                meta(pool, true, false),
                meta(escrow_vault, true, false),
                meta(Pubkey::new_unique(), true, false), // escrow_ata
                meta(Pubkey::new_unique(), true, false), // user_token_account
            ];
            deposit_metas.extend(dummy_spl_metas());
            deposit_metas.push(meta(user.pubkey(), true, true));
            deposit_metas.push(meta(anchor_lang::solana_program::system_program::ID, false, false));
            LiteSvmProgram::new(ipow_conversion::ID).accounts(RawAccounts(deposit_metas))
        }
        .args(ipow_conversion::client::args::DepositConversion {})
        .instruction()
        .unwrap();
    ctx.execute_instruction(deposit_ix, &[&user]).unwrap().assert_success();

    let tx_raw = tx_paying(&user_bitcoin_program, BITCOIN_AMOUNT + 1_000);
    let hash1 = Sha256::digest(&tx_raw);
    let hash2 = Sha256::digest(hash1);
    let mut txid_le = [0u8; 32];
    txid_le.copy_from_slice(&hash2);

    let header_pda = seed_header(&mut ctx, 0, txid_le);

    let claimant_balance_before = ctx.svm.get_balance(&claimant.pubkey()).unwrap_or(0);
    let escrow_balance_before = ctx.svm.get_balance(&escrow_vault).unwrap_or(0);

    // The claimant themselves proves they paid the user's Bitcoin address,
    // releasing the user's own deposit to themselves.
    let submit_ix = LiteSvmProgram::new(ipow_conversion::ID)
        .accounts(RawAccounts(submit_proof_cache_metas(
            conversion,
            pool,
            escrow_vault,
            stake_escrow,
            header_pda,
            claimant.pubkey(),
            user.pubkey(),
            claimant.pubkey(),
        )))
        .args(ipow_conversion::client::args::SubmitProofCache {
            tx_raw,
            vout_index: 0,
            proof_block_height: 0,
            branch_le: vec![],
            index: 0,
        })
        .instruction()
        .unwrap();
    ctx.execute_instruction(submit_ix, &[&claimant])
        .unwrap()
        .assert_success();

    let conv: ipow_conversion::accounts::Conversion = ctx.get_account(&conversion).unwrap();
    assert!(conv.proof_verified);

    // Escrow releases the user's deposit + fee to the claimant, and
    // separately refunds the claimant's own stake — checked via the
    // claimant's own balance (not a fee-payer in this tx, so uncontaminated).
    let escrow_balance_after = ctx.svm.get_balance(&escrow_vault).unwrap_or(0);
    assert_eq!(
        escrow_balance_before - escrow_balance_after,
        NATIVE_AMOUNT + 5_000_000
    );
    let claimant_balance_after = ctx.svm.get_balance(&claimant.pubkey()).unwrap_or(0);
    assert!(claimant_balance_after > claimant_balance_before + NATIVE_AMOUNT);
}

/// The core new mechanic this redesign adds: a claimant who wins the
/// auction and then goes dark. Verifies both reclaim outcomes on the
/// bitcoin->native path (real reservation already made, so it's always the
/// "confirmed" branch — the forfeited stake pays straight to the user,
/// `required_bond` escalates) and that a *second* claimant can then pick
/// up the exact same reservation and complete it (earning only their own
/// stake back — no bounty component, since the forfeit already went to
/// the user directly at reclaim time).
#[test]
fn reclaim_forfeits_stake_to_user_and_lets_a_new_claimant_finish() {
    // `token->bitcoin` specifically — this is now the *only* direction
    // where a claimant posts a stake at all (`bitcoin->token` no longer
    // does, since its self-escrowed collateral already carries the same
    // exposure — see `propose_claim_conversion`'s own doc comment), so
    // it's the only direction where "stake forfeits into bounty" is a
    // real scenario to exercise.
    let (mut ctx, _admin, ipow_global_state, conversion_global_state) = setup_initialized();
    let user = ctx.svm.create_funded_account(10_000_000_000).unwrap();
    let claimant_a = ctx.svm.create_funded_account(10_000_000_000).unwrap();
    let claimant_b = ctx.svm.create_funded_account(10_000_000_000).unwrap();

    let (pool, _) = ctx
        .svm
        .get_pda_with_bump(&[b"pool", Pubkey::default().as_ref()], &ipow_conversion::ID);
    let (escrow_vault, _) = ctx
        .svm
        .get_pda_with_bump(&[b"escrow", Pubkey::default().as_ref()], &ipow_conversion::ID);
    let (stake_escrow, _) = ctx
        .svm
        .get_pda_with_bump(&[b"stake_escrow"], &ipow_conversion::ID);

    const BITCOIN_AMOUNT: u64 = 100_000;
    const NATIVE_AMOUNT: u64 = 1_000_000_000;
    const REQUIRED_BOND: u64 = 500_000_000;

    let (conversion, _) = ctx
        .svm
        .get_pda_with_bump(&[b"conversion", &1u64.to_le_bytes()], &ipow_conversion::ID);

    let user_bitcoin_program: Vec<u8> = {
        let mut p = vec![0x76, 0xa9, 0x14];
        p.extend_from_slice(&[0x44u8; 20]);
        p.extend_from_slice(&[0x88, 0xac]);
        p
    };

    let commit_ix = LiteSvmProgram::new(ipow_conversion::ID)
        .accounts(ipow_conversion::client::accounts::CommitTokenToBitcoin {
            conversion_global_state,
            fee_pool: pool,
            fee_escrow: escrow_vault,
            conversion,
            network_config: None,
            user: user.pubkey(),
            system_program: anchor_lang::solana_program::system_program::ID,
        })
        .args(ipow_conversion::client::args::CommitTokenToBitcoin {
            native_amount: NATIVE_AMOUNT,
            bitcoin_amount: BITCOIN_AMOUNT,
            network_id: 0,
            network_address: vec![],
            user_program: user_bitcoin_program.clone(),
            required_bond: REQUIRED_BOND,
            token_mint: Pubkey::default(),
            extra_tokens: vec![],
        })
        .instruction()
        .unwrap();
    ctx.execute_instruction(commit_ix, &[&user]).unwrap().assert_success();

    let (used_program_pda, _) = ctx
        .svm
        .get_pda_with_bump(&[b"used_prog", &[0u8; 32]], &ipow_conversion::ID);

    // claimant_a wins — native->bitcoin never self-escrows anything at
    // propose time (only `bitcoin->token` does), so only the stake moves.
    let propose_a_ix = LiteSvmProgram::new(ipow_conversion::ID)
        .accounts(RawAccounts(propose_claim_metas(
            stake_escrow,
            conversion,
            claimant_a.pubkey(),
            Pubkey::new_unique(),
            used_program_pda,
            pool,
            escrow_vault,
            Pubkey::new_unique(),
            Pubkey::new_unique(),
            dummy_spl_metas(),
            claimant_a.pubkey(),
        )))
        .args(ipow_conversion::client::args::ProposeClaimConversion {
            stake_amount: REQUIRED_BOND,
            proposed_rate_amount: BITCOIN_AMOUNT,
            duty_window_seconds: 3600,
            ipow_receive_program: vec![],
            program_hash: [0u8; 32],
        })
        .instruction()
        .unwrap();
    ctx.execute_instruction(propose_a_ix, &[&claimant_a])
        .unwrap()
        .assert_success();

    advance_clock(&mut ctx, 61);

    let finalize_a_ix = LiteSvmProgram::new(ipow_conversion::ID)
        .accounts(ipow_conversion::client::accounts::FinalizeClaimConversion {
            conversion,
            ipow_global_state,
        })
        .args(ipow_conversion::client::args::FinalizeClaimConversion {})
        .instruction()
        .unwrap();
    ctx.execute_instruction(finalize_a_ix, &[&claimant_a])
        .unwrap()
        .assert_success();

    // User deposits real value — required for `reclaim_expired_conversion`
    // to *forfeit* (rather than merely refund) claimant_a's stake if they
    // default: for `token->bitcoin`, forfeiture only triggers once a real
    // commitment (the deposit) exists.
    let deposit_ix = {
        let mut deposit_metas = vec![
            meta(ipow_global_state, false, false),
            meta(conversion, true, false),
            meta(pool, true, false),
            meta(escrow_vault, true, false),
            meta(Pubkey::new_unique(), true, false),
            meta(Pubkey::new_unique(), true, false),
        ];
        deposit_metas.extend(dummy_spl_metas());
        deposit_metas.push(meta(user.pubkey(), true, true));
        deposit_metas.push(meta(anchor_lang::solana_program::system_program::ID, false, false));
        LiteSvmProgram::new(ipow_conversion::ID).accounts(RawAccounts(deposit_metas))
    }
    .args(ipow_conversion::client::args::DepositConversion {})
    .instruction()
    .unwrap();
    ctx.execute_instruction(deposit_ix, &[&user]).unwrap().assert_success();

    // claimant_a goes dark — duty window (3600s) plus a margin passes with
    // no proof ever submitted.
    advance_clock(&mut ctx, 3601);

    let stake_escrow_before = ctx.svm.get_balance(&stake_escrow).unwrap_or(0);
    let user_balance_before_reclaim = ctx.svm.get_balance(&user.pubkey()).unwrap_or(0);

    let reclaim_ix = LiteSvmProgram::new(ipow_conversion::ID)
        .accounts(ipow_conversion::client::accounts::ReclaimExpiredConversion {
            stake_escrow,
            conversion,
            previous_claimant: claimant_a.pubkey(),
            user: user.pubkey(),
            system_program: anchor_lang::solana_program::system_program::ID,
        })
        .args(ipow_conversion::client::args::ReclaimExpiredConversion {})
        .instruction()
        .unwrap();
    // Permissionless — the user themselves reclaims, not claimant_a. Note
    // `user` is also this tx's own fee-payer here, so their balance change
    // is checked with a margin below, not exact equality.
    ctx.execute_instruction(reclaim_ix, &[&user]).unwrap().assert_success();

    let conv: ipow_conversion::accounts::Conversion = ctx.get_account(&conversion).unwrap();
    // Forfeited (not refunded to claimant_a): stake escrow pays out the
    // forfeited amount immediately, straight to the user — not held for
    // whoever finishes later.
    assert_eq!(
        stake_escrow_before - ctx.svm.get_balance(&stake_escrow).unwrap_or(0),
        REQUIRED_BOND
    );
    let user_balance_after_reclaim = ctx.svm.get_balance(&user.pubkey()).unwrap_or(0);
    assert!(user_balance_after_reclaim > user_balance_before_reclaim + REQUIRED_BOND - 10_000);
    assert_eq!(conv.required_bond, REQUIRED_BOND);
    assert_eq!(conv.responsible_operator, Pubkey::default());
    assert_eq!(conv.operator_duty_expires_at, 0);

    // claimant_b picks up the same already-deposited conversion and
    // finishes it, earning only their own stake back — no bounty
    // component, since claimant_a's forfeit already paid the user
    // directly at reclaim time above.
    let propose_b_ix = LiteSvmProgram::new(ipow_conversion::ID)
        .accounts(RawAccounts(propose_claim_metas(
            stake_escrow,
            conversion,
            claimant_b.pubkey(),
            Pubkey::new_unique(),
            used_program_pda,
            pool,
            escrow_vault,
            Pubkey::new_unique(),
            Pubkey::new_unique(),
            dummy_spl_metas(),
            claimant_b.pubkey(),
        )))
        .args(ipow_conversion::client::args::ProposeClaimConversion {
            // Must meet the escalated required_bond, matching the
            // forfeited stake.
            stake_amount: REQUIRED_BOND,
            proposed_rate_amount: BITCOIN_AMOUNT,
            duty_window_seconds: 3600,
            ipow_receive_program: vec![],
            program_hash: [0u8; 32],
        })
        .instruction()
        .unwrap();
    ctx.execute_instruction(propose_b_ix, &[&claimant_b])
        .unwrap()
        .assert_success();

    advance_clock(&mut ctx, 61);

    let finalize_b_ix = LiteSvmProgram::new(ipow_conversion::ID)
        .accounts(ipow_conversion::client::accounts::FinalizeClaimConversion {
            conversion,
            ipow_global_state,
        })
        .args(ipow_conversion::client::args::FinalizeClaimConversion {})
        .instruction()
        .unwrap();
    ctx.execute_instruction(finalize_b_ix, &[&claimant_b])
        .unwrap()
        .assert_success();

    let tx_raw = tx_paying(&user_bitcoin_program, BITCOIN_AMOUNT + 1_000);
    let hash1 = Sha256::digest(&tx_raw);
    let hash2 = Sha256::digest(hash1);
    let mut txid_le = [0u8; 32];
    txid_le.copy_from_slice(&hash2);

    let header_pda = seed_header(&mut ctx, 0, txid_le);

    let claimant_b_balance_before = ctx.svm.get_balance(&claimant_b.pubkey()).unwrap_or(0);

    // claimant_b themselves proves they paid the user's Bitcoin address,
    // releasing the user's own deposit to themselves.
    let submit_ix = LiteSvmProgram::new(ipow_conversion::ID)
        .accounts(RawAccounts(submit_proof_cache_metas(
            conversion,
            pool,
            escrow_vault,
            stake_escrow,
            header_pda,
            claimant_b.pubkey(),
            user.pubkey(),
            claimant_b.pubkey(),
        )))
        .args(ipow_conversion::client::args::SubmitProofCache {
            tx_raw,
            vout_index: 0,
            proof_block_height: 0,
            branch_le: vec![],
            index: 0,
        })
        .instruction()
        .unwrap();
    ctx.execute_instruction(submit_ix, &[&claimant_b])
        .unwrap()
        .assert_success();

    let conv: ipow_conversion::accounts::Conversion = ctx.get_account(&conversion).unwrap();
    assert!(conv.proof_verified);

    // claimant_b earns their own stake back plus the commit fee — nothing
    // extra from claimant_a's forfeit, which already went to the user
    // directly at reclaim time (not a fee-payer in this tx, so
    // uncontaminated).
    let claimant_b_balance_after = ctx.svm.get_balance(&claimant_b.pubkey()).unwrap_or(0);
    assert!(claimant_b_balance_after > claimant_b_balance_before + REQUIRED_BOND);
}

/// The actual point of this pass: a bitcoin->native conversion moving a
/// real SPL token, not native SOL. Same auction cycle as the native test
/// above, but `native_amount` pays out in the SPL mint while the commit
/// fee and auction stake stay native SOL throughout — exactly the split
/// `docs/DESIGN_V2.md` describes.
#[test]
fn spl_token_bitcoin_to_native_conversion_pays_out_the_real_mint() {
    let (mut ctx, admin, ipow_global_state, conversion_global_state) = setup_initialized();
    let user = ctx.svm.create_funded_account(10_000_000_000).unwrap();
    let claimant = ctx.svm.create_funded_account(10_000_000_000).unwrap();

    let mint = litesvm_token::CreateMint::new(&mut ctx.svm, &admin)
        .authority(&admin.pubkey())
        .decimals(6)
        .send()
        .unwrap();

    let claimant_token_account = litesvm_token::CreateAssociatedTokenAccount::new(&mut ctx.svm, &claimant, &mint)
        .send()
        .unwrap();

    const BITCOIN_AMOUNT: u64 = 100_000;
    const SPL_AMOUNT: u64 = 5_000_000; // 5 tokens at 6 decimals
    const REQUIRED_BOND: u64 = 500_000_000; // 0.5 SOL — stake is always native

    // The claimant is minted exactly what they'll self-escrow at propose
    // time — no separate governance-funded pool anymore.
    litesvm_token::MintTo::new(&mut ctx.svm, &admin, &mint, &claimant_token_account, SPL_AMOUNT)
        .send()
        .unwrap();

    let (pool, _) = ctx.svm.get_pda_with_bump(&[b"pool", mint.as_ref()], &ipow_conversion::ID);
    let (escrow_vault, _) = ctx.svm.get_pda_with_bump(&[b"escrow", mint.as_ref()], &ipow_conversion::ID);
    let escrow_ata = spl_associated_token_account_interface::address::get_associated_token_address_with_program_id(
        &escrow_vault,
        &mint,
        &litesvm_token::spl_token::ID,
    );

    let (conversion, _) = ctx
        .svm
        .get_pda_with_bump(&[b"conversion", &1u64.to_le_bytes()], &ipow_conversion::ID);

    let (native_pool, _) = ctx
        .svm
        .get_pda_with_bump(&[b"pool", Pubkey::default().as_ref()], &ipow_conversion::ID);
    let (native_escrow, _) = ctx
        .svm
        .get_pda_with_bump(&[b"escrow", Pubkey::default().as_ref()], &ipow_conversion::ID);

    let commit_ix = LiteSvmProgram::new(ipow_conversion::ID)
        .accounts(ipow_conversion::client::accounts::CommitBitcoinToToken {
            conversion_global_state,
            fee_pool: native_pool,
            fee_escrow: native_escrow,
            conversion,
            user: user.pubkey(),
            system_program: anchor_lang::solana_program::system_program::ID,
        })
        .args(ipow_conversion::client::args::CommitBitcoinToToken {
            bitcoin_amount: BITCOIN_AMOUNT,
            native_amount: SPL_AMOUNT,
            network_id: 0,
            network_address: vec![],
            user_program: vec![0x11u8; 20],
            token_mint: mint,
            extra_tokens: vec![],
        })
        .instruction()
        .unwrap();
    ctx.execute_instruction(commit_ix, &[&user]).unwrap().assert_success();

    let ipow_program: Vec<u8> = {
        let mut p = vec![0x76, 0xa9, 0x14];
        p.extend_from_slice(&[0x22u8; 20]);
        p.extend_from_slice(&[0x88, 0xac]);
        p
    };
    let program_hash: [u8; 32] = Sha256::digest(&ipow_program).into();
    let (used_program_pda, _) = ctx
        .svm
        .get_pda_with_bump(&[b"used_prog", &program_hash], &ipow_conversion::ID);

    // Claimant self-escrows the real SPL_AMOUNT of the mint right here.
    let propose_ix = LiteSvmProgram::new(ipow_conversion::ID)
        .accounts(RawAccounts(propose_claim_metas(
            ctx.svm.get_pda_with_bump(&[b"stake_escrow"], &ipow_conversion::ID).0,
            conversion,
            claimant.pubkey(),
            Pubkey::new_unique(), // previous_claimant_token_account, unused (first claim)
            used_program_pda,
            pool,
            escrow_vault,
            escrow_ata,
            claimant_token_account,
            real_spl_metas(mint, litesvm_token::spl_token::ID),
            claimant.pubkey(),
        )))
        .args(ipow_conversion::client::args::ProposeClaimConversion {
            stake_amount: REQUIRED_BOND,
            proposed_rate_amount: SPL_AMOUNT,
            duty_window_seconds: 3600,
            ipow_receive_program: ipow_program.clone(),
            program_hash,
        })
        .instruction()
        .unwrap();
    ctx.execute_instruction(propose_ix, &[&claimant]).unwrap().assert_success();

    advance_clock(&mut ctx, 61);

    let finalize_ix = LiteSvmProgram::new(ipow_conversion::ID)
        .accounts(ipow_conversion::client::accounts::FinalizeClaimConversion {
            conversion,
            ipow_global_state,
        })
        .args(ipow_conversion::client::args::FinalizeClaimConversion {})
        .instruction()
        .unwrap();
    ctx.execute_instruction(finalize_ix, &[&claimant]).unwrap().assert_success();

    let conv: ipow_conversion::accounts::Conversion = ctx.get_account(&conversion).unwrap();
    assert_eq!(conv.reserved_native, SPL_AMOUNT);

    let tx_raw = tx_paying(&ipow_program, BITCOIN_AMOUNT + 1_000);
    let txid_le = {
        let hash1 = Sha256::digest(&tx_raw);
        let hash2 = Sha256::digest(hash1);
        let mut out = [0u8; 32];
        out.copy_from_slice(&hash2);
        out
    };
    let header_pda = seed_header(&mut ctx, 0, txid_le);

    // The user's own token account for this mint doesn't exist yet — the
    // program creates it idempotently as part of the payout.
    let user_token_account = spl_associated_token_account_interface::address::get_associated_token_address_with_program_id(
        &user.pubkey(),
        &mint,
        &litesvm_token::spl_token::ID,
    );

    let submit_metas = vec![
        meta(conversion, true, false),
        meta(pool, true, false),
        meta(escrow_vault, true, false),
        meta(escrow_ata, true, false),
        meta(native_pool, true, false),
        meta(native_escrow, true, false),
        meta(ctx.svm.get_pda_with_bump(&[b"stake_escrow"], &ipow_conversion::ID).0, true, false),
        meta(header_pda, false, false),
        meta(claimant.pubkey(), true, false),
        meta(Pubkey::new_unique(), true, false), // operator_token_account, unused on this path
        meta(user.pubkey(), true, false),
        meta(user_token_account, true, false),
        meta(mint, false, false),
        meta(litesvm_token::spl_token::ID, false, false),
        meta(spl_associated_token_account_interface::program::ID, false, false),
        meta(user.pubkey(), true, true),
        meta(anchor_lang::solana_program::system_program::ID, false, false),
    ];
    let submit_ix = LiteSvmProgram::new(ipow_conversion::ID)
        .accounts(RawAccounts(submit_metas))
        .args(ipow_conversion::client::args::SubmitProofCache {
            tx_raw,
            vout_index: 0,
            proof_block_height: 0,
            branch_le: vec![],
            index: 0,
        })
        .instruction()
        .unwrap();
    ctx.execute_instruction(submit_ix, &[&user]).unwrap().assert_success();

    let conv: ipow_conversion::accounts::Conversion = ctx.get_account(&conversion).unwrap();
    assert!(conv.proof_verified);

    let user_balance: litesvm_token::spl_token::state::Account =
        litesvm_token::get_spl_account(&ctx.svm, &user_token_account).unwrap();
    assert_eq!(user_balance.amount, SPL_AMOUNT);

    // Escrow held exactly what the claimant self-escrowed — fully paid out.
    let escrow_balance: litesvm_token::spl_token::state::Account =
        litesvm_token::get_spl_account(&ctx.svm, &escrow_ata).unwrap();
    assert_eq!(escrow_balance.amount, 0);
}

/// Same flow again, but the mint is a plain (no-extension) Token-2022 mint
/// rather than classic SPL Token — validates `require_spl_programs`/
/// `transfer_value_in`/`transfer_value_out`'s Token-2022 branch actually
/// works end to end, not just that it compiles.
#[test]
fn token2022_bitcoin_to_native_conversion_pays_out_the_real_mint() {
    let (mut ctx, admin, ipow_global_state, conversion_global_state) = setup_initialized();
    let user = ctx.svm.create_funded_account(10_000_000_000).unwrap();
    let claimant = ctx.svm.create_funded_account(10_000_000_000).unwrap();

    let token_2022_id = spl_token_2022_interface::ID;

    let mint = litesvm_token::CreateMint::new(&mut ctx.svm, &admin)
        .authority(&admin.pubkey())
        .decimals(6)
        .token_program_id(&token_2022_id)
        .send()
        .unwrap();

    let claimant_token_account =
        litesvm_token::CreateAssociatedTokenAccount::new(&mut ctx.svm, &claimant, &mint)
            .token_program_id(&token_2022_id)
            .send()
            .unwrap();

    const BITCOIN_AMOUNT: u64 = 100_000;
    const SPL_AMOUNT: u64 = 5_000_000;
    const REQUIRED_BOND: u64 = 500_000_000;

    // The claimant is minted exactly what they'll self-escrow at propose
    // time — no separate governance-funded pool anymore.
    litesvm_token::MintTo::new(&mut ctx.svm, &admin, &mint, &claimant_token_account, SPL_AMOUNT)
        .token_program_id(&token_2022_id)
        .send()
        .unwrap();

    let (pool, _) = ctx.svm.get_pda_with_bump(&[b"pool", mint.as_ref()], &ipow_conversion::ID);
    let (escrow_vault, _) = ctx.svm.get_pda_with_bump(&[b"escrow", mint.as_ref()], &ipow_conversion::ID);
    let escrow_ata = spl_associated_token_account_interface::address::get_associated_token_address_with_program_id(
        &escrow_vault,
        &mint,
        &token_2022_id,
    );

    let (conversion, _) = ctx
        .svm
        .get_pda_with_bump(&[b"conversion", &1u64.to_le_bytes()], &ipow_conversion::ID);

    let (native_pool, _) = ctx
        .svm
        .get_pda_with_bump(&[b"pool", Pubkey::default().as_ref()], &ipow_conversion::ID);
    let (native_escrow, _) = ctx
        .svm
        .get_pda_with_bump(&[b"escrow", Pubkey::default().as_ref()], &ipow_conversion::ID);

    let commit_ix = LiteSvmProgram::new(ipow_conversion::ID)
        .accounts(ipow_conversion::client::accounts::CommitBitcoinToToken {
            conversion_global_state,
            fee_pool: native_pool,
            fee_escrow: native_escrow,
            conversion,
            user: user.pubkey(),
            system_program: anchor_lang::solana_program::system_program::ID,
        })
        .args(ipow_conversion::client::args::CommitBitcoinToToken {
            bitcoin_amount: BITCOIN_AMOUNT,
            native_amount: SPL_AMOUNT,
            network_id: 0,
            network_address: vec![],
            user_program: vec![0x11u8; 20],
            token_mint: mint,
            extra_tokens: vec![],
        })
        .instruction()
        .unwrap();
    ctx.execute_instruction(commit_ix, &[&user]).unwrap().assert_success();

    let ipow_program: Vec<u8> = {
        let mut p = vec![0x76, 0xa9, 0x14];
        p.extend_from_slice(&[0x33u8; 20]);
        p.extend_from_slice(&[0x88, 0xac]);
        p
    };
    let program_hash: [u8; 32] = Sha256::digest(&ipow_program).into();
    let (used_program_pda, _) = ctx
        .svm
        .get_pda_with_bump(&[b"used_prog", &program_hash], &ipow_conversion::ID);

    // Claimant self-escrows the real SPL_AMOUNT of the Token-2022 mint
    // right here.
    let propose_ix = LiteSvmProgram::new(ipow_conversion::ID)
        .accounts(RawAccounts(propose_claim_metas(
            ctx.svm.get_pda_with_bump(&[b"stake_escrow"], &ipow_conversion::ID).0,
            conversion,
            claimant.pubkey(),
            Pubkey::new_unique(),
            used_program_pda,
            pool,
            escrow_vault,
            escrow_ata,
            claimant_token_account,
            real_spl_metas(mint, token_2022_id),
            claimant.pubkey(),
        )))
        .args(ipow_conversion::client::args::ProposeClaimConversion {
            stake_amount: REQUIRED_BOND,
            proposed_rate_amount: SPL_AMOUNT,
            duty_window_seconds: 3600,
            ipow_receive_program: ipow_program.clone(),
            program_hash,
        })
        .instruction()
        .unwrap();
    ctx.execute_instruction(propose_ix, &[&claimant]).unwrap().assert_success();

    advance_clock(&mut ctx, 61);

    let finalize_ix = LiteSvmProgram::new(ipow_conversion::ID)
        .accounts(ipow_conversion::client::accounts::FinalizeClaimConversion {
            conversion,
            ipow_global_state,
        })
        .args(ipow_conversion::client::args::FinalizeClaimConversion {})
        .instruction()
        .unwrap();
    ctx.execute_instruction(finalize_ix, &[&claimant]).unwrap().assert_success();

    let tx_raw = tx_paying(&ipow_program, BITCOIN_AMOUNT + 1_000);
    let txid_le = {
        let hash1 = Sha256::digest(&tx_raw);
        let hash2 = Sha256::digest(hash1);
        let mut out = [0u8; 32];
        out.copy_from_slice(&hash2);
        out
    };
    let header_pda = seed_header(&mut ctx, 0, txid_le);

    let user_token_account = spl_associated_token_account_interface::address::get_associated_token_address_with_program_id(
        &user.pubkey(),
        &mint,
        &token_2022_id,
    );

    let submit_metas = vec![
        meta(conversion, true, false),
        meta(pool, true, false),
        meta(escrow_vault, true, false),
        meta(escrow_ata, true, false),
        meta(native_pool, true, false),
        meta(native_escrow, true, false),
        meta(ctx.svm.get_pda_with_bump(&[b"stake_escrow"], &ipow_conversion::ID).0, true, false),
        meta(header_pda, false, false),
        meta(claimant.pubkey(), true, false),
        meta(Pubkey::new_unique(), true, false),
        meta(user.pubkey(), true, false),
        meta(user_token_account, true, false),
        meta(mint, false, false),
        meta(token_2022_id, false, false),
        meta(spl_associated_token_account_interface::program::ID, false, false),
        meta(user.pubkey(), true, true),
        meta(anchor_lang::solana_program::system_program::ID, false, false),
    ];
    let submit_ix = LiteSvmProgram::new(ipow_conversion::ID)
        .accounts(RawAccounts(submit_metas))
        .args(ipow_conversion::client::args::SubmitProofCache {
            tx_raw,
            vout_index: 0,
            proof_block_height: 0,
            branch_le: vec![],
            index: 0,
        })
        .instruction()
        .unwrap();
    ctx.execute_instruction(submit_ix, &[&user]).unwrap().assert_success();

    let conv: ipow_conversion::accounts::Conversion = ctx.get_account(&conversion).unwrap();
    assert!(conv.proof_verified);

    let user_balance: spl_token_2022_interface::state::Account =
        litesvm_token::get_spl_account(&ctx.svm, &user_token_account).unwrap();
    assert_eq!(user_balance.amount, SPL_AMOUNT);
}

/// The new bundle capability's happy path: a single `token->bitcoin`
/// conversion locks a native-SOL primary slot *and* one extra real SPL
/// mint, both settled by the same single Bitcoin payment proof. Exercises
/// `deposit_conversion`/`submit_proof_cache`'s new `remaining_accounts`
/// loop for real — the highest-risk new code from this pass.
#[test]
fn bundle_native_to_bitcoin_conversion_pays_out_primary_and_extra_token() {
    let (mut ctx, admin, ipow_global_state, conversion_global_state) = setup_initialized();
    let user = ctx.svm.create_funded_account(10_000_000_000).unwrap();
    let claimant = ctx.svm.create_funded_account(10_000_000_000).unwrap();

    let extra_mint = litesvm_token::CreateMint::new(&mut ctx.svm, &admin)
        .authority(&admin.pubkey())
        .decimals(6)
        .send()
        .unwrap();
    let user_extra_token_account =
        litesvm_token::CreateAssociatedTokenAccount::new(&mut ctx.svm, &user, &extra_mint)
            .send()
            .unwrap();
    const EXTRA_AMOUNT: u64 = 2_000_000;
    litesvm_token::MintTo::new(&mut ctx.svm, &admin, &extra_mint, &user_extra_token_account, EXTRA_AMOUNT)
        .send()
        .unwrap();

    let (pool, _) = ctx
        .svm
        .get_pda_with_bump(&[b"pool", Pubkey::default().as_ref()], &ipow_conversion::ID);
    let (escrow_vault, _) = ctx
        .svm
        .get_pda_with_bump(&[b"escrow", Pubkey::default().as_ref()], &ipow_conversion::ID);
    let (stake_escrow, _) = ctx
        .svm
        .get_pda_with_bump(&[b"stake_escrow"], &ipow_conversion::ID);

    let (extra_escrow_vault, _) = ctx
        .svm
        .get_pda_with_bump(&[b"escrow", extra_mint.as_ref()], &ipow_conversion::ID);
    let extra_escrow_ata = spl_associated_token_account_interface::address::get_associated_token_address_with_program_id(
        &extra_escrow_vault,
        &extra_mint,
        &litesvm_token::spl_token::ID,
    );

    const BITCOIN_AMOUNT: u64 = 100_000;
    const NATIVE_AMOUNT: u64 = 1_000_000_000; // 1 SOL
    const REQUIRED_BOND: u64 = 500_000_000; // 0.5 SOL

    let (conversion, _) = ctx
        .svm
        .get_pda_with_bump(&[b"conversion", &1u64.to_le_bytes()], &ipow_conversion::ID);

    let user_bitcoin_program: Vec<u8> = {
        let mut p = vec![0x76, 0xa9, 0x14];
        p.extend_from_slice(&[0x44u8; 20]);
        p.extend_from_slice(&[0x88, 0xac]);
        p
    };

    let commit_ix = LiteSvmProgram::new(ipow_conversion::ID)
        .accounts(ipow_conversion::client::accounts::CommitTokenToBitcoin {
            conversion_global_state,
            fee_pool: pool,
            fee_escrow: escrow_vault,
            conversion,
            network_config: None,
            user: user.pubkey(),
            system_program: anchor_lang::solana_program::system_program::ID,
        })
        .args(ipow_conversion::client::args::CommitTokenToBitcoin {
            native_amount: NATIVE_AMOUNT,
            bitcoin_amount: BITCOIN_AMOUNT,
            network_id: 0,
            network_address: vec![],
            user_program: user_bitcoin_program.clone(),
            required_bond: REQUIRED_BOND,
            token_mint: Pubkey::default(),
            extra_tokens: vec![ipow_conversion::types::TokenAmount {
                mint: extra_mint,
                amount: EXTRA_AMOUNT,
            }],
        })
        .instruction()
        .unwrap();
    ctx.execute_instruction(commit_ix, &[&user]).unwrap().assert_success();

    let (used_program_pda, _) = ctx
        .svm
        .get_pda_with_bump(&[b"used_prog", &[0u8; 32]], &ipow_conversion::ID);
    let propose_ix = LiteSvmProgram::new(ipow_conversion::ID)
        .accounts(RawAccounts(propose_claim_metas(
            stake_escrow,
            conversion,
            claimant.pubkey(),
            Pubkey::new_unique(),
            used_program_pda,
            pool,
            escrow_vault,
            Pubkey::new_unique(),
            Pubkey::new_unique(),
            dummy_spl_metas(),
            claimant.pubkey(),
        )))
        .args(ipow_conversion::client::args::ProposeClaimConversion {
            stake_amount: REQUIRED_BOND,
            proposed_rate_amount: BITCOIN_AMOUNT,
            duty_window_seconds: 3600,
            ipow_receive_program: vec![],
            program_hash: [0u8; 32],
        })
        .instruction()
        .unwrap();
    ctx.execute_instruction(propose_ix, &[&claimant])
        .unwrap()
        .assert_success();

    advance_clock(&mut ctx, 61);

    let finalize_ix = LiteSvmProgram::new(ipow_conversion::ID)
        .accounts(ipow_conversion::client::accounts::FinalizeClaimConversion {
            conversion,
            ipow_global_state,
        })
        .args(ipow_conversion::client::args::FinalizeClaimConversion {})
        .instruction()
        .unwrap();
    ctx.execute_instruction(finalize_ix, &[&claimant])
        .unwrap()
        .assert_success();

    // Deposit: primary (native, via typed accounts) + the one extra SPL
    // token, via `remaining_accounts` — [mint, token_program, escrow_vault,
    // escrow_ata, user_token_account], same order the handler expects.
    let deposit_ix = {
        let mut deposit_metas = vec![
            meta(ipow_global_state, false, false),
            meta(conversion, true, false),
            meta(pool, true, false),
            meta(escrow_vault, true, false),
            meta(Pubkey::new_unique(), true, false), // escrow_ata (native, unused)
            meta(Pubkey::new_unique(), true, false), // user_token_account (native, unused)
        ];
        deposit_metas.extend(dummy_spl_metas_real_ata_program());
        deposit_metas.push(meta(user.pubkey(), true, true));
        deposit_metas.push(meta(anchor_lang::solana_program::system_program::ID, false, false));
        deposit_metas.push(meta(extra_mint, false, false));
        deposit_metas.push(meta(litesvm_token::spl_token::ID, false, false));
        deposit_metas.push(meta(extra_escrow_vault, true, false));
        deposit_metas.push(meta(extra_escrow_ata, true, false));
        deposit_metas.push(meta(user_extra_token_account, true, false));
        LiteSvmProgram::new(ipow_conversion::ID).accounts(RawAccounts(deposit_metas))
    }
    .args(ipow_conversion::client::args::DepositConversion {})
    .instruction()
    .unwrap();
    ctx.execute_instruction(deposit_ix, &[&user]).unwrap().assert_success();

    let extra_escrow_balance_after_deposit: litesvm_token::spl_token::state::Account =
        litesvm_token::get_spl_account(&ctx.svm, &extra_escrow_ata).unwrap();
    assert_eq!(extra_escrow_balance_after_deposit.amount, EXTRA_AMOUNT);

    let tx_raw = tx_paying(&user_bitcoin_program, BITCOIN_AMOUNT + 1_000);
    let hash1 = Sha256::digest(&tx_raw);
    let hash2 = Sha256::digest(hash1);
    let mut txid_le = [0u8; 32];
    txid_le.copy_from_slice(&hash2);
    let header_pda = seed_header(&mut ctx, 0, txid_le);

    let claimant_extra_token_account =
        spl_associated_token_account_interface::address::get_associated_token_address_with_program_id(
            &claimant.pubkey(),
            &extra_mint,
            &litesvm_token::spl_token::ID,
        );

    let claimant_balance_before = ctx.svm.get_balance(&claimant.pubkey()).unwrap_or(0);

    let mut submit_metas = submit_proof_cache_metas(
        conversion,
        pool,
        escrow_vault,
        stake_escrow,
        header_pda,
        claimant.pubkey(),
        user.pubkey(),
        claimant.pubkey(),
    );
    // The extra-token loop reuses the primary `spl.associated_token_program`
    // typed field (index 14: ..., spl.mint, spl.token_program,
    // spl.associated_token_program, signer, system_program) — patch it to
    // the real program since the primary slot here is native/dummy.
    submit_metas[14] = meta(spl_associated_token_account_interface::program::ID, false, false);
    submit_metas.push(meta(extra_mint, false, false));
    submit_metas.push(meta(litesvm_token::spl_token::ID, false, false));
    submit_metas.push(meta(extra_escrow_vault, true, false));
    submit_metas.push(meta(extra_escrow_ata, true, false));
    submit_metas.push(meta(claimant_extra_token_account, true, false));

    let submit_ix = LiteSvmProgram::new(ipow_conversion::ID)
        .accounts(RawAccounts(submit_metas))
        .args(ipow_conversion::client::args::SubmitProofCache {
            tx_raw,
            vout_index: 0,
            proof_block_height: 0,
            branch_le: vec![],
            index: 0,
        })
        .instruction()
        .unwrap();
    ctx.execute_instruction(submit_ix, &[&claimant]).unwrap().assert_success();

    let conv: ipow_conversion::accounts::Conversion = ctx.get_account(&conversion).unwrap();
    assert!(conv.proof_verified);

    // Primary (native SOL) payout landed, same as the plain native test.
    let claimant_balance_after = ctx.svm.get_balance(&claimant.pubkey()).unwrap_or(0);
    assert!(claimant_balance_after > claimant_balance_before + NATIVE_AMOUNT);

    // The bundle's extra token landed too — the actual new mechanic.
    let claimant_extra_balance: litesvm_token::spl_token::state::Account =
        litesvm_token::get_spl_account(&ctx.svm, &claimant_extra_token_account).unwrap();
    assert_eq!(claimant_extra_balance.amount, EXTRA_AMOUNT);
    let extra_escrow_balance_after_payout: litesvm_token::spl_token::state::Account =
        litesvm_token::get_spl_account(&ctx.svm, &extra_escrow_ata).unwrap();
    assert_eq!(extra_escrow_balance_after_payout.amount, 0);
}

/// Bundle refund path: a bundle is committed, an auction finalizes, and the
/// user deposits both the primary and extra token — but no proof is ever
/// submitted before the proof window closes.
/// `refund_no_proof_native_to_bitcoin` must return *both* tokens to the
/// user, not just the primary.
#[test]
fn bundle_refund_no_proof_returns_primary_and_extra_token() {
    let (mut ctx, admin, ipow_global_state, conversion_global_state) = setup_initialized();
    let user = ctx.svm.create_funded_account(10_000_000_000).unwrap();
    let claimant = ctx.svm.create_funded_account(10_000_000_000).unwrap();

    let extra_mint = litesvm_token::CreateMint::new(&mut ctx.svm, &admin)
        .authority(&admin.pubkey())
        .decimals(6)
        .send()
        .unwrap();
    let user_extra_token_account =
        litesvm_token::CreateAssociatedTokenAccount::new(&mut ctx.svm, &user, &extra_mint)
            .send()
            .unwrap();
    const EXTRA_AMOUNT: u64 = 3_000_000;
    litesvm_token::MintTo::new(&mut ctx.svm, &admin, &extra_mint, &user_extra_token_account, EXTRA_AMOUNT)
        .send()
        .unwrap();

    let (pool, _) = ctx
        .svm
        .get_pda_with_bump(&[b"pool", Pubkey::default().as_ref()], &ipow_conversion::ID);
    let (escrow_vault, _) = ctx
        .svm
        .get_pda_with_bump(&[b"escrow", Pubkey::default().as_ref()], &ipow_conversion::ID);
    let (stake_escrow, _) = ctx
        .svm
        .get_pda_with_bump(&[b"stake_escrow"], &ipow_conversion::ID);

    let (extra_escrow_vault, _) = ctx
        .svm
        .get_pda_with_bump(&[b"escrow", extra_mint.as_ref()], &ipow_conversion::ID);
    let extra_escrow_ata = spl_associated_token_account_interface::address::get_associated_token_address_with_program_id(
        &extra_escrow_vault,
        &extra_mint,
        &litesvm_token::spl_token::ID,
    );

    const BITCOIN_AMOUNT: u64 = 100_000;
    const NATIVE_AMOUNT: u64 = 1_000_000_000;
    const REQUIRED_BOND: u64 = 500_000_000;

    let (conversion, _) = ctx
        .svm
        .get_pda_with_bump(&[b"conversion", &1u64.to_le_bytes()], &ipow_conversion::ID);

    let user_bitcoin_program: Vec<u8> = {
        let mut p = vec![0x76, 0xa9, 0x14];
        p.extend_from_slice(&[0x55u8; 20]);
        p.extend_from_slice(&[0x88, 0xac]);
        p
    };

    let commit_ix = LiteSvmProgram::new(ipow_conversion::ID)
        .accounts(ipow_conversion::client::accounts::CommitTokenToBitcoin {
            conversion_global_state,
            fee_pool: pool,
            fee_escrow: escrow_vault,
            conversion,
            network_config: None,
            user: user.pubkey(),
            system_program: anchor_lang::solana_program::system_program::ID,
        })
        .args(ipow_conversion::client::args::CommitTokenToBitcoin {
            native_amount: NATIVE_AMOUNT,
            bitcoin_amount: BITCOIN_AMOUNT,
            network_id: 0,
            network_address: vec![],
            user_program: user_bitcoin_program.clone(),
            required_bond: REQUIRED_BOND,
            token_mint: Pubkey::default(),
            extra_tokens: vec![ipow_conversion::types::TokenAmount {
                mint: extra_mint,
                amount: EXTRA_AMOUNT,
            }],
        })
        .instruction()
        .unwrap();
    ctx.execute_instruction(commit_ix, &[&user]).unwrap().assert_success();

    let (used_program_pda, _) = ctx
        .svm
        .get_pda_with_bump(&[b"used_prog", &[0u8; 32]], &ipow_conversion::ID);
    let propose_ix = LiteSvmProgram::new(ipow_conversion::ID)
        .accounts(RawAccounts(propose_claim_metas(
            stake_escrow,
            conversion,
            claimant.pubkey(),
            Pubkey::new_unique(),
            used_program_pda,
            pool,
            escrow_vault,
            Pubkey::new_unique(),
            Pubkey::new_unique(),
            dummy_spl_metas(),
            claimant.pubkey(),
        )))
        .args(ipow_conversion::client::args::ProposeClaimConversion {
            stake_amount: REQUIRED_BOND,
            proposed_rate_amount: BITCOIN_AMOUNT,
            duty_window_seconds: 3600,
            ipow_receive_program: vec![],
            program_hash: [0u8; 32],
        })
        .instruction()
        .unwrap();
    ctx.execute_instruction(propose_ix, &[&claimant])
        .unwrap()
        .assert_success();

    advance_clock(&mut ctx, 61);

    let finalize_ix = LiteSvmProgram::new(ipow_conversion::ID)
        .accounts(ipow_conversion::client::accounts::FinalizeClaimConversion {
            conversion,
            ipow_global_state,
        })
        .args(ipow_conversion::client::args::FinalizeClaimConversion {})
        .instruction()
        .unwrap();
    ctx.execute_instruction(finalize_ix, &[&claimant])
        .unwrap()
        .assert_success();

    let deposit_ix = {
        let mut deposit_metas = vec![
            meta(ipow_global_state, false, false),
            meta(conversion, true, false),
            meta(pool, true, false),
            meta(escrow_vault, true, false),
            meta(Pubkey::new_unique(), true, false),
            meta(Pubkey::new_unique(), true, false),
        ];
        deposit_metas.extend(dummy_spl_metas_real_ata_program());
        deposit_metas.push(meta(user.pubkey(), true, true));
        deposit_metas.push(meta(anchor_lang::solana_program::system_program::ID, false, false));
        deposit_metas.push(meta(extra_mint, false, false));
        deposit_metas.push(meta(litesvm_token::spl_token::ID, false, false));
        deposit_metas.push(meta(extra_escrow_vault, true, false));
        deposit_metas.push(meta(extra_escrow_ata, true, false));
        deposit_metas.push(meta(user_extra_token_account, true, false));
        LiteSvmProgram::new(ipow_conversion::ID).accounts(RawAccounts(deposit_metas))
    }
    .args(ipow_conversion::client::args::DepositConversion {})
    .instruction()
    .unwrap();
    ctx.execute_instruction(deposit_ix, &[&user]).unwrap().assert_success();

    let user_extra_balance_after_deposit: litesvm_token::spl_token::state::Account =
        litesvm_token::get_spl_account(&ctx.svm, &user_extra_token_account).unwrap();
    assert_eq!(user_extra_balance_after_deposit.amount, 0);

    // Push the ipow tip height far enough that the proof window has
    // closed (`window_start_height` 0 + `PROOF_BLOCKS_WINDOW` 40), without
    // ever submitting a proof.
    bump_global_tip_height(&mut ctx, ipow_global_state, 100);

    let refund_ix = {
        let mut refund_metas = vec![
            meta(ipow_global_state, false, false),
            meta(conversion, true, false),
            meta(pool, true, false),
            meta(escrow_vault, true, false),
            meta(Pubkey::new_unique(), true, false), // escrow_ata (native, unused)
            meta(pool, true, false),                 // fee_pool (dup, native)
            meta(escrow_vault, true, false),         // fee_escrow (dup, native)
            meta(user.pubkey(), true, false),
            meta(Pubkey::new_unique(), true, false), // user_token_account (native, unused)
        ];
        refund_metas.extend(dummy_spl_metas_real_ata_program());
        refund_metas.push(meta(user.pubkey(), true, true)); // payer
        refund_metas.push(meta(anchor_lang::solana_program::system_program::ID, false, false));
        refund_metas.push(meta(extra_mint, false, false));
        refund_metas.push(meta(litesvm_token::spl_token::ID, false, false));
        refund_metas.push(meta(extra_escrow_vault, true, false));
        refund_metas.push(meta(extra_escrow_ata, true, false));
        refund_metas.push(meta(user_extra_token_account, true, false));
        LiteSvmProgram::new(ipow_conversion::ID).accounts(RawAccounts(refund_metas))
    }
    .args(ipow_conversion::client::args::RefundNoProofNativeToBitcoin {})
    .instruction()
    .unwrap();

    let user_native_balance_before = ctx.svm.get_balance(&user.pubkey()).unwrap_or(0);
    ctx.execute_instruction(refund_ix, &[&user]).unwrap().assert_success();

    // Primary (native SOL) refunded back to the user.
    let user_native_balance_after = ctx.svm.get_balance(&user.pubkey()).unwrap_or(0);
    assert!(user_native_balance_after > user_native_balance_before);

    // The bundle's extra token refunded too — not just the primary.
    let user_extra_balance_after_refund: litesvm_token::spl_token::state::Account =
        litesvm_token::get_spl_account(&ctx.svm, &user_extra_token_account).unwrap();
    assert_eq!(user_extra_balance_after_refund.amount, EXTRA_AMOUNT);
    let extra_escrow_balance_after_refund: litesvm_token::spl_token::state::Account =
        litesvm_token::get_spl_account(&ctx.svm, &extra_escrow_ata).unwrap();
    assert_eq!(extra_escrow_balance_after_refund.amount, 0);
}

/// Security test: `deposit_conversion`'s bundle loop re-derives each extra
/// token's `escrow_vault` PDA and rejects a mismatched one rather than
/// trusting whatever `remaining_accounts` entry the caller passed — this is
/// the highest-risk new code from this pass (a wrong/attacker-supplied
/// remaining account is a fund-safety bug, not a UX one). A wrong escrow
/// vault must revert with `InvalidRemainingAccount`, not silently deposit
/// into the wrong place.
#[test]
fn bundle_deposit_rejects_mismatched_remaining_account() {
    let (mut ctx, admin, ipow_global_state, conversion_global_state) = setup_initialized();
    let user = ctx.svm.create_funded_account(10_000_000_000).unwrap();
    let claimant = ctx.svm.create_funded_account(10_000_000_000).unwrap();

    let extra_mint = litesvm_token::CreateMint::new(&mut ctx.svm, &admin)
        .authority(&admin.pubkey())
        .decimals(6)
        .send()
        .unwrap();
    let user_extra_token_account =
        litesvm_token::CreateAssociatedTokenAccount::new(&mut ctx.svm, &user, &extra_mint)
            .send()
            .unwrap();
    const EXTRA_AMOUNT: u64 = 1_000_000;
    litesvm_token::MintTo::new(&mut ctx.svm, &admin, &extra_mint, &user_extra_token_account, EXTRA_AMOUNT)
        .send()
        .unwrap();

    let (pool, _) = ctx
        .svm
        .get_pda_with_bump(&[b"pool", Pubkey::default().as_ref()], &ipow_conversion::ID);
    let (escrow_vault, _) = ctx
        .svm
        .get_pda_with_bump(&[b"escrow", Pubkey::default().as_ref()], &ipow_conversion::ID);
    let (stake_escrow, _) = ctx
        .svm
        .get_pda_with_bump(&[b"stake_escrow"], &ipow_conversion::ID);

    const BITCOIN_AMOUNT: u64 = 100_000;
    const NATIVE_AMOUNT: u64 = 1_000_000_000;
    const REQUIRED_BOND: u64 = 500_000_000;

    let (conversion, _) = ctx
        .svm
        .get_pda_with_bump(&[b"conversion", &1u64.to_le_bytes()], &ipow_conversion::ID);

    let user_bitcoin_program: Vec<u8> = {
        let mut p = vec![0x76, 0xa9, 0x14];
        p.extend_from_slice(&[0x66u8; 20]);
        p.extend_from_slice(&[0x88, 0xac]);
        p
    };

    let commit_ix = LiteSvmProgram::new(ipow_conversion::ID)
        .accounts(ipow_conversion::client::accounts::CommitTokenToBitcoin {
            conversion_global_state,
            fee_pool: pool,
            fee_escrow: escrow_vault,
            conversion,
            network_config: None,
            user: user.pubkey(),
            system_program: anchor_lang::solana_program::system_program::ID,
        })
        .args(ipow_conversion::client::args::CommitTokenToBitcoin {
            native_amount: NATIVE_AMOUNT,
            bitcoin_amount: BITCOIN_AMOUNT,
            network_id: 0,
            network_address: vec![],
            user_program: user_bitcoin_program.clone(),
            required_bond: REQUIRED_BOND,
            token_mint: Pubkey::default(),
            extra_tokens: vec![ipow_conversion::types::TokenAmount {
                mint: extra_mint,
                amount: EXTRA_AMOUNT,
            }],
        })
        .instruction()
        .unwrap();
    ctx.execute_instruction(commit_ix, &[&user]).unwrap().assert_success();

    let (used_program_pda, _) = ctx
        .svm
        .get_pda_with_bump(&[b"used_prog", &[0u8; 32]], &ipow_conversion::ID);
    let propose_ix = LiteSvmProgram::new(ipow_conversion::ID)
        .accounts(RawAccounts(propose_claim_metas(
            stake_escrow,
            conversion,
            claimant.pubkey(),
            Pubkey::new_unique(),
            used_program_pda,
            pool,
            escrow_vault,
            Pubkey::new_unique(),
            Pubkey::new_unique(),
            dummy_spl_metas(),
            claimant.pubkey(),
        )))
        .args(ipow_conversion::client::args::ProposeClaimConversion {
            stake_amount: REQUIRED_BOND,
            proposed_rate_amount: BITCOIN_AMOUNT,
            duty_window_seconds: 3600,
            ipow_receive_program: vec![],
            program_hash: [0u8; 32],
        })
        .instruction()
        .unwrap();
    ctx.execute_instruction(propose_ix, &[&claimant])
        .unwrap()
        .assert_success();

    advance_clock(&mut ctx, 61);

    let finalize_ix = LiteSvmProgram::new(ipow_conversion::ID)
        .accounts(ipow_conversion::client::accounts::FinalizeClaimConversion {
            conversion,
            ipow_global_state,
        })
        .args(ipow_conversion::client::args::FinalizeClaimConversion {})
        .instruction()
        .unwrap();
    ctx.execute_instruction(finalize_ix, &[&claimant])
        .unwrap()
        .assert_success();

    // Attacker-shaped deposit: everything correct except the extra token's
    // `escrow_vault`, which is some unrelated account instead of the real
    // re-derivable PDA for `extra_mint`.
    let bogus_escrow_vault = Pubkey::new_unique();
    let bogus_escrow_ata = spl_associated_token_account_interface::address::get_associated_token_address_with_program_id(
        &bogus_escrow_vault,
        &extra_mint,
        &litesvm_token::spl_token::ID,
    );
    let deposit_ix = {
        let mut deposit_metas = vec![
            meta(ipow_global_state, false, false),
            meta(conversion, true, false),
            meta(pool, true, false),
            meta(escrow_vault, true, false),
            meta(Pubkey::new_unique(), true, false),
            meta(Pubkey::new_unique(), true, false),
        ];
        deposit_metas.extend(dummy_spl_metas_real_ata_program());
        deposit_metas.push(meta(user.pubkey(), true, true));
        deposit_metas.push(meta(anchor_lang::solana_program::system_program::ID, false, false));
        deposit_metas.push(meta(extra_mint, false, false));
        deposit_metas.push(meta(litesvm_token::spl_token::ID, false, false));
        deposit_metas.push(meta(bogus_escrow_vault, true, false));
        deposit_metas.push(meta(bogus_escrow_ata, true, false));
        deposit_metas.push(meta(user_extra_token_account, true, false));
        LiteSvmProgram::new(ipow_conversion::ID).accounts(RawAccounts(deposit_metas))
    }
    .args(ipow_conversion::client::args::DepositConversion {})
    .instruction()
    .unwrap();
    ctx.execute_instruction(deposit_ix, &[&user])
        .unwrap()
        .assert_anchor_error("InvalidRemainingAccount");

    // No funds moved — the user's extra token balance is untouched.
    let user_extra_balance: litesvm_token::spl_token::state::Account =
        litesvm_token::get_spl_account(&ctx.svm, &user_extra_token_account).unwrap();
    assert_eq!(user_extra_balance.amount, EXTRA_AMOUNT);
}

/// Cap enforcement: `commit_token_to_bitcoin` must reject a bundle carrying
/// more than 3 extra tokens (4 total including the primary).
#[test]
fn bundle_commit_rejects_more_than_three_extra_tokens() {
    let (mut ctx, _admin, _ipow_global_state, conversion_global_state) = setup_initialized();
    let user = ctx.svm.create_funded_account(10_000_000_000).unwrap();

    let (pool, _) = ctx
        .svm
        .get_pda_with_bump(&[b"pool", Pubkey::default().as_ref()], &ipow_conversion::ID);
    let (escrow_vault, _) = ctx
        .svm
        .get_pda_with_bump(&[b"escrow", Pubkey::default().as_ref()], &ipow_conversion::ID);

    const BITCOIN_AMOUNT: u64 = 100_000;
    const NATIVE_AMOUNT: u64 = 1_000_000_000;
    const REQUIRED_BOND: u64 = 500_000_000;

    let (conversion, _) = ctx
        .svm
        .get_pda_with_bump(&[b"conversion", &1u64.to_le_bytes()], &ipow_conversion::ID);

    let user_bitcoin_program: Vec<u8> = {
        let mut p = vec![0x76, 0xa9, 0x14];
        p.extend_from_slice(&[0x77u8; 20]);
        p.extend_from_slice(&[0x88, 0xac]);
        p
    };

    // 4 extra tokens — one over the cap of 3.
    let extra_tokens: Vec<ipow_conversion::types::TokenAmount> = (0..4)
        .map(|_| ipow_conversion::types::TokenAmount {
            mint: Pubkey::new_unique(),
            amount: 1_000,
        })
        .collect();

    let commit_ix = LiteSvmProgram::new(ipow_conversion::ID)
        .accounts(ipow_conversion::client::accounts::CommitTokenToBitcoin {
            conversion_global_state,
            fee_pool: pool,
            fee_escrow: escrow_vault,
            conversion,
            network_config: None,
            user: user.pubkey(),
            system_program: anchor_lang::solana_program::system_program::ID,
        })
        .args(ipow_conversion::client::args::CommitTokenToBitcoin {
            native_amount: NATIVE_AMOUNT,
            bitcoin_amount: BITCOIN_AMOUNT,
            network_id: 0,
            network_address: vec![],
            user_program: user_bitcoin_program,
            required_bond: REQUIRED_BOND,
            token_mint: Pubkey::default(),
            extra_tokens,
        })
        .instruction()
        .unwrap();
    ctx.execute_instruction(commit_ix, &[&user])
        .unwrap()
        .assert_anchor_error("TooManyTokens");
}

/// `open_bundle_tunnel`'s base mechanic, single-token: the opener self-
/// funds `native_amount` atomically (no separate propose/finalize), and
/// later `submit_proof_cache` — called by `dest`, the recipient, exactly
/// like a normal `bitcoin->token` payout — releases it once a real
/// Bitcoin payment against the opener's registered script is proven.
#[test]
fn open_bundle_tunnel_single_token_pays_out_via_submit_proof_cache() {
    let (mut ctx, admin, ipow_global_state, conversion_global_state) = setup_initialized();
    let opener = ctx.svm.create_funded_account(10_000_000_000).unwrap();
    let dest = ctx.svm.create_funded_account(10_000_000_000).unwrap();

    const NETWORK_ID: u64 = 7;
    let network_config = add_network(&mut ctx, conversion_global_state, &admin, NETWORK_ID);

    let (pool, _) = ctx
        .svm
        .get_pda_with_bump(&[b"pool", Pubkey::default().as_ref()], &ipow_conversion::ID);
    let (escrow_vault, _) = ctx
        .svm
        .get_pda_with_bump(&[b"escrow", Pubkey::default().as_ref()], &ipow_conversion::ID);
    let (conversion, _) = ctx
        .svm
        .get_pda_with_bump(&[b"conversion", &1u64.to_le_bytes()], &ipow_conversion::ID);

    const NATIVE_AMOUNT: u64 = 1_000_000_000; // 1 SOL
    const BITCOIN_AMOUNT: u64 = 100_000;

    let ipow_program: Vec<u8> = {
        let mut p = vec![0x76, 0xa9, 0x14];
        p.extend_from_slice(&[0x99u8; 20]);
        p.extend_from_slice(&[0x88, 0xac]);
        p
    };
    let program_hash: [u8; 32] = Sha256::digest(&ipow_program).into();
    let (used_program_pda, _) = ctx
        .svm
        .get_pda_with_bump(&[b"used_prog", &program_hash], &ipow_conversion::ID);

    let open_ix = LiteSvmProgram::new(ipow_conversion::ID)
        .accounts(RawAccounts(open_bundle_tunnel_metas(
            conversion_global_state,
            conversion,
            Some(network_config),
            used_program_pda,
            pool,
            escrow_vault,
            Pubkey::new_unique(), // escrow_ata (native, unused)
            Pubkey::new_unique(), // opener_token_account (native, unused)
            dummy_spl_metas(),
            ipow_global_state,
            opener.pubkey(),
        )))
        .args(ipow_conversion::client::args::OpenBundleTunnel {
            native_amount: NATIVE_AMOUNT,
            bitcoin_amount: BITCOIN_AMOUNT,
            token_mint: Pubkey::default(),
            extra_tokens: vec![],
            dest_address: dest.pubkey(),
            network_id: NETWORK_ID,
            network_address: vec![0x11u8; 20],
            duty_window_seconds: 3600,
            ipow_receive_program: ipow_program.clone(),
            program_hash,
        })
        .instruction()
        .unwrap();

    let escrow_balance_before = ctx.svm.get_balance(&escrow_vault).unwrap_or(0);
    ctx.execute_instruction(open_ix, &[&opener]).unwrap().assert_success();
    let escrow_balance_after_open = ctx.svm.get_balance(&escrow_vault).unwrap_or(0);
    assert_eq!(escrow_balance_after_open - escrow_balance_before, NATIVE_AMOUNT);

    let conv: ipow_conversion::accounts::Conversion = ctx.get_account(&conversion).unwrap();
    assert!(conv.window_started);
    assert_eq!(conv.responsible_operator, opener.pubkey());
    assert_eq!(conv.reserved_native, NATIVE_AMOUNT);
    assert_eq!(conv.user, dest.pubkey());

    let tx_raw = tx_paying(&ipow_program, BITCOIN_AMOUNT + 1_000);
    let hash1 = Sha256::digest(&tx_raw);
    let hash2 = Sha256::digest(hash1);
    let mut txid_le = [0u8; 32];
    txid_le.copy_from_slice(&hash2);
    let header_pda = seed_header(&mut ctx, 0, txid_le);

    let (stake_escrow, _) = ctx
        .svm
        .get_pda_with_bump(&[b"stake_escrow"], &ipow_conversion::ID);
    let submit_metas = submit_proof_cache_metas(
        conversion,
        pool,
        escrow_vault,
        stake_escrow,
        header_pda,
        opener.pubkey(),
        dest.pubkey(),
        dest.pubkey(),
    );
    let submit_ix = LiteSvmProgram::new(ipow_conversion::ID)
        .accounts(RawAccounts(submit_metas))
        .args(ipow_conversion::client::args::SubmitProofCache {
            tx_raw,
            vout_index: 0,
            proof_block_height: 0,
            branch_le: vec![],
            index: 0,
        })
        .instruction()
        .unwrap();
    ctx.execute_instruction(submit_ix, &[&dest]).unwrap().assert_success();

    let escrow_balance_after_payout = ctx.svm.get_balance(&escrow_vault).unwrap_or(0);
    assert_eq!(escrow_balance_after_open - escrow_balance_after_payout, NATIVE_AMOUNT);
}

/// The actual point of this feature: a bundle (native primary + one real
/// SPL extra token) self-escrowed atomically by the opener, then released
/// in full — not just the primary — once `dest` proves the real Bitcoin
/// payment. Exercises `submit_proof_cache`'s new `bitcoin->token`-side
/// bundle payout loop, the fix this feature needed.
#[test]
fn open_bundle_tunnel_bundle_pays_out_primary_and_extra_token() {
    let (mut ctx, admin, ipow_global_state, conversion_global_state) = setup_initialized();
    let opener = ctx.svm.create_funded_account(10_000_000_000).unwrap();
    let dest = ctx.svm.create_funded_account(10_000_000_000).unwrap();

    const NETWORK_ID: u64 = 7;
    let network_config = add_network(&mut ctx, conversion_global_state, &admin, NETWORK_ID);

    let extra_mint = litesvm_token::CreateMint::new(&mut ctx.svm, &admin)
        .authority(&admin.pubkey())
        .decimals(6)
        .send()
        .unwrap();
    let opener_extra_token_account =
        litesvm_token::CreateAssociatedTokenAccount::new(&mut ctx.svm, &opener, &extra_mint)
            .send()
            .unwrap();
    const EXTRA_AMOUNT: u64 = 2_000_000;
    litesvm_token::MintTo::new(&mut ctx.svm, &admin, &extra_mint, &opener_extra_token_account, EXTRA_AMOUNT)
        .send()
        .unwrap();

    let (extra_escrow_vault, _) = ctx
        .svm
        .get_pda_with_bump(&[b"escrow", extra_mint.as_ref()], &ipow_conversion::ID);
    let extra_escrow_ata = spl_associated_token_account_interface::address::get_associated_token_address_with_program_id(
        &extra_escrow_vault,
        &extra_mint,
        &litesvm_token::spl_token::ID,
    );

    let (pool, _) = ctx
        .svm
        .get_pda_with_bump(&[b"pool", Pubkey::default().as_ref()], &ipow_conversion::ID);
    let (escrow_vault, _) = ctx
        .svm
        .get_pda_with_bump(&[b"escrow", Pubkey::default().as_ref()], &ipow_conversion::ID);
    let (conversion, _) = ctx
        .svm
        .get_pda_with_bump(&[b"conversion", &1u64.to_le_bytes()], &ipow_conversion::ID);

    const NATIVE_AMOUNT: u64 = 1_000_000_000;
    const BITCOIN_AMOUNT: u64 = 100_000;

    let ipow_program: Vec<u8> = {
        let mut p = vec![0x76, 0xa9, 0x14];
        p.extend_from_slice(&[0xaau8; 20]);
        p.extend_from_slice(&[0x88, 0xac]);
        p
    };
    let program_hash: [u8; 32] = Sha256::digest(&ipow_program).into();
    let (used_program_pda, _) = ctx
        .svm
        .get_pda_with_bump(&[b"used_prog", &program_hash], &ipow_conversion::ID);

    let mut open_metas = open_bundle_tunnel_metas(
        conversion_global_state,
        conversion,
        Some(network_config),
        used_program_pda,
        pool,
        escrow_vault,
        Pubkey::new_unique(),
        Pubkey::new_unique(),
        dummy_spl_metas_real_ata_program(),
        ipow_global_state,
        opener.pubkey(),
    );
    open_metas.push(meta(extra_mint, false, false));
    open_metas.push(meta(litesvm_token::spl_token::ID, false, false));
    open_metas.push(meta(extra_escrow_vault, true, false));
    open_metas.push(meta(extra_escrow_ata, true, false));
    open_metas.push(meta(opener_extra_token_account, true, false));

    let open_ix = LiteSvmProgram::new(ipow_conversion::ID)
        .accounts(RawAccounts(open_metas))
        .args(ipow_conversion::client::args::OpenBundleTunnel {
            native_amount: NATIVE_AMOUNT,
            bitcoin_amount: BITCOIN_AMOUNT,
            token_mint: Pubkey::default(),
            extra_tokens: vec![ipow_conversion::types::TokenAmount {
                mint: extra_mint,
                amount: EXTRA_AMOUNT,
            }],
            dest_address: dest.pubkey(),
            network_id: NETWORK_ID,
            network_address: vec![0x11u8; 20],
            duty_window_seconds: 3600,
            ipow_receive_program: ipow_program.clone(),
            program_hash,
        })
        .instruction()
        .unwrap();
    ctx.execute_instruction(open_ix, &[&opener]).unwrap().assert_success();

    let extra_escrow_balance_after_open: litesvm_token::spl_token::state::Account =
        litesvm_token::get_spl_account(&ctx.svm, &extra_escrow_ata).unwrap();
    assert_eq!(extra_escrow_balance_after_open.amount, EXTRA_AMOUNT);

    let tx_raw = tx_paying(&ipow_program, BITCOIN_AMOUNT + 1_000);
    let hash1 = Sha256::digest(&tx_raw);
    let hash2 = Sha256::digest(hash1);
    let mut txid_le = [0u8; 32];
    txid_le.copy_from_slice(&hash2);
    let header_pda = seed_header(&mut ctx, 0, txid_le);

    let (stake_escrow, _) = ctx
        .svm
        .get_pda_with_bump(&[b"stake_escrow"], &ipow_conversion::ID);
    let mut submit_metas = submit_proof_cache_metas(
        conversion,
        pool,
        escrow_vault,
        stake_escrow,
        header_pda,
        opener.pubkey(),
        dest.pubkey(),
        dest.pubkey(),
    );
    // The primary spl slot must also carry the real ATA program — the
    // bundle payout loop reuses it, same reason `dummy_spl_metas_real_
    // ata_program()` exists.
    submit_metas[14] = meta(spl_associated_token_account_interface::program::ID, false, false);

    let dest_extra_token_account =
        spl_associated_token_account_interface::address::get_associated_token_address_with_program_id(
            &dest.pubkey(),
            &extra_mint,
            &litesvm_token::spl_token::ID,
        );
    submit_metas.push(meta(extra_mint, false, false));
    submit_metas.push(meta(litesvm_token::spl_token::ID, false, false));
    submit_metas.push(meta(extra_escrow_vault, true, false));
    submit_metas.push(meta(extra_escrow_ata, true, false));
    submit_metas.push(meta(dest_extra_token_account, true, false));

    let submit_ix = LiteSvmProgram::new(ipow_conversion::ID)
        .accounts(RawAccounts(submit_metas))
        .args(ipow_conversion::client::args::SubmitProofCache {
            tx_raw,
            vout_index: 0,
            proof_block_height: 0,
            branch_le: vec![],
            index: 0,
        })
        .instruction()
        .unwrap();
    ctx.execute_instruction(submit_ix, &[&dest]).unwrap().assert_success();

    let dest_extra_balance: litesvm_token::spl_token::state::Account =
        litesvm_token::get_spl_account(&ctx.svm, &dest_extra_token_account).unwrap();
    assert_eq!(dest_extra_balance.amount, EXTRA_AMOUNT);
    let extra_escrow_balance_after_payout: litesvm_token::spl_token::state::Account =
        litesvm_token::get_spl_account(&ctx.svm, &extra_escrow_ata).unwrap();
    assert_eq!(extra_escrow_balance_after_payout.amount, 0);
}

/// Guaranteed resolution: the opener never submits a proof and the duty
/// window expires — `claim_native_operator_expired` must force-release
/// *every* token in the bundle to `dest`, not just the primary. Exercises
/// the other fix this feature needed.
#[test]
fn open_bundle_tunnel_guaranteed_resolution_force_claims_all_tokens() {
    let (mut ctx, admin, ipow_global_state, conversion_global_state) = setup_initialized();
    let opener = ctx.svm.create_funded_account(10_000_000_000).unwrap();
    let dest = ctx.svm.create_funded_account(10_000_000_000).unwrap();

    const NETWORK_ID: u64 = 7;
    let network_config = add_network(&mut ctx, conversion_global_state, &admin, NETWORK_ID);

    let extra_mint = litesvm_token::CreateMint::new(&mut ctx.svm, &admin)
        .authority(&admin.pubkey())
        .decimals(6)
        .send()
        .unwrap();
    let opener_extra_token_account =
        litesvm_token::CreateAssociatedTokenAccount::new(&mut ctx.svm, &opener, &extra_mint)
            .send()
            .unwrap();
    const EXTRA_AMOUNT: u64 = 3_000_000;
    litesvm_token::MintTo::new(&mut ctx.svm, &admin, &extra_mint, &opener_extra_token_account, EXTRA_AMOUNT)
        .send()
        .unwrap();

    let (extra_escrow_vault, _) = ctx
        .svm
        .get_pda_with_bump(&[b"escrow", extra_mint.as_ref()], &ipow_conversion::ID);
    let extra_escrow_ata = spl_associated_token_account_interface::address::get_associated_token_address_with_program_id(
        &extra_escrow_vault,
        &extra_mint,
        &litesvm_token::spl_token::ID,
    );

    let (pool, _) = ctx
        .svm
        .get_pda_with_bump(&[b"pool", Pubkey::default().as_ref()], &ipow_conversion::ID);
    let (escrow_vault, _) = ctx
        .svm
        .get_pda_with_bump(&[b"escrow", Pubkey::default().as_ref()], &ipow_conversion::ID);
    let (conversion, _) = ctx
        .svm
        .get_pda_with_bump(&[b"conversion", &1u64.to_le_bytes()], &ipow_conversion::ID);

    const NATIVE_AMOUNT: u64 = 1_000_000_000;
    const BITCOIN_AMOUNT: u64 = 100_000;
    const DUTY_WINDOW_SECONDS: i64 = 3600;

    let ipow_program: Vec<u8> = {
        let mut p = vec![0x76, 0xa9, 0x14];
        p.extend_from_slice(&[0xbbu8; 20]);
        p.extend_from_slice(&[0x88, 0xac]);
        p
    };
    let program_hash: [u8; 32] = Sha256::digest(&ipow_program).into();
    let (used_program_pda, _) = ctx
        .svm
        .get_pda_with_bump(&[b"used_prog", &program_hash], &ipow_conversion::ID);

    let mut open_metas = open_bundle_tunnel_metas(
        conversion_global_state,
        conversion,
        Some(network_config),
        used_program_pda,
        pool,
        escrow_vault,
        Pubkey::new_unique(),
        Pubkey::new_unique(),
        dummy_spl_metas_real_ata_program(),
        ipow_global_state,
        opener.pubkey(),
    );
    open_metas.push(meta(extra_mint, false, false));
    open_metas.push(meta(litesvm_token::spl_token::ID, false, false));
    open_metas.push(meta(extra_escrow_vault, true, false));
    open_metas.push(meta(extra_escrow_ata, true, false));
    open_metas.push(meta(opener_extra_token_account, true, false));

    let open_ix = LiteSvmProgram::new(ipow_conversion::ID)
        .accounts(RawAccounts(open_metas))
        .args(ipow_conversion::client::args::OpenBundleTunnel {
            native_amount: NATIVE_AMOUNT,
            bitcoin_amount: BITCOIN_AMOUNT,
            token_mint: Pubkey::default(),
            extra_tokens: vec![ipow_conversion::types::TokenAmount {
                mint: extra_mint,
                amount: EXTRA_AMOUNT,
            }],
            dest_address: dest.pubkey(),
            network_id: NETWORK_ID,
            network_address: vec![0x11u8; 20],
            duty_window_seconds: DUTY_WINDOW_SECONDS,
            ipow_receive_program: ipow_program,
            program_hash,
        })
        .instruction()
        .unwrap();
    ctx.execute_instruction(open_ix, &[&opener]).unwrap().assert_success();

    // Duty window expires with no proof ever submitted.
    advance_clock(&mut ctx, DUTY_WINDOW_SECONDS + 1);

    let dest_extra_token_account =
        spl_associated_token_account_interface::address::get_associated_token_address_with_program_id(
            &dest.pubkey(),
            &extra_mint,
            &litesvm_token::spl_token::ID,
        );

    let mut claim_metas = vec![
        meta(conversion, true, false),
        meta(pool, true, false),
        meta(escrow_vault, true, false),
        meta(Pubkey::new_unique(), true, false), // escrow_ata (native, unused)
        meta(dest.pubkey(), true, false),
        meta(Pubkey::new_unique(), true, false), // user_token_account (native, unused)
    ];
    claim_metas.extend(dummy_spl_metas_real_ata_program());
    claim_metas.push(meta(opener.pubkey(), true, true)); // payer — permissionless, anyone can call
    claim_metas.push(meta(anchor_lang::solana_program::system_program::ID, false, false));
    claim_metas.push(meta(extra_mint, false, false));
    claim_metas.push(meta(litesvm_token::spl_token::ID, false, false));
    claim_metas.push(meta(extra_escrow_vault, true, false));
    claim_metas.push(meta(extra_escrow_ata, true, false));
    claim_metas.push(meta(dest_extra_token_account, true, false));

    let claim_ix = LiteSvmProgram::new(ipow_conversion::ID)
        .accounts(RawAccounts(claim_metas))
        .args(ipow_conversion::client::args::ClaimNativeOperatorExpired {})
        .instruction()
        .unwrap();

    let escrow_balance_before = ctx.svm.get_balance(&escrow_vault).unwrap_or(0);
    ctx.execute_instruction(claim_ix, &[&opener]).unwrap().assert_success();
    let escrow_balance_after = ctx.svm.get_balance(&escrow_vault).unwrap_or(0);
    assert_eq!(escrow_balance_before - escrow_balance_after, NATIVE_AMOUNT);

    let dest_extra_balance: litesvm_token::spl_token::state::Account =
        litesvm_token::get_spl_account(&ctx.svm, &dest_extra_token_account).unwrap();
    assert_eq!(dest_extra_balance.amount, EXTRA_AMOUNT);
    let extra_escrow_balance_after: litesvm_token::spl_token::state::Account =
        litesvm_token::get_spl_account(&ctx.svm, &extra_escrow_ata).unwrap();
    assert_eq!(extra_escrow_balance_after.amount, 0);
}

/// Cap enforcement: `open_bundle_tunnel` must reject a bundle carrying
/// more than 3 extra tokens, same cap as `commit_token_to_bitcoin`.
#[test]
fn open_bundle_tunnel_rejects_more_than_three_extra_tokens() {
    let (mut ctx, _admin, ipow_global_state, conversion_global_state) = setup_initialized();
    let opener = ctx.svm.create_funded_account(10_000_000_000).unwrap();
    let dest = ctx.svm.create_funded_account(10_000_000_000).unwrap();

    let (pool, _) = ctx
        .svm
        .get_pda_with_bump(&[b"pool", Pubkey::default().as_ref()], &ipow_conversion::ID);
    let (escrow_vault, _) = ctx
        .svm
        .get_pda_with_bump(&[b"escrow", Pubkey::default().as_ref()], &ipow_conversion::ID);
    let (conversion, _) = ctx
        .svm
        .get_pda_with_bump(&[b"conversion", &1u64.to_le_bytes()], &ipow_conversion::ID);

    let ipow_program: Vec<u8> = {
        let mut p = vec![0x76, 0xa9, 0x14];
        p.extend_from_slice(&[0xccu8; 20]);
        p.extend_from_slice(&[0x88, 0xac]);
        p
    };
    let program_hash: [u8; 32] = Sha256::digest(&ipow_program).into();
    let (used_program_pda, _) = ctx
        .svm
        .get_pda_with_bump(&[b"used_prog", &program_hash], &ipow_conversion::ID);

    let extra_tokens: Vec<ipow_conversion::types::TokenAmount> = (0..4)
        .map(|_| ipow_conversion::types::TokenAmount {
            mint: Pubkey::new_unique(),
            amount: 1_000,
        })
        .collect();

    let open_ix = LiteSvmProgram::new(ipow_conversion::ID)
        .accounts(RawAccounts(open_bundle_tunnel_metas(
            conversion_global_state,
            conversion,
            None,
            used_program_pda,
            pool,
            escrow_vault,
            Pubkey::new_unique(),
            Pubkey::new_unique(),
            dummy_spl_metas(),
            ipow_global_state,
            opener.pubkey(),
        )))
        .args(ipow_conversion::client::args::OpenBundleTunnel {
            native_amount: 1_000_000_000,
            bitcoin_amount: 100_000,
            token_mint: Pubkey::default(),
            extra_tokens,
            dest_address: dest.pubkey(),
            network_id: 7,
            network_address: vec![],
            duty_window_seconds: 3600,
            ipow_receive_program: ipow_program,
            program_hash,
        })
        .instruction()
        .unwrap();
    ctx.execute_instruction(open_ix, &[&opener])
        .unwrap()
        .assert_anchor_error("TooManyTokens");
}

/// `open_bundle_tunnel` is tunnel-only: `network_id == 0` must be
/// rejected, so this isn't a backdoor around `commit_bitcoin_to_token`'s
/// normal auctioned path for direct-Bitcoin, single-token conversions.
#[test]
fn open_bundle_tunnel_rejects_network_id_zero() {
    let (mut ctx, _admin, ipow_global_state, conversion_global_state) = setup_initialized();
    let opener = ctx.svm.create_funded_account(10_000_000_000).unwrap();
    let dest = ctx.svm.create_funded_account(10_000_000_000).unwrap();

    let (pool, _) = ctx
        .svm
        .get_pda_with_bump(&[b"pool", Pubkey::default().as_ref()], &ipow_conversion::ID);
    let (escrow_vault, _) = ctx
        .svm
        .get_pda_with_bump(&[b"escrow", Pubkey::default().as_ref()], &ipow_conversion::ID);
    let (conversion, _) = ctx
        .svm
        .get_pda_with_bump(&[b"conversion", &1u64.to_le_bytes()], &ipow_conversion::ID);

    let ipow_program: Vec<u8> = {
        let mut p = vec![0x76, 0xa9, 0x14];
        p.extend_from_slice(&[0xddu8; 20]);
        p.extend_from_slice(&[0x88, 0xac]);
        p
    };
    let program_hash: [u8; 32] = Sha256::digest(&ipow_program).into();
    let (used_program_pda, _) = ctx
        .svm
        .get_pda_with_bump(&[b"used_prog", &program_hash], &ipow_conversion::ID);

    let open_ix = LiteSvmProgram::new(ipow_conversion::ID)
        .accounts(RawAccounts(open_bundle_tunnel_metas(
            conversion_global_state,
            conversion,
            None,
            used_program_pda,
            pool,
            escrow_vault,
            Pubkey::new_unique(),
            Pubkey::new_unique(),
            dummy_spl_metas(),
            ipow_global_state,
            opener.pubkey(),
        )))
        .args(ipow_conversion::client::args::OpenBundleTunnel {
            native_amount: 1_000_000_000,
            bitcoin_amount: 100_000,
            token_mint: Pubkey::default(),
            extra_tokens: vec![],
            dest_address: dest.pubkey(),
            network_id: 0,
            network_address: vec![],
            duty_window_seconds: 3600,
            ipow_receive_program: ipow_program,
            program_hash,
        })
        .instruction()
        .unwrap();
    ctx.execute_instruction(open_ix, &[&opener])
        .unwrap()
        .assert_anchor_error("IncorrectNetwork");
}

/// `add_network` must reject the same malformed configs `iPoWV1.sol`'s
/// own `addNetwork` already rejects: `network_id == 0`, `min_addr_len ==
/// 0`, and `min_addr_len > max_addr_len`.
#[test]
fn add_network_rejects_invalid_configs() {
    let (mut ctx, admin, _ipow_global_state, conversion_global_state) = setup_initialized();

    let bad_configs: [(u64, u16, u16); 3] = [(0, 1, 32), (7, 0, 32), (7, 32, 1)];
    for (network_id, min_addr_len, max_addr_len) in bad_configs {
        let (network_config, _) = ctx
            .svm
            .get_pda_with_bump(&[b"network", &network_id.to_le_bytes()], &ipow_conversion::ID);
        let ix = LiteSvmProgram::new(ipow_conversion::ID)
            .accounts(ipow_conversion::client::accounts::AddNetwork {
                conversion_global_state,
                network_config,
                admin: admin.pubkey(),
                system_program: anchor_lang::solana_program::system_program::ID,
            })
            .args(ipow_conversion::client::args::AddNetwork {
                network_id,
                min_addr_len,
                max_addr_len,
            })
            .instruction()
            .unwrap();
        ctx.execute_instruction(ix, &[&admin])
            .unwrap()
            .assert_anchor_error("InvalidNetworkConfig");
    }
}

/// `remove_network` closes the PDA and refunds its rent to `admin` — the
/// Solana-native equivalent of the EVM side's `delete networkConfigs[...]`
/// — and a later `add_network` for the same `network_id` can `init` it
/// fresh afterward.
#[test]
fn remove_network_closes_account_and_allows_re_adding() {
    let (mut ctx, admin, _ipow_global_state, conversion_global_state) = setup_initialized();
    const NETWORK_ID: u64 = 7;

    let network_config = add_network(&mut ctx, conversion_global_state, &admin, NETWORK_ID);
    assert!(ctx.svm.get_account(&network_config).is_some());

    let admin_balance_before = ctx.svm.get_balance(&admin.pubkey()).unwrap_or(0);

    let remove_ix = LiteSvmProgram::new(ipow_conversion::ID)
        .accounts(ipow_conversion::client::accounts::RemoveNetwork {
            conversion_global_state,
            network_config,
            admin: admin.pubkey(),
            system_program: anchor_lang::solana_program::system_program::ID,
        })
        .args(ipow_conversion::client::args::RemoveNetwork {
            network_id: NETWORK_ID,
        })
        .instruction()
        .unwrap();
    ctx.execute_instruction(remove_ix, &[&admin]).unwrap().assert_success();

    // Account is gone (or zeroed/reassigned to the system program) — either
    // way it no longer holds the `SupportedNetwork` discriminator/data.
    let closed = ctx.svm.get_account(&network_config);
    assert!(closed.is_none() || closed.unwrap().data.is_empty());
    assert!(ctx.svm.get_balance(&admin.pubkey()).unwrap_or(0) > admin_balance_before);

    // Re-adding the same network_id now succeeds (the PDA is free again).
    // Force a fresh blockhash first — otherwise this second `add_network`
    // call is byte-identical to the first (same signer/accounts/args) and
    // litesvm rejects it as already-processed before it even reaches the
    // program.
    ctx.svm.expire_blockhash();
    let network_config_again = add_network(&mut ctx, conversion_global_state, &admin, NETWORK_ID);
    assert_eq!(network_config_again, network_config);
    let conv: ipow_conversion::accounts::SupportedNetwork = ctx.get_account(&network_config).unwrap();
    assert!(conv.is_active);
}

/// The new auctioned `bitcoin->token` bundle: `commit_bitcoin_to_token`
/// fixes a bundle (native primary + one real SPL extra) and a `bitcoin_
/// amount` ceiling; claimant A meets the ask exactly, claimant B out-bids
/// with a *lower* `bitcoin_amount` — asserts A's full bundle (not just
/// the primary) refunds, B's full bundle self-escrows, and `submit_proof_
/// cache` pays the whole bundle out to the user once B proves payment.
#[test]
fn bitcoin_to_token_bundle_auction_lowest_bitcoin_wins_and_pays_out() {
    let (mut ctx, admin, ipow_global_state, conversion_global_state) = setup_initialized();
    let user = ctx.svm.create_funded_account(10_000_000_000).unwrap();
    let claimant_a = ctx.svm.create_funded_account(10_000_000_000).unwrap();
    let claimant_b = ctx.svm.create_funded_account(10_000_000_000).unwrap();

    let extra_mint = litesvm_token::CreateMint::new(&mut ctx.svm, &admin)
        .authority(&admin.pubkey())
        .decimals(6)
        .send()
        .unwrap();
    const EXTRA_AMOUNT: u64 = 2_000_000;

    let claimant_a_extra_token_account =
        litesvm_token::CreateAssociatedTokenAccount::new(&mut ctx.svm, &claimant_a, &extra_mint)
            .send()
            .unwrap();
    litesvm_token::MintTo::new(&mut ctx.svm, &admin, &extra_mint, &claimant_a_extra_token_account, EXTRA_AMOUNT)
        .send()
        .unwrap();
    let claimant_b_extra_token_account =
        litesvm_token::CreateAssociatedTokenAccount::new(&mut ctx.svm, &claimant_b, &extra_mint)
            .send()
            .unwrap();
    litesvm_token::MintTo::new(&mut ctx.svm, &admin, &extra_mint, &claimant_b_extra_token_account, EXTRA_AMOUNT)
        .send()
        .unwrap();

    let (pool, _) = ctx
        .svm
        .get_pda_with_bump(&[b"pool", Pubkey::default().as_ref()], &ipow_conversion::ID);
    let (escrow_vault, _) = ctx
        .svm
        .get_pda_with_bump(&[b"escrow", Pubkey::default().as_ref()], &ipow_conversion::ID);
    let (stake_escrow, _) = ctx
        .svm
        .get_pda_with_bump(&[b"stake_escrow"], &ipow_conversion::ID);
    let (extra_escrow_vault, _) = ctx
        .svm
        .get_pda_with_bump(&[b"escrow", extra_mint.as_ref()], &ipow_conversion::ID);
    let extra_escrow_ata = spl_associated_token_account_interface::address::get_associated_token_address_with_program_id(
        &extra_escrow_vault,
        &extra_mint,
        &litesvm_token::spl_token::ID,
    );
    let (conversion, _) = ctx
        .svm
        .get_pda_with_bump(&[b"conversion", &1u64.to_le_bytes()], &ipow_conversion::ID);

    const BITCOIN_ASK: u64 = 200_000;
    const NATIVE_AMOUNT: u64 = 1_000_000_000;
    const REQUIRED_BOND: u64 = 500_000_000;

    let user_bitcoin_program: Vec<u8> = {
        let mut p = vec![0x76, 0xa9, 0x14];
        p.extend_from_slice(&[0xeeu8; 20]);
        p.extend_from_slice(&[0x88, 0xac]);
        p
    };

    let commit_ix = LiteSvmProgram::new(ipow_conversion::ID)
        .accounts(ipow_conversion::client::accounts::CommitBitcoinToToken {
            conversion_global_state,
            fee_pool: pool,
            fee_escrow: escrow_vault,
            conversion,
            user: user.pubkey(),
            system_program: anchor_lang::solana_program::system_program::ID,
        })
        .args(ipow_conversion::client::args::CommitBitcoinToToken {
            bitcoin_amount: BITCOIN_ASK,
            native_amount: NATIVE_AMOUNT,
            network_id: 0,
            network_address: vec![],
            user_program: user_bitcoin_program.clone(),
            token_mint: Pubkey::default(),
            extra_tokens: vec![ipow_conversion::types::TokenAmount {
                mint: extra_mint,
                amount: EXTRA_AMOUNT,
            }],
        })
        .instruction()
        .unwrap();
    ctx.execute_instruction(commit_ix, &[&user]).unwrap().assert_success();

    // Claimant A proposes at exactly the fixed ask (first claim).
    let program_a: Vec<u8> = {
        let mut p = vec![0x76, 0xa9, 0x14];
        p.extend_from_slice(&[0x01u8; 20]);
        p.extend_from_slice(&[0x88, 0xac]);
        p
    };
    let hash_a: [u8; 32] = Sha256::digest(&program_a).into();
    let (used_program_pda_a, _) = ctx
        .svm
        .get_pda_with_bump(&[b"used_prog", &hash_a], &ipow_conversion::ID);

    let mut propose_a_metas = propose_claim_metas(
        stake_escrow,
        conversion,
        claimant_a.pubkey(), // previous_claimant, unused on a first claim
        Pubkey::new_unique(),
        used_program_pda_a,
        pool,
        escrow_vault,
        Pubkey::new_unique(),
        Pubkey::new_unique(), // claimant_token_account (native primary, unused)
        dummy_spl_metas_real_ata_program(),
        claimant_a.pubkey(),
    );
    propose_a_metas.push(meta(extra_mint, false, false));
    propose_a_metas.push(meta(litesvm_token::spl_token::ID, false, false));
    propose_a_metas.push(meta(extra_escrow_vault, true, false));
    propose_a_metas.push(meta(extra_escrow_ata, true, false));
    propose_a_metas.push(meta(Pubkey::new_unique(), true, false)); // previous_claimant_extra_token_account, unused
    propose_a_metas.push(meta(claimant_a_extra_token_account, true, false));

    let propose_a_ix = LiteSvmProgram::new(ipow_conversion::ID)
        .accounts(RawAccounts(propose_a_metas))
        .args(ipow_conversion::client::args::ProposeClaimConversion {
            stake_amount: REQUIRED_BOND,
            proposed_rate_amount: BITCOIN_ASK,
            duty_window_seconds: 3600,
            ipow_receive_program: program_a.clone(),
            program_hash: hash_a,
        })
        .instruction()
        .unwrap();
    ctx.execute_instruction(propose_a_ix, &[&claimant_a])
        .unwrap()
        .assert_success();

    let extra_escrow_after_a: litesvm_token::spl_token::state::Account =
        litesvm_token::get_spl_account(&ctx.svm, &extra_escrow_ata).unwrap();
    assert_eq!(extra_escrow_after_a.amount, EXTRA_AMOUNT);

    // Claimant B out-bids with a LOWER bitcoin_amount than the ask.
    const BITCOIN_LOWER: u64 = 150_000;
    let program_b: Vec<u8> = {
        let mut p = vec![0x76, 0xa9, 0x14];
        p.extend_from_slice(&[0x02u8; 20]);
        p.extend_from_slice(&[0x88, 0xac]);
        p
    };
    let hash_b: [u8; 32] = Sha256::digest(&program_b).into();
    let (used_program_pda_b, _) = ctx
        .svm
        .get_pda_with_bump(&[b"used_prog", &hash_b], &ipow_conversion::ID);

    let claimant_a_extra_before: u64 =
        litesvm_token::get_spl_account::<litesvm_token::spl_token::state::Account>(
            &ctx.svm,
            &claimant_a_extra_token_account,
        )
        .unwrap()
        .amount;

    let mut propose_b_metas = propose_claim_metas(
        stake_escrow,
        conversion,
        claimant_a.pubkey(),
        Pubkey::new_unique(), // previous_claimant_token_account (native primary, unused)
        used_program_pda_b,
        pool,
        escrow_vault,
        Pubkey::new_unique(),
        Pubkey::new_unique(),
        dummy_spl_metas_real_ata_program(),
        claimant_b.pubkey(),
    );
    propose_b_metas.push(meta(extra_mint, false, false));
    propose_b_metas.push(meta(litesvm_token::spl_token::ID, false, false));
    propose_b_metas.push(meta(extra_escrow_vault, true, false));
    propose_b_metas.push(meta(extra_escrow_ata, true, false));
    propose_b_metas.push(meta(claimant_a_extra_token_account, true, false)); // refund target
    propose_b_metas.push(meta(claimant_b_extra_token_account, true, false)); // new escrow source

    let propose_b_ix = LiteSvmProgram::new(ipow_conversion::ID)
        .accounts(RawAccounts(propose_b_metas))
        .args(ipow_conversion::client::args::ProposeClaimConversion {
            stake_amount: REQUIRED_BOND,
            proposed_rate_amount: BITCOIN_LOWER,
            duty_window_seconds: 3600,
            ipow_receive_program: program_b.clone(),
            program_hash: hash_b,
        })
        .instruction()
        .unwrap();
    ctx.execute_instruction(propose_b_ix, &[&claimant_b])
        .unwrap()
        .assert_success();

    // A's full bundle refunded — not just the primary.
    let claimant_a_extra_after: litesvm_token::spl_token::state::Account =
        litesvm_token::get_spl_account(&ctx.svm, &claimant_a_extra_token_account).unwrap();
    assert_eq!(claimant_a_extra_after.amount, claimant_a_extra_before + EXTRA_AMOUNT);

    // B's bundle now escrowed instead (A's refund + B's re-escrow nets to
    // the same EXTRA_AMOUNT sitting in escrow throughout).
    let extra_escrow_after_b: litesvm_token::spl_token::state::Account =
        litesvm_token::get_spl_account(&ctx.svm, &extra_escrow_ata).unwrap();
    assert_eq!(extra_escrow_after_b.amount, EXTRA_AMOUNT);

    let conv: ipow_conversion::accounts::Conversion = ctx.get_account(&conversion).unwrap();
    assert_eq!(conv.bitcoin_amount, BITCOIN_LOWER);
    assert_eq!(conv.native_amount, NATIVE_AMOUNT); // fixed, never competed on
    assert_eq!(conv.responsible_operator, claimant_b.pubkey());

    advance_clock(&mut ctx, 61);
    let finalize_ix = LiteSvmProgram::new(ipow_conversion::ID)
        .accounts(ipow_conversion::client::accounts::FinalizeClaimConversion {
            conversion,
            ipow_global_state,
        })
        .args(ipow_conversion::client::args::FinalizeClaimConversion {})
        .instruction()
        .unwrap();
    ctx.execute_instruction(finalize_ix, &[&claimant_b])
        .unwrap()
        .assert_success();

    let tx_raw = tx_paying(&program_b, BITCOIN_LOWER + 1_000);
    let hash1 = Sha256::digest(&tx_raw);
    let hash2 = Sha256::digest(hash1);
    let mut txid_le = [0u8; 32];
    txid_le.copy_from_slice(&hash2);
    let header_pda = seed_header(&mut ctx, 0, txid_le);

    let user_extra_token_account =
        spl_associated_token_account_interface::address::get_associated_token_address_with_program_id(
            &user.pubkey(),
            &extra_mint,
            &litesvm_token::spl_token::ID,
        );

    let mut submit_metas = submit_proof_cache_metas(
        conversion,
        pool,
        escrow_vault,
        stake_escrow,
        header_pda,
        claimant_b.pubkey(),
        user.pubkey(),
        user.pubkey(),
    );
    submit_metas[14] = meta(spl_associated_token_account_interface::program::ID, false, false);
    submit_metas.push(meta(extra_mint, false, false));
    submit_metas.push(meta(litesvm_token::spl_token::ID, false, false));
    submit_metas.push(meta(extra_escrow_vault, true, false));
    submit_metas.push(meta(extra_escrow_ata, true, false));
    submit_metas.push(meta(user_extra_token_account, true, false));

    let submit_ix = LiteSvmProgram::new(ipow_conversion::ID)
        .accounts(RawAccounts(submit_metas))
        .args(ipow_conversion::client::args::SubmitProofCache {
            tx_raw,
            vout_index: 0,
            proof_block_height: 0,
            branch_le: vec![],
            index: 0,
        })
        .instruction()
        .unwrap();
    ctx.execute_instruction(submit_ix, &[&user]).unwrap().assert_success();

    let user_extra_balance: litesvm_token::spl_token::state::Account =
        litesvm_token::get_spl_account(&ctx.svm, &user_extra_token_account).unwrap();
    assert_eq!(user_extra_balance.amount, EXTRA_AMOUNT);
}

/// Rate validation for a bundle claim: a first claim above the fixed
/// Bitcoin ask reverts `AboveAskedRate`; a later claim that doesn't
/// strictly beat (go lower than) the current best reverts `RateNotBetter`.
#[test]
fn bitcoin_to_token_bundle_rejects_bad_rates() {
    let (mut ctx, admin, _ipow_global_state, conversion_global_state) = setup_initialized();
    let user = ctx.svm.create_funded_account(10_000_000_000).unwrap();
    let claimant_a = ctx.svm.create_funded_account(10_000_000_000).unwrap();
    let claimant_b = ctx.svm.create_funded_account(10_000_000_000).unwrap();

    let extra_mint = litesvm_token::CreateMint::new(&mut ctx.svm, &admin)
        .authority(&admin.pubkey())
        .decimals(6)
        .send()
        .unwrap();
    const EXTRA_AMOUNT: u64 = 1_000_000;
    let claimant_a_extra_token_account =
        litesvm_token::CreateAssociatedTokenAccount::new(&mut ctx.svm, &claimant_a, &extra_mint)
            .send()
            .unwrap();
    litesvm_token::MintTo::new(&mut ctx.svm, &admin, &extra_mint, &claimant_a_extra_token_account, EXTRA_AMOUNT)
        .send()
        .unwrap();

    let (pool, _) = ctx
        .svm
        .get_pda_with_bump(&[b"pool", Pubkey::default().as_ref()], &ipow_conversion::ID);
    let (escrow_vault, _) = ctx
        .svm
        .get_pda_with_bump(&[b"escrow", Pubkey::default().as_ref()], &ipow_conversion::ID);
    let (stake_escrow, _) = ctx
        .svm
        .get_pda_with_bump(&[b"stake_escrow"], &ipow_conversion::ID);
    let (extra_escrow_vault, _) = ctx
        .svm
        .get_pda_with_bump(&[b"escrow", extra_mint.as_ref()], &ipow_conversion::ID);
    let extra_escrow_ata = spl_associated_token_account_interface::address::get_associated_token_address_with_program_id(
        &extra_escrow_vault,
        &extra_mint,
        &litesvm_token::spl_token::ID,
    );
    let (conversion, _) = ctx
        .svm
        .get_pda_with_bump(&[b"conversion", &1u64.to_le_bytes()], &ipow_conversion::ID);

    const BITCOIN_ASK: u64 = 200_000;
    const NATIVE_AMOUNT: u64 = 1_000_000_000;
    const REQUIRED_BOND: u64 = 500_000_000;

    let commit_ix = LiteSvmProgram::new(ipow_conversion::ID)
        .accounts(ipow_conversion::client::accounts::CommitBitcoinToToken {
            conversion_global_state,
            fee_pool: pool,
            fee_escrow: escrow_vault,
            conversion,
            user: user.pubkey(),
            system_program: anchor_lang::solana_program::system_program::ID,
        })
        .args(ipow_conversion::client::args::CommitBitcoinToToken {
            bitcoin_amount: BITCOIN_ASK,
            native_amount: NATIVE_AMOUNT,
            network_id: 0,
            network_address: vec![],
            user_program: vec![0x11u8; 20],
            token_mint: Pubkey::default(),
            extra_tokens: vec![ipow_conversion::types::TokenAmount {
                mint: extra_mint,
                amount: EXTRA_AMOUNT,
            }],
        })
        .instruction()
        .unwrap();
    ctx.execute_instruction(commit_ix, &[&user]).unwrap().assert_success();

    let build_propose_metas = |claimant: Pubkey,
                                program_hash: [u8; 32],
                                extra_claimant_token_account: Pubkey|
     -> Vec<solana_program::instruction::AccountMeta> {
        let (used_program_pda, _) =
            Pubkey::find_program_address(&[b"used_prog", &program_hash], &ipow_conversion::ID);
        let mut metas = propose_claim_metas(
            stake_escrow,
            conversion,
            claimant,
            Pubkey::new_unique(),
            used_program_pda,
            pool,
            escrow_vault,
            Pubkey::new_unique(),
            Pubkey::new_unique(),
            dummy_spl_metas_real_ata_program(),
            claimant,
        );
        metas.push(meta(extra_mint, false, false));
        metas.push(meta(litesvm_token::spl_token::ID, false, false));
        metas.push(meta(extra_escrow_vault, true, false));
        metas.push(meta(extra_escrow_ata, true, false));
        metas.push(meta(Pubkey::new_unique(), true, false));
        metas.push(meta(extra_claimant_token_account, true, false));
        metas
    };

    // First claim ABOVE the fixed ask must revert.
    let program_too_high: Vec<u8> = {
        let mut p = vec![0x76, 0xa9, 0x14];
        p.extend_from_slice(&[0x03u8; 20]);
        p.extend_from_slice(&[0x88, 0xac]);
        p
    };
    let hash_too_high: [u8; 32] = Sha256::digest(&program_too_high).into();
    let propose_too_high_ix = LiteSvmProgram::new(ipow_conversion::ID)
        .accounts(RawAccounts(build_propose_metas(
            claimant_a.pubkey(),
            hash_too_high,
            claimant_a_extra_token_account,
        )))
        .args(ipow_conversion::client::args::ProposeClaimConversion {
            stake_amount: REQUIRED_BOND,
            proposed_rate_amount: BITCOIN_ASK + 1,
            duty_window_seconds: 3600,
            ipow_receive_program: program_too_high,
            program_hash: hash_too_high,
        })
        .instruction()
        .unwrap();
    ctx.execute_instruction(propose_too_high_ix, &[&claimant_a])
        .unwrap()
        .assert_anchor_error("AboveAskedRate");

    // A real first claim at the ask succeeds.
    let program_a: Vec<u8> = {
        let mut p = vec![0x76, 0xa9, 0x14];
        p.extend_from_slice(&[0x04u8; 20]);
        p.extend_from_slice(&[0x88, 0xac]);
        p
    };
    let hash_a: [u8; 32] = Sha256::digest(&program_a).into();
    let propose_a_ix = LiteSvmProgram::new(ipow_conversion::ID)
        .accounts(RawAccounts(build_propose_metas(
            claimant_a.pubkey(),
            hash_a,
            claimant_a_extra_token_account,
        )))
        .args(ipow_conversion::client::args::ProposeClaimConversion {
            stake_amount: REQUIRED_BOND,
            proposed_rate_amount: BITCOIN_ASK,
            duty_window_seconds: 3600,
            ipow_receive_program: program_a,
            program_hash: hash_a,
        })
        .instruction()
        .unwrap();
    ctx.execute_instruction(propose_a_ix, &[&claimant_a])
        .unwrap()
        .assert_success();

    // A second claim at the SAME rate (not strictly lower) must revert.
    let claimant_b_extra_token_account =
        litesvm_token::CreateAssociatedTokenAccount::new(&mut ctx.svm, &claimant_b, &extra_mint)
            .send()
            .unwrap();
    litesvm_token::MintTo::new(&mut ctx.svm, &admin, &extra_mint, &claimant_b_extra_token_account, EXTRA_AMOUNT)
        .send()
        .unwrap();
    let program_not_better: Vec<u8> = {
        let mut p = vec![0x76, 0xa9, 0x14];
        p.extend_from_slice(&[0x05u8; 20]);
        p.extend_from_slice(&[0x88, 0xac]);
        p
    };
    let hash_not_better: [u8; 32] = Sha256::digest(&program_not_better).into();
    let mut propose_not_better_metas = propose_claim_metas(
        stake_escrow,
        conversion,
        claimant_a.pubkey(),
        Pubkey::new_unique(),
        ctx.svm.get_pda_with_bump(&[b"used_prog", &hash_not_better], &ipow_conversion::ID).0,
        pool,
        escrow_vault,
        Pubkey::new_unique(),
        Pubkey::new_unique(),
        dummy_spl_metas_real_ata_program(),
        claimant_b.pubkey(),
    );
    propose_not_better_metas.push(meta(extra_mint, false, false));
    propose_not_better_metas.push(meta(litesvm_token::spl_token::ID, false, false));
    propose_not_better_metas.push(meta(extra_escrow_vault, true, false));
    propose_not_better_metas.push(meta(extra_escrow_ata, true, false));
    propose_not_better_metas.push(meta(claimant_a_extra_token_account, true, false));
    propose_not_better_metas.push(meta(claimant_b_extra_token_account, true, false));
    let propose_not_better_ix = LiteSvmProgram::new(ipow_conversion::ID)
        .accounts(RawAccounts(propose_not_better_metas))
        .args(ipow_conversion::client::args::ProposeClaimConversion {
            stake_amount: REQUIRED_BOND,
            proposed_rate_amount: BITCOIN_ASK,
            duty_window_seconds: 3600,
            ipow_receive_program: program_not_better,
            program_hash: hash_not_better,
        })
        .instruction()
        .unwrap();
    ctx.execute_instruction(propose_not_better_ix, &[&claimant_b])
        .unwrap()
        .assert_anchor_error("RateNotBetter");
}

/// Reclaim-and-reopen with a bundle: claimant A wins, then goes dark past
/// the duty window. Claimant B reclaims the forfeited stake and picks up
/// the *same already-escrowed bundle* without re-funding it (`window_
/// started` gates the self-escrow block, same as the single-token case),
/// completes it, and earns the forfeited bounty.
#[test]
fn bitcoin_to_token_bundle_reclaim_lets_new_claimant_finish_without_refunding() {
    let (mut ctx, admin, ipow_global_state, conversion_global_state) = setup_initialized();
    let user = ctx.svm.create_funded_account(10_000_000_000).unwrap();
    let claimant_a = ctx.svm.create_funded_account(10_000_000_000).unwrap();
    let claimant_b = ctx.svm.create_funded_account(10_000_000_000).unwrap();

    let extra_mint = litesvm_token::CreateMint::new(&mut ctx.svm, &admin)
        .authority(&admin.pubkey())
        .decimals(6)
        .send()
        .unwrap();
    const EXTRA_AMOUNT: u64 = 3_000_000;
    let claimant_a_extra_token_account =
        litesvm_token::CreateAssociatedTokenAccount::new(&mut ctx.svm, &claimant_a, &extra_mint)
            .send()
            .unwrap();
    litesvm_token::MintTo::new(&mut ctx.svm, &admin, &extra_mint, &claimant_a_extra_token_account, EXTRA_AMOUNT)
        .send()
        .unwrap();

    let (pool, _) = ctx
        .svm
        .get_pda_with_bump(&[b"pool", Pubkey::default().as_ref()], &ipow_conversion::ID);
    let (escrow_vault, _) = ctx
        .svm
        .get_pda_with_bump(&[b"escrow", Pubkey::default().as_ref()], &ipow_conversion::ID);
    let (stake_escrow, _) = ctx
        .svm
        .get_pda_with_bump(&[b"stake_escrow"], &ipow_conversion::ID);
    let (extra_escrow_vault, _) = ctx
        .svm
        .get_pda_with_bump(&[b"escrow", extra_mint.as_ref()], &ipow_conversion::ID);
    let extra_escrow_ata = spl_associated_token_account_interface::address::get_associated_token_address_with_program_id(
        &extra_escrow_vault,
        &extra_mint,
        &litesvm_token::spl_token::ID,
    );
    let (conversion, _) = ctx
        .svm
        .get_pda_with_bump(&[b"conversion", &1u64.to_le_bytes()], &ipow_conversion::ID);

    const BITCOIN_ASK: u64 = 200_000;
    const NATIVE_AMOUNT: u64 = 1_000_000_000;
    const REQUIRED_BOND: u64 = 500_000_000;

    let commit_ix = LiteSvmProgram::new(ipow_conversion::ID)
        .accounts(ipow_conversion::client::accounts::CommitBitcoinToToken {
            conversion_global_state,
            fee_pool: pool,
            fee_escrow: escrow_vault,
            conversion,
            user: user.pubkey(),
            system_program: anchor_lang::solana_program::system_program::ID,
        })
        .args(ipow_conversion::client::args::CommitBitcoinToToken {
            bitcoin_amount: BITCOIN_ASK,
            native_amount: NATIVE_AMOUNT,
            network_id: 0,
            network_address: vec![],
            user_program: vec![0x11u8; 20],
            token_mint: Pubkey::default(),
            extra_tokens: vec![ipow_conversion::types::TokenAmount {
                mint: extra_mint,
                amount: EXTRA_AMOUNT,
            }],
        })
        .instruction()
        .unwrap();
    ctx.execute_instruction(commit_ix, &[&user]).unwrap().assert_success();

    let program_a: Vec<u8> = {
        let mut p = vec![0x76, 0xa9, 0x14];
        p.extend_from_slice(&[0x06u8; 20]);
        p.extend_from_slice(&[0x88, 0xac]);
        p
    };
    let hash_a: [u8; 32] = Sha256::digest(&program_a).into();
    let (used_program_pda_a, _) = ctx
        .svm
        .get_pda_with_bump(&[b"used_prog", &hash_a], &ipow_conversion::ID);
    let mut propose_a_metas = propose_claim_metas(
        stake_escrow,
        conversion,
        claimant_a.pubkey(),
        Pubkey::new_unique(),
        used_program_pda_a,
        pool,
        escrow_vault,
        Pubkey::new_unique(),
        Pubkey::new_unique(),
        dummy_spl_metas_real_ata_program(),
        claimant_a.pubkey(),
    );
    propose_a_metas.push(meta(extra_mint, false, false));
    propose_a_metas.push(meta(litesvm_token::spl_token::ID, false, false));
    propose_a_metas.push(meta(extra_escrow_vault, true, false));
    propose_a_metas.push(meta(extra_escrow_ata, true, false));
    propose_a_metas.push(meta(Pubkey::new_unique(), true, false));
    propose_a_metas.push(meta(claimant_a_extra_token_account, true, false));
    let propose_a_ix = LiteSvmProgram::new(ipow_conversion::ID)
        .accounts(RawAccounts(propose_a_metas))
        .args(ipow_conversion::client::args::ProposeClaimConversion {
            stake_amount: REQUIRED_BOND,
            proposed_rate_amount: BITCOIN_ASK,
            duty_window_seconds: 3600,
            ipow_receive_program: program_a.clone(),
            program_hash: hash_a,
        })
        .instruction()
        .unwrap();
    ctx.execute_instruction(propose_a_ix, &[&claimant_a])
        .unwrap()
        .assert_success();

    advance_clock(&mut ctx, 61);
    let finalize_ix = LiteSvmProgram::new(ipow_conversion::ID)
        .accounts(ipow_conversion::client::accounts::FinalizeClaimConversion {
            conversion,
            ipow_global_state,
        })
        .args(ipow_conversion::client::args::FinalizeClaimConversion {})
        .instruction()
        .unwrap();
    ctx.execute_instruction(finalize_ix, &[&claimant_a])
        .unwrap()
        .assert_success();

    // Claimant A goes dark — duty window (3600s) plus a margin passes.
    advance_clock(&mut ctx, 3601);

    let reclaim_ix = LiteSvmProgram::new(ipow_conversion::ID)
        .accounts(ipow_conversion::client::accounts::ReclaimExpiredConversion {
            stake_escrow,
            conversion,
            previous_claimant: claimant_a.pubkey(),
            user: user.pubkey(),
            system_program: anchor_lang::solana_program::system_program::ID,
        })
        .args(ipow_conversion::client::args::ReclaimExpiredConversion {})
        .instruction()
        .unwrap();
    ctx.execute_instruction(reclaim_ix, &[&user]).unwrap().assert_success();

    let conv: ipow_conversion::accounts::Conversion = ctx.get_account(&conversion).unwrap();
    // No stake was ever posted for `bitcoin->token` — nothing to forfeit,
    // nothing paid to the user here (unlike `token->bitcoin`'s own
    // reclaim test).
    assert_eq!(conv.responsible_operator, Pubkey::default());

    // The bundle is still fully escrowed — untouched by the reclaim.
    let extra_escrow_after_reclaim: litesvm_token::spl_token::state::Account =
        litesvm_token::get_spl_account(&ctx.svm, &extra_escrow_ata).unwrap();
    assert_eq!(extra_escrow_after_reclaim.amount, EXTRA_AMOUNT);

    // Claimant B picks up the exact same reservation — `window_started` is
    // already true, so no re-escrow happens; no stake is posted either
    // (this direction never posts one).
    let program_b: Vec<u8> = {
        let mut p = vec![0x76, 0xa9, 0x14];
        p.extend_from_slice(&[0x07u8; 20]);
        p.extend_from_slice(&[0x88, 0xac]);
        p
    };
    let hash_b: [u8; 32] = Sha256::digest(&program_b).into();
    let (used_program_pda_b, _) = ctx
        .svm
        .get_pda_with_bump(&[b"used_prog", &hash_b], &ipow_conversion::ID);
    let claimant_b_balance_before_propose = ctx.svm.get_balance(&claimant_b.pubkey()).unwrap_or(0);
    let propose_b_metas = propose_claim_metas(
        stake_escrow,
        conversion,
        Pubkey::new_unique(), // previous_claimant, unused (is_first_claim after reclaim)
        Pubkey::new_unique(),
        used_program_pda_b,
        pool,
        escrow_vault,
        Pubkey::new_unique(),
        Pubkey::new_unique(),
        dummy_spl_metas_real_ata_program(),
        claimant_b.pubkey(),
    );
    // No `remaining_accounts` appended: `window_started == true`, so the
    // self-escrow block (and its bundle loop) never runs at all.
    let propose_b_ix = LiteSvmProgram::new(ipow_conversion::ID)
        .accounts(RawAccounts(propose_b_metas))
        .args(ipow_conversion::client::args::ProposeClaimConversion {
            stake_amount: REQUIRED_BOND,
            proposed_rate_amount: BITCOIN_ASK,
            duty_window_seconds: 3600,
            ipow_receive_program: program_b.clone(),
            program_hash: hash_b,
        })
        .instruction()
        .unwrap();
    ctx.execute_instruction(propose_b_ix, &[&claimant_b])
        .unwrap()
        .assert_success();

    advance_clock(&mut ctx, 61);
    let finalize_b_ix = LiteSvmProgram::new(ipow_conversion::ID)
        .accounts(ipow_conversion::client::accounts::FinalizeClaimConversion {
            conversion,
            ipow_global_state,
        })
        .args(ipow_conversion::client::args::FinalizeClaimConversion {})
        .instruction()
        .unwrap();
    ctx.execute_instruction(finalize_b_ix, &[&claimant_b])
        .unwrap()
        .assert_success();

    let tx_raw = tx_paying(&program_b, BITCOIN_ASK + 1_000);
    let hash1 = Sha256::digest(&tx_raw);
    let hash2 = Sha256::digest(hash1);
    let mut txid_le = [0u8; 32];
    txid_le.copy_from_slice(&hash2);
    let header_pda = seed_header(&mut ctx, 0, txid_le);

    let user_extra_token_account =
        spl_associated_token_account_interface::address::get_associated_token_address_with_program_id(
            &user.pubkey(),
            &extra_mint,
            &litesvm_token::spl_token::ID,
        );
    let mut submit_metas = submit_proof_cache_metas(
        conversion,
        pool,
        escrow_vault,
        stake_escrow,
        header_pda,
        claimant_b.pubkey(),
        user.pubkey(),
        user.pubkey(),
    );
    submit_metas[14] = meta(spl_associated_token_account_interface::program::ID, false, false);
    submit_metas.push(meta(extra_mint, false, false));
    submit_metas.push(meta(litesvm_token::spl_token::ID, false, false));
    submit_metas.push(meta(extra_escrow_vault, true, false));
    submit_metas.push(meta(extra_escrow_ata, true, false));
    submit_metas.push(meta(user_extra_token_account, true, false));
    let submit_ix = LiteSvmProgram::new(ipow_conversion::ID)
        .accounts(RawAccounts(submit_metas))
        .args(ipow_conversion::client::args::SubmitProofCache {
            tx_raw,
            vout_index: 0,
            proof_block_height: 0,
            branch_le: vec![],
            index: 0,
        })
        .instruction()
        .unwrap();
    ctx.execute_instruction(submit_ix, &[&user]).unwrap().assert_success();

    let user_extra_balance: litesvm_token::spl_token::state::Account =
        litesvm_token::get_spl_account(&ctx.svm, &user_extra_token_account).unwrap();
    assert_eq!(user_extra_balance.amount, EXTRA_AMOUNT);

    // Claimant B's balance moves (commit fee, if any) but never involved a
    // stake in either direction — nothing to earn back or forfeit here.
    let claimant_b_balance_after = ctx.svm.get_balance(&claimant_b.pubkey()).unwrap_or(0);
    assert!(claimant_b_balance_after >= claimant_b_balance_before_propose);
}

/// Cap enforcement via the second entry point: 4 extra tokens through
/// `commit_bitcoin_to_token` reverts `TooManyTokens`, same validation
/// already tested for `commit_token_to_bitcoin`.
#[test]
fn bitcoin_to_token_bundle_commit_rejects_more_than_three_extra_tokens() {
    let (mut ctx, _admin, _ipow_global_state, conversion_global_state) = setup_initialized();
    let user = ctx.svm.create_funded_account(10_000_000_000).unwrap();

    let (pool, _) = ctx
        .svm
        .get_pda_with_bump(&[b"pool", Pubkey::default().as_ref()], &ipow_conversion::ID);
    let (escrow_vault, _) = ctx
        .svm
        .get_pda_with_bump(&[b"escrow", Pubkey::default().as_ref()], &ipow_conversion::ID);
    let (conversion, _) = ctx
        .svm
        .get_pda_with_bump(&[b"conversion", &1u64.to_le_bytes()], &ipow_conversion::ID);

    let extra_tokens: Vec<ipow_conversion::types::TokenAmount> = (0..4)
        .map(|_| ipow_conversion::types::TokenAmount {
            mint: Pubkey::new_unique(),
            amount: 1_000,
        })
        .collect();

    let commit_ix = LiteSvmProgram::new(ipow_conversion::ID)
        .accounts(ipow_conversion::client::accounts::CommitBitcoinToToken {
            conversion_global_state,
            fee_pool: pool,
            fee_escrow: escrow_vault,
            conversion,
            user: user.pubkey(),
            system_program: anchor_lang::solana_program::system_program::ID,
        })
        .args(ipow_conversion::client::args::CommitBitcoinToToken {
            bitcoin_amount: 100_000,
            native_amount: 1_000_000_000,
            network_id: 0,
            network_address: vec![],
            user_program: vec![0x11u8; 20],
            token_mint: Pubkey::default(),
            extra_tokens,
        })
        .instruction()
        .unwrap();
    ctx.execute_instruction(commit_ix, &[&user])
        .unwrap()
        .assert_anchor_error("TooManyTokens");
}

/// `bitcoin->token` never posts a stake — confirms a claimant passing a
/// deliberately nonzero `stake_amount` anyway still succeeds (it's simply
/// ignored, not required to be zero), and that nothing is silently swept
/// into the shared `stake_escrow` pool: the claimant's own balance only
/// reflects what they actually self-escrowed (`NATIVE_AMOUNT`), never an
/// additional stake on top.
#[test]
fn bitcoin_to_token_ignores_a_nonzero_stake_amount() {
    let (mut ctx, _admin, _ipow_global_state, conversion_global_state) = setup_initialized();
    let user = ctx.svm.create_funded_account(10_000_000_000).unwrap();
    let claimant = ctx.svm.create_funded_account(10_000_000_000).unwrap();

    let (pool, _) = ctx
        .svm
        .get_pda_with_bump(&[b"pool", Pubkey::default().as_ref()], &ipow_conversion::ID);
    let (escrow_vault, _) = ctx
        .svm
        .get_pda_with_bump(&[b"escrow", Pubkey::default().as_ref()], &ipow_conversion::ID);
    let (stake_escrow, _) = ctx
        .svm
        .get_pda_with_bump(&[b"stake_escrow"], &ipow_conversion::ID);
    let (conversion, _) = ctx
        .svm
        .get_pda_with_bump(&[b"conversion", &1u64.to_le_bytes()], &ipow_conversion::ID);

    const BITCOIN_AMOUNT: u64 = 100_000;
    const NATIVE_AMOUNT: u64 = 1_000_000_000;
    // Deliberately nonzero, larger than a typical bond would be — must be
    // fully ignored, not silently accepted-and-lost.
    const IGNORED_STAKE_AMOUNT: u64 = 500_000_000;

    let commit_ix = LiteSvmProgram::new(ipow_conversion::ID)
        .accounts(ipow_conversion::client::accounts::CommitBitcoinToToken {
            conversion_global_state,
            fee_pool: pool,
            fee_escrow: escrow_vault,
            conversion,
            user: user.pubkey(),
            system_program: anchor_lang::solana_program::system_program::ID,
        })
        .args(ipow_conversion::client::args::CommitBitcoinToToken {
            bitcoin_amount: BITCOIN_AMOUNT,
            native_amount: NATIVE_AMOUNT,
            network_id: 0,
            network_address: vec![],
            user_program: vec![0x11u8; 20],
            token_mint: Pubkey::default(),
            extra_tokens: vec![],
        })
        .instruction()
        .unwrap();
    ctx.execute_instruction(commit_ix, &[&user]).unwrap().assert_success();

    let ipow_program: Vec<u8> = {
        let mut p = vec![0x76, 0xa9, 0x14];
        p.extend_from_slice(&[0x09u8; 20]);
        p.extend_from_slice(&[0x88, 0xac]);
        p
    };
    let program_hash: [u8; 32] = Sha256::digest(&ipow_program).into();
    let (used_program_pda, _) = ctx
        .svm
        .get_pda_with_bump(&[b"used_prog", &program_hash], &ipow_conversion::ID);

    let claimant_balance_before = ctx.svm.get_balance(&claimant.pubkey()).unwrap_or(0);
    let stake_escrow_balance_before = ctx.svm.get_balance(&stake_escrow).unwrap_or(0);

    let propose_ix = LiteSvmProgram::new(ipow_conversion::ID)
        .accounts(RawAccounts(propose_claim_metas(
            stake_escrow,
            conversion,
            claimant.pubkey(),
            Pubkey::new_unique(),
            used_program_pda,
            pool,
            escrow_vault,
            Pubkey::new_unique(),
            Pubkey::new_unique(),
            dummy_spl_metas(),
            claimant.pubkey(),
        )))
        .args(ipow_conversion::client::args::ProposeClaimConversion {
            stake_amount: IGNORED_STAKE_AMOUNT,
            proposed_rate_amount: NATIVE_AMOUNT,
            duty_window_seconds: 3600,
            ipow_receive_program: ipow_program,
            program_hash,
        })
        .instruction()
        .unwrap();
    ctx.execute_instruction(propose_ix, &[&claimant]).unwrap().assert_success();

    // `stake_escrow`'s balance is completely untouched — nothing was
    // swept into the shared pool.
    let stake_escrow_balance_after = ctx.svm.get_balance(&stake_escrow).unwrap_or(0);
    assert_eq!(stake_escrow_balance_after, stake_escrow_balance_before);

    // The claimant's own balance only reflects self-escrowing NATIVE_AMOUNT
    // (into `escrow_vault`) plus this tx's own fee — never an additional
    // `IGNORED_STAKE_AMOUNT` on top.
    let claimant_balance_after = ctx.svm.get_balance(&claimant.pubkey()).unwrap_or(0);
    let claimant_drop = claimant_balance_before - claimant_balance_after;
    assert!(claimant_drop >= NATIVE_AMOUNT);
    assert!(claimant_drop < NATIVE_AMOUNT + IGNORED_STAKE_AMOUNT);

    let conv: ipow_conversion::accounts::Conversion = ctx.get_account(&conversion).unwrap();
    assert_eq!(conv.staked_bond, 0);
}
