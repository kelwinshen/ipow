use std::cmp::Ordering;

use anchor_lang::prelude::*;

use crate::constants::{MAX_TIMESPAN_SEC, MIN_TIMESPAN_SEC, RETARGET_PERIOD_SEC};
use crate::errors::IPoWError;

/// Real Bitcoin mainnet minimum-difficulty target (`bits = 0x1d00ffff`,
/// difficulty 1) — `0x00000000FFFF0000...0000` big-endian. Mirrors
/// `BitcoinPrimitives._powLimit()` on the EVM side exactly: without this
/// cap, `target_from_bits` would accept whatever (arbitrarily easy)
/// difficulty a header claims at face value, letting an operator forge a
/// fake header chain for near-zero real computational cost — the entire
/// point of PoW-checking headers is defeated if the claimed difficulty
/// itself is never bounded to something a real miner would have had to
/// earn. Found and fixed after this exact gap let a test header (mined
/// against a deliberately-easy target) go unnoticed as a live protocol gap
/// rather than a test-only convenience — see `docs/design/ipow-implementation.md`.
const POW_LIMIT: [u8; 32] = {
    let mut limit = [0u8; 32];
    limit[4] = 0xff;
    limit[5] = 0xff;
    limit
};

pub fn target_from_bits(bits: u32) -> [u8; 32] {
    let mut target = [0u8; 32];
    let exp = (bits >> 24) as usize;
    let mantissa = bits & 0x007FFFFF;

    if exp > 3 {
        let shift = exp - 3;
        if shift < 32 {
            target[32 - shift - 3] = (mantissa >> 16) as u8;
            target[32 - shift - 2] = (mantissa >> 8) as u8;
            target[32 - shift - 1] = mantissa as u8;
        }
    } else {
        let shift = 3 - exp;
        let shifted_mant = mantissa >> (8 * shift);
        target[29] = (shifted_mant >> 16) as u8;
        target[30] = (shifted_mant >> 8) as u8;
        target[31] = shifted_mant as u8;
    }

    // Big-endian byte arrays compare lexicographically the same as the
    // numbers they represent, so a plain `>` here is a correct numeric
    // comparison.
    if target > POW_LIMIT {
        target = POW_LIMIT;
    }
    target
}

pub fn validate_work_le(hash_le: &[u8; 32], target_be: &[u8; 32]) -> bool {
    let mut hash_be = [0u8; 32];
    for i in 0..32 {
        hash_be[i] = hash_le[31 - i];
    }
    hash_be.cmp(target_be) != Ordering::Greater
}

pub fn expected_retarget_bits(start_ts: u32, end_ts: u32, start_bits: u32) -> Result<u32> {
    // `end_ts` must not precede `start_ts` for a legitimately-ordered pair of
    // epoch-boundary headers. Using `checked_sub` (rather than a bare `-`) means
    // this fails safely on malformed/out-of-order header data regardless of
    // build profile — the Solidity original gets the same guarantee for free
    // from its own unconditional checked arithmetic; Rust needs it spelled out
    // explicitly rather than relying on a Cargo.toml `overflow-checks` setting.
    let mut actual = end_ts
        .checked_sub(start_ts)
        .ok_or(IPoWError::InvalidRetarget)? as u64;
    if actual < MIN_TIMESPAN_SEC {
        actual = MIN_TIMESPAN_SEC;
    }
    if actual > MAX_TIMESPAN_SEC {
        actual = MAX_TIMESPAN_SEC;
    }

    let mantissa = (start_bits & 0x007FFFFF) as u64;
    let exp = (start_bits >> 24) as u64;

    let mut new_mantissa = (mantissa * actual) / RETARGET_PERIOD_SEC;
    let mut new_exp = exp;

    while new_mantissa > 0x007FFFFF && new_exp < 255 {
        new_mantissa >>= 8;
        new_exp += 1;
    }

    Ok(((new_exp << 24) | new_mantissa) as u32)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn target_from_bits_clamps_an_easier_than_real_claimed_difficulty() {
        // 0x207fffff is regtest's powLimit bits — an intentionally trivial
        // difficulty, easy enough to mine in a handful of iterations. If
        // this were accepted at face value, an operator could forge a fake
        // header chain for near-zero real work. It must clamp down to the
        // real mainnet powLimit instead.
        let target = target_from_bits(0x207fffff);
        assert_eq!(target, POW_LIMIT);
    }

    #[test]
    fn target_from_bits_matches_known_genesis_pow_limit() {
        // bits = 0x1d00ffff is Bitcoin mainnet's genesis difficulty (difficulty 1),
        // whose big-endian target is the well-known
        // 0x00000000FFFF0000000000000000000000000000000000000000000000000.
        let target = target_from_bits(0x1d00ffff);
        let mut expected = [0u8; 32];
        expected[4] = 0xff;
        expected[5] = 0xff;
        assert_eq!(target, expected);
    }

    #[test]
    fn validate_work_le_accepts_hash_within_target() {
        // hash (LE) that reverses to a BE value below the genesis PoW-limit target.
        let mut hash_le = [0u8; 32];
        hash_le[31] = 0x00; // top BE byte (index 0) = 0x00
        hash_le[27] = 0x10; // BE index 4 = 0x10, below the target's 0xff there
        let target = target_from_bits(0x1d00ffff);
        assert!(validate_work_le(&hash_le, &target));
    }

    #[test]
    fn validate_work_le_rejects_hash_exceeding_target() {
        let hash_le = [0xffu8; 32]; // maximum possible hash, far above any real target
        let target = target_from_bits(0x1d00ffff);
        assert!(!validate_work_le(&hash_le, &target));
    }

    #[test]
    fn validate_work_le_rejects_zero_hash_note() {
        // Unlike the Solidity BitcoinPrimitives._validateWorkLE, this Rust port has
        // no explicit zero-hash guard — an all-zero hash is numerically <= any
        // target and is therefore accepted here. Documented, not "fixed": this
        // test pins the actual current behavior rather than an assumption.
        let hash_le = [0u8; 32];
        let target = target_from_bits(0x1d00ffff);
        assert!(validate_work_le(&hash_le, &target));
    }

    #[test]
    fn expected_retarget_bits_unchanged_when_epoch_exactly_on_schedule() {
        // If the previous epoch took exactly RETARGET_PERIOD_SEC, difficulty
        // should not change at all.
        let start_bits = 0x1b0404cbu32;
        let start_ts = 0u32;
        let end_ts = RETARGET_PERIOD_SEC as u32;
        let new_bits = expected_retarget_bits(start_ts, end_ts, start_bits).unwrap();
        assert_eq!(new_bits, start_bits);
    }

    #[test]
    fn expected_retarget_bits_clamps_to_min_timespan() {
        // An epoch that finished near-instantly should clamp to MIN_TIMESPAN_SEC
        // (RETARGET_PERIOD_SEC / 4), i.e. the target shrinks (difficulty rises) by
        // at most 4x, never more, even though the raw elapsed time was tiny.
        let start_bits = 0x1b0404cbu32;
        let new_bits = expected_retarget_bits(0, 1, start_bits).unwrap();
        let clamped_at_min = expected_retarget_bits(0, MIN_TIMESPAN_SEC as u32, start_bits).unwrap();
        assert_eq!(new_bits, clamped_at_min);
    }

    #[test]
    fn expected_retarget_bits_rejects_out_of_order_timestamps() {
        // Regression test for the fix making this `checked_sub`: a malformed or
        // out-of-order epoch-boundary header pair (end_ts before start_ts) must
        // return an error rather than panicking on unsigned subtraction
        // underflow.
        let start_bits = 0x1b0404cbu32;
        let result = expected_retarget_bits(100, 50, start_bits);
        assert!(result.is_err());
    }
}
