//! BETA v2 (docs/DESIGN_V2.md §6.2): builds, signs and (optionally)
//! broadcasts one *statement-chain anchor* from the operator's mainnet
//! wallet:
//!
//!   input[0]  = the party's currently registered outpoint (its chain head)
//!   output[0] = new chain head, a dust-sized P2WPKH back to the operator's
//!               own address (recoverable — never a hand-made script)
//!   output[1] = OP_RETURN 0x22 | ver=0x01 | kind | sha256(statement)
//!
//! The statement bytes are produced by `scripts/beta_factory_e2e.ts
//! ACTION=statement-*` on the Solana side (identical encoding to
//! BetaVault.sol). Prints the txid, the full witness tx (for broadcast) and
//! the witness-stripped legacy serialization both chains hash and parse.
//!
//! Usage:
//!   cargo run --example anchor_statement -- <prev_txid_be> <prev_vout> <kind 1..4> <statement_hex> <head_sats> <fee_sats>
//! Optional funding input (when the chain head alone can't pay head+fee):
//!   FUND_MNEMONIC_KEY=<.env key of a BIP-39 mnemonic> FUND_INDEX=<n> FUND_TXID=<be> FUND_VOUT=<n>
//! spends m/84'/0'/0'/0/<n> of that wallet as input[1]; change returns to the operator's main address.
//! Dry-run unless ANCHOR_STATEMENT_CONFIRM=yes.

use anyhow::{anyhow, Result};
use bitcoin::absolute::LockTime;
use bitcoin::consensus::encode::serialize;
use bitcoin::secp256k1::{Message, Secp256k1};
use bitcoin::sighash::{EcdsaSighashType, SighashCache};
use bitcoin::{
    Address as BTCAddress, Amount, OutPoint, ScriptBuf, Sequence, Transaction, TxIn, TxOut, Txid,
    Witness,
};
use bitcoin::bip32::{DerivationPath, Xpriv};
use bitcoin::CompressedPublicKey;
use ipow_core::btc::btc_service::{mnemonic_to_seed_unchecked, to_legacy_serialization_strict};
use bitcoin::hashes::{sha256, Hash};
use std::str::FromStr;

fn to_hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}
fn from_hex(s: &str) -> Result<Vec<u8>> {
    let s = s.strip_prefix("0x").unwrap_or(s);
    (0..s.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&s[i..i + 2], 16).map_err(|e| anyhow!(e)))
        .collect()
}
fn read_env_file(key: &str) -> Result<String> {
    let path = "/Users/kelwin/ipow/core/operator/.env";
    let content = std::fs::read_to_string(path)?;
    for line in content.lines() {
        if let Some(rest) = line.strip_prefix(&format!("{key}=")) {
            return Ok(rest.trim().trim_matches('"').to_string());
        }
    }
    Err(anyhow!("{key} not found in .env"))
}

#[derive(serde::Deserialize)]
struct Utxo {
    txid: String,
    vout: u32,
    value: u64,
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().collect();
    if args.len() != 7 {
        eprintln!("Usage: {} <prev_txid_be> <prev_vout> <kind> <statement_hex> <head_sats> <fee_sats>", args[0]);
        std::process::exit(1);
    }
    let prev_txid = Txid::from_str(&args[1])?;
    let prev_vout: u32 = args[2].parse()?;
    let kind: u8 = args[3].parse()?;
    let statement = from_hex(&args[4])?;
    let head_sats: u64 = args[5].parse()?;
    let fee_sats: u64 = args[6].parse()?;
    if statement.first().copied() != Some(kind) {
        return Err(anyhow!("statement[0] must equal kind"));
    }

    let wif = read_env_file("OPERATOR_BTC_WALLET_PRIVATE_KEY")?;
    let address = read_env_file("OPERATOR_BTC_WALLET_ADDRESS")?;
    let network = bitcoin::Network::Bitcoin;
    let private_key = bitcoin::PrivateKey::from_wif(&wif)?;
    let secp = Secp256k1::new();
    let own_script = BTCAddress::from_str(&address)?.require_network(network)?.script_pubkey();

    // The outpoint being spent must be ours and confirmed; fetch its value.
    let client = reqwest::Client::new();
    let utxos: Vec<Utxo> = client
        .get(format!("https://blockstream.info/api/address/{address}/utxo"))
        .send()
        .await?
        .json()
        .await?;
    // HEAD_VALUE / FUND_VALUE let a chain of anchors be built offline before
    // its parents are broadcast (values then can't be looked up on esplora).
    let head_value: u64 = match std::env::var("HEAD_VALUE") {
        Ok(v) => v.parse()?,
        Err(_) => utxos
            .iter()
            .find(|u| u.txid == args[1] && u.vout == prev_vout)
            .ok_or_else(|| anyhow!("outpoint {}:{} is not an unspent output of {address}", args[1], prev_vout))?
            .value,
    };
    let utxo = Utxo { txid: args[1].clone(), vout: prev_vout, value: head_value };
    // EXTRA_OUT: an additional output[2] to the operator's own address, e.g. a
    // fresh chain head for another party; change moves to output[3].
    let extra_out: u64 = std::env::var("EXTRA_OUT").ok().map(|v| v.parse::<u64>()).transpose()?.unwrap_or(0);
    // Optional second input: FUND_MAIN=1 (operator's main wallet) or a mnemonic-derived wallet.
    struct Fund { txid: Txid, vout: u32, value: u64, key: bitcoin::PrivateKey, script: ScriptBuf }
    let fund: Option<Fund> = if std::env::var("FUND_MAIN").is_ok() {
        let ftxid = Txid::from_str(&std::env::var("FUND_TXID")?)?;
        let fvout: u32 = std::env::var("FUND_VOUT")?.parse()?;
        let value: u64 = match std::env::var("FUND_VALUE") {
            Ok(v) => v.parse()?,
            Err(_) => utxos
                .iter()
                .find(|u| u.txid == ftxid.to_string() && u.vout == fvout)
                .ok_or_else(|| anyhow!("funding outpoint not found on {address}"))?
                .value,
        };
        println!("funding input (main wallet): {ftxid}:{fvout} = {value} sats");
        Some(Fund { txid: ftxid, vout: fvout, value, key: private_key, script: own_script.clone() })
    } else { match std::env::var("FUND_MNEMONIC_KEY") {
        Ok(k) => {
            let mnemonic = read_env_file(&k)?;
            let idx: u32 = std::env::var("FUND_INDEX")?.parse()?;
            let ftxid = Txid::from_str(&std::env::var("FUND_TXID")?)?;
            let fvout: u32 = std::env::var("FUND_VOUT")?.parse()?;
            let seed = mnemonic_to_seed_unchecked(&mnemonic, "");
            let xprv = Xpriv::new_master(network, &seed)?;
            let path: DerivationPath = format!("m/84'/0'/0'/0/{idx}").parse()?;
            let child = xprv.derive_priv(&secp, &path)?;
            let key = bitcoin::PrivateKey { inner: child.private_key, network: network.into(), compressed: true };
            let pk = CompressedPublicKey::from_private_key(&secp, &key)?;
            let addr = BTCAddress::p2wpkh(&pk, network);
            let futxos: Vec<Utxo> = client
                .get(format!("https://blockstream.info/api/address/{addr}/utxo"))
                .send().await?.json().await?;
            let fu = futxos.iter().find(|u| u.txid == ftxid.to_string() && u.vout == fvout)
                .ok_or_else(|| anyhow!("funding outpoint not found on {addr}"))?;
            println!("funding input: {addr} {}:{} = {} sats (index {idx})", fu.txid, fu.vout, fu.value);
            Some(Fund { txid: ftxid, vout: fvout, value: fu.value, key, script: addr.script_pubkey() })
        }
        Err(_) => None,
    } };
    let total_in = utxo.value + fund.as_ref().map(|f| f.value).unwrap_or(0);
    if total_in < head_sats + fee_sats + extra_out {
        return Err(anyhow!("inputs hold {total_in} sats, need head {head_sats} + fee {fee_sats} + extra {extra_out}"));
    }
    let change = total_in - head_sats - fee_sats - extra_out;

    let hash = sha256::Hash::hash(&statement).to_byte_array();
    let mut payload = vec![0x6a, 0x22, 0x01, kind];
    payload.extend_from_slice(&hash);

    let mut tx = Transaction {
        version: bitcoin::transaction::Version::TWO,
        lock_time: LockTime::ZERO,
        input: vec![TxIn {
            previous_output: OutPoint { txid: prev_txid, vout: prev_vout },
            script_sig: ScriptBuf::new(),
            sequence: Sequence::ENABLE_RBF_NO_LOCKTIME,
            witness: Witness::new(),
        }],
        output: vec![
            TxOut { value: Amount::from_sat(head_sats), script_pubkey: own_script.clone() },
            TxOut { value: Amount::ZERO, script_pubkey: ScriptBuf::from(payload) },
        ],
    };
    if let Some(f) = &fund {
        tx.input.push(TxIn {
            previous_output: OutPoint { txid: f.txid, vout: f.vout },
            script_sig: ScriptBuf::new(),
            sequence: Sequence::ENABLE_RBF_NO_LOCKTIME,
            witness: Witness::new(),
        });
    }
    if extra_out > 0 {
        tx.output.push(TxOut { value: Amount::from_sat(extra_out), script_pubkey: own_script.clone() });
    }
    // Change ≥ dust goes back to the operator's main wallet; smaller change is left as fee.
    if change >= 294 {
        tx.output.push(TxOut { value: Amount::from_sat(change), script_pubkey: own_script.clone() });
    }
    let mut sign = |i: usize, script: &ScriptBuf, value: u64, key: &bitcoin::PrivateKey, tx: &mut Transaction| -> Result<()> {
        let sighash = SighashCache::new(&*tx).p2wpkh_signature_hash(i, script, Amount::from_sat(value), EcdsaSighashType::All)?;
        let sig = secp.sign_ecdsa(&Message::from_digest_slice(&sighash[..])?, &key.inner);
        let mut sig_ser = sig.serialize_der().to_vec();
        sig_ser.push(EcdsaSighashType::All as u8);
        tx.input[i].witness.push(sig_ser);
        tx.input[i].witness.push(key.public_key(&secp).to_bytes());
        Ok(())
    };
    sign(0, &own_script, utxo.value, &private_key, &mut tx)?;
    if let Some(f) = &fund {
        sign(1, &f.script, f.value, &f.key, &mut tx)?;
    }

    let tx_hex = to_hex(&serialize(&tx));
    let legacy = to_legacy_serialization_strict(&format!("0x{tx_hex}"))?;
    println!("statement_sha256: {}", to_hex(&hash));
    println!("txid: {}", tx.compute_txid());
    println!("new_head: {}:0 ({head_sats} sats)", tx.compute_txid());
    for (i, o) in tx.output.iter().enumerate().skip(2) {
        println!("output[{i}]: {} sats", o.value.to_sat());
    }
    println!("vsize: {} fee: {fee_sats} sats", tx.vsize());
    println!("witness_tx_hex: {tx_hex}");
    println!("legacy_tx_hex: {}", to_hex(&legacy));

    if std::env::var("ANCHOR_STATEMENT_CONFIRM").as_deref() != Ok("yes") {
        println!("\nDRY RUN -- set ANCHOR_STATEMENT_CONFIRM=yes to broadcast.");
        return Ok(());
    }
    let resp = client
        .post("https://blockstream.info/api/tx")
        .header("Content-Type", "text/plain")
        .body(tx_hex)
        .send()
        .await?;
    println!("broadcast: {} {}", resp.status(), resp.text().await?);
    Ok(())
}
