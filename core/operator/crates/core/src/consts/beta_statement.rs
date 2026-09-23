//! Beta's Bitcoin statement wire formats (DESIGN_V2.md §6.2/§8.20/§8.21).
//!
//! These are the exact byte layouts `BetaVault.sol`/`BetaHub.sol`/
//! `beta-factory`'s `_parseAnchor`/`process_anchor.rs` expect, embedded as
//! the `sha256`-hashed payload of a statement-chain anchor's `OP_RETURN`
//! (see `crate::btc::statement_chain`). This module only encodes/decodes
//! the statement bytes themselves — it has no opinion about Bitcoin at all.
//!
//! Verified against real, on-chain-accepted statements built by hand this
//! session (see this module's tests) before being trusted here.

use anyhow::{bail, Result};

pub const KIND_MINT: u8 = 0x01;
pub const KIND_ATTEST: u8 = 0x05;

pub const MINT_STATEMENT_LEN: usize = 74;
pub const ATTEST_STATEMENT_LEN: usize = 33;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MintStatement {
    pub composition_id: u64,
    pub component_index: u8,
    pub lock_id: u64,
    /// Solana pubkey bytes, or an EVM address left-padded to 32 bytes —
    /// opaque to this encoding either way.
    pub target_user: [u8; 32],
    pub nonce: u64,
    pub units: u64,
    pub deadline: i64,
}

impl MintStatement {
    pub fn encode(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(MINT_STATEMENT_LEN);
        out.push(KIND_MINT);
        out.extend_from_slice(&self.composition_id.to_be_bytes());
        out.push(self.component_index);
        out.extend_from_slice(&self.lock_id.to_be_bytes());
        out.extend_from_slice(&self.target_user);
        out.extend_from_slice(&self.nonce.to_be_bytes());
        out.extend_from_slice(&self.units.to_be_bytes());
        out.extend_from_slice(&self.deadline.to_be_bytes());
        debug_assert_eq!(out.len(), MINT_STATEMENT_LEN);
        out
    }

    pub fn decode(buf: &[u8]) -> Result<Self> {
        if buf.len() != MINT_STATEMENT_LEN {
            bail!(
                "MINT statement must be {MINT_STATEMENT_LEN} bytes, got {}",
                buf.len()
            );
        }
        if buf[0] != KIND_MINT {
            bail!("expected kind 0x{KIND_MINT:02x}, got 0x{:02x}", buf[0]);
        }
        let mut o = 1usize;
        let composition_id = u64::from_be_bytes(buf[o..o + 8].try_into()?);
        o += 8;
        let component_index = buf[o];
        o += 1;
        let lock_id = u64::from_be_bytes(buf[o..o + 8].try_into()?);
        o += 8;
        let mut target_user = [0u8; 32];
        target_user.copy_from_slice(&buf[o..o + 32]);
        o += 32;
        let nonce = u64::from_be_bytes(buf[o..o + 8].try_into()?);
        o += 8;
        let units = u64::from_be_bytes(buf[o..o + 8].try_into()?);
        o += 8;
        let deadline = i64::from_be_bytes(buf[o..o + 8].try_into()?);
        o += 8;
        debug_assert_eq!(o, MINT_STATEMENT_LEN);
        Ok(Self {
            composition_id,
            component_index,
            lock_id,
            target_user,
            nonce,
            units,
            deadline,
        })
    }

    /// `target_user` as a 20-byte EVM address left-padded to 32 bytes
    /// (`bytes32(uint256(uint160(addr)))`, this session's convention).
    pub fn from_evm_user(
        composition_id: u64,
        component_index: u8,
        lock_id: u64,
        user: [u8; 20],
        nonce: u64,
        units: u64,
        deadline: i64,
    ) -> Self {
        let mut target_user = [0u8; 32];
        target_user[12..].copy_from_slice(&user);
        Self {
            composition_id,
            component_index,
            lock_id,
            target_user,
            nonce,
            units,
            deadline,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AttestStatement {
    /// Little-endian Bitcoin txid of the anchor being attested — matches
    /// `mintAttester`/`anchors` keying on both `BetaVault.sol`/`BetaHub.sol`.
    pub target_txid_le: [u8; 32],
}

impl AttestStatement {
    pub fn encode(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(ATTEST_STATEMENT_LEN);
        out.push(KIND_ATTEST);
        out.extend_from_slice(&self.target_txid_le);
        debug_assert_eq!(out.len(), ATTEST_STATEMENT_LEN);
        out
    }

    pub fn decode(buf: &[u8]) -> Result<Self> {
        if buf.len() != ATTEST_STATEMENT_LEN {
            bail!(
                "ATTEST statement must be {ATTEST_STATEMENT_LEN} bytes, got {}",
                buf.len()
            );
        }
        if buf[0] != KIND_ATTEST {
            bail!("expected kind 0x{KIND_ATTEST:02x}, got 0x{:02x}", buf[0]);
        }
        let mut target_txid_le = [0u8; 32];
        target_txid_le.copy_from_slice(&buf[1..33]);
        Ok(Self { target_txid_le })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn from_hex(s: &str) -> Vec<u8> {
        let s = s.strip_prefix("0x").unwrap_or(s);
        (0..s.len())
            .step_by(2)
            .map(|i| u8::from_str_radix(&s[i..i + 2], 16).unwrap())
            .collect()
    }

    fn to_hex(bytes: &[u8]) -> String {
        bytes.iter().map(|b| format!("{b:02x}")).collect()
    }

    /// The real MINT statement built and successfully processed on Base
    /// this session (see `live_multi_network_fix_party.mjs`'s `base` entry):
    /// compositionId=2, componentIndex=1, lockId=1, hubUser=this session's
    /// Solana wallet's raw pubkey bytes, nonce=2, units=1,
    /// deadline=1790724106.
    #[test]
    fn decodes_real_base_mint_statement() {
        let hex = "010000000000000002010000000000000001e0e72437e8caeddcb5368336c372742770ee2e3bb3b8e426a6d961e57a14f74a00000000000000020000000000000001000000006abc480a";
        let buf = from_hex(hex);
        assert_eq!(buf.len(), MINT_STATEMENT_LEN);

        let stmt = MintStatement::decode(&buf).unwrap();
        assert_eq!(stmt.composition_id, 2);
        assert_eq!(stmt.component_index, 1);
        assert_eq!(stmt.lock_id, 1);
        assert_eq!(
            to_hex(&stmt.target_user),
            "e0e72437e8caeddcb5368336c372742770ee2e3bb3b8e426a6d961e57a14f74a"
        );
        assert_eq!(stmt.nonce, 2);
        assert_eq!(stmt.units, 1);
        assert_eq!(stmt.deadline, 1790724106);

        // Round-trips exactly.
        assert_eq!(to_hex(&stmt.encode()), hex);
    }

    #[test]
    fn encodes_evm_user_left_padded() {
        let user: [u8; 20] =
            hex_literal_20("9784b80bc7b95753f2a2732649f3a1136b4c2bf1");
        let stmt = MintStatement::from_evm_user(
            1, 0, 1, user, 1, 1, 1_790_724_106,
        );
        assert_eq!(&stmt.target_user[..12], &[0u8; 12]);
        assert_eq!(&stmt.target_user[12..], &user);
        let buf = stmt.encode();
        assert_eq!(MintStatement::decode(&buf).unwrap(), stmt);
    }

    #[test]
    fn attest_round_trips() {
        // Real ATTEST target this session: the Ethereum-hub/Tempo-leg MINT
        // anchor's little-endian txid (verified via independent byte-swap
        // of the real blockstream.info txid before being used on-chain).
        let hex =
            "cda34be9d34cfc6f6ed4a3078af7a279aa2e70b946c49f47ed8552e79c561781";
        let raw = from_hex(hex);
        assert_eq!(raw.len(), 32);
        let mut target = [0u8; 32];
        target.copy_from_slice(&raw);

        let stmt = AttestStatement { target_txid_le: target };
        let encoded = stmt.encode();
        assert_eq!(encoded.len(), ATTEST_STATEMENT_LEN);
        assert_eq!(encoded[0], KIND_ATTEST);
        assert_eq!(AttestStatement::decode(&encoded).unwrap(), stmt);
    }

    fn hex_literal_20(s: &str) -> [u8; 20] {
        let v = from_hex(s);
        let mut out = [0u8; 20];
        out.copy_from_slice(&v);
        out
    }
}
