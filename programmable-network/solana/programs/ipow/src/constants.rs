pub const BPS_DENOM: u64 = 10_000;
pub const APPROVAL_WINDOW_SEC: i64 = 15 * 60;
pub const DEPOSIT_BLOCKS_WINDOW: u64 = 10;
pub const PROOF_BLOCKS_WINDOW: u64 = 40;
pub const RESERVE_MARGIN_BPS: u64 = 10_000;
pub const CONFIRMATIONS_REQUIRED: u64 = 1;
pub const DIFF_PERIOD: u64 = 2016;
pub const RETARGET_PERIOD_SEC: u64 = 14 * 24 * 60 * 60;
pub const MIN_TIMESPAN_SEC: u64 = RETARGET_PERIOD_SEC / 4;
pub const MAX_TIMESPAN_SEC: u64 = RETARGET_PERIOD_SEC * 4;
/// `commit_global_header` is operator-first: anyone may extend the chain by
/// one header once the current tip has sat unextended for this long. Keeps
/// the operator's confirmation policy in the fast path while making header
/// withholding impossible for longer than this (docs/DESIGN_V2.md §6.9).
pub const PERMISSIONLESS_HEADER_DELAY_SEC: i64 = 30 * 60;
