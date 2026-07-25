use anchor_lang;
use anchor_litesvm::{AnchorContext, AnchorLiteSVM, TestHelpers};
use solana_program::pubkey::Pubkey;
use solana_signer::Signer;

anchor_lang::declare_program!(ipow);

fn new_ctx() -> AnchorContext {
    AnchorLiteSVM::build_with_program(ipow::ID, include_bytes!("../../../target/deploy/ipow.so"))
}

#[test]
fn initialize_sets_global_state() {
    let mut ctx = new_ctx();
    let admin = ctx.svm.create_funded_account(10_000_000_000).unwrap();
    let operator = Pubkey::new_unique();

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
            operator,
            commit_fee_bps: 50,
        })
        .instruction()
        .unwrap();

    ctx.execute_instruction(ix, &[&admin])
        .unwrap()
        .assert_success();

    let state: ipow::accounts::GlobalState = ctx.get_account(&global_state).unwrap();
    assert_eq!(state.operator, operator);
    assert_eq!(state.commit_fee_bps, 50);
    assert_eq!(state.next_tx_id, 1);
}
