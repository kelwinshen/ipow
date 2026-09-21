use anchor_lang::prelude::*;
use ipow::bitcoin::read_var_int;
use ipow::state::GlobalHeader;
use sha2::{Digest, Sha256};

use crate::constants::*;
use crate::errors::FactoryError;
use crate::state::Party;

pub fn dsha256(b: &[u8]) -> [u8; 32] {
    let h1 = Sha256::digest(b);
    let h2 = Sha256::digest(h1);
    let mut out = [0u8; 32];
    out.copy_from_slice(&h2);
    out
}

/// Same merkle walk `ipow`/`ipow-conversion`'s proof paths use.
pub fn verify_inclusion(
    txid_le: [u8; 32],
    header: &GlobalHeader,
    proof_block_height: u64,
    branch_le: &[[u8; 32]],
    index: u64,
) -> Result<()> {
    require!(header.height == proof_block_height, FactoryError::InvalidHeader);
    let mut current = txid_le;
    let mut idx = index;
    for sibling in branch_le {
        let mut hasher = Sha256::new();
        if idx % 2 == 0 {
            hasher.update(current);
            hasher.update(sibling);
        } else {
            hasher.update(sibling);
            hasher.update(current);
        }
        let h1 = hasher.finalize();
        let h2 = Sha256::digest(h1);
        current.copy_from_slice(&h2);
        idx /= 2;
    }
    require!(current == header.merkle_root_le, FactoryError::InvalidMerkleBranch);
    Ok(())
}

pub struct ParsedAnchor {
    pub in0_txid_le: [u8; 32],
    pub in0_vout: u32,
    /// `None` if output[1] is absent or not a well-formed anchor payload —
    /// a skippable (stalled) anchor rather than a hard error.
    pub payload: Option<AnchorPayload>,
}

pub struct AnchorPayload {
    pub kind: u8,
    pub statement_hash: [u8; 32],
}

/// Reads input[0]'s outpoint and output[1]'s OP_RETURN payload from a
/// witness-stripped raw transaction (DESIGN_V2 §6.2). Witness framing is
/// rejected because the txid is `dsha256` of the stripped serialization —
/// the caller strips, we hash.
pub fn parse_anchor(tx: &[u8]) -> Result<ParsedAnchor> {
    require!(tx.len() >= 4 + 1 + 36 + 1 + 4 + 1, FactoryError::MalformedTx);
    let mut o = 4;
    require!(
        !(tx[o] == 0x00 && tx[o + 1] == 0x01),
        FactoryError::WitnessSerialization
    );
    let (in_count, next) = read_var_int(tx, o)?;
    o = next;
    require!(in_count >= 1, FactoryError::MalformedTx);
    require!(o + 36 <= tx.len(), FactoryError::MalformedTx);
    let mut in0_txid_le = [0u8; 32];
    in0_txid_le.copy_from_slice(&tx[o..o + 32]);
    let in0_vout = u32::from_le_bytes(tx[o + 32..o + 36].try_into().unwrap());
    for _ in 0..in_count {
        o += 36;
        let (slen, next) = read_var_int(tx, o)?;
        o = next + slen as usize + 4;
        require!(o <= tx.len(), FactoryError::MalformedTx);
    }
    let (out_count, next) = read_var_int(tx, o)?;
    o = next;
    require!(out_count >= 1, FactoryError::MalformedTx);
    let mut payload = None;
    for j in 0..out_count as usize {
        require!(o + 8 <= tx.len(), FactoryError::MalformedTx);
        o += 8;
        let (slen, next) = read_var_int(tx, o)?;
        o = next;
        require!(o + slen as usize <= tx.len(), FactoryError::MalformedTx);
        if j == 1 {
            let script = &tx[o..o + slen as usize];
            if script.len() == 2 + ANCHOR_PAYLOAD_LEN
                && script[0] == 0x6a
                && script[1] as usize == ANCHOR_PAYLOAD_LEN
                && script[2] == ANCHOR_VERSION
            {
                let mut statement_hash = [0u8; 32];
                statement_hash.copy_from_slice(&script[4..36]);
                payload = Some(AnchorPayload {
                    kind: script[3],
                    statement_hash,
                });
            }
        }
        o += slen as usize;
    }
    Ok(ParsedAnchor {
        in0_txid_le,
        in0_vout,
        payload,
    })
}

/// Full anchor check: txid, inclusion, on-chain-of-this-party. On success
/// advances the party's pointer to `(txid, 0)` and returns the parsed tx.
pub fn verify_and_advance(
    party: &mut Party,
    txid_le: [u8; 32],
    tx_raw: &[u8],
    header: &GlobalHeader,
    proof_block_height: u64,
    branch_le: &[[u8; 32]],
    index: u64,
) -> Result<ParsedAnchor> {
    require!(dsha256(tx_raw) == txid_le, FactoryError::TxidMismatch);
    verify_inclusion(txid_le, header, proof_block_height, branch_le, index)?;
    let parsed = parse_anchor(tx_raw)?;
    require!(
        parsed.in0_txid_le == party.anchor_txid_le && parsed.in0_vout == party.anchor_vout,
        FactoryError::NotOnStatementChain
    );
    party.anchor_txid_le = txid_le;
    party.anchor_vout = 0;
    party.seq = party.seq.checked_add(1).ok_or(FactoryError::Overflow)?;
    Ok(parsed)
}
