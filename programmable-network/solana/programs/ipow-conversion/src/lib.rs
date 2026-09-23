use anchor_lang::prelude::*;

pub mod constants;
pub mod errors;
pub mod state;
pub mod utils;

// See the note in `ipow`'s own lib.rs on why instruction modules live as
// direct children of the crate root and are named after their `Accounts`
// struct — same anchor-lang 1.0 `#[program]` macro workaround applies here.
pub mod add_network;
pub mod claim_native_operator_expired;
pub mod commit_bitcoin_to_token;
pub mod commit_token_to_bitcoin;
pub mod deposit_conversion;
pub mod finalize_claim_conversion;
pub mod initialize;
pub mod open_bundle_tunnel;
pub mod propose_claim_conversion;
pub mod reclaim_expired_conversion;
pub mod refund_no_proof_native_to_bitcoin;
pub mod remove_network;
pub mod spl_accounts;
pub mod submit_proof_cache;

pub use add_network::AddNetwork;
pub(crate) use add_network::__client_accounts_add_network;
#[allow(unused_imports)]
pub(crate) use add_network::__cpi_client_accounts_add_network;
pub use claim_native_operator_expired::ClaimNativeOperatorExpired;
pub(crate) use claim_native_operator_expired::__client_accounts_claim_native_operator_expired;
#[allow(unused_imports)]
pub(crate) use claim_native_operator_expired::__cpi_client_accounts_claim_native_operator_expired;
pub use commit_bitcoin_to_token::CommitBitcoinToToken;
pub(crate) use commit_bitcoin_to_token::__client_accounts_commit_bitcoin_to_token;
#[allow(unused_imports)]
pub(crate) use commit_bitcoin_to_token::__cpi_client_accounts_commit_bitcoin_to_token;
pub use commit_token_to_bitcoin::CommitTokenToBitcoin;
pub(crate) use commit_token_to_bitcoin::__client_accounts_commit_token_to_bitcoin;
#[allow(unused_imports)]
pub(crate) use commit_token_to_bitcoin::__cpi_client_accounts_commit_token_to_bitcoin;
pub use deposit_conversion::DepositConversion;
pub(crate) use deposit_conversion::__client_accounts_deposit_conversion;
#[allow(unused_imports)]
pub(crate) use deposit_conversion::__cpi_client_accounts_deposit_conversion;
pub use finalize_claim_conversion::FinalizeClaimConversion;
pub(crate) use finalize_claim_conversion::__client_accounts_finalize_claim_conversion;
#[allow(unused_imports)]
pub(crate) use finalize_claim_conversion::__cpi_client_accounts_finalize_claim_conversion;
pub use initialize::Initialize;
pub(crate) use initialize::__client_accounts_initialize;
#[allow(unused_imports)]
pub(crate) use initialize::__cpi_client_accounts_initialize;
pub use open_bundle_tunnel::OpenBundleTunnel;
pub(crate) use open_bundle_tunnel::__client_accounts_open_bundle_tunnel;
#[allow(unused_imports)]
pub(crate) use open_bundle_tunnel::__cpi_client_accounts_open_bundle_tunnel;
pub use propose_claim_conversion::ProposeClaimConversion;
pub(crate) use propose_claim_conversion::__client_accounts_propose_claim_conversion;
#[allow(unused_imports)]
pub(crate) use propose_claim_conversion::__cpi_client_accounts_propose_claim_conversion;
pub use reclaim_expired_conversion::ReclaimExpiredConversion;
pub(crate) use reclaim_expired_conversion::__client_accounts_reclaim_expired_conversion;
#[allow(unused_imports)]
pub(crate) use reclaim_expired_conversion::__cpi_client_accounts_reclaim_expired_conversion;
pub use refund_no_proof_native_to_bitcoin::RefundNoProofNativeToBitcoin;
pub(crate) use refund_no_proof_native_to_bitcoin::__client_accounts_refund_no_proof_native_to_bitcoin;
#[allow(unused_imports)]
pub(crate) use refund_no_proof_native_to_bitcoin::__cpi_client_accounts_refund_no_proof_native_to_bitcoin;
pub use remove_network::RemoveNetwork;
pub(crate) use remove_network::__client_accounts_remove_network;
#[allow(unused_imports)]
pub(crate) use remove_network::__cpi_client_accounts_remove_network;
pub use spl_accounts::SplAccounts;
// Unlike every other struct here, `SplAccounts` is never used directly as a
// `Context<...>` parameter (only as a composite field inside other Accounts
// structs), so the `#[program]` macro's own codegen in this file never
// references either re-export below — both are only consumed by the
// instruction files that embed `SplAccounts`.
#[allow(unused_imports)]
pub(crate) use spl_accounts::__client_accounts_spl_accounts;
#[allow(unused_imports)]
pub(crate) use spl_accounts::__cpi_client_accounts_spl_accounts;
pub use submit_proof_cache::SubmitProofCache;
pub(crate) use submit_proof_cache::__client_accounts_submit_proof_cache;
#[allow(unused_imports)]
pub(crate) use submit_proof_cache::__cpi_client_accounts_submit_proof_cache;

declare_id!("FbwXLABMqUeRPw85C7MpMS2LQi4mveXR9a9fA79DfS3V");

/// Value-moving Native<->Bitcoin conversion, split out as its own program —
/// see `docs/DESIGN_V2.md`. Replaces the single-fixed-operator `Conversion`
/// that still lives, unmodified, inside `ipow`: the claiming role here is a
/// permissionless windowed staked auction (`ipow-message-relay`'s
/// `propose_claim`/`finalize_claim`/`reclaim_expired_message` mechanics,
/// grafted onto Conversion's own deposit/proof/refund flow). Reads `ipow`'s
/// existing header relay (`GlobalState`/`GlobalHeader`) directly,
/// cross-program, read-only.
#[program]
pub mod ipow_conversion {
    use super::*;

    pub fn initialize(
        ctx: Context<Initialize>,
        governance: Pubkey,
        commit_fee_bps: u16,
    ) -> Result<()> {
        initialize::handler(ctx, governance, commit_fee_bps)
    }

    pub fn add_network(
        ctx: Context<AddNetwork>,
        network_id: u64,
        min_addr_len: u16,
        max_addr_len: u16,
    ) -> Result<()> {
        add_network::handler(ctx, network_id, min_addr_len, max_addr_len)
    }

    pub fn remove_network(ctx: Context<RemoveNetwork>, network_id: u64) -> Result<()> {
        remove_network::handler(ctx, network_id)
    }

    #[allow(clippy::too_many_arguments)]
    pub fn commit_token_to_bitcoin(
        ctx: Context<CommitTokenToBitcoin>,
        native_amount: u64,
        bitcoin_amount: u64,
        network_id: u64,
        network_address: Vec<u8>,
        user_program: Vec<u8>,
        required_bond: u64,
        token_mint: Pubkey,
        extra_tokens: Vec<state::TokenAmount>,
    ) -> Result<()> {
        commit_token_to_bitcoin::handler(
            ctx,
            native_amount,
            bitcoin_amount,
            network_id,
            network_address,
            user_program,
            required_bond,
            token_mint,
            extra_tokens,
        )
    }

    #[allow(clippy::too_many_arguments)]
    #[allow(clippy::too_many_arguments)]
    pub fn commit_bitcoin_to_token(
        ctx: Context<CommitBitcoinToToken>,
        bitcoin_amount: u64,
        native_amount: u64,
        network_id: u64,
        network_address: Vec<u8>,
        user_program: Vec<u8>,
        token_mint: Pubkey,
        extra_tokens: Vec<state::TokenAmount>,
    ) -> Result<()> {
        commit_bitcoin_to_token::handler(
            ctx,
            bitcoin_amount,
            native_amount,
            network_id,
            network_address,
            user_program,
            token_mint,
            extra_tokens,
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub fn open_bundle_tunnel<'info>(
        ctx: Context<'info, OpenBundleTunnel<'info>>,
        native_amount: u64,
        bitcoin_amount: u64,
        token_mint: Pubkey,
        extra_tokens: Vec<state::TokenAmount>,
        dest_address: Pubkey,
        network_id: u64,
        network_address: Vec<u8>,
        duty_window_seconds: i64,
        ipow_receive_program: Vec<u8>,
        program_hash: [u8; 32],
    ) -> Result<()> {
        open_bundle_tunnel::handler(
            ctx,
            native_amount,
            bitcoin_amount,
            token_mint,
            extra_tokens,
            dest_address,
            network_id,
            network_address,
            duty_window_seconds,
            ipow_receive_program,
            program_hash,
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub fn propose_claim_conversion<'info>(
        ctx: Context<'info, ProposeClaimConversion<'info>>,
        stake_amount: u64,
        proposed_rate_amount: u64,
        duty_window_seconds: i64,
        ipow_receive_program: Vec<u8>,
        program_hash: [u8; 32],
    ) -> Result<()> {
        propose_claim_conversion::handler(
            ctx,
            stake_amount,
            proposed_rate_amount,
            duty_window_seconds,
            ipow_receive_program,
            program_hash,
        )
    }

    pub fn finalize_claim_conversion(ctx: Context<FinalizeClaimConversion>) -> Result<()> {
        finalize_claim_conversion::handler(ctx)
    }

    pub fn reclaim_expired_conversion(ctx: Context<ReclaimExpiredConversion>) -> Result<()> {
        reclaim_expired_conversion::handler(ctx)
    }

    pub fn deposit_conversion<'info>(
        ctx: Context<'info, DepositConversion<'info>>,
    ) -> Result<()> {
        deposit_conversion::handler(ctx)
    }

    pub fn submit_proof_cache<'info>(
        ctx: Context<'info, SubmitProofCache<'info>>,
        tx_raw: Vec<u8>,
        vout_index: u64,
        proof_block_height: u64,
        branch_le: Vec<[u8; 32]>,
        index: u64,
    ) -> Result<()> {
        submit_proof_cache::handler(ctx, tx_raw, vout_index, proof_block_height, branch_le, index)
    }

    pub fn refund_no_proof_native_to_bitcoin<'info>(
        ctx: Context<'info, RefundNoProofNativeToBitcoin<'info>>,
    ) -> Result<()> {
        refund_no_proof_native_to_bitcoin::handler(ctx)
    }

    pub fn claim_native_operator_expired<'info>(
        ctx: Context<'info, ClaimNativeOperatorExpired<'info>>,
    ) -> Result<()> {
        claim_native_operator_expired::handler(ctx)
    }
}
