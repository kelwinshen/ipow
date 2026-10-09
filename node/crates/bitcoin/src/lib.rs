//! Bitcoin for the iPoW node. Hashes are in the byte order Bitcoin uses
//! inside a header and a transaction; explorers print them reversed.

pub mod explorer;
pub mod memory;
pub mod merkle;
pub mod tx;
pub mod view;
pub mod wallet;

use sha2::{Digest, Sha256};

pub fn sha256d(data: &[u8]) -> [u8; 32] {
    let once = Sha256::digest(data);
    Sha256::digest(once).into()
}

/// Reverses a hash between header byte order and the order explorers print.
pub fn reversed(h: [u8; 32]) -> [u8; 32] {
    let mut r = h;
    r.reverse();
    r
}

/// The fields of an 80-byte header.
pub struct Header<'a>(pub &'a [u8; 80]);

impl Header<'_> {
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
