use anchor_lang::prelude::*;

/// Governance-settable parameters. Governance sets numbers; it never
/// judges whether a statement was true (DESIGN_V2 §6.7).
#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, InitSpace)]
pub struct FactoryParams {
    /// Composition: lamports of SOL backing one whole BETA.
    pub sol_per_unit: u64,
    /// Composition: gwei of ETH backing one whole BETA (checked on Ethereum).
    pub eth_gwei_per_unit: u64,
    /// v2 rate window — kept for layout compatibility; unused in v3
    /// (exposure is per-action escrow, DESIGN_V2 §7.5).
    pub window_blocks: u64,
    pub mint_cap_units_per_window: u64,
    /// A stalled anchor (preimage never revealed) may be skipped once the
    /// header carrying it is this old.
    pub t_skip_secs: i64,
    /// v3 challenge window: a queued MINT executes early only on an
    /// ATTEST, otherwise after this long if no VETO stands; escrows settle
    /// at its end (DESIGN_V2 §7). (Field position inherited from v2's
    /// `t_pause_secs`.)
    pub t_challenge_secs: i64,
    pub unbond_delay_secs: i64,
    pub min_operator_bond: u64,
    pub min_auditor_bond: u64,
    /// Slash per unit for a false statement about Solana; paid to the
    /// named user (false MINT) or to insurance (false RELEASE).
    pub comp_lamports_per_unit: u64,
    /// Slash for an auditor whose veto of a RELEASE was wrong; reward for
    /// one that was right (paid from insurance when funded).
    pub veto_slash_lamports: u64,
    pub veto_reward_lamports: u64,
    /// Share of any slash paid to whoever submitted the anchor.
    pub bounty_bps: u16,
}

#[account]
#[derive(InitSpace)]
pub struct FactoryConfig {
    pub governance: Pubkey,
    pub beta_mint: Pubkey,
    pub vault_bump: u8,
    pub mint_authority_bump: u8,
    pub bond_escrow_bump: u8,
    pub insurance_bump: u8,
    pub params: FactoryParams,
    pub paused: bool,

    /// Rate window: Bitcoin height the current window started at (from the
    /// header relay's tip when the first mint of the window executed) and
    /// units minted in it.
    pub window_start_height: u64,
    pub window_units: u64,

    /// Whole-BETA units currently outstanding — equals the ETH claims the
    /// Ethereum vault must honor.
    pub eth_claims_units: u64,
    /// Lamports in the vault backing outstanding BETA (excludes pending locks).
    pub reserve_lamports: u64,
    /// Lamports in the vault held for not-yet-minted pending locks.
    pub pending_lamports: u64,
    pub next_burn_id: u64,
}
