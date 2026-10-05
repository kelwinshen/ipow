//! The settings of a node, read from a YAML file. The file holds no
//! secrets: it names the environment variables that hold keys and endpoints
//! with keys, and those come from the environment or a `.env` file that git
//! ignores.
//!
//! ```yaml
//! roles: [operator, guardian]        # any mix of operator, guardian, attester
//! bitcoin:
//!   explorer: https://mempool.space/api
//!   wallet_key_env: BTC_WALLET_KEY   # needed only for the operator role
//! networks:
//!   - name: ethereum-sepolia
//!     kind: evm
//!     rpc_env: SEPOLIA_RPC_URL
//!     key_env: SEPOLIA_KEY
//!     light_client: "0x..."
//!     protocol: "0x..."
//!     operator: { max_bid: "2000000000000000000" }
//!     guardian: { max_deposit: "100000000000000000" }
//! ```

use std::collections::HashSet;

use serde::Deserialize;
use thiserror::Error;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    Operator,
    Guardian,
    Attester,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum NetworkKind {
    Evm,
    Svm,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Settings {
    pub roles: Vec<Role>,
    pub bitcoin: BitcoinSettings,
    pub networks: Vec<NetworkSettings>,
    /// Seconds between two rounds of a role on a network.
    #[serde(default = "default_interval")]
    pub interval_seconds: u64,
    /// The protocol's vault (spec section 11), for the pair Ethereum and
    /// Solana: the operator carries records, the guardian checks claims.
    pub vault: Option<VaultSettings>,
    /// Tunnels between programmable networks (docs/drafts/ipow-conversion-tunnel.md,
    /// Q6): an HTTP API where an app asks this operator what it trades and
    /// for a quote, and registers the buy its user opened as a tunnel's.
    pub tunnel_api: Option<TunnelApiSettings>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TunnelApiSettings {
    /// Where to listen, e.g. `127.0.0.1:8787`.
    pub listen: String,
    /// An environment variable holding a key callers must send in the
    /// `x-tunnel-key` header: an app's server holds it, browsers do not.
    /// Without it, anyone who reaches `listen` may ask for quotes.
    #[serde(default)]
    pub key_env: Option<String>,
    /// The most satoshis one tunnel moves (Conversion's cap is 100,000 on
    /// the test networks).
    #[serde(default = "default_tunnel_max_sats")]
    pub max_sats: u64,
    /// Whether the quote takes the destination buy's job fees, in sats at
    /// this operator's price for that network's coin, off what the user
    /// receives (Q5). Off on test networks, whose coins and fees are worth
    /// nothing: the operator absorbs them.
    #[serde(default = "default_true")]
    pub price_fees: bool,
}

fn default_true() -> bool {
    true
}

pub fn default_tunnel_max_sats() -> u64 {
    100_000
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct VaultSettings {
    /// The operator: claim deposits to keep in each vault.
    #[serde(default = "default_deposits")]
    pub deposits: u32,
    /// The pairs of networks it works for: a vault per pair (D132).
    pub pairs: Vec<VaultPair>,
}

/// One pair of networks and its two vaults (D132).
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct VaultPair {
    /// The names, in `networks`, of the pair's two networks.
    pub networks: [String; 2],
    /// Each one's vault, in the same order: the pair's vault contract on an
    /// EVM network, the vault program on Solana.
    pub vaults: [String; 2],
    /// The operator: the assets it carries records of, with its bonds and
    /// fees in each (section 11.9). An asset not listed is never carried,
    /// so a token anyone registered costs it nothing.
    #[serde(default)]
    pub assets: Vec<VaultAsset>,
    /// The operator and the guardian: what to pay, on each network in the
    /// same order, to open a checkpoint job when no real block is above a
    /// message (for the guardian, a lie it would bring). None: never opens
    /// one.
    #[serde(default)]
    pub checkpoint_paid: [Option<String>; 2],
    /// The operator: the file that keeps every batch it wrote on this
    /// pair's chain. One per pair.
    pub journal: String,
}

/// One asset the operator carries. Amounts in record units: the receipt's
/// smallest unit (gwei for ETH, lamports for SOL).
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct VaultAsset {
    /// Its home: the name of one of the pair's networks.
    pub home: String,
    /// Its number in its home vault, the network's coin being 0.
    pub asset: u32,
    /// The bond to keep on its home network, in the asset: it covers locks
    /// carried to the other network. 0 carries none.
    #[serde(default)]
    pub bond_home: u64,
    /// The bond to keep on the other network, in its receipt: it covers
    /// burns and give-ups carried home. 0 carries none.
    #[serde(default)]
    pub bond_receipt: u64,
    /// The least fee of a lock or burn worth carrying. A record whose fee
    /// another operator already earned, and whose claims were all refused,
    /// is carried anyway, so that its user is not stranded.
    #[serde(default)]
    pub min_fee: u64,
    /// The fast paths (section 11.7). None: it attests no lock and pays no
    /// burn of this asset at once.
    pub fast: Option<FastSettings>,
}

/// What the operator attests and pays at once, in record units.
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FastSettings {
    /// The least fast fee worth attesting a lock or paying a burn.
    pub min_fee: u64,
    /// The largest lock or burn it attests or pays: it locks 1.25 times a
    /// lock's amount in receipts, or pays a burn's amount in the asset, until
    /// the claim carrying it is accepted.
    pub max: u64,
}

fn default_deposits() -> u32 {
    5
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BitcoinSettings {
    /// An Esplora-compatible API for reading real Bitcoin.
    pub explorer: String,
    /// Others asked, in order, when it cannot answer (it times out, or
    /// fails with a server error).
    #[serde(default)]
    pub fallback_explorers: Vec<String>,
    /// The environment variable that holds the Bitcoin wallet key. Needed
    /// only when the node runs as an operator.
    pub wallet_key_env: Option<String>,
    /// The highest fee rate the wallet pays, in satoshis per virtual byte,
    /// whatever the explorer says and however often a transaction is
    /// replaced to pay more.
    #[serde(default = "default_max_fee_rate")]
    pub max_fee_rate: f64,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NetworkSettings {
    pub name: String,
    pub kind: NetworkKind,
    /// The environment variable that holds the network's endpoint.
    pub rpc_env: String,
    /// The environment variable that holds the node's key on this network.
    pub key_env: String,
    /// The light client: a contract address on EVM, a program id on Solana.
    pub light_client: String,
    /// The protocol: a contract address on EVM, a program id on Solana.
    pub protocol: String,
    /// Limits per role on this network, in its smallest unit (D84).
    pub operator: Option<OperatorLimits>,
    pub guardian: Option<GuardianLimits>,
    pub attester: Option<AttesterLimits>,
    /// The Conversion application on this network, for an operator that
    /// takes swaps (docs/drafts/ipow-conversion-app.md).
    pub conversion: Option<ConversionSettings>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ConversionSettings {
    /// Conversion's address: its contract, or its program.
    pub address: String,
    /// The coins it swaps, and at what price. A swap of a coin not listed
    /// is never taken.
    pub coins: Vec<CoinPrice>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CoinPrice {
    /// `native` for the network's own coin, or the token's address.
    pub token: String,
    /// What apps show it as (ETH, AAPL); the token itself when left out.
    #[serde(default)]
    pub symbol: Option<String>,
    /// Decimal places of one whole coin: 18 for ETH, 9 for SOL.
    pub decimals: u8,
    /// Users selling: the most satoshis it pays for one whole coin.
    pub pay_sats: u64,
    /// Users buying: the fewest satoshis it takes for one whole coin.
    pub ask_sats: u64,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OperatorLimits {
    /// The most bond it locks for one job.
    pub max_bid: String,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GuardianLimits {
    /// The most it puts up as the deposit of one challenge.
    pub max_deposit: String,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AttesterLimits {
    /// The most it locks for one job.
    pub max_lock: String,
}

fn default_interval() -> u64 {
    15
}

fn default_max_fee_rate() -> f64 {
    200.0
}

#[derive(Debug, Error, PartialEq)]
pub enum SettingsError {
    #[error("the settings are not valid YAML: {0}")]
    Parse(String),
    #[error("choose at least one role: operator, guardian or attester")]
    NoRole,
    #[error("the role {0:?} is listed twice")]
    RepeatedRole(Role),
    #[error("list at least one network")]
    NoNetwork,
    #[error("the network name {0} is used twice")]
    RepeatedNetwork(String),
    #[error("the operator role needs bitcoin.wallet_key_env, the variable that holds its Bitcoin key")]
    OperatorWithoutWallet,
    #[error("the network {network} has no limits for the role {role:?}; add `{section}:` to it")]
    MissingLimits { network: String, role: Role, section: &'static str },
    #[error("the amount {0} is not a whole number")]
    BadAmount(String),
    #[error("bitcoin.max_fee_rate must be above 0")]
    BadFeeRate,
    #[error("a coin of the network {0} has more than 38 decimal places")]
    TooManyDecimals(String),
    #[error("vault.deposits must be at least 1: with none, no claim of this operator ever opens")]
    NoVaultDeposits,
    #[error("vault.assets lists asset {0} of one network twice")]
    RepeatedVaultAsset(u32),
    #[error("vault.assets bonds more than 8 assets on one network: a chain keeps bonds in at most 8 there")]
    TooManyBonds,
    #[error("the vault names {0}, which is not a network of `networks` (or, for an asset's home, not one of its pair's)")]
    VaultNetwork(String),
    #[error("a vault pair needs two different networks, not both Solana: {0}")]
    BadVaultPair(String),
    #[error("the vault pair {0} is listed twice")]
    RepeatedVaultPair(String),
    #[error("two vault pairs share the journal {0}: each pair keeps its own")]
    RepeatedJournal(String),
}

impl Settings {
    pub fn parse(text: &str) -> Result<Settings, SettingsError> {
        let s: Settings = serde_yaml::from_str(text).map_err(|e| SettingsError::Parse(e.to_string()))?;
        s.validate()?;
        Ok(s)
    }

    pub fn runs(&self, role: Role) -> bool {
        self.roles.contains(&role)
    }

    fn validate(&self) -> Result<(), SettingsError> {
        if self.roles.is_empty() {
            return Err(SettingsError::NoRole);
        }
        let mut seen = HashSet::new();
        for r in &self.roles {
            if !seen.insert(*r) {
                return Err(SettingsError::RepeatedRole(*r));
            }
        }
        if self.networks.is_empty() {
            return Err(SettingsError::NoNetwork);
        }
        let mut names = HashSet::new();
        for n in &self.networks {
            if !names.insert(n.name.clone()) {
                return Err(SettingsError::RepeatedNetwork(n.name.clone()));
            }
        }
        for n in &self.networks {
            if let Some(c) = &n.conversion
                && c.coins.iter().any(|coin| coin.decimals > 38)
            {
                return Err(SettingsError::TooManyDecimals(n.name.clone()));
            }
        }
        if let Some(v) = &self.vault {
            if v.deposits == 0 {
                return Err(SettingsError::NoVaultDeposits);
            }
            let mut journals = HashSet::new();
            let mut listed = HashSet::new();
            for pair in &v.pairs {
                let mut key = pair.networks.clone();
                key.sort();
                if !listed.insert(key) {
                    return Err(SettingsError::RepeatedVaultPair(pair.networks.join(" and ")));
                }
                let kinds: Vec<NetworkKind> = pair
                    .networks
                    .iter()
                    .map(|name| self.networks.iter().find(|n| &n.name == name).map(|n| n.kind).ok_or(SettingsError::VaultNetwork(name.clone())))
                    .collect::<Result<_, _>>()?;
                // Two different networks; Solana pairs with an EVM network.
                if pair.networks[0] == pair.networks[1] || kinds.iter().all(|k| *k == NetworkKind::Svm) {
                    return Err(SettingsError::BadVaultPair(pair.networks.join(" and ")));
                }
                if !journals.insert(pair.journal.clone()) {
                    return Err(SettingsError::RepeatedJournal(pair.journal.clone()));
                }
                let mut seen = HashSet::new();
                for a in &pair.assets {
                    if !pair.networks.contains(&a.home) {
                        return Err(SettingsError::VaultNetwork(a.home.clone()));
                    }
                    if !seen.insert((a.home.clone(), a.asset)) {
                        return Err(SettingsError::RepeatedVaultAsset(a.asset));
                    }
                }
                // D129: a chain keeps bonds in at most 8 assets on a network:
                // its home bonds there and its receipt bonds of the other's.
                for net in &pair.networks {
                    let bonded = pair.assets.iter().filter(|a| if &a.home == net { a.bond_home > 0 } else { a.bond_receipt > 0 }).count();
                    if bonded > crate::vault::MAX_ASSETS {
                        return Err(SettingsError::TooManyBonds);
                    }
                }
                for amount in pair.checkpoint_paid.iter().flatten() {
                    amount.parse::<u128>().map_err(|_| SettingsError::BadAmount(amount.clone()))?;
                }
            }
        }
        if !(self.bitcoin.max_fee_rate > 0.0) {
            return Err(SettingsError::BadFeeRate);
        }
        if self.runs(Role::Operator) && self.bitcoin.wallet_key_env.is_none() {
            return Err(SettingsError::OperatorWithoutWallet);
        }
        // Every chosen role needs its limits on every network, so that the
        // node never spends without a stated maximum.
        for n in &self.networks {
            let checks: [(Role, Option<&String>, &'static str); 3] = [
                (Role::Operator, n.operator.as_ref().map(|l| &l.max_bid), "operator"),
                (Role::Guardian, n.guardian.as_ref().map(|l| &l.max_deposit), "guardian"),
                (Role::Attester, n.attester.as_ref().map(|l| &l.max_lock), "attester"),
            ];
            for (role, limit, section) in checks {
                if !self.runs(role) {
                    continue;
                }
                let value = limit.ok_or(SettingsError::MissingLimits {
                    network: n.name.clone(),
                    role,
                    section,
                })?;
                value.parse::<u128>().map_err(|_| SettingsError::BadAmount(value.clone()))?;
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const NETWORK: &str = r#"
  - name: ethereum-sepolia
    kind: evm
    rpc_env: SEPOLIA_RPC_URL
    key_env: SEPOLIA_KEY
    light_client: "0x01"
    protocol: "0x02"
    operator: { max_bid: "2000000000000000000" }
    guardian: { max_deposit: "100000000000000000" }
    attester: { max_lock: "1000000000000000000" }
"#;

    fn with(roles: &str, wallet: bool, networks: &str) -> String {
        let wallet = if wallet { "\n  wallet_key_env: BTC_WALLET_KEY" } else { "" };
        format!("roles: {roles}\nbitcoin:\n  explorer: https://mempool.space/api{wallet}\nnetworks:{networks}")
    }

    #[test]
    fn reads_every_mix_of_roles() {
        for roles in ["[operator]", "[guardian]", "[attester]", "[operator, guardian]", "[operator, guardian, attester]"] {
            let s = Settings::parse(&with(roles, true, NETWORK)).unwrap();
            assert_eq!(s.networks[0].kind, NetworkKind::Evm);
            assert_eq!(s.interval_seconds, 15);
        }
    }

    #[test]
    fn lets_a_guardian_run_without_a_bitcoin_wallet() {
        let s = Settings::parse(&with("[guardian]", false, NETWORK)).unwrap();
        assert!(s.runs(Role::Guardian));
        assert!(!s.runs(Role::Operator));
    }

    #[test]
    fn needs_a_bitcoin_wallet_for_an_operator() {
        assert_eq!(Settings::parse(&with("[operator]", false, NETWORK)).unwrap_err(), SettingsError::OperatorWithoutWallet);
    }

    #[test]
    fn needs_a_role_and_a_network_and_no_repeats() {
        assert_eq!(Settings::parse(&with("[]", true, NETWORK)).unwrap_err(), SettingsError::NoRole);
        assert_eq!(Settings::parse(&with("[guardian, guardian]", true, NETWORK)).unwrap_err(), SettingsError::RepeatedRole(Role::Guardian));
        assert_eq!(Settings::parse(&with("[guardian]", true, " []")).unwrap_err(), SettingsError::NoNetwork);
        let twice = format!("{NETWORK}{NETWORK}");
        assert_eq!(
            Settings::parse(&with("[guardian]", true, &twice)).unwrap_err(),
            SettingsError::RepeatedNetwork("ethereum-sepolia".into())
        );
    }

    #[test]
    fn reads_the_vault_and_checks_its_networks() {
        let sol = r#"
  - name: solana-devnet
    kind: svm
    rpc_env: SOLANA_RPC_URL
    key_env: SOLANA_KEY
    light_client: "Ctotq1SSaDJ2EUSe4sMdau92qBesyutH7GPaTRiYp4vJ"
    protocol: "ChYhovM8vm2tuMRaRFn4m6etG979bjixVa71fXBDwjPL"
    guardian: { max_deposit: "100000000" }
"#;
        let vault = r#"
vault:
  pairs:
    - networks: [ethereum-sepolia, solana-devnet]
      vaults: ["0x03", "2e1ZUB5eqA6bQfH3f9pbfvibfie7WnvEJeKif53VAm7p"]
      journal: eth-sol.journal
      checkpoint_paid: ["2000000000000000000", null]
      assets:
        - { home: ethereum-sepolia, asset: 0, bond_home: 2000000000, bond_receipt: 1000000000, min_fee: 10, fast: { min_fee: 5, max: 100000000 } }
        - { home: solana-devnet, asset: 0, bond_home: 5000000000 }"#;
        let text = format!("{}{vault}", with("[guardian]", true, &format!("{NETWORK}{sol}")));
        let s = Settings::parse(&text).unwrap();
        let v = s.vault.unwrap();
        assert_eq!(v.deposits, 5);
        let pair = &v.pairs[0];
        assert_eq!(pair.journal, "eth-sol.journal");
        assert_eq!(pair.assets.len(), 2);
        assert_eq!(pair.assets[1].home, "solana-devnet");
        assert_eq!(pair.assets[1].bond_receipt, 0);
        assert!(pair.assets[1].fast.is_none());
        assert_eq!(pair.assets[0].fast.as_ref().unwrap().max, 100_000_000);
        assert!(pair.checkpoint_paid[1].is_none());
        let twice = text.replace("home: solana-devnet, asset: 0", "home: ethereum-sepolia, asset: 0");
        assert_eq!(Settings::parse(&twice).unwrap_err(), SettingsError::RepeatedVaultAsset(0));
        // A network that is not listed, or not one of the pair's for an asset.
        let unknown = text.replace("networks: [ethereum-sepolia, solana-devnet]", "networks: [base-sepolia, solana-devnet]");
        assert_eq!(Settings::parse(&unknown).unwrap_err(), SettingsError::VaultNetwork("base-sepolia".into()));
        let elsewhere = text.replace("home: solana-devnet, asset: 0", "home: base-sepolia, asset: 0");
        assert_eq!(Settings::parse(&elsewhere).unwrap_err(), SettingsError::VaultNetwork("base-sepolia".into()));
        // Two different networks, not both Solana.
        let same = text.replace("networks: [ethereum-sepolia, solana-devnet]", "networks: [solana-devnet, solana-devnet]");
        assert!(matches!(Settings::parse(&same).unwrap_err(), SettingsError::BadVaultPair(_)));
        // A journal per pair.
        // The same pair twice, in any order.
        let twice = format!(
            "{text}\n    - networks: [solana-devnet, ethereum-sepolia]\n      vaults: [\"2e1ZUB5eqA6bQfH3f9pbfvibfie7WnvEJeKif53VAm7p\", \"0x04\"]\n      journal: other.journal"
        );
        assert!(matches!(Settings::parse(&twice).unwrap_err(), SettingsError::RepeatedVaultPair(_)));
        // A journal per pair: Base and Solana may not share Ethereum's.
        let base = NETWORK.replace("ethereum-sepolia", "base-sepolia");
        let three = format!("{}{vault}", with("[guardian]", true, &format!("{NETWORK}{sol}{base}")));
        let shared = format!(
            "{three}\n    - networks: [base-sepolia, solana-devnet]\n      vaults: [\"0x05\", \"2e1ZUB5eqA6bQfH3f9pbfvibfie7WnvEJeKif53VAm7p\"]\n      journal: eth-sol.journal"
        );
        assert_eq!(Settings::parse(&shared).unwrap_err(), SettingsError::RepeatedJournal("eth-sol.journal".into()));
        // With a journal of its own, both pairs are fine.
        let own = shared.replacen("journal: eth-sol.journal", "journal: base-sol.journal", 2).replacen("journal: base-sol.journal", "journal: eth-sol.journal", 1);
        assert_eq!(Settings::parse(&own).unwrap().vault.unwrap().pairs.len(), 2);
        assert_eq!(Settings::parse(&text.replace("  pairs:", "  deposits: 0\n  pairs:")).unwrap_err(), SettingsError::NoVaultDeposits);
        let bad = text.replace("\"2000000000000000000\"", "\"2 ETH\"");
        assert_eq!(Settings::parse(&bad).unwrap_err(), SettingsError::BadAmount("2 ETH".into()));
    }

    #[test]
    fn rejects_an_unknown_role() {
        assert!(matches!(Settings::parse(&with("[miner]", true, NETWORK)).unwrap_err(), SettingsError::Parse(_)));
    }

    #[test]
    fn needs_the_limits_of_every_chosen_role_on_every_network() {
        let no_limits = r#"
  - name: solana-devnet
    kind: svm
    rpc_env: SOLANA_RPC_URL
    key_env: SOLANA_KEY
    light_client: "Ctotq1SSaDJ2EUSe4sMdau92qBesyutH7GPaTRiYp4vJ"
    protocol: "ChYhovM8vm2tuMRaRFn4m6etG979bjixVa71fXBDwjPL"
"#;
        assert_eq!(
            Settings::parse(&with("[attester]", true, no_limits)).unwrap_err(),
            SettingsError::MissingLimits { network: "solana-devnet".into(), role: Role::Attester, section: "attester" }
        );
    }

    #[test]
    fn rejects_an_amount_that_is_not_a_whole_number() {
        let bad = NETWORK.replace("2000000000000000000", "2 ETH");
        assert_eq!(Settings::parse(&with("[operator]", true, &bad)).unwrap_err(), SettingsError::BadAmount("2 ETH".into()));
    }

    #[test]
    fn rejects_a_key_written_into_the_file() {
        // There is no field for a key itself; an unknown field is refused.
        let leaked = NETWORK.replace("key_env: SEPOLIA_KEY", "key_env: SEPOLIA_KEY\n    private_key: \"0xabc\"");
        assert!(matches!(Settings::parse(&with("[guardian]", true, &leaked)).unwrap_err(), SettingsError::Parse(_)));
    }
}
