//! Reads what the protocol needs from a Bitcoin transaction. The same rules
//! as `contracts/protocol/BitcoinTxLib.sol`. The transaction is in the
//! serialization without witness data, the one whose double SHA-256 is the
//! txid.

use anchor_lang::prelude::*;

use crate::errors::ProtocolError;

#[derive(Default)]
pub struct TxView {
    /// The coin that input `input_index` spends.
    pub spent_txid: [u8; 32],
    pub spent_vout: u32,
    /// Whether output `coin_index` exists and is not an OP_RETURN.
    pub has_coin: bool,
    /// Whether output `tag_index` is OP_RETURN followed by 32 bytes.
    pub has_tag: bool,
    pub tag: [u8; 32],
}

fn var_int(b: &[u8], o: usize) -> Result<(u64, usize)> {
    require!(o < b.len(), ProtocolError::MalformedTx);
    let first = b[o];
    if first < 0xFD {
        return Ok((first as u64, o + 1));
    }
    let size = match first {
        0xFD => 2,
        0xFE => 4,
        _ => 8,
    };
    require!(o + 1 + size <= b.len(), ProtocolError::MalformedTx);
    let mut v = 0u64;
    for i in 0..size {
        v |= (b[o + 1 + i] as u64) << (8 * i);
    }
    // Bitcoin accepts only the shortest way to write a number.
    let least: u64 = match size {
        2 => 0xFD,
        4 => 0x1_0000,
        _ => 0x1_0000_0000,
    };
    require!(v >= least, ProtocolError::MalformedTx);
    Ok((v, o + 1 + size))
}

/// Reads one input and two outputs. The whole transaction must parse, to
/// the last byte.
pub fn read(raw: &[u8], input_index: u64, coin_index: u64, tag_index: u64) -> Result<TxView> {
    require!(raw.len() >= 10, ProtocolError::MalformedTx);
    let mut o = 4;
    require!(!(raw[o] == 0x00 && raw[o + 1] == 0x01), ProtocolError::WitnessSerialization);

    let mut v = TxView::default();
    let (inputs, next) = var_int(raw, o)?;
    o = next;
    require!(inputs > 0 && input_index < inputs, ProtocolError::MalformedTx);
    for i in 0..inputs {
        require!(o + 36 <= raw.len(), ProtocolError::MalformedTx);
        if i == input_index {
            v.spent_txid.copy_from_slice(&raw[o..o + 32]);
            v.spent_vout = u32::from_le_bytes(raw[o + 32..o + 36].try_into().unwrap());
        }
        o += 36;
        let (script_len, next) = var_int(raw, o)?;
        o = next
            .checked_add(script_len as usize)
            .and_then(|x| x.checked_add(4))
            .ok_or(ProtocolError::MalformedTx)?;
        require!(o <= raw.len(), ProtocolError::MalformedTx);
    }

    let (outputs, next) = var_int(raw, o)?;
    o = next;
    require!(outputs > 0, ProtocolError::MalformedTx);
    for j in 0..outputs {
        require!(o + 8 <= raw.len(), ProtocolError::MalformedTx);
        o += 8;
        let (script_len, next) = var_int(raw, o)?;
        o = next;
        let end = o.checked_add(script_len as usize).ok_or(ProtocolError::MalformedTx)?;
        require!(end <= raw.len(), ProtocolError::MalformedTx);
        let script = &raw[o..end];
        if j == coin_index {
            v.has_coin = !script.is_empty() && script[0] != 0x6a;
        }
        if j == tag_index && script.len() == 34 && script[0] == 0x6a && script[1] == 0x20 {
            v.has_tag = true;
            v.tag.copy_from_slice(&script[2..34]);
        }
        o = end;
    }
    // The lock time, and nothing after it.
    require!(o + 4 == raw.len(), ProtocolError::MalformedTx);
    Ok(v)
}
