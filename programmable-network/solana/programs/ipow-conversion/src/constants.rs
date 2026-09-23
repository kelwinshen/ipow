pub const BPS_DENOM: u64 = 10_000;
pub const RESERVE_MARGIN_BPS: u64 = 10_000;
pub const DEPOSIT_BLOCKS_WINDOW: u64 = 10;
pub const PROOF_BLOCKS_WINDOW: u64 = 40;
pub const DIFF_PERIOD: u64 = 2016;

/// Same rationale as `ipow-message-relay`'s own `CLAIM_WINDOW_SEC`/
/// `CLAIM_QUIET_PERIOD_SEC`: a windowed staked auction needs an outer cap
/// on how long claiming stays open at all, and a quiet period so an
/// uncontested claim resolves promptly while a contested one keeps
/// extending as better stakes land.
pub const CLAIM_WINDOW_SEC: i64 = 15 * 60;
/// Kept short (1 minute) rather than message-relay's original 5 minutes —
/// Kelwin's explicit call for this program: an uncontested claim should
/// resolve quickly, not sit through a long quiet period.
pub const CLAIM_QUIET_PERIOD_SEC: i64 = 60;
