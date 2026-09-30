//! Stateless Bitcoin math for the light client, the same rules as
//! `contracts/protocol/BitcoinHeaderLib.sol` on Ethereum.
//!
//! Numbers of 256 bits are `U256`, four 64-bit limbs, least significant
//! first. Hashes are kept in the byte order Bitcoin uses inside a header.

use sha2::{Digest, Sha256};

pub const HEADER_LENGTH: usize = 80;
pub const EPOCH_BLOCKS: u32 = 2016;
/// The time Bitcoin aims for per epoch.
pub const TARGET_TIMESPAN: u64 = 14 * 24 * 60 * 60;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Default)]
pub struct U256(pub [u64; 4]);

impl U256 {
    pub const ZERO: U256 = U256([0, 0, 0, 0]);
    pub const MAX: U256 = U256([u64::MAX; 4]);

    pub fn from_u64(v: u64) -> Self {
        U256([v, 0, 0, 0])
    }

    /// From 32 bytes, most significant first.
    pub fn from_be_bytes(b: &[u8; 32]) -> Self {
        let mut limbs = [0u64; 4];
        for i in 0..4 {
            let mut chunk = [0u8; 8];
            chunk.copy_from_slice(&b[24 - 8 * i..32 - 8 * i]);
            limbs[i] = u64::from_be_bytes(chunk);
        }
        U256(limbs)
    }

    pub fn to_be_bytes(&self) -> [u8; 32] {
        let mut out = [0u8; 32];
        for i in 0..4 {
            out[24 - 8 * i..32 - 8 * i].copy_from_slice(&self.0[i].to_be_bytes());
        }
        out
    }

    /// From a hash as it sits in a header: the byte order is reversed.
    pub fn from_le_bytes(b: &[u8; 32]) -> Self {
        let mut be = *b;
        be.reverse();
        Self::from_be_bytes(&be)
    }

    pub fn is_zero(&self) -> bool {
        self.0 == [0, 0, 0, 0]
    }

    /// Ordering of numbers: the most significant limb first.
    fn cmp_num(&self, other: &Self) -> core::cmp::Ordering {
        for i in (0..4).rev() {
            match self.0[i].cmp(&other.0[i]) {
                core::cmp::Ordering::Equal => continue,
                o => return o,
            }
        }
        core::cmp::Ordering::Equal
    }

    pub fn lt(&self, other: &Self) -> bool {
        self.cmp_num(other) == core::cmp::Ordering::Less
    }

    pub fn gt(&self, other: &Self) -> bool {
        self.cmp_num(other) == core::cmp::Ordering::Greater
    }

    pub fn checked_add(&self, other: &Self) -> Option<Self> {
        let mut out = [0u64; 4];
        let mut carry = 0u128;
        for i in 0..4 {
            let s = self.0[i] as u128 + other.0[i] as u128 + carry;
            out[i] = s as u64;
            carry = s >> 64;
        }
        if carry != 0 {
            None
        } else {
            Some(U256(out))
        }
    }

    pub fn wrapping_sub(&self, other: &Self) -> Self {
        let mut out = [0u64; 4];
        let mut borrow = 0i128;
        for i in 0..4 {
            let d = self.0[i] as i128 - other.0[i] as i128 - borrow;
            if d < 0 {
                out[i] = (d + (1i128 << 64)) as u64;
                borrow = 1;
            } else {
                out[i] = d as u64;
                borrow = 0;
            }
        }
        U256(out)
    }

    /// `self * m`, or `None` when it does not fit.
    pub fn checked_mul_u64(&self, m: u64) -> Option<Self> {
        let mut out = [0u64; 4];
        let mut carry = 0u128;
        for i in 0..4 {
            let p = self.0[i] as u128 * m as u128 + carry;
            out[i] = p as u64;
            carry = p >> 64;
        }
        if carry != 0 {
            None
        } else {
            Some(U256(out))
        }
    }

    pub fn div_u64(&self, d: u64) -> Self {
        self.div_rem_u64(d).0
    }

    pub fn div_rem_u64(&self, d: u64) -> (Self, u64) {
        let mut out = [0u64; 4];
        let mut rem = 0u128;
        for i in (0..4).rev() {
            let cur = (rem << 64) | self.0[i] as u128;
            out[i] = (cur / d as u128) as u64;
            rem = cur % d as u128;
        }
        (U256(out), rem as u64)
    }

    fn shl1(&self) -> Self {
        let mut out = [0u64; 4];
        for i in (0..4).rev() {
            out[i] = self.0[i] << 1;
            if i > 0 {
                out[i] |= self.0[i - 1] >> 63;
            }
        }
        U256(out)
    }

    fn bit(&self, n: usize) -> bool {
        (self.0[n / 64] >> (n % 64)) & 1 == 1
    }

    /// `self / d` by long division. `d` must not be zero.
    pub fn div(&self, d: &Self) -> Self {
        let mut q = U256::ZERO;
        let mut r = U256::ZERO;
        for n in (0..256).rev() {
            r = r.shl1();
            if self.bit(n) {
                r.0[0] |= 1;
            }
            if !r.lt(d) {
                r = r.wrapping_sub(d);
                q.0[n / 64] |= 1 << (n % 64);
            }
        }
        q
    }

    /// The number of bytes needed to write this number.
    fn byte_len(&self) -> u32 {
        let be = self.to_be_bytes();
        for (i, b) in be.iter().enumerate() {
            if *b != 0 {
                return (32 - i) as u32;
            }
        }
        0
    }

    /// `self >> (8 * n)`, or `self << (8 * -n)` when n is negative.
    fn shift_bytes(&self, n: i32) -> Self {
        let be = self.to_be_bytes();
        let mut out = [0u8; 32];
        for i in 0..32i32 {
            let src = i - n;
            if (0..32).contains(&src) {
                out[i as usize] = be[src as usize];
            }
        }
        U256::from_be_bytes(&out)
    }
}

#[derive(Debug, PartialEq, Eq)]
pub enum BitsError {
    Invalid,
}

/// Compact "nBits" -> 256-bit target. Rejects what Bitcoin rejects: a
/// negative value, zero, and a value that does not fit 256 bits.
pub fn target_from_bits(bits: u32) -> Result<U256, BitsError> {
    let exp = (bits >> 24) as i32;
    let mant = (bits & 0x007F_FFFF) as u64;
    if bits & 0x0080_0000 != 0 || mant == 0 {
        return Err(BitsError::Invalid);
    }
    if exp > 34 || (mant > 0xFF && exp > 33) || (mant > 0xFFFF && exp > 32) {
        return Err(BitsError::Invalid);
    }
    let target = U256::from_u64(mant).shift_bytes(3 - exp);
    if target.is_zero() {
        return Err(BitsError::Invalid);
    }
    Ok(target)
}

/// 256-bit target -> compact "nBits", the way Bitcoin's GetCompact does it.
pub fn bits_from_target(target: &U256) -> u32 {
    let mut size = target.byte_len();
    let mut compact = target.shift_bytes(size as i32 - 3).0[0];
    if compact & 0x0080_0000 != 0 {
        compact >>= 8;
        size += 1;
    }
    (compact as u32) | (size << 24)
}

/// The difficulty of the first block of a new epoch, by Bitcoin's rule.
pub fn retarget(last_bits: u32, first_time: u32, last_time: u32, pow_limit: &U256) -> Result<u32, BitsError> {
    let mut timespan = (last_time as u64).saturating_sub(first_time as u64);
    timespan = timespan.clamp(TARGET_TIMESPAN / 4, TARGET_TIMESPAN * 4);
    let target = target_from_bits(last_bits)?;
    // target * timespan / TARGET_TIMESPAN without a 512-bit product: split
    // the target into a quotient and a remainder of TARGET_TIMESPAN. When the
    // result does not fit 256 bits it is above any limit anyway.
    let (q, r) = target.div_rem_u64(TARGET_TIMESPAN);
    let exact = ((r as u128 * timespan as u128) / TARGET_TIMESPAN as u128) as u64;
    let next = q
        .checked_mul_u64(timespan)
        .and_then(|v| v.checked_add(&U256::from_u64(exact)));
    let next = match next {
        Some(v) if !v.gt(pow_limit) => v,
        _ => *pow_limit,
    };
    Ok(bits_from_target(&next))
}

/// The mining work one block at this target stands for: 2^256 / (target + 1).
pub fn work(target: &U256) -> U256 {
    // (~target / (target + 1)) + 1, as Bitcoin computes it.
    let not_target = U256::MAX.wrapping_sub(target);
    let plus_one = target.checked_add(&U256::from_u64(1)).unwrap_or(U256::MAX);
    not_target.div(&plus_one).checked_add(&U256::from_u64(1)).unwrap_or(U256::MAX)
}

pub fn sha256d(data: &[u8]) -> [u8; 32] {
    let once = Sha256::digest(data);
    let twice = Sha256::digest(once);
    let mut out = [0u8; 32];
    out.copy_from_slice(&twice);
    out
}

pub struct Header<'a>(pub &'a [u8]);

impl<'a> Header<'a> {
    pub fn hash(&self) -> [u8; 32] {
        sha256d(self.0)
    }
    pub fn prev(&self) -> [u8; 32] {
        self.0[4..36].try_into().unwrap()
    }
    pub fn merkle_root(&self) -> [u8; 32] {
        self.0[36..68].try_into().unwrap()
    }
    pub fn time(&self) -> u32 {
        u32::from_le_bytes(self.0[68..72].try_into().unwrap())
    }
    pub fn bits(&self) -> u32 {
        u32::from_le_bytes(self.0[72..76].try_into().unwrap())
    }
}

/// Rebuilds the Merkle root from a leaf and its siblings, all in header
/// byte order.
pub fn merkle_root_from(leaf: [u8; 32], siblings: &[[u8; 32]], mut index: u64) -> [u8; 32] {
    let mut h = leaf;
    let mut buf = [0u8; 64];
    for s in siblings {
        if index % 2 == 0 {
            buf[..32].copy_from_slice(&h);
            buf[32..].copy_from_slice(s);
        } else {
            buf[..32].copy_from_slice(s);
            buf[32..].copy_from_slice(&h);
        }
        h = sha256d(&buf);
        index /= 2;
    }
    h
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pow_limit() -> U256 {
        U256::from_u64(0xFFFF).shift_bytes(-26)
    }

    #[test]
    fn compact_round_trips_real_values() {
        for bits in [0x17021ec5u32, 0x1702355e, 0x1d00ffff, 0x207fffff, 0x20400000] {
            let t = target_from_bits(bits).unwrap();
            assert_eq!(bits_from_target(&t), bits);
        }
    }

    #[test]
    fn compact_moves_a_mantissa_that_would_use_the_sign_bit() {
        assert_eq!(bits_from_target(&U256::from_u64(0x80_0000)), 0x0400_8000);
    }

    #[test]
    fn compact_rejects_negative_zero_and_oversized() {
        assert!(target_from_bits(0x1d80ffff).is_err());
        assert!(target_from_bits(0x1d000000).is_err());
        assert!(target_from_bits(0x2300ffff).is_err());
    }

    #[test]
    fn retarget_of_epoch_480_matches_bitcoin() {
        // Epoch 479: first block 965664 at 1788640367, last block 967679 at
        // 1789801627, bits 0x1702355e. Epoch 480 has bits 0x17021ec5.
        assert_eq!(retarget(0x1702355e, 1788640367, 1789801627, &pow_limit()).unwrap(), 0x17021ec5);
    }

    #[test]
    fn retarget_halves_the_target_when_blocks_came_twice_as_fast() {
        let easy = 0x207fffff;
        let limit = target_from_bits(easy).unwrap();
        assert_eq!(retarget(easy, 0, (TARGET_TIMESPAN / 2) as u32, &limit).unwrap(), 0x203fffff);
    }

    #[test]
    fn work_of_difficulty_one_is_about_2_to_the_32() {
        let w = work(&pow_limit());
        assert_eq!(w, U256::from_u64(0x1_0001_0001));
    }

    #[test]
    fn division_matches_small_numbers() {
        assert_eq!(U256::from_u64(1000).div(&U256::from_u64(7)), U256::from_u64(142));
    }
}
