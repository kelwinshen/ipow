use anchor_lang::prelude::*;

/// One extra token in a `token->bitcoin` bundle, beyond the primary
/// `token_mint`/`native_amount` slot — see `Conversion.extra_tokens`.
#[derive(AnchorSerialize, AnchorDeserialize, Clone, InitSpace)]
pub struct TokenAmount {
    pub mint: Pubkey,
    pub amount: u64,
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, PartialEq, Eq, InitSpace)]
pub enum ConversionStatus {
    None,
    /// User committed; claim auction is open (`propose_claim_conversion`).
    Committed,
    /// A claim won the auction and was locked in (`finalize_claim_conversion`).
    /// Same slot the old design's `Approved` occupied — duty window running.
    Approved,
    /// native->bitcoin only: user has deposited real value into escrow.
    Deposited,
    Completed,
    Refunded,
}

/// See `docs/DESIGN_V2.md` for the full design this implements. Two
/// structural changes from the version that still lives, unmodified,
/// inside `ipow`:
///
/// 1. No single fixed `global_state.operator` — `responsible_operator` is
///    won through a windowed staked auction (`propose_claim_conversion` /
///    `finalize_claim_conversion` / `reclaim_expired_conversion`), the same
///    mechanics already proven on `ipow-message-relay`'s
///    `MessageCommitment`. See this struct's own auction fields below.
/// 2. `token_mint` generalizes `native_amount` from "always native SOL" to
///    "native SOL if `Pubkey::default()`, else this SPL mint" — every
///    instruction that moves `native_amount` branches on this once.
#[account]
#[derive(InitSpace)]
pub struct Conversion {
    pub tx_id: u64,
    pub user: Pubkey,
    pub is_native_to_bitcoin: bool,
    /// `Pubkey::default()` = native SOL; otherwise the SPL mint this
    /// conversion's `native_amount` is denominated in.
    pub token_mint: Pubkey,

    #[max_len(80)]
    pub user_program: Vec<u8>,
    #[max_len(80)]
    pub ipow_receive_program: Vec<u8>,
    #[max_len(64)]
    pub network_address: Vec<u8>,
    pub network_id: u64,

    pub native_amount: u64,
    pub bitcoin_amount: u64,
    pub commit_fee: u64,
    pub reserved_native: u64,

    /// `token->bitcoin` only — up to 3 additional (mint, amount) pairs
    /// beyond the primary `token_mint`/`native_amount` slot, settled by
    /// the same single `bitcoin_amount` proof. Always empty for
    /// `bitcoin->token` and for any single-token `token->bitcoin`
    /// conversion — every instruction's existing behavior is unchanged
    /// when it is.
    #[max_len(3)]
    pub extra_tokens: Vec<TokenAmount>,

    pub created_at: i64,
    pub deposited_at: i64,
    pub operator_duty_expires_at: i64,

    pub status: ConversionStatus,

    pub window_started: bool,
    pub window_start_height: u64,
    pub epoch_start_height: u64,

    pub proof_verified: bool,
    pub proof_txid_le: [u8; 32],
    pub proof_block_height: u64,

    // --- Auction fields, mirroring `ipow-message-relay`'s `MessageCommitment` ---
    /// `Pubkey::default()` until `finalize_claim_conversion` locks in a winner.
    pub responsible_operator: Pubkey,
    /// What the current/winning claimant actually staked.
    pub staked_bond: u64,
    /// Minimum stake to claim; escalates on forfeiture via
    /// `reclaim_expired_conversion`, same rule as `MessageCommitment.
    /// required_bond`.
    pub required_bond: u64,
    /// Start of the current claim window (reset on every reopen).
    pub claim_started_at: i64,
    /// Timestamp of the most recently accepted claim — `finalize_claim_
    /// conversion`'s quiet-period check counts from this.
    pub last_claim_at: i64,
    /// Duty window length the winning claimant commits to, in seconds —
    /// supplied as part of their claim (like the old design's caller-supplied
    /// `duty_window_seconds`, now part of the bid rather than fixed by the
    /// approving operator).
    pub duty_window_seconds: i64,
}
