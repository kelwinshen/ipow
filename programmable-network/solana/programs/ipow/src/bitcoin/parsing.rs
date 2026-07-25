use anchor_lang::prelude::*;

use crate::errors::IPoWError;

pub fn parse_output_at(tx_raw: &[u8], vout_index: usize) -> Result<(u64, Vec<u8>)> {
    let mut o: usize = 0;
    require!(tx_raw.len() >= 4, IPoWError::TransactionTooShort);
    o += 4;

    if tx_raw.len() >= o + 2 && tx_raw[o] == 0x00 && tx_raw[o + 1] == 0x01 {
        o += 2;
    }

    let (in_count, next_o) = read_var_int(tx_raw, o)?;
    o = next_o;

    for _ in 0..in_count {
        o += 36;
        let (slen, next_o2) = read_var_int(tx_raw, o)?;
        o = next_o2 + slen as usize + 4;
        require!(o <= tx_raw.len(), IPoWError::TransactionOverflow);
    }

    let (out_count, next_o3) = read_var_int(tx_raw, o)?;
    o = next_o3;

    require!(vout_index < out_count as usize, IPoWError::VoutOutOfBounds);

    for j in 0..(out_count as usize) {
        require!(o + 8 <= tx_raw.len(), IPoWError::ValueOutOfBounds);

        let mut val_bytes = [0u8; 8];
        val_bytes.copy_from_slice(&tx_raw[o..o + 8]);
        let value_sats = u64::from_le_bytes(val_bytes);
        o += 8;

        let (prog_len, next_o4) = read_var_int(tx_raw, o)?;
        o = next_o4;
        require!(
            o + prog_len as usize <= tx_raw.len(),
            IPoWError::ProgramOutOfBounds
        );

        if j == vout_index {
            let program = tx_raw[o..o + prog_len as usize].to_vec();
            return Ok((value_sats, program));
        }
        o += prog_len as usize;
    }
    Err(error!(IPoWError::VoutOutOfBounds))
}

pub fn read_var_int(b: &[u8], o: usize) -> Result<(u64, usize)> {
    require!(o < b.len(), IPoWError::VarIntOutOfBounds);
    let p = b[o];
    if p < 0xFD {
        Ok((p as u64, o + 1))
    } else if p == 0xFD {
        require!(o + 3 <= b.len(), IPoWError::Var16OutOfBounds);
        let v = u16::from_le_bytes(b[o + 1..o + 3].try_into().unwrap());
        Ok((v as u64, o + 3))
    } else if p == 0xFE {
        require!(o + 5 <= b.len(), IPoWError::Var32OutOfBounds);
        let v = u32::from_le_bytes(b[o + 1..o + 5].try_into().unwrap());
        Ok((v as u64, o + 5))
    } else {
        require!(o + 9 <= b.len(), IPoWError::Var64OutOfBounds);
        let v = u64::from_le_bytes(b[o + 1..o + 9].try_into().unwrap());
        Ok((v, o + 9))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn read_var_int_single_byte() {
        let (v, next) = read_var_int(&[0x2a], 0).unwrap();
        assert_eq!(v, 0x2a);
        assert_eq!(next, 1);
    }

    #[test]
    fn read_var_int_0xfd_prefixed_u16() {
        let (v, next) = read_var_int(&[0xfd, 0x34, 0x12], 0).unwrap();
        assert_eq!(v, 0x1234);
        assert_eq!(next, 3);
    }

    #[test]
    fn read_var_int_0xfe_prefixed_u32() {
        let (v, next) = read_var_int(&[0xfe, 0x01, 0x00, 0x00, 0x00], 0).unwrap();
        assert_eq!(v, 1);
        assert_eq!(next, 5);
    }

    #[test]
    fn read_var_int_0xff_prefixed_u64() {
        let (v, next) = read_var_int(&[0xff, 1, 0, 0, 0, 0, 0, 0, 0], 0).unwrap();
        assert_eq!(v, 1);
        assert_eq!(next, 9);
    }

    #[test]
    fn read_var_int_out_of_bounds() {
        let err = read_var_int(&[], 0).unwrap_err();
        assert_eq!(err, error!(IPoWError::VarIntOutOfBounds));
    }

    #[test]
    fn parse_output_at_simple_single_input_output_tx() {
        // A realistic single-input, single-output legacy transaction. Deliberately
        // not using 0 inputs: an inCount byte of 0x00 followed by an outCount byte
        // of 0x01 is indistinguishable from the SegWit marker+flag (0x00, 0x01) —
        // the parser correctly treats that as SegWit framing, so a 0-input tx isn't
        // realistic test data (mirrors the same caveat hit writing the Solidity
        // equivalent of this parser).
        let mut tx_raw = Vec::new();
        tx_raw.extend_from_slice(&1u32.to_le_bytes()); // version
        tx_raw.push(0x01); // 1 input
        tx_raw.extend_from_slice(&[0u8; 32]); // outpoint txid
        tx_raw.extend_from_slice(&[0u8; 4]); // outpoint vout index
        tx_raw.push(0x00); // scriptSig length = 0
        tx_raw.extend_from_slice(&[0xffu8; 4]); // sequence
        tx_raw.push(0x01); // 1 output
        tx_raw.extend_from_slice(&1000u64.to_le_bytes()); // value = 1000 sats
        tx_raw.push(0x02); // script length = 2
        tx_raw.extend_from_slice(&[0xab, 0xcd]); // script bytes

        let (value_sats, program) = parse_output_at(&tx_raw, 0).unwrap();
        assert_eq!(value_sats, 1000);
        assert_eq!(program, vec![0xab, 0xcd]);
    }

    #[test]
    fn parse_output_at_vout_out_of_bounds() {
        let mut tx_raw = Vec::new();
        tx_raw.extend_from_slice(&1u32.to_le_bytes()); // version
        tx_raw.push(0x01); // 1 input
        tx_raw.extend_from_slice(&[0u8; 32]);
        tx_raw.extend_from_slice(&[0u8; 4]);
        tx_raw.push(0x00);
        tx_raw.extend_from_slice(&[0xffu8; 4]);
        tx_raw.push(0x00); // 0 outputs

        let err = parse_output_at(&tx_raw, 0).unwrap_err();
        assert_eq!(err, error!(IPoWError::VoutOutOfBounds));
    }

    #[test]
    fn parse_output_at_transaction_too_short() {
        let err = parse_output_at(&[0x01, 0x02], 0).unwrap_err();
        assert_eq!(err, error!(IPoWError::TransactionTooShort));
    }
}
