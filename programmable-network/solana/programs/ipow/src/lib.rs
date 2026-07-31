use anchor_lang::prelude::*;

pub mod bitcoin;
pub mod constants;
pub mod errors;
pub mod state;
pub mod utils;

// NOTE: instruction modules are declared as direct children of the crate root
// (not nested under an `instructions/` subdirectory), and each module is named
// after its `Accounts` struct in snake_case rather than after the public
// instruction function name (they differ for several instructions here, e.g.
// `approve_and_start_with_anchor` uses `ApproveConversion`). Both constraints work
// around a confirmed, currently-unresolved bug in anchor-lang 1.0's `#[program]`
// macro: it generates a `__client_accounts_<snake_case(AccountsStructName)>`
// support module and expects to find it at `crate::<that same name>`, failing
// with "unresolved import `crate`" otherwise. See
// https://github.com/solana-foundation/anchor/issues/3690. The public instruction
// names below (and thus their Anchor discriminators / the generated IDL) are
// unaffected — only internal module/file naming had to change.
pub mod add_liquidity;
pub mod add_network;
pub mod approve_conversion;
pub mod claim_native_operator_expired;
pub mod close_no_bitcoin;
pub mod commit_bitcoin_to_native;
pub mod commit_header;
pub mod commit_native_to_bitcoin;
pub mod deposit_conversion;
pub mod finalize_proof_cached;
pub mod initialize;
pub mod operator_open_tunnel;
pub mod refund_conversion;
pub mod refund_no_proof_native_to_bitcoin;
pub mod remove_liquidity;
pub mod submit_proof_cache;
pub mod update_network;

// Each module's `handler` fn is intentionally NOT re-exported here (every file
// names its entry point `handler`, so a blanket glob re-export would collide).
// Only the per-instruction `Accounts` context structs are re-exported by name,
// since those need to be visible as bare types (e.g. `Context<Initialize>`) in
// the `#[program]` block below, which calls each module's `handler` via its
// fully qualified path instead.
// Each `pub use x::Y;` re-exports the Accounts struct itself (needed for
// `Context<Y>` below); each paired `pub(crate) use x::__client_accounts_x;`
// re-exports the sibling module `#[derive(Accounts)]` auto-generates next to it
// (matching visibility — it's `pub(crate)`, not fully `pub`, hence `pub(crate)
// use` here too). Both are required to work around the anchor-lang 1.0
// `#[program]` macro bug described above: its own codegen expects that
// auto-generated module reachable at a bare `crate::__client_accounts_x` path,
// which only happens automatically when the struct lives directly in lib.rs.
pub use add_liquidity::AddLiquidity;
pub(crate) use add_liquidity::__client_accounts_add_liquidity;
pub use add_network::AddNetwork;
pub(crate) use add_network::__client_accounts_add_network;
pub use approve_conversion::ApproveConversion;
pub(crate) use approve_conversion::__client_accounts_approve_conversion;
pub use claim_native_operator_expired::ClaimNativeOperatorExpired;
pub(crate) use claim_native_operator_expired::__client_accounts_claim_native_operator_expired;
pub use close_no_bitcoin::CloseNoBitcoin;
pub(crate) use close_no_bitcoin::__client_accounts_close_no_bitcoin;
pub use commit_bitcoin_to_native::CommitBitcoinToNative;
pub(crate) use commit_bitcoin_to_native::__client_accounts_commit_bitcoin_to_native;
pub use commit_header::CommitHeader;
pub(crate) use commit_header::__client_accounts_commit_header;
pub use commit_native_to_bitcoin::CommitNativeToBitcoin;
pub(crate) use commit_native_to_bitcoin::__client_accounts_commit_native_to_bitcoin;
pub use deposit_conversion::DepositConversion;
pub(crate) use deposit_conversion::__client_accounts_deposit_conversion;
pub use finalize_proof_cached::FinalizeProofCached;
pub(crate) use finalize_proof_cached::__client_accounts_finalize_proof_cached;
pub use initialize::Initialize;
pub(crate) use initialize::__client_accounts_initialize;
pub use operator_open_tunnel::OperatorOpenTunnel;
pub(crate) use operator_open_tunnel::__client_accounts_operator_open_tunnel;
pub use refund_conversion::RefundConversion;
pub(crate) use refund_conversion::__client_accounts_refund_conversion;
pub use refund_no_proof_native_to_bitcoin::RefundNoProofNativeToBitcoin;
pub(crate) use refund_no_proof_native_to_bitcoin::__client_accounts_refund_no_proof_native_to_bitcoin;
pub use remove_liquidity::RemoveLiquidity;
pub(crate) use remove_liquidity::__client_accounts_remove_liquidity;
pub use submit_proof_cache::SubmitProofCache;
pub(crate) use submit_proof_cache::__client_accounts_submit_proof_cache;
pub use update_network::UpdateNetwork;
pub(crate) use update_network::__client_accounts_update_network;

declare_id!("EsmGbkui9ZFC6Fch9J6xoyNRZSp6TwfvcjS88beP1Vem");

#[program]
pub mod ipow {
    use super::*;

    pub fn initialize(
        ctx: Context<Initialize>,
        operator: Pubkey,
        commit_fee_bps: u16,
    ) -> Result<()> {
        initialize::handler(ctx, operator, commit_fee_bps)
    }

    pub fn add_network(
        ctx: Context<AddNetwork>,
        network_id: u64,
        min_addr_len: u16,
        max_addr_len: u16,
    ) -> Result<()> {
        add_network::handler(ctx, network_id, min_addr_len, max_addr_len)
    }

    pub fn add_liquidity(ctx: Context<AddLiquidity>, amount: u64) -> Result<()> {
        add_liquidity::handler(ctx, amount)
    }

    pub fn remove_liquidity(ctx: Context<RemoveLiquidity>, amount: u64) -> Result<()> {
        remove_liquidity::handler(ctx, amount)
    }

    pub fn update_network(
        ctx: Context<UpdateNetwork>,
        _network_id: u64,
        min_addr_len: u16,
        max_addr_len: u16,
        is_active: bool,
    ) -> Result<()> {
        update_network::handler(ctx, _network_id, min_addr_len, max_addr_len, is_active)
    }

    pub fn commit_native_to_bitcoin(
        ctx: Context<CommitNativeToBitcoin>,
        native_amount: u64,
        bitcoin_amount: u64,
        network_id: u64,
        network_address: Vec<u8>,
        user_program: Vec<u8>,
    ) -> Result<()> {
        commit_native_to_bitcoin::handler(
            ctx,
            native_amount,
            bitcoin_amount,
            network_id,
            network_address,
            user_program,
        )
    }

    pub fn commit_bitcoin_to_native(
        ctx: Context<CommitBitcoinToNative>,
        bitcoin_amount: u64,
        native_amount: u64,
        network_id: u64,
        network_address: Vec<u8>,
        user_program: Vec<u8>,
    ) -> Result<()> {
        commit_bitcoin_to_native::handler(
            ctx,
            bitcoin_amount,
            native_amount,
            network_id,
            network_address,
            user_program,
        )
    }

    pub fn operator_open_tunnel(
        ctx: Context<OperatorOpenTunnel>,
        bitcoin_amount: u64,
        native_amount: u64,
        network_id: u64,
        dest_address: Pubkey,
        network_address: Vec<u8>,
        duty_window_seconds: i64,
        ipow_receive_program: Vec<u8>,
        locked_anchor_height: u64,
        program_hash: [u8; 32],
    ) -> Result<()> {
        operator_open_tunnel::handler(
            ctx,
            bitcoin_amount,
            native_amount,
            network_id,
            dest_address,
            network_address,
            duty_window_seconds,
            ipow_receive_program,
            locked_anchor_height,
            program_hash,
        )
    }

    pub fn approve_and_start_with_anchor(
        ctx: Context<ApproveConversion>,
        duty_window_seconds: i64,
        ipow_receive_program: Vec<u8>,
        program_hash: [u8; 32],
    ) -> Result<()> {
        approve_conversion::handler(
            ctx,
            duty_window_seconds,
            ipow_receive_program,
            program_hash,
        )
    }

    pub fn deposit_approved_conversion(ctx: Context<DepositConversion>) -> Result<()> {
        deposit_conversion::handler(ctx)
    }

    pub fn refund_if_not_approved(ctx: Context<RefundConversion>) -> Result<()> {
        refund_conversion::handler(ctx)
    }

    pub fn claim_native_after_operator_expired(
        ctx: Context<ClaimNativeOperatorExpired>,
    ) -> Result<()> {
        claim_native_operator_expired::handler(ctx)
    }

    pub fn refund_after_no_proof_native_to_bitcoin(
        ctx: Context<RefundNoProofNativeToBitcoin>,
    ) -> Result<()> {
        refund_no_proof_native_to_bitcoin::handler(ctx)
    }

    pub fn close_no_bitcoin_bitcoin_to_native(ctx: Context<CloseNoBitcoin>) -> Result<()> {
        close_no_bitcoin::handler(ctx)
    }

    pub fn commit_global_header(
        ctx: Context<CommitHeader>,
        header_80: [u8; 80],
        height: u64,
    ) -> Result<()> {
        commit_header::handler(ctx, header_80, height)
    }

    pub fn submit_bitcoin_proof_cache(
        ctx: Context<SubmitProofCache>,
        tx_raw: Vec<u8>,
        vout_index: u64,
        proof_block_height: u64,
        branch_le: Vec<[u8; 32]>,
        index: u64,
    ) -> Result<()> {
        submit_proof_cache::handler(
            ctx,
            tx_raw,
            vout_index,
            proof_block_height,
            branch_le,
            index,
        )
    }

    pub fn try_finalize_cached_proof(ctx: Context<FinalizeProofCached>) -> Result<()> {
        finalize_proof_cached::handler(ctx)
    }
}
