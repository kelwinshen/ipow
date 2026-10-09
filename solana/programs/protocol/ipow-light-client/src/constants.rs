//! Rules fixed by the protocol. The same numbers as `iPoWLightClient.sol`.

/// D44: an epoch start is the first 6 blocks of an epoch.
pub const EPOCH_START_BLOCKS: usize = 6;
/// D39: an anchor may be at most 2 hours old.
pub const MAX_ANCHOR_AGE: i64 = 2 * 60 * 60;
/// D57, D45: 4 weeks.
pub const MAX_EPOCH_START_AGE: i64 = 4 * 7 * 24 * 60 * 60;
/// Bitcoin's own rule: a block's time is at most 2 hours ahead.
pub const MAX_FUTURE_TIME: i64 = 2 * 60 * 60;
/// D70: a window is at most 100 blocks, so no walk is longer.
pub const MAX_WALK: u32 = 100;
/// D45 compares epoch starts by the day of their first block.
pub const COMPARE_STEP: i64 = 24 * 60 * 60;
/// D101: a block or an epoch start may be closed 8 weeks after it was stored,
/// and its rent goes back to whoever stored it. No rule ever needs one that
/// old: an anchor is at most 2 hours old, the questions for a parent go back
/// at most 2,016 blocks (about 4 weeks when Bitcoin is slow), and the lock of
/// a job runs at most about 10.5 days after its anchor.
pub const RETENTION: i64 = 8 * 7 * 24 * 60 * 60;
/// How many days the day table keeps: more than the 29 a comparison reads.
pub const DAY_SLOTS: usize = 32;
/// How many headers one `extend` call takes. A Solana transaction is at
/// most 1,232 bytes. With 7 headers, an instruction that raises the compute
/// limit (extending 8 blocks uses about 240,000 units, more than the default
/// 200,000) and one that sets a priority fee, it is about 1,160 bytes.
pub const MAX_EXTEND: usize = 7;

/// D38: minimum difficulty 2^45: the target 0xFFFF * 2^163, big-endian.
pub const MIN_DIFFICULTY_TARGET: [u8; 32] = {
    let mut t = [0u8; 32];
    // 163 = 20 * 8 + 3, so 0xFFFF << 163 = 0x7FFF8 << 160.
    t[32 - 20 - 3] = 0x07;
    t[32 - 20 - 2] = 0xFF;
    t[32 - 20 - 1] = 0xF8;
    t
};
/// The easiest target Bitcoin allows (difficulty 1): 0xFFFF * 2^208.
pub const POW_LIMIT: [u8; 32] = {
    let mut t = [0u8; 32];
    t[4] = 0xFF;
    t[5] = 0xFF;
    t
};

pub const CONFIG_SEED: &[u8] = b"config";
pub const DAYS_SEED: &[u8] = b"days";
pub const NODE_SEED: &[u8] = b"node";
pub const EPOCH_START_SEED: &[u8] = b"epoch_start";
pub const WALK_SEED: &[u8] = b"walk";
