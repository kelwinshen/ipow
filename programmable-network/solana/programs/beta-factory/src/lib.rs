use anchor_lang::prelude::*;

pub mod anchor_verify;
pub mod constants;
pub mod errors;
pub mod state;
pub mod statement;
pub mod utils;

// Same anchor-lang 1.0 `#[program]` macro workaround as `ipow-conversion`:
// instruction modules are direct children named after their `Accounts`
// struct, re-exported with their generated client modules.
pub mod approve_pending;
pub mod audit_skipped_release;
pub mod burn_redeem;
pub mod exercise_mint;
pub mod expire_pending;
pub mod fund_rewards;
pub mod initialize;
pub mod lock_sol;
pub mod process_anchor;
pub mod register_party;
pub mod request_unbond;
pub mod set_params;
pub mod settle_mint;
pub mod skip_anchor;
pub mod top_up_bond;
pub mod withdraw_bond;

pub use approve_pending::ApprovePending;
pub(crate) use approve_pending::__client_accounts_approve_pending;
#[allow(unused_imports)]
pub(crate) use approve_pending::__cpi_client_accounts_approve_pending;
pub use audit_skipped_release::AuditSkippedRelease;
pub(crate) use audit_skipped_release::__client_accounts_audit_skipped_release;
#[allow(unused_imports)]
pub(crate) use audit_skipped_release::__cpi_client_accounts_audit_skipped_release;
pub use burn_redeem::BurnRedeem;
pub(crate) use burn_redeem::__client_accounts_burn_redeem;
#[allow(unused_imports)]
pub(crate) use burn_redeem::__cpi_client_accounts_burn_redeem;
pub use exercise_mint::ExerciseMint;
pub(crate) use exercise_mint::__client_accounts_exercise_mint;
#[allow(unused_imports)]
pub(crate) use exercise_mint::__cpi_client_accounts_exercise_mint;
pub use expire_pending::ExpirePending;
pub(crate) use expire_pending::__client_accounts_expire_pending;
#[allow(unused_imports)]
pub(crate) use expire_pending::__cpi_client_accounts_expire_pending;
pub use fund_rewards::FundRewards;
pub(crate) use fund_rewards::__client_accounts_fund_rewards;
#[allow(unused_imports)]
pub(crate) use fund_rewards::__cpi_client_accounts_fund_rewards;
pub use initialize::Initialize;
pub(crate) use initialize::__client_accounts_initialize;
#[allow(unused_imports)]
pub(crate) use initialize::__cpi_client_accounts_initialize;
pub use lock_sol::LockSol;
pub(crate) use lock_sol::__client_accounts_lock_sol;
#[allow(unused_imports)]
pub(crate) use lock_sol::__cpi_client_accounts_lock_sol;
pub use process_anchor::ProcessAnchor;
pub(crate) use process_anchor::__client_accounts_process_anchor;
#[allow(unused_imports)]
pub(crate) use process_anchor::__cpi_client_accounts_process_anchor;
pub use register_party::RegisterParty;
pub(crate) use register_party::__client_accounts_register_party;
#[allow(unused_imports)]
pub(crate) use register_party::__cpi_client_accounts_register_party;
pub use request_unbond::RequestUnbond;
pub(crate) use request_unbond::__client_accounts_request_unbond;
#[allow(unused_imports)]
pub(crate) use request_unbond::__cpi_client_accounts_request_unbond;
pub use set_params::SetParams;
pub(crate) use set_params::__client_accounts_set_params;
#[allow(unused_imports)]
pub(crate) use set_params::__cpi_client_accounts_set_params;
pub use settle_mint::SettleMint;
pub(crate) use settle_mint::__client_accounts_settle_mint;
#[allow(unused_imports)]
pub(crate) use settle_mint::__cpi_client_accounts_settle_mint;
pub use skip_anchor::SkipAnchor;
pub(crate) use skip_anchor::__client_accounts_skip_anchor;
#[allow(unused_imports)]
pub(crate) use skip_anchor::__cpi_client_accounts_skip_anchor;
pub use top_up_bond::TopUpBond;
pub(crate) use top_up_bond::__client_accounts_top_up_bond;
#[allow(unused_imports)]
pub(crate) use top_up_bond::__cpi_client_accounts_top_up_bond;
pub use withdraw_bond::WithdrawBond;
pub(crate) use withdraw_bond::__client_accounts_withdraw_bond;
#[allow(unused_imports)]
pub(crate) use withdraw_bond::__cpi_client_accounts_withdraw_bond;

use state::{FactoryParams, PartyKind};

declare_id!("3GUPbVjjBRNEYafHprb2fnh6Wzyif9a4gWphSrhkPVEf");

/// BETA v2 factory — `docs/DESIGN_V2.md` §6. One fungible BETA on Solana,
/// its SOL half held here, its ETH half held by `BetaVault.sol`. Cross-chain
/// claims are Bitcoin transactions on each party's statement chain; this
/// program judges claims about Solana facts, acts provisionally on claims
/// about Ethereum facts, and slashes the bonds it holds.
#[program]
pub mod beta_factory {
    use super::*;

    pub fn initialize(ctx: Context<Initialize>, governance: Pubkey, params: FactoryParams) -> Result<()> {
        initialize::handler(ctx, governance, params)
    }

    pub fn set_params(ctx: Context<SetParams>, params: FactoryParams, paused: bool) -> Result<()> {
        set_params::handler(ctx, params, paused)
    }

    pub fn register_party(
        ctx: Context<RegisterParty>,
        party_id: [u8; 32],
        kind: PartyKind,
        anchor_txid_le: [u8; 32],
        anchor_vout: u32,
        bond: u64,
    ) -> Result<()> {
        register_party::handler(ctx, party_id, kind, anchor_txid_le, anchor_vout, bond)
    }

    pub fn top_up_bond(ctx: Context<TopUpBond>, amount: u64) -> Result<()> {
        top_up_bond::handler(ctx, amount)
    }

    pub fn request_unbond(ctx: Context<RequestUnbond>) -> Result<()> {
        request_unbond::handler(ctx)
    }

    pub fn withdraw_bond(ctx: Context<WithdrawBond>) -> Result<()> {
        withdraw_bond::handler(ctx)
    }

    pub fn lock_sol(ctx: Context<LockSol>, nonce: u64, units: u64, deadline: i64, attest_fee: u64) -> Result<()> {
        lock_sol::handler(ctx, nonce, units, deadline, attest_fee)
    }

    pub fn approve_pending(ctx: Context<ApprovePending>, nonce: u64, eth_lock_id: u64) -> Result<()> {
        approve_pending::handler(ctx, nonce, eth_lock_id)
    }

    pub fn expire_pending(ctx: Context<ExpirePending>, nonce: u64) -> Result<()> {
        expire_pending::handler(ctx, nonce)
    }

    pub fn fund_rewards(ctx: Context<FundRewards>, amount: u64) -> Result<()> {
        fund_rewards::handler(ctx, amount)
    }

    #[allow(clippy::too_many_arguments)]
    pub fn process_anchor(
        ctx: Context<ProcessAnchor>,
        txid_le: [u8; 32],
        statement: Vec<u8>,
        tx_raw: Vec<u8>,
        proof_block_height: u64,
        branch_le: Vec<[u8; 32]>,
        index: u64,
    ) -> Result<()> {
        process_anchor::handler(ctx, txid_le, statement, tx_raw, proof_block_height, branch_le, index)
    }

    pub fn exercise_mint(ctx: Context<ExerciseMint>, txid_le: [u8; 32]) -> Result<()> {
        exercise_mint::handler(ctx, txid_le)
    }

    pub fn settle_mint(ctx: Context<SettleMint>, txid_le: [u8; 32]) -> Result<()> {
        settle_mint::handler(ctx, txid_le)
    }

    pub fn burn_redeem(ctx: Context<BurnRedeem>, units: u64, to_eth: [u8; 20]) -> Result<()> {
        burn_redeem::handler(ctx, units, to_eth)
    }

    #[allow(clippy::too_many_arguments)]
    pub fn skip_anchor(
        ctx: Context<SkipAnchor>,
        txid_le: [u8; 32],
        tx_raw: Vec<u8>,
        proof_block_height: u64,
        branch_le: Vec<[u8; 32]>,
        index: u64,
    ) -> Result<()> {
        skip_anchor::handler(ctx, txid_le, tx_raw, proof_block_height, branch_le, index)
    }

    pub fn audit_skipped_release(ctx: Context<AuditSkippedRelease>, txid_le: [u8; 32], statement: Vec<u8>) -> Result<()> {
        audit_skipped_release::handler(ctx, txid_le, statement)
    }
}
