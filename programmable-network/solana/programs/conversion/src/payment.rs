//! Reads the payments of a Bitcoin transaction: whether an output pays a
//! script at least an amount, and whether an input spends a coin. The
//! transaction is without witness data; it must parse to the last byte.

use anchor_lang::prelude::*;

use crate::errors::ConversionError;

struct Reader<'a> {
    raw: &'a [u8],
    o: usize,
}

impl<'a> Reader<'a> {
    fn take(&mut self, n: usize) -> Result<&'a [u8]> {
        let end = self.o.checked_add(n).ok_or(ConversionError::MalformedTx)?;
        require!(end <= self.raw.len(), ConversionError::MalformedTx);
        let s = &self.raw[self.o..end];
        self.o = end;
        Ok(s)
    }

    /// Bitcoin's variable length integer, in its shortest form only.
    fn var_int(&mut self) -> Result<usize> {
        let first = self.take(1)?[0];
        let (size, least) = match first {
            0..=0xfc => return Ok(first as usize),
            0xfd => (2, 0xfd),
            0xfe => (4, 0x1_0000),
            _ => (8, 0x1_0000_0000u64),
        };
        let b = self.take(size)?;
        let mut v = 0u64;
        for (i, x) in b.iter().enumerate() {
            v |= (*x as u64) << (8 * i);
        }
        require!(v >= least, ConversionError::MalformedTx);
        usize::try_from(v).map_err(|_| error!(ConversionError::MalformedTx))
    }
}

pub struct Parsed<'a> {
    /// The coins the inputs spend.
    pub spends: Vec<([u8; 32], u32)>,
    /// Each output's value and script.
    pub outputs: Vec<(u64, &'a [u8])>,
}

pub fn parse(raw: &[u8]) -> Result<Parsed<'_>> {
    require!(raw.len() >= 10, ConversionError::MalformedTx);
    // The serialization with witness data has a zero where the count of
    // inputs would be.
    require!(!(raw[4] == 0 && raw[5] == 1), ConversionError::MalformedTx);
    let mut r = Reader { raw, o: 4 };
    let inputs = r.var_int()?;
    require!(inputs > 0, ConversionError::MalformedTx);
    let mut spends = Vec::with_capacity(inputs.min(64));
    for _ in 0..inputs {
        let txid: [u8; 32] = r.take(32)?.try_into().unwrap();
        let vout = u32::from_le_bytes(r.take(4)?.try_into().unwrap());
        let len = r.var_int()?;
        r.take(len)?;
        r.take(4)?;
        spends.push((txid, vout));
    }
    let count = r.var_int()?;
    require!(count > 0, ConversionError::MalformedTx);
    let mut outputs = Vec::with_capacity(count.min(64));
    for _ in 0..count {
        let value = u64::from_le_bytes(r.take(8)?.try_into().unwrap());
        let len = r.var_int()?;
        outputs.push((value, r.take(len)?));
    }
    r.take(4)?;
    require!(r.o == raw.len(), ConversionError::MalformedTx);
    Ok(Parsed { spends, outputs })
}

fn pays(output: &(u64, &[u8]), script: &[u8], sats: u64) -> bool {
    !script.is_empty() && output.1 == script && output.0 >= sats
}

/// Whether any output pays exactly `script` at least `sats`.
pub fn pays_at_least(raw: &[u8], script: &[u8], sats: u64) -> Result<bool> {
    Ok(parse(raw)?.outputs.iter().any(|o| pays(o, script, sats)))
}

/// Whether output `vout` pays exactly `script` at least `sats`.
pub fn output_pays(raw: &[u8], vout: u32, script: &[u8], sats: u64) -> Result<bool> {
    Ok(parse(raw)?.outputs.get(vout as usize).is_some_and(|o| pays(o, script, sats)))
}

/// Whether an input spends output `vout` of `txid`.
pub fn spends(raw: &[u8], txid: &[u8; 32], vout: u32) -> Result<bool> {
    Ok(parse(raw)?.spends.iter().any(|(t, v)| t == txid && *v == vout))
}
