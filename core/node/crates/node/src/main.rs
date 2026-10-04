//! `ipow-node --settings node.yml` runs the roles chosen in the settings on
//! the networks listed there, until stopped with Ctrl-C.

use std::collections::HashMap;
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
use ipow_node::vault::{CarriedAsset, Side, VaultGuardian, VaultOperator, VaultOperatorSettings};
use ipow_protocol_core::vault::VaultApp;
use ipow_network_svm::conversion::SvmConversion;
use ipow_node::swaps::Swaps;
use ipow_node::tunnel::{TunnelBook, TunnelNetwork, Tunnels};
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
use ipow_protocol_core::settings::{NetworkKind, NetworkSettings, Role, Settings, VaultPair};
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
/// the network itself as its kind, to connect vaults on it.
struct Connected {
    net: Arc<dyn ProtocolNetwork>,
    conversion: Option<Arc<dyn ConversionApp>>,
    kind: Kind,
}

#[derive(Clone)]
enum Kind {
    Evm(Arc<EvmNetwork>),
    Svm(Arc<SvmNetwork>),
}

fn connect(n: &NetworkSettings) -> anyhow::Result<Connected> {
    let (rpc, key) = (env(&n.rpc_env)?, env(&n.key_env)?);
    Ok(match n.kind {
        NetworkKind::Evm => {
            let net = Arc::new(EvmNetwork::connect(&n.name, &rpc, &key, &n.light_client, &n.protocol)?);
            let conversion = match &n.conversion {
                Some(c) => Some(Arc::new(EvmConversion::new(net.clone(), &c.address)?) as Arc<dyn ConversionApp>),
                None => None,
            };
            Connected { net: net.clone(), conversion, kind: Kind::Evm(net) }
        }
        NetworkKind::Svm => {
            let net = Arc::new(SvmNetwork::connect(&n.name, Arc::new(Rpc::new(&rpc)), parse_key(&key)?, &n.light_client, &n.protocol)?);
            let conversion = match &n.conversion {
                Some(c) => Some(Arc::new(SvmConversion::new(net.clone(), &c.address)?) as Arc<dyn ConversionApp>),
                None => None,
            };
            Connected { net: net.clone(), conversion, kind: Kind::Svm(net) }
        }
    })
}

/// The two sides of a vault pair (D132). The EVM side's vault is read first:
/// it tells a Solana side which peer its pair has.
async fn pair_sides(pair: &VaultPair, kinds: &HashMap<String, (Kind, Arc<dyn ProtocolNetwork>)>) -> anyhow::Result<[Side; 2]> {
    let mut evm: [Option<Arc<dyn VaultApp>>; 2] = [None, None];
    for i in 0..2 {
        if let (Kind::Evm(net), _) = &kinds[&pair.networks[i]] {
            evm[i] = Some(Arc::new(EvmVault::connect(net.clone(), &pair.vaults[i]).await?) as Arc<dyn VaultApp>);
        }
    }
    let mut sides = vec![];
    for i in 0..2 {
        let (kind, net) = &kinds[&pair.networks[i]];
        let vault = match kind {
            Kind::Evm(_) => evm[i].clone().unwrap(),
            Kind::Svm(svm) => {
                let peer = evm[1 - i].as_ref().map(|v| v.network_id()).context("Solana pairs with an EVM network")?;
                Arc::new(SvmVault::new(svm.clone(), &pair.vaults[i], peer)?) as Arc<dyn VaultApp>
            }
        };
        sides.push(Side { vault, net: net.clone() });
    }
    let [a, b] = [sides.remove(0), sides.remove(0)];
    ipow_node::vault::check_pair(&a, &b).await.with_context(|| format!("the vault pair {}", pair.networks.join(" and ")))?;
    Ok([a, b])
}

/// What to pay for a checkpoint job, as the settings already checked.
fn checkpoint_paid(pair: &VaultPair) -> [Option<u128>; 2] {
    [0, 1].map(|i| pair.checkpoint_paid[i].as_ref().map(|a| a.parse().expect("checked when the settings were read")))
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
    let btc: Arc<dyn BitcoinView> = Arc::new(Explorer::with_fallbacks(&settings.bitcoin.explorer, &settings.bitcoin.fallback_explorers)?);
    let wallet = match &settings.bitcoin.wallet_key_env {
        Some(key) if settings.runs(Role::Operator) => {
            let w = Wallet::from_wif(&env(key)?)?;
            info!(address = %w.address(), "Bitcoin wallet");
            Some(Arc::new(SharedWallet::new(w, btc.clone(), settings.bitcoin.max_fee_rate)))
        }
        _ => None,
    };

    let mut pairs: Vec<(Arc<dyn Worker>, Arc<dyn ProtocolNetwork>)> = vec![];
    // The networks whose Conversion this operator serves, for the tunnel API.
    let mut tunnel_networks: HashMap<String, TunnelNetwork> = HashMap::new();
    let tunnel_book = Arc::new(TunnelBook::default());
    let mut kinds: HashMap<String, (Kind, Arc<dyn ProtocolNetwork>)> = HashMap::new();
    for n in &settings.networks {
        let Connected { net: network, conversion, kind } = connect(n)?;
        if let (Some(app), Some(c)) = (&conversion, &n.conversion) {
            tunnel_networks.insert(n.name.clone(), TunnelNetwork { app: app.clone(), coins: c.coins.clone() });
            tunnel_book.add_network(&n.name, app.clone(), network.clone());
        }
        kinds.insert(n.name.clone(), (kind, network.clone()));
        info!(network = %n.name, address = %network.me(), "connected");
        for role in &settings.roles {
            let worker: Arc<dyn Worker> = match role {
                Role::Operator => {
                    let wallet = wallet.clone().expect("checked when the settings were read");
                    let mut operator = Operator::new(wallet, amount(n.operator.as_ref().map(|l| &l.max_bid)));
                    if let (Some(app), Some(c)) = (&conversion, &n.conversion) {
                        operator = operator.with_swaps(Arc::new(Swaps::new(app.clone(), c.coins.clone(), &n.name).with_tunnels(tunnel_book.clone())));
                    }
                    Arc::new(operator)
                }
                Role::Guardian => Arc::new(Guardian::new(btc.clone(), amount(n.guardian.as_ref().map(|l| &l.max_deposit)))),
                Role::Attester => Arc::new(Attester::new(btc.clone(), amount(n.attester.as_ref().map(|l| &l.max_lock)))),
            };
            pairs.push((worker, network.clone()));
        }
    }

    // The vault's roles, one of each per pair (D132), work on both of its
    // networks at once; the supervisor names them by the pair's first.
    if let Some(v) = &settings.vault {
        for pair in &v.pairs {
            let [a, b] = pair_sides(pair, &kinds).await?;
            let number = |name: &String| if *name == pair.networks[0] { a.vault.network_id() } else { b.vault.network_id() };
            if settings.runs(Role::Operator) {
                let wallet = wallet.clone().expect("checked when the settings were read");
                let assets = pair
                    .assets
                    .iter()
                    .map(|x| CarriedAsset {
                        home: number(&x.home),
                        asset: x.asset,
                        bond_home: x.bond_home,
                        bond_receipt: x.bond_receipt,
                        min_fee: x.min_fee,
                        fast: x.fast.clone(),
                    })
                    .collect();
                let op = VaultOperator::new(
                    a.clone(),
                    b.clone(),
                    btc.clone(),
                    wallet,
                    VaultOperatorSettings { assets, deposits: v.deposits, checkpoint_paid: checkpoint_paid(pair), journal: PathBuf::from(&pair.journal) },
                )?;
                info!(pair = %pair.networks.join(" and "), journal = %pair.journal, "vault operator");
                pairs.push((Arc::new(op), a.net.clone()));
            }
            if settings.runs(Role::Guardian) {
                let g = VaultGuardian::new(a.clone(), b.clone(), btc.clone(), checkpoint_paid(pair))?;
                info!(pair = %pair.networks.join(" and "), "vault guardian");
                pairs.push((Arc::new(g), a.net.clone()));
            }
        }
    }

    // The tunnel API: an operator's, for the networks it takes swaps on.
    if let Some(api) = &settings.tunnel_api {
        anyhow::ensure!(settings.runs(Role::Operator), "the tunnel API needs the operator role");
        let key = match &api.key_env {
            Some(name) => {
                let k = env(name)?;
                anyhow::ensure!(k.len() >= 16, "the tunnel API's key is too short: at least 16 characters");
                secrets::hide(&k);
                Some(k)
            }
            None => None,
        };
        let wallet = wallet.clone().expect("checked when the settings were read");
        let tunnels = Arc::new(Tunnels::new(tunnel_networks, api.clone(), key, tunnel_book.clone(), wallet));
        tokio::spawn(async move {
            if let Err(e) = tunnels.serve().await {
                tracing::error!(error = %e, "the tunnel API stopped");
            }
        });
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
