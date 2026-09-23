use anchor_lang::prelude::*;

/// One per `token_mint` (native SOL uses `Pubkey::default()`) — replaces
/// `ipow::GlobalState`'s single `total_locked_deposits`/`total_reserved_
/// native`/`total_held_commit_fees` fields, which only worked because every
/// conversion moved the same native SOL. Also the escrow PDA's own bump:
/// `seeds = [b"escrow", token_mint.as_ref()]` is this program's real vault
/// (a `SystemAccount` when `token_mint` is default, an SPL token-account
/// authority otherwise) — kept per-mint for the same reason liquidity is
/// tracked per-mint below.
#[account]
#[derive(InitSpace)]
pub struct Pool {
    pub token_mint: Pubkey,
    pub escrow_bump: u8,

    /// native->bitcoin: sum of real user deposits currently held pending
    /// proof or refund.
    pub total_locked_deposits: u64,
    /// bitcoin->native: sum of `native_amount` self-escrowed by claimants
    /// (`propose_claim_conversion`) for in-flight, not-yet-paid-out
    /// conversions of this mint — a pure aggregate for introspection, not
    /// load-bearing for correctness: each conversion's own escrow is backed
    /// by that specific claimant's own transfer, verified at escrow time via
    /// `transfer_value_in`'s balance-delta measurement, not by this counter.
    pub total_reserved: u64,
    pub total_held_commit_fees: u64,
}
