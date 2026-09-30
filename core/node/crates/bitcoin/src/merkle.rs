//! Merkle proofs of a transaction in a block.

use crate::sha256d;

/// The Merkle root of `txids` (header byte order), and the proof of the one
/// at `index`: its siblings from the leaf up. Bitcoin repeats the last hash
/// of a level with an odd count.
pub fn root_and_proof(txids: &[[u8; 32]], index: usize) -> ([u8; 32], Vec<[u8; 32]>) {
    assert!(index < txids.len(), "the index is outside the block");
    let mut level = txids.to_vec();
    let mut siblings = vec![];
    let mut at = index;
    while level.len() > 1 {
        if level.len() % 2 == 1 {
            level.push(*level.last().unwrap());
        }
        siblings.push(level[at ^ 1]);
        level = level
            .chunks(2)
            .map(|pair| {
                let mut buf = [0u8; 64];
                buf[..32].copy_from_slice(&pair[0]);
                buf[32..].copy_from_slice(&pair[1]);
                sha256d(&buf)
            })
            .collect();
        at /= 2;
    }
    (level[0], siblings)
}

/// Rebuilds the root from a leaf and its siblings.
pub fn root_from(leaf: [u8; 32], siblings: &[[u8; 32]], mut index: u64) -> [u8; 32] {
    let mut h = leaf;
    for s in siblings {
        let mut buf = [0u8; 64];
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

    #[test]
    fn every_proof_rebuilds_the_root() {
        for n in 1..12usize {
            let txids: Vec<[u8; 32]> = (0..n).map(|i| sha256d(&[i as u8])).collect();
            for i in 0..n {
                let (root, siblings) = root_and_proof(&txids, i);
                assert_eq!(root_from(txids[i], &siblings, i as u64), root);
            }
        }
    }
}
