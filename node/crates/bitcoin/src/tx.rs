//! Bitcoin transactions: the serialization without witness data (whose
//! double SHA-256 is the txid), and a simple builder for the node's tagged
//! transactions.

use anyhow::{bail, ensure};

use crate::sha256d;

fn var_int(n: usize, out: &mut Vec<u8>) {
    match n {
        0..=0xfc => out.push(n as u8),
        0xfd..=0xffff => {
            out.push(0xfd);
            out.extend_from_slice(&(n as u16).to_le_bytes());
        }
        _ => {
            out.push(0xfe);
            out.extend_from_slice(&(n as u32).to_le_bytes());
        }
    }
}

fn read_var_int(b: &[u8], o: &mut usize) -> anyhow::Result<usize> {
    ensure!(*o < b.len(), "the transaction is cut short");
    let first = b[*o];
    *o += 1;
    let size = match first {
        0..=0xfc => return Ok(first as usize),
        0xfd => 2,
        0xfe => 4,
        _ => 8,
    };
    ensure!(*o + size <= b.len(), "the transaction is cut short");
    let mut v = 0u64;
    for i in 0..size {
        v |= (b[*o + i] as u64) << (8 * i);
    }
    *o += size;
    Ok(v as usize)
}

/// The transaction without its witness data, whose double SHA-256 is the
/// txid. A transaction without witness data comes back unchanged.
pub fn strip_witness(raw: &[u8]) -> anyhow::Result<Vec<u8>> {
    ensure!(raw.len() >= 10, "the transaction is cut short");
    if !(raw[4] == 0 && raw[5] == 1) {
        return Ok(raw.to_vec());
    }
    let mut o = 6;
    let start = o;
    let inputs = read_var_int(raw, &mut o)?;
    for _ in 0..inputs {
        o += 36;
        let len = read_var_int(raw, &mut o)?;
        o += len + 4;
    }
    let outputs = read_var_int(raw, &mut o)?;
    for _ in 0..outputs {
        o += 8;
        let len = read_var_int(raw, &mut o)?;
        o += len;
    }
    ensure!(o <= raw.len(), "the transaction is cut short");
    let body_end = o;
    for _ in 0..inputs {
        let items = read_var_int(raw, &mut o)?;
        for _ in 0..items {
            let len = read_var_int(raw, &mut o)?;
            o += len;
        }
    }
    if o + 4 != raw.len() {
        bail!("the transaction does not end where its lock time should");
    }
    let mut out = raw[..4].to_vec();
    out.extend_from_slice(&raw[start..body_end]);
    out.extend_from_slice(&raw[o..]);
    Ok(out)
}

pub fn txid(raw_without_witness: &[u8]) -> [u8; 32] {
    sha256d(raw_without_witness)
}

/// An output: a value in satoshis and a script.
pub struct Output {
    pub value: u64,
    pub script: Vec<u8>,
}

/// `OP_RETURN` followed by 32 bytes: how a tagged transaction carries its
/// payload (D6, D80).
pub fn op_return(payload: &[u8; 32]) -> Output {
    let mut script = vec![0x6a, 0x20];
    script.extend_from_slice(payload);
    Output { value: 0, script }
}

/// A transaction without witness data, with empty input scripts. For tests,
/// and as the unsigned body the wallet signs.
pub fn build(inputs: &[([u8; 32], u32)], outputs: &[Output]) -> Vec<u8> {
    let mut tx = vec![];
    tx.extend_from_slice(&2u32.to_le_bytes());
    var_int(inputs.len(), &mut tx);
    for (txid, vout) in inputs {
        tx.extend_from_slice(txid);
        tx.extend_from_slice(&vout.to_le_bytes());
        var_int(0, &mut tx);
        tx.extend_from_slice(&[0xff; 4]);
    }
    var_int(outputs.len(), &mut tx);
    for o in outputs {
        tx.extend_from_slice(&o.value.to_le_bytes());
        var_int(o.script.len(), &mut tx);
        tx.extend_from_slice(&o.script);
    }
    tx.extend_from_slice(&[0u8; 4]);
    tx
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keeps_a_transaction_without_witness_data() {
        let tx = build(&[([1; 32], 0)], &[op_return(&[2; 32])]);
        assert_eq!(strip_witness(&tx).unwrap(), tx);
    }

    #[test]
    fn strips_witness_data_to_the_txid_serialization() {
        // A built transaction, then the same one with witness data added:
        // stripping must give back the first, so both have one txid.
        let body = build(&[([3; 32], 1)], &[Output { value: 546, script: vec![0x00, 0x14, 0x11, 0x11] }]);
        // Insert the marker and flag, and one witness item of 2 bytes.
        let mut segwit = body[..4].to_vec();
        segwit.extend_from_slice(&[0x00, 0x01]);
        segwit.extend_from_slice(&body[4..body.len() - 4]);
        segwit.extend_from_slice(&[0x01, 0x02, 0xaa, 0xbb]);
        segwit.extend_from_slice(&body[body.len() - 4..]);
        assert_eq!(strip_witness(&segwit).unwrap(), body);
        assert_eq!(txid(&strip_witness(&segwit).unwrap()), txid(&body));
    }
}
