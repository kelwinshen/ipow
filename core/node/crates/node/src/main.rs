//! `ipow-node --settings node.yml` runs the roles chosen in the settings on
//! the networks listed there, until stopped with Ctrl-C.

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use anyhow::Context;
use clap::Parser;
use ipow_bitcoin::explorer::Explorer;
use ipow_bitcoin::view::BitcoinView;
use ipow_bitcoin::wallet::Wallet;
use ipow_network_evm::conversion::EvmConversion;
use ipow_network_evm::network::EvmNetwork;
use ipow_network_evm::vault::EvmVault;
use ipow_network_svm::vault::SvmVault;
use ipow_node::vault::{Side, VaultGuardian, VaultOperator, VaultOperatorSettings};
use ipow_protocol_core::vault::VaultApp;
use ipow_network_svm::conversion::SvmConversion;
use ipow_node::swaps::Swaps;
use ipow_protocol_core::conversion::ConversionApp;
use ipow_network_svm::chain::Rpc;
use ipow_network_svm::network::{SvmNetwork, parse_key};
use ipow_node::attester::Attester;
use ipow_node::guardian::Guardian;
use ipow_node::operator::Operator;
use ipow_node::secrets;
use ipow_node::supervisor::{self, Timing, Worker};
use ipow_node::wallet::SharedWallet;
use ipow_protocol_core::network::ProtocolNetwork;
use ipow_protocol_core::settings::{NetworkKind, NetworkSettings, Role, Settings};
use tokio::sync::watch;
use tracing::info;

#[derive(Parser, Debug)]
#[command(version, about = "The iPoW node: runs the operator, guardian and attester roles")]
struct Cli {
    /// The settings file. It holds no secrets; see crates/protocol/src/settings.rs.
    #[arg(long, default_value = "node.yml")]
    settings: PathBuf,
    /// Only read and check the settings, then stop.
    #[arg(long)]
    check: bool,
}

/// A secret or an endpoint, from the environment. Registered with
/// `secrets`, so that no log shows it.
fn env(name: &str) -> anyhow::Result<String> {
    let value = std::env::var(name).with_context(|| format!("the environment variable {name} is not set"))?;
    secrets::hide(&value);
    Ok(value)
}

/// A network, its Conversion application when the settings name one, and
/// the protocol's vault on it when the settings' `vault` names it.
struct Connected {
    net: Arc<dyn ProtocolNetwork>,
    conversion: Option<Arc<dyn ConversionApp>>,
    vault: Option<Arc<dyn VaultApp>>,
}

fn connect(n: &NetworkSettings, vault: Option<&str>) -> anyhow::Result<Connected> {
    let (rpc, key) = (env(&n.rpc_env)?, env(&n.key_env)?);
    Ok(match n.kind {
        NetworkKind::Evm => {
            let net = Arc::new(EvmNetwork::connect(&n.name, &rpc, &key, &n.light_client, &n.protocol)?);
            let conversion = match &n.conversion {
                Some(c) => Some(Arc::new(EvmConversion::new(net.clone(), &c.address)?) as Arc<dyn ConversionApp>),
                None => None,
            };
            let vault = match vault {
                Some(a) => Some(Arc::new(EvmVault::new(net.clone(), a)?) as Arc<dyn VaultApp>),
                None => None,
            };
            Connected { net, conversion, vault }
        }
        NetworkKind::Svm => {
            let net = Arc::new(SvmNetwork::connect(&n.name, Arc::new(Rpc::new(&rpc)), parse_key(&key)?, &n.light_client, &n.protocol)?);
            let conversion = match &n.conversion {
                Some(c) => Some(Arc::new(SvmConversion::new(net.clone(), &c.address)?) as Arc<dyn ConversionApp>),
                None => None,
            };
            let vault = match vault {
                Some(a) => Some(Arc::new(SvmVault::new(net.clone(), a)?) as Arc<dyn VaultApp>),
                None => None,
            };
            Connected { net, conversion, vault }
        }
    })
}

/// An amount the settings already checked.
fn amount(value: Option<&String>) -> u128 {
    value.and_then(|v| v.parse().ok()).expect("checked when the settings were read")
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()))
        .init();
    let cli = Cli::parse();
    let text = std::fs::read_to_string(&cli.settings)
        .map_err(|e| anyhow::anyhow!("cannot read {}: {e}", cli.settings.display()))?;
    let settings = Settings::parse(&text)?;
    info!(roles = ?settings.roles, networks = settings.networks.len(), "settings are valid");
    if cli.check {
        return Ok(());
    }
    // Keys and endpoints, from `.env` when there is one.
    let _ = dotenvy::dotenv();

    // An explorer URL may carry an API key too.
    secrets::hide(&settings.bitcoin.explorer);
    let btc: Arc<dyn BitcoinView> = Arc::new(Explorer::new(&settings.bitcoin.explorer)?);
    let wallet = match &settings.bitcoin.wallet_key_env {
        Some(key) if settings.runs(Role::Operator) => {
            let w = Wallet::from_wif(&env(key)?)?;
            info!(address = %w.address(), "Bitcoin wallet");
            Some(Arc::new(SharedWallet::new(w, btc.clone(), settings.bitcoin.max_fee_rate)))
        }
        _ => None,
    };

    let mut pairs: Vec<(Arc<dyn Worker>, Arc<dyn ProtocolNetwork>)> = vec![];
    let mut vault_sides: [Option<Side>; 2] = [None, None];
    for n in &settings.networks {
        let vault_address = settings.vault.as_ref().and_then(|v| {
            if v.ethereum == n.name {
                Some(v.ethereum_vault.as_str())
            } else if v.solana == n.name {
                Some(v.solana_vault.as_str())
            } else {
                None
            }
        });
        let Connected { net: network, conversion, vault } = connect(n, vault_address)?;
        if let (Some(v), Some(vault)) = (&settings.vault, vault) {
            let i = if v.ethereum == n.name { 0 } else { 1 };
            vault_sides[i] = Some(Side { vault, net: network.clone() });
        }
        info!(network = %n.name, address = %network.me(), "connected");
        for role in &settings.roles {
            let worker: Arc<dyn Worker> = match role {
                Role::Operator => {
                    let wallet = wallet.clone().expect("checked when the settings were read");
                    let mut operator = Operator::new(wallet, amount(n.operator.as_ref().map(|l| &l.max_bid)));
                    if let (Some(app), Some(c)) = (&conversion, &n.conversion) {
                        operator = operator.with_swaps(Arc::new(Swaps::new(app.clone(), c.coins.clone(), &n.name)));
                    }
                    Arc::new(operator)
                }
                Role::Guardian => Arc::new(Guardian::new(btc.clone(), amount(n.guardian.as_ref().map(|l| &l.max_deposit)))),
                Role::Attester => Arc::new(Attester::new(btc.clone(), amount(n.attester.as_ref().map(|l| &l.max_lock)))),
            };
            pairs.push((worker, network.clone()));
        }
    }

    // The vault's roles work on both networks at once; the supervisor names
    // them by the Ethereum side.
    if let (Some(v), [Some(eth), Some(sol)]) = (&settings.vault, &vault_sides) {
        if settings.runs(Role::Operator) {
            let wallet = wallet.clone().expect("checked when the settings were read");
            let op = VaultOperator::new(
                eth.clone(),
                sol.clone(),
                btc.clone(),
                wallet,
                VaultOperatorSettings {
                    eth_bond: v.bond.parse().expect("checked when the settings were read"),
                    veth_bond: v.veth_bond.parse().expect("checked when the settings were read"),
                    deposits: v.deposits,
                    min_fee_gwei: v.min_fee_gwei,
                    checkpoint_paid: [
                        v.checkpoint_paid_ethereum.as_ref().map(|a| a.parse().expect("checked when the settings were read")),
                        v.checkpoint_paid_solana.as_ref().map(|a| a.parse().expect("checked when the settings were read")),
                    ],
                    journal: PathBuf::from(&v.journal),
                },
            );
            info!(journal = %v.journal, "vault operator");
            pairs.push((Arc::new(op), eth.net.clone()));
        }
        if settings.runs(Role::Guardian) {
            pairs.push((Arc::new(VaultGuardian::new(eth.clone(), sol.clone())), eth.net.clone()));
        }
    }

    let (stop, stopped) = watch::channel(false);
    let tasks = supervisor::start(pairs, Timing::from_interval(Duration::from_secs(settings.interval_seconds)), stopped);
    tokio::signal::ctrl_c().await?;
    info!("stopping");
    stop.send(true)?;
    for t in tasks {
        let _ = t.await;
    }
    Ok(())
}
