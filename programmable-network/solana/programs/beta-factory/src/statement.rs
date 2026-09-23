use anchor_lang::prelude::*;
use sha2::{Digest, Sha256};

use crate::constants::*;
use crate::errors::FactoryError;

/// Canonical statement encodings, identical on Solana and every reserve
/// chain so `sha256(bytes)` matches everywhere (DESIGN_V2 §6.3, MINT
/// generalized for compositions in §8.4). All integers big-endian.
///
/// MINT    = 0x01 | composition_id u64 | component_index u8 | lock_id u64 | sol_user 32 | nonce u64 | units u64 | deadline i64  (74 bytes)
/// RELEASE = 0x02 | lock_id u64 | burn_id u64 | to 20 | units u64                                                               (45 bytes)
/// VETO    = 0x03 | target_party_id 32 | target_txid_le 32 (all-zero = dead-veto)                                                (65 bytes)
/// CANCEL  = 0x04 | lock_id u64                                                                                                  (9 bytes)
///
/// MINT is generalized for compositions (§8.4); RELEASE and CANCEL are
/// not yet (§8.6, "redeem generalizes the same way" — designed, not
/// built this pass) and keep the single-component shape §7 already has,
/// just with `eth_lock_id` renamed `lock_id` since it was never really
/// Ethereum-specific. MINT is NOT YET matched on any reserve-chain vault
/// (§8.9 step 2) — this is the Solana hub-side half; a reserve chain's
/// own contract needs the equivalent update before any multi-component
/// mint can complete end to end.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Statement {
    Mint {
        composition_id: u64,
        component_index: u8,
        lock_id: u64,
        sol_user: Pubkey,
        nonce: u64,
        units: u64,
        deadline: i64,
    },
    Release {
        lock_id: u64,
        burn_id: u64,
        to_eth: [u8; 20],
        units: u64,
    },
    Veto {
        target_party_id: [u8; 32],
        target_txid_le: [u8; 32],
    },
    Cancel {
        lock_id: u64,
    },
    /// v3: "that MINT/RELEASE is true, and I escrow for it" (33 bytes).
    Attest {
        target_txid_le: [u8; 32],
    },
    /// v3: "that MINT/RELEASE is true; lift the veto" (33 bytes).
    Clear {
        target_txid_le: [u8; 32],
    },
    /// v3: "that operator is not dead on Ethereum; un-pause" (33 bytes).
    Alive {
        target_party_id: [u8; 32],
    },
}

fn u64_at(b: &[u8], o: usize) -> u64 {
    u64::from_be_bytes(b[o..o + 8].try_into().unwrap())
}

impl Statement {
    pub fn kind(&self) -> u8 {
        match self {
            Statement::Mint { .. } => KIND_MINT,
            Statement::Release { .. } => KIND_RELEASE,
            Statement::Veto { .. } => KIND_VETO,
            Statement::Cancel { .. } => KIND_CANCEL,
            Statement::Attest { .. } => KIND_ATTEST,
            Statement::Clear { .. } => KIND_CLEAR,
            Statement::Alive { .. } => KIND_ALIVE,
        }
    }

    pub fn hash(bytes: &[u8]) -> [u8; 32] {
        let mut out = [0u8; 32];
        out.copy_from_slice(&Sha256::digest(bytes));
        out
    }

    pub fn decode(b: &[u8]) -> Result<Self> {
        require!(!b.is_empty(), FactoryError::MalformedStatement);
        match b[0] {
            KIND_MINT => {
                require!(b.len() == 74, FactoryError::MalformedStatement);
                Ok(Statement::Mint {
                    composition_id: u64_at(b, 1),
                    component_index: b[9],
                    lock_id: u64_at(b, 10),
                    sol_user: Pubkey::new_from_array(b[18..50].try_into().unwrap()),
                    nonce: u64_at(b, 50),
                    units: u64_at(b, 58),
                    deadline: u64_at(b, 66) as i64,
                })
            }
            KIND_RELEASE => {
                require!(b.len() == 45, FactoryError::MalformedStatement);
                let mut to_eth = [0u8; 20];
                to_eth.copy_from_slice(&b[17..37]);
                Ok(Statement::Release {
                    lock_id: u64_at(b, 1),
                    burn_id: u64_at(b, 9),
                    to_eth,
                    units: u64_at(b, 37),
                })
            }
            KIND_VETO => {
                require!(b.len() == 65, FactoryError::MalformedStatement);
                let mut target_party_id = [0u8; 32];
                target_party_id.copy_from_slice(&b[1..33]);
                let mut target_txid_le = [0u8; 32];
                target_txid_le.copy_from_slice(&b[33..65]);
                Ok(Statement::Veto {
                    target_party_id,
                    target_txid_le,
                })
            }
            KIND_CANCEL => {
                require!(b.len() == 9, FactoryError::MalformedStatement);
                Ok(Statement::Cancel {
                    lock_id: u64_at(b, 1),
                })
            }
            KIND_ATTEST | KIND_CLEAR | KIND_ALIVE => {
                require!(b.len() == 33, FactoryError::MalformedStatement);
                let mut t = [0u8; 32];
                t.copy_from_slice(&b[1..33]);
                Ok(match b[0] {
                    KIND_ATTEST => Statement::Attest { target_txid_le: t },
                    KIND_CLEAR => Statement::Clear { target_txid_le: t },
                    _ => Statement::Alive { target_party_id: t },
                })
            }
            _ => Err(error!(FactoryError::MalformedStatement)),
        }
    }

    pub fn encode(&self) -> Vec<u8> {
        let mut v = Vec::with_capacity(65);
        v.push(self.kind());
        match self {
            Statement::Mint {
                composition_id,
                component_index,
                lock_id,
                sol_user,
                nonce,
                units,
                deadline,
            } => {
                v.extend_from_slice(&composition_id.to_be_bytes());
                v.push(*component_index);
                v.extend_from_slice(&lock_id.to_be_bytes());
                v.extend_from_slice(sol_user.as_ref());
                v.extend_from_slice(&nonce.to_be_bytes());
                v.extend_from_slice(&units.to_be_bytes());
                v.extend_from_slice(&(*deadline as u64).to_be_bytes());
            }
            Statement::Release {
                lock_id,
                burn_id,
                to_eth,
                units,
            } => {
                v.extend_from_slice(&lock_id.to_be_bytes());
                v.extend_from_slice(&burn_id.to_be_bytes());
                v.extend_from_slice(to_eth);
                v.extend_from_slice(&units.to_be_bytes());
            }
            Statement::Veto {
                target_party_id,
                target_txid_le,
            } => {
                v.extend_from_slice(target_party_id);
                v.extend_from_slice(target_txid_le);
            }
            Statement::Cancel { lock_id } => {
                v.extend_from_slice(&lock_id.to_be_bytes());
            }
            Statement::Attest { target_txid_le } | Statement::Clear { target_txid_le } => {
                v.extend_from_slice(target_txid_le);
            }
            Statement::Alive { target_party_id } => {
                v.extend_from_slice(target_party_id);
            }
        }
        v
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip_all_kinds() {
        let cases = vec![
            Statement::Mint {
                composition_id: 1,
                component_index: 1,
                lock_id: 7,
                sol_user: Pubkey::new_unique(),
                nonce: 3,
                units: 2,
                deadline: 1_800_000_000,
            },
            Statement::Release {
                lock_id: 7,
                burn_id: 9,
                to_eth: [0xabu8; 20],
                units: 1,
            },
            Statement::Veto {
                target_party_id: [1u8; 32],
                target_txid_le: [0u8; 32],
            },
            Statement::Cancel { lock_id: 5 },
            Statement::Attest { target_txid_le: [2u8; 32] },
            Statement::Clear { target_txid_le: [3u8; 32] },
            Statement::Alive { target_party_id: [4u8; 32] },
        ];
        for s in cases {
            let bytes = s.encode();
            assert_eq!(Statement::decode(&bytes).unwrap(), s);
        }
    }
}
