//! Beta's own per-network config — deliberately separate from
//! `EvmConfig`/`dependencies::config::EvmConfig`: Beta's `iPoWV1` header
//! source is a genuinely different deployed instance than Conversion's on
//! the same chain (verified this session: Ethereum Sepolia's Conversion
//! `iPoWV1` is `0x28eCB9ec...`, Beta's is `0xB8ab960D...`), and not every
//! network the operator knows for Conversion has Beta deployed at all.
//!
//! Reads from a separate `beta_networks:` section in `config.yml` (not
//! `networks:`, which stays Conversion's), keyed by
//! `EvmNetwork::string_identifier()` directly — no historical aliasing
//! needed here since this section is new.

use crate::network::EvmNetwork;
use anyhow::{anyhow, Context, Result};
use config::{Config, File};
use std::env;

#[derive(Clone)]
pub struct BetaConfig {
    pub network: EvmNetwork,
    pub rpc_url: String,
    pub operator_private_key: String,
    /// Beta's own `iPoWV1` header-source address on this chain.
    pub ipow_headers_address: String,
    /// Set only on networks configured as a mint hub (`BetaHub`).
    pub hub_address: Option<String>,
    /// Set only on networks configured as a remote leg (`BetaVault`).
    pub vault_address: Option<String>,
    /// This operator's Beta party identity on this chain (32 bytes,
    /// hex-encoded in config — was a hardcoded `0x05`-repeated convention
    /// in every ad hoc script this session; now a real config value).
    pub party_id: [u8; 32],
}

impl BetaConfig {
    pub fn load(network: EvmNetwork) -> Result<Self> {
        dotenvy::dotenv().ok();

        let settings = Config::builder()
            .add_source(File::with_name("config").required(true))
            .build()
            .context("Failed to load config.yml")?;

        let net_key = network.string_identifier();
        let base = format!("beta_networks.{net_key}");

        let rpc_url = settings
            .get_string(&format!("{base}.rpc_url"))
            .with_context(|| {
                format!("Missing {base}.rpc_url in config.yml — this network has no Beta config yet")
            })?;

        let ipow_headers_address = settings
            .get_string(&format!("{base}.ipow_headers_address"))
            .with_context(|| format!("Missing {base}.ipow_headers_address"))?;

        let hub_address =
            settings.get_string(&format!("{base}.hub_address")).ok();
        let vault_address =
            settings.get_string(&format!("{base}.vault_address")).ok();

        if hub_address.is_none() && vault_address.is_none() {
            return Err(anyhow!(
                "{base} has no hub_address or vault_address set — nothing \
                 for a Beta engine to do on this network"
            ));
        }

        let operator_private_key =
            env::var(network.operator_private_key()).with_context(|| {
                format!(
                    "Missing {} in .env",
                    network.operator_private_key()
                )
            })?;

        let party_id_hex = env::var(beta_party_id_env(network))
            .with_context(|| {
                format!("Missing {} in .env", beta_party_id_env(network))
            })?;
        let party_id = parse_party_id(&party_id_hex)?;

        Ok(Self {
            network,
            rpc_url,
            operator_private_key,
            ipow_headers_address,
            hub_address,
            vault_address,
            party_id,
        })
    }
}

/// `.env` key for this network's Beta party identity, e.g.
/// `ETHEREUM_BETA_PARTY_ID`.
pub fn beta_party_id_env(network: EvmNetwork) -> String {
    format!(
        "{}_BETA_PARTY_ID",
        network.string_identifier().to_uppercase()
    )
}

fn parse_party_id(hex_str: &str) -> Result<[u8; 32]> {
    let s = hex_str.strip_prefix("0x").unwrap_or(hex_str);
    if s.len() != 64 {
        return Err(anyhow!(
            "party id must be 32 bytes (64 hex chars), got {} chars",
            s.len()
        ));
    }
    let mut out = [0u8; 32];
    for i in 0..32 {
        out[i] = u8::from_str_radix(&s[i * 2..i * 2 + 2], 16)
            .map_err(|e| anyhow!("invalid party id hex: {e}"))?;
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Read-only regression check: every network wired into
    /// `config.yml`'s new `beta_networks:` section this session parses
    /// cleanly, with the expected role(s) set. Makes no network calls —
    /// `config`/`env` parsing only.
    #[test]
    fn all_configured_beta_networks_load() {
        // `config`/`dotenvy` resolve `config.yml`/`.env` relative to CWD at
        // call time, and Cargo runs each crate's test binary with CWD set
        // to that crate's own directory — not the workspace root where
        // these files actually live (same pre-existing constraint as
        // `EvmConfig::load`, which every real binary invocation already
        // satisfies by being run from `core/operator/`).
        std::env::set_current_dir(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../../.."),
        )
        .expect("failed to cd to core/operator for config.yml/.env lookup");

        let cases: &[(EvmNetwork, bool, bool)] = &[
            (EvmNetwork::EthereumSepolia, true, true),
            (EvmNetwork::Hedera, true, true),
            (EvmNetwork::PolkadotHub, true, true),
            (EvmNetwork::Base, true, true),
            (EvmNetwork::Robinhood, true, true),
            (EvmNetwork::Hyperliquid, true, true),
            (EvmNetwork::Tempo, true, true),
        ];

        for &(network, expect_hub, expect_vault) in cases {
            let cfg = BetaConfig::load(network).unwrap_or_else(|e| {
                panic!(
                    "expected {} to have valid beta_networks config: {e}",
                    network.string_identifier()
                )
            });
            assert_eq!(
                cfg.hub_address.is_some(),
                expect_hub,
                "{}: hub_address",
                network.string_identifier()
            );
            assert_eq!(
                cfg.vault_address.is_some(),
                expect_vault,
                "{}: vault_address",
                network.string_identifier()
            );
            assert!(!cfg.ipow_headers_address.is_empty());
            assert!(!cfg.rpc_url.is_empty());
            assert!(!cfg.operator_private_key.is_empty());
            assert_eq!(cfg.party_id.len(), 32);
        }
    }
}
