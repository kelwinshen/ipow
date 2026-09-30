//! The operator's Bitcoin wallet: one key, one pay-to-witness-public-key-hash
//! address, on Bitcoin mainnet. It builds and signs the transactions the
//! protocol reads (D30, D80):
//!
//! | Output | What |
//! |---|---|
//! | 0 to n-1 | 546 satoshis to the wallet: the next chain head of each of the n networks the message is for |
//! | n | `OP_RETURN` with the 32-byte payload |
//! | n+1 to n+k | Payments an application asks for, such as BTC paid to a user in a conversion |
//! | after them | Change to the wallet, when there is enough |
//!
//! Inputs 0 to n-1 are the chain heads being spent, one per network; the
//! output with the same number is that network's next chain head (N23). One
//! transaction thus carries a message for several networks (D80). The
//! other inputs pay the fee, or are coins an application asks to spend
//! (a receipt of a user's payment), each signed with its own key. A first
//! chain head has no chain head to spend: its input 0 is a funding coin, and
//! n is 1.
//!
//! A per-swap key (`derive`) comes from the wallet's key by BIP32, so every
//! address it names can be recovered from that one key.

use anyhow::{Context, bail, ensure};
use bitcoin::absolute::LockTime;
use bitcoin::hashes::Hash;
use bitcoin::secp256k1::{Message, Secp256k1, SecretKey};
use bitcoin::sighash::{EcdsaSighashType, SighashCache};
use bitcoin::transaction::Version;
use bitcoin::{Address, Amount, CompressedPublicKey, Network, OutPoint, PrivateKey, ScriptBuf, Sequence, Transaction, TxIn, TxOut, Txid, Witness};

/// The value of a chain head, the least an output to a witness address may
/// carry. Coins of this size are never used to pay fees, so a chain head is
/// never spent by mistake.
pub const CHAIN_HEAD_VALUE: u64 = 546;

/// Change below this is left to the miner.
const MIN_CHANGE: u64 = 1_000;

/// A coin of the wallet.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Spend {
    /// Header byte order, as everywhere in the node.
    pub txid: [u8; 32],
    pub vout: u32,
    pub value: u64,
}

pub struct Signed {
    /// With witness data: what is broadcast.
    pub raw: Vec<u8>,
    /// Without witness data: what a proof carries.
    pub stripped: Vec<u8>,
    pub txid: [u8; 32],
    /// The change coin, if any.
    pub change: Option<Spend>,
}

pub struct Wallet {
    key: PrivateKey,
    public: CompressedPublicKey,
    script: ScriptBuf,
}

impl Wallet {
    /// From a mainnet private key in WIF. The key never leaves this struct.
    pub fn from_wif(wif: &str) -> anyhow::Result<Self> {
        let key = PrivateKey::from_wif(wif.trim()).context("the Bitcoin key is not a valid WIF key")?;
        ensure!(key.network == Network::Bitcoin.into(), "the Bitcoin key is not a mainnet key");
        let secp = Secp256k1::signing_only();
        let public = CompressedPublicKey::from_private_key(&secp, &key).context("the key must be compressed")?;
        let script = ScriptBuf::new_p2wpkh(&public.wpubkey_hash());
        Ok(Wallet { key, public, script })
    }

    pub fn address(&self) -> String {
        Address::p2wpkh(&self.public, Network::Bitcoin).to_string()
    }

    pub fn script(&self) -> &[u8] {
        self.script.as_bytes()
    }

    /// The virtual size of a transaction of `inputs` inputs and `heads`
    /// chain head outputs, with change.
    pub fn vsize(inputs: usize, heads: usize) -> u64 {
        Self::vsize_with(inputs, heads, 0)
    }

    /// The same, with `extra` more outputs of up to 34 bytes of script.
    pub fn vsize_with(inputs: usize, heads: usize, extra: usize) -> u64 {
        // Version, counts, lock time and segwit marker: about 11. Each input
        // about 68. Each witness output 31 (43 for a longer script), and the
        // OP_RETURN 43.
        11 + 68 * inputs as u64 + 31 * (heads as u64 + 1) + 43 * extra as u64 + 43
    }

    /// The same, with outputs of these scripts: 9 bytes each and the script.
    pub fn vsize_outputs(inputs: usize, heads: usize, extra: &[(u64, Vec<u8>)]) -> u64 {
        Self::vsize_with(inputs, heads, 0) + extra.iter().map(|(_, s)| 9 + s.len() as u64).sum::<u64>()
    }

    /// A key of its own for one use, derived from this wallet's key by
    /// BIP32 at `path` (hardened steps), so it can always be recovered.
    pub fn derive(&self, path: &[u32]) -> anyhow::Result<Wallet> {
        use bitcoin::bip32::{ChildNumber, Xpriv};
        let secp = Secp256k1::new();
        let mut x = Xpriv::new_master(Network::Bitcoin, &self.key.inner.secret_bytes())?;
        for i in path {
            x = x.derive_priv(&secp, &[ChildNumber::from_hardened_idx(*i & 0x7fff_ffff)?])?;
        }
        let key = PrivateKey::new(x.private_key, Network::Bitcoin);
        let public = CompressedPublicKey::from_private_key(&secp, &key).context("the key must be compressed")?;
        let script = ScriptBuf::new_p2wpkh(&public.wpubkey_hash());
        Ok(Wallet { key, public, script })
    }

    /// Builds and signs a transaction: inputs in the given order, outputs 0
    /// to `heads - 1` the next chain heads, output `heads` `OP_RETURN
    /// payload`, then change. `fee` is in satoshis.
    pub fn tagged(&self, inputs: &[Spend], heads: usize, payload: &[u8; 32], fee: u64) -> anyhow::Result<Signed> {
        let with: Vec<(Spend, &Wallet)> = inputs.iter().map(|i| (*i, self)).collect();
        self.build(&with, heads, payload, &[], fee)
    }

    /// The general form: each input signed with its own wallet's key, and
    /// `extra` outputs (value, script) after the `OP_RETURN`. Change goes to
    /// this wallet.
    pub fn build(&self, inputs: &[(Spend, &Wallet)], heads: usize, payload: &[u8; 32], extra: &[(u64, Vec<u8>)], fee: u64) -> anyhow::Result<Signed> {
        ensure!(!inputs.is_empty(), "a transaction needs a coin to spend");
        ensure!(heads >= 1 && heads <= inputs.len(), "a chain head output needs a coin");
        let total: u64 = inputs.iter().map(|(i, _)| i.value).sum();
        let paid: u64 = extra.iter().map(|(v, _)| *v).sum();
        let Some(change) = total.checked_sub(CHAIN_HEAD_VALUE * heads as u64 + paid + fee) else {
            bail!("the coins hold {total} satoshis, less than the chain heads, the payments and the fee");
        };

        let mut op_return = vec![0x6a, 0x20];
        op_return.extend_from_slice(payload);
        let mut output: Vec<TxOut> =
            (0..heads).map(|_| TxOut { value: Amount::from_sat(CHAIN_HEAD_VALUE), script_pubkey: self.script.clone() }).collect();
        output.push(TxOut { value: Amount::ZERO, script_pubkey: ScriptBuf::from_bytes(op_return) });
        for (value, script) in extra {
            output.push(TxOut { value: Amount::from_sat(*value), script_pubkey: ScriptBuf::from_bytes(script.clone()) });
        }
        if change >= MIN_CHANGE {
            output.push(TxOut { value: Amount::from_sat(change), script_pubkey: self.script.clone() });
        }
        let mut tx = Transaction {
            version: Version::TWO,
            lock_time: LockTime::ZERO,
            input: inputs
                .iter()
                .map(|(i, _)| TxIn {
                    previous_output: OutPoint { txid: Txid::from_byte_array(i.txid), vout: i.vout },
                    script_sig: ScriptBuf::new(),
                    // Replaceable, so a stuck transaction can pay more.
                    sequence: Sequence::ENABLE_RBF_NO_LOCKTIME,
                    witness: Witness::new(),
                })
                .collect(),
            output,
        };

        let secp = Secp256k1::signing_only();
        let mut witnesses = vec![];
        {
            let mut cache = SighashCache::new(&tx);
            for (index, (input, w)) in inputs.iter().enumerate() {
                let secret = SecretKey::from_slice(&w.key.inner.secret_bytes())?;
                let sighash = cache.p2wpkh_signature_hash(index, &w.script, Amount::from_sat(input.value), EcdsaSighashType::All)?;
                let sig = secp.sign_ecdsa(&Message::from_digest(sighash.to_byte_array()), &secret);
                let sig = bitcoin::ecdsa::Signature { signature: sig, sighash_type: EcdsaSighashType::All };
                witnesses.push(Witness::p2wpkh(&sig, &w.public.0));
            }
        }
        for (input, w) in tx.input.iter_mut().zip(witnesses) {
            input.witness = w;
        }

        let raw = bitcoin::consensus::serialize(&tx);
        let stripped = crate::tx::strip_witness(&raw)?;
        let txid = tx.compute_txid().to_byte_array();
        debug_assert_eq!(crate::tx::txid(&stripped), txid);
        let change_vout = heads + 1 + extra.len();
        let change = (tx.output.len() == change_vout + 1).then_some(Spend { txid, vout: change_vout as u32, value: change });
        Ok(Signed { raw, stripped, txid, change })
    }
}

impl std::fmt::Debug for Wallet {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // Never the key.
        write!(f, "Wallet({})", self.address())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // A key made for this test only, holding nothing: the secret 1.
    const WIF: &str = "KwDiBf89QgGbjEhKnhXJuH7LrciVrZi3qYjgd9M7rFU73sVHnoWn";

    #[test]
    fn builds_a_transaction_the_protocol_reads() {
        let w = Wallet::from_wif(WIF).unwrap();
        assert_eq!(w.address(), "bc1qw508d6qejxtdg4y5r3zarvary0c5xw7kv8f3t4");
        let head = Spend { txid: [1; 32], vout: 0, value: 546 };
        let funding = Spend { txid: [2; 32], vout: 3, value: 50_000 };
        let s = w.tagged(&[head, funding], 1, &[9; 32], 1_000).unwrap();

        let tx: Transaction = bitcoin::consensus::deserialize(&s.raw).unwrap();
        assert_eq!(tx.input[0].previous_output.txid.to_byte_array(), [1; 32]);
        assert_eq!(tx.input[1].previous_output.vout, 3);
        assert_eq!(tx.output[0].value.to_sat(), 546);
        assert_eq!(tx.output[0].script_pubkey.as_bytes(), w.script());
        assert_eq!(&tx.output[1].script_pubkey.as_bytes()[2..], &[9; 32]);
        assert_eq!(tx.output[2].value.to_sat(), 50_000 + 546 - 546 - 1_000);
        assert_eq!(crate::tx::txid(&s.stripped), s.txid);
        assert_eq!(s.change, Some(Spend { txid: s.txid, vout: 2, value: 49_000 }));

        // Every signature checks against its input's value.
        let secp = Secp256k1::verification_only();
        let mut cache = SighashCache::new(&tx);
        for (i, value) in [546u64, 50_000].into_iter().enumerate() {
            let w_items: Vec<&[u8]> = tx.input[i].witness.iter().collect();
            let sig = bitcoin::ecdsa::Signature::from_slice(w_items[0]).unwrap();
            let public = bitcoin::secp256k1::PublicKey::from_slice(w_items[1]).unwrap();
            let sighash = cache.p2wpkh_signature_hash(i, &ScriptBuf::from_bytes(w.script().to_vec()), Amount::from_sat(value), EcdsaSighashType::All).unwrap();
            secp.verify_ecdsa(&Message::from_digest(sighash.to_byte_array()), &sig.signature, &public).unwrap();
        }
    }

    #[test]
    fn builds_one_transaction_for_two_networks() {
        let w = Wallet::from_wif(WIF).unwrap();
        let heads = [Spend { txid: [1; 32], vout: 0, value: 546 }, Spend { txid: [3; 32], vout: 1, value: 546 }];
        let funding = Spend { txid: [2; 32], vout: 0, value: 50_000 };
        let s = w.tagged(&[heads[0], heads[1], funding], 2, &[9; 32], 1_000).unwrap();
        let tx: Transaction = bitcoin::consensus::deserialize(&s.raw).unwrap();
        // Input k spends network k's chain head; output k is its next one.
        assert_eq!(tx.input[1].previous_output.txid.to_byte_array(), [3; 32]);
        assert_eq!(tx.output[0].value.to_sat(), 546);
        assert_eq!(tx.output[1].value.to_sat(), 546);
        assert_eq!(&tx.output[2].script_pubkey.as_bytes()[2..], &[9; 32]);
        assert_eq!(tx.output[3].value.to_sat(), 50_000 - 1_000);
        assert_eq!(s.change.unwrap().vout, 3);
    }

    #[test]
    fn pays_an_extra_output_and_signs_an_input_with_a_derived_key() {
        let w = Wallet::from_wif(WIF).unwrap();
        let swap = w.derive(&[7, 1, 42]).unwrap();
        // Recoverable: the same path gives the same key.
        assert_eq!(swap.address(), w.derive(&[7, 1, 42]).unwrap().address());
        assert_ne!(swap.address(), w.address());
        let head = Spend { txid: [1; 32], vout: 0, value: 546 };
        let paid = Spend { txid: [4; 32], vout: 0, value: 5_000_000 };
        let user = vec![0x00, 0x14, 0x22, 0x22];
        let s = w.build(&[(head, &w), (paid, &swap)], 1, &[9; 32], &[(4_000_000, user.clone())], 1_000).unwrap();
        let tx: Transaction = bitcoin::consensus::deserialize(&s.raw).unwrap();
        assert_eq!(tx.output[2].value.to_sat(), 4_000_000);
        assert_eq!(tx.output[2].script_pubkey.as_bytes(), &user[..]);
        assert_eq!(s.change.unwrap().vout, 3);
        // Input 1 verifies against the derived key.
        let secp = Secp256k1::verification_only();
        let mut cache = SighashCache::new(&tx);
        let items: Vec<&[u8]> = tx.input[1].witness.iter().collect();
        let sig = bitcoin::ecdsa::Signature::from_slice(items[0]).unwrap();
        let public = bitcoin::secp256k1::PublicKey::from_slice(items[1]).unwrap();
        let sighash = cache.p2wpkh_signature_hash(1, &ScriptBuf::from_bytes(swap.script().to_vec()), Amount::from_sat(5_000_000), EcdsaSighashType::All).unwrap();
        secp.verify_ecdsa(&Message::from_digest(sighash.to_byte_array()), &sig.signature, &public).unwrap();
    }

    #[test]
    fn refuses_too_little_and_other_networks() {
        let w = Wallet::from_wif(WIF).unwrap();
        assert!(w.tagged(&[Spend { txid: [1; 32], vout: 0, value: 1_000 }], 1, &[0; 32], 1_000).is_err());
        // The same secret as a testnet key.
        assert!(Wallet::from_wif("cMahea7zqjxrtgAbB7LSGbcQUr1uX1ojuat9jZodMN87JcbXMTcA").is_err());
        assert!(!format!("{w:?}").contains(WIF));
    }
}
