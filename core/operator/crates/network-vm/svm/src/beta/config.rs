//! Beta's own Solana config — parallel to EVM's `beta::config::BetaConfig`,
//! separate from `dependencies::config::SolanaConfig` (Conversion's own,
//! whose `contract_address` points at a different, older deployment —
//! confirmed this session by comparing against `Anchor.toml`'s current
//! program ids). Reads from a `beta_networks.solana` section in
//! `config.yml`, distinct from Conversion's `networks.solana_devnet`.

use anyhow::{Context, Result};
use config::{Config, File};
use std::env;

#[derive(Clone)]
pub struct BetaSvmConfig {
    pub rpc_url: String,
    /// Base58-encoded operator keypair, same format `SolanaConfig` already
    /// uses.
    pub operator_private_key: String,
    /// `beta_factory`'s program id.
    pub hub_program_id: String,
    /// `ipow`'s program id — the header-relay source `process_anchor`
    /// reads `GlobalHeader` accounts from. A genuinely different program
    /// than Conversion's own (mirrors EVM Beta's separate `iPoW`
    /// instance), but on Solana a program is a singleton per deployment,
    /// so unlike EVM this is very likely the *same* `ipow` program
    /// Conversion's `networks.solana_devnet.contract_address` should
    /// also point at (that entry currently looks stale — out of scope
    /// here).
    pub ipow_program_id: String,
    /// This operator's Beta party identity on Solana (32 bytes) — the
    /// same cross-chain identity registered on every EVM hub/spoke too.
    pub party_id: [u8; 32],
}

impl BetaSvmConfig {
    pub fn load() -> Result<Self> {
        dotenvy::dotenv().ok();

        let settings = Config::builder()
            .add_source(File::with_name("config").required(true))
            .build()
            .context("Failed to load config.yml")?;

        let base = "beta_networks.solana";

        let rpc_url = settings
            .get_string(&format!("{base}.rpc_url"))
            .with_context(|| {
                format!("Missing {base}.rpc_url in config.yml — Solana has no Beta config yet")
            })?;

        let hub_program_id =
            settings.get_string(&format!("{base}.hub_program_id")).with_context(|| {
                format!("Missing {base}.hub_program_id (beta_factory's program id)")
            })?;

        let ipow_program_id =
            settings.get_string(&format!("{base}.ipow_program_id")).with_context(|| {
                format!("Missing {base}.ipow_program_id (ipow's program id)")
            })?;

        // Reuses the same operator wallet Conversion's Solana role already
        // uses (SOLANA_DEVNET_OPERATOR_KEY) — same "one shared operator
        // identity everywhere" convention already established for every
        // EVM network's own BetaConfig, not a new secret.
        let operator_private_key = env::var("SOLANA_DEVNET_OPERATOR_KEY")
            .context("Missing SOLANA_DEVNET_OPERATOR_KEY in .env")?;

        let party_id_hex = env::var("SOLANA_BETA_PARTY_ID")
            .context("Missing SOLANA_BETA_PARTY_ID in .env")?;
        let party_id = parse_party_id(&party_id_hex)?;

        Ok(Self {
            rpc_url,
            operator_private_key,
            hub_program_id,
            ipow_program_id,
            party_id,
        })
    }
}

fn parse_party_id(hex_str: &str) -> Result<[u8; 32]> {
    let s = hex_str.strip_prefix("0x").unwrap_or(hex_str);
    if s.len() != 64 {
        anyhow::bail!("party id must be 32 bytes (64 hex chars), got {} chars", s.len());
    }
    let mut out = [0u8; 32];
    for i in 0..32 {
        out[i] = u8::from_str_radix(&s[i * 2..i * 2 + 2], 16)
            .map_err(|e| anyhow::anyhow!("invalid party id hex: {e}"))?;
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Read-only: config.yml/.env parsing only, no network calls — same
    /// CWD-relative-to-core/operator constraint as every other
    /// `*Config::load` test in this workspace.
    #[test]
    fn beta_networks_solana_loads() {
        std::env::set_current_dir(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../.."),
        )
        .expect("failed to cd to core/operator for config.yml/.env lookup");

        let cfg = BetaSvmConfig::load()
            .expect("expected beta_networks.solana + SOLANA_BETA_PARTY_ID to be configured");
        assert!(!cfg.rpc_url.is_empty());
        assert!(!cfg.hub_program_id.is_empty());
        assert!(!cfg.ipow_program_id.is_empty());
        assert!(!cfg.operator_private_key.is_empty());
        assert_eq!(cfg.party_id.len(), 32);
    }
}
