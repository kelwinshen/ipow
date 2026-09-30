//! BETA v2 (docs/design/ipow-implementation.md §6.2): builds, signs and (optionally)
//! broadcasts one *statement-chain anchor* from the operator's mainnet
//! wallet. Thin CLI wrapper over `ipow_core::btc::statement_chain` — the
//! actual tx-building/signing logic now lives there so `beta_operator` can
//! drive the same code in a loop instead of a human re-running this.
//!
//! Usage:
//!   cargo run --example anchor_statement -- <prev_txid_be> <prev_vout> <kind 1..4> <statement_hex> <head_sats> <fee_sats>
//! Optional funding input (when the chain head alone can't pay head+fee):
//!   FUND_MNEMONIC_KEY=<.env key of a BIP-39 mnemonic> FUND_INDEX=<n> FUND_TXID=<be> FUND_VOUT=<n>
//! spends m/84'/0'/0'/0/<n> of that wallet as input[1]; change returns to the operator's main address.
//! Dry-run unless ANCHOR_STATEMENT_CONFIRM=yes.

use anyhow::{anyhow, Result};
use bitcoin::Txid;
use ipow_core::btc::statement_chain::{
    build_and_sign_statement_anchor, broadcast, fetch_utxo_value,
    parse_address, parse_wif, ChainHead, FundingInput,
    StatementAnchorParams,
};
use std::str::FromStr;

const ESPLORA_BASE: &str = "https://blockstream.info/api";

fn from_hex(s: &str) -> Result<Vec<u8>> {
    let s = s.strip_prefix("0x").unwrap_or(s);
    (0..s.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&s[i..i + 2], 16).map_err(|e| anyhow!(e)))
        .collect()
}

fn to_hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
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

    let network = bitcoin::Network::Bitcoin;
    let wif = read_env_file("OPERATOR_BTC_WALLET_PRIVATE_KEY")?;
    let address_str = read_env_file("OPERATOR_BTC_WALLET_ADDRESS")?;
    let operator_private_key = parse_wif(&wif)?;
    let operator_address = parse_address(&address_str, network)?;

    let client = reqwest::Client::new();

    // HEAD_VALUE lets a chain of anchors be built offline before its
    // parents are broadcast (the real value then can't be looked up yet).
    let head_value: u64 = match std::env::var("HEAD_VALUE") {
        Ok(v) => v.parse()?,
        Err(_) => {
            fetch_utxo_value(
                &client,
                ESPLORA_BASE,
                &address_str,
                &prev_txid,
                prev_vout,
            )
            .await?
        },
    };
    let head = ChainHead { txid: prev_txid, vout: prev_vout, value_sats: head_value };

    // EXTRA_OUT: an additional output[2] to the operator's own address,
    // e.g. a fresh chain head for another party; change moves to output[3].
    let extra_out_sats: u64 = std::env::var("EXTRA_OUT")
        .ok()
        .map(|v| v.parse::<u64>())
        .transpose()?
        .unwrap_or(0);

    // Optional second input: FUND_MAIN=1 (operator's main wallet) or a
    // mnemonic-derived wallet via FUND_MNEMONIC_KEY.
    let funding: Option<FundingInput> = if std::env::var("FUND_MAIN").is_ok() {
        let ftxid = Txid::from_str(&std::env::var("FUND_TXID")?)?;
        let fvout: u32 = std::env::var("FUND_VOUT")?.parse()?;
        let value_sats: u64 = match std::env::var("FUND_VALUE") {
            Ok(v) => v.parse()?,
            Err(_) => {
                fetch_utxo_value(
                    &client,
                    ESPLORA_BASE,
                    &address_str,
                    &ftxid,
                    fvout,
                )
                .await?
            },
        };
        println!("funding input (main wallet): {ftxid}:{fvout} = {value_sats} sats");
        Some(FundingInput {
            txid: ftxid,
            vout: fvout,
            value_sats,
            private_key: operator_private_key,
            script_pubkey: operator_address.script_pubkey(),
        })
    } else if let Ok(k) = std::env::var("FUND_MNEMONIC_KEY") {
        let mnemonic = read_env_file(&k)?;
        let idx: u32 = std::env::var("FUND_INDEX")?.parse()?;
        let ftxid = Txid::from_str(&std::env::var("FUND_TXID")?)?;
        let fvout: u32 = std::env::var("FUND_VOUT")?.parse()?;

        let probe = FundingInput::from_mnemonic(
            &mnemonic, idx, network, ftxid, fvout, 0,
        )?;
        let addr = bitcoin::Address::from_script(
            &probe.script_pubkey,
            network,
        )?;
        let value_sats = fetch_utxo_value(
            &client,
            ESPLORA_BASE,
            &addr.to_string(),
            &ftxid,
            fvout,
        )
        .await?;
        println!("funding input: {addr} {ftxid}:{fvout} = {value_sats} sats (index {idx})");
        Some(FundingInput::from_mnemonic(
            &mnemonic, idx, network, ftxid, fvout, value_sats,
        )?)
    } else {
        None
    };

    let result = build_and_sign_statement_anchor(StatementAnchorParams {
        head,
        funding,
        kind,
        statement,
        head_sats,
        fee_sats,
        extra_out_sats,
        operator_private_key,
        operator_address,
    })?;

    println!("statement_sha256: {}", to_hex(&result.statement_sha256));
    println!("txid: {}", result.txid);
    println!("new_head: {}:0 ({head_sats} sats)", result.txid);
    println!("legacy_tx_hex: {}", to_hex(&result.legacy_tx_bytes));
    println!("witness_tx_hex: {}", result.witness_tx_hex);

    if std::env::var("ANCHOR_STATEMENT_CONFIRM").as_deref() != Ok("yes") {
        println!("\nDRY RUN -- set ANCHOR_STATEMENT_CONFIRM=yes to broadcast.");
        return Ok(());
    }
    let body = broadcast(&client, ESPLORA_BASE, &result.witness_tx_hex).await?;
    println!("broadcast: {body}");
    Ok(())
}
