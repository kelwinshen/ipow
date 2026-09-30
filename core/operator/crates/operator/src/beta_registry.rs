//! Resolves every configured Beta network into its adapters, keyed by
//! iPoW's own numeric network id (5=Base, 6=Robinhood, 7=Tempo,
//! 8=Hyperliquid, 2=Ethereum, 1=Hedera, 4=Polkadot — design/ipow-implementation.md §8.19)
//! so the engine can dispatch a composition component to the right chain.
//!
//! Header freshness reuses `EvmStreamingAdapter`/`EvmChainProvider`
//! completely as-is — the plan's central claim, validated here: point a
//! synthetic `EvmConfig` at Beta's own `ipow_headers_address` (not
//! Conversion's `contract_address` for this chain) and the existing
//! Conversion-engine code keeps Beta's headers fresh with zero changes.

use ipow_core::{
    dependencies::context::CoreContext,
    traits::{
        beta_adapter::{BetaHubAdapter, BetaSpokeAdapter},
        chain_provider_adapter::ChainProviderAdapter,
        streaming_adapter::StreamingAdapter,
    },
};
use ipow_evm_revm::{
    beta::{
        config::BetaConfig, context::BetaContext,
        hub_adapter::new_hub_adapter, spoke_adapter::new_spoke_adapter,
    },
    dependencies::{config::EvmConfig, context::EvmContext},
    evm_provider::EvmChainProvider,
    network::EvmNetwork,
    streaming_adapter::EvmStreamingAdapter,
};
use ipow_svm::{
    beta::{config::BetaSvmConfig, context::BetaSvmContext, hub_adapter::new_hub_adapter as new_svm_hub_adapter},
    dependencies::{config::SolanaConfig, context::SolanaContext},
    network::SolanaNetwork,
    streaming_adapter::SolanaStreamingAdapter,
    svm_provider::SvmChainProvider,
};
use std::{collections::HashMap, sync::Arc};

/// Solana's own protocol network id (reserved, per design/ipow-implementation.md's
/// registry) — used only as this registry's own map key, since nothing
/// ever targets Solana as a *remote* leg (`HubCompositionRegistry`'s own
/// EVM-side guard rejects it, and no Solana spoke program exists) and
/// Solana's own composition never needs to look itself up by this id
/// either (its local leg is always `network_id == 0` internally).
const SOLANA_SELF_NETWORK_ID: u64 = 3;

pub struct BetaNetworkHandle {
    /// Human-readable label for logging only (`"ethereum"`, `"solana"`,
    /// ...) — every actual dispatch already goes through the trait
    /// objects below, chain-agnostically.
    pub network_label: String,
    pub self_network_id: u64,
    pub hub: Option<Arc<dyn BetaHubAdapter>>,
    pub spoke: Option<Arc<dyn BetaSpokeAdapter>>,
    pub streaming: Arc<dyn StreamingAdapter>,
    pub party_id: [u8; 32],
}

pub struct BetaRegistry {
    pub by_self_network_id: HashMap<u64, Arc<BetaNetworkHandle>>,
}

/// All EVM networks the operator might have Beta config for. A network
/// missing its `beta_networks.<name>` section in config.yml is silently
/// skipped, not an error — most Conversion networks don't have Beta
/// deployed at all yet.
const CANDIDATE_NETWORKS: &[EvmNetwork] = &[
    EvmNetwork::EthereumSepolia,
    EvmNetwork::Hedera,
    EvmNetwork::PolkadotHub,
    EvmNetwork::Base,
    EvmNetwork::Robinhood,
    EvmNetwork::Hyperliquid,
    EvmNetwork::Tempo,
];

impl BetaRegistry {
    pub async fn load(core_ctx: Arc<CoreContext>) -> anyhow::Result<Self> {
        let mut by_self_network_id = HashMap::new();

        for &evm_network in CANDIDATE_NETWORKS {
            let cfg = match BetaConfig::load(evm_network) {
                Ok(c) => c,
                Err(e) => {
                    tracing::debug!(
                        network = evm_network.string_identifier(),
                        error = %e,
                        "skipping network: no Beta config"
                    );
                    continue;
                },
            };

            let handle = match build_evm_handle(cfg, core_ctx.clone()).await {
                Ok(h) => h,
                Err(e) => {
                    tracing::warn!(
                        network = evm_network.string_identifier(),
                        error = %e,
                        "failed to build Beta handle for configured network"
                    );
                    continue;
                },
            };

            tracing::info!(
                network = %handle.network_label,
                self_network_id = handle.self_network_id,
                has_hub = handle.hub.is_some(),
                has_spoke = handle.spoke.is_some(),
                "Beta network registered"
            );

            by_self_network_id.insert(handle.self_network_id, Arc::new(handle));
        }

        match build_solana_handle(core_ctx.clone()).await {
            Ok(Some(handle)) => {
                tracing::info!(
                    network = %handle.network_label,
                    self_network_id = handle.self_network_id,
                    has_hub = handle.hub.is_some(),
                    "Beta network registered"
                );
                by_self_network_id.insert(handle.self_network_id, Arc::new(handle));
            },
            Ok(None) => {
                tracing::debug!("skipping solana: no beta_networks.solana config");
            },
            Err(e) => {
                tracing::warn!(error = %e, "failed to build Solana Beta handle");
            },
        }

        Ok(Self { by_self_network_id })
    }
}

async fn build_evm_handle(
    beta_cfg: BetaConfig,
    core_ctx: Arc<CoreContext>,
) -> anyhow::Result<BetaNetworkHandle> {
    let party_id = beta_cfg.party_id;
    let evm_network = beta_cfg.network;
    let ipow_headers_address = beta_cfg.ipow_headers_address.clone();

    let beta_ctx = BetaContext::init(beta_cfg).await?;
    let self_network_id = evm_network.beta_self_network_id();

    if beta_ctx.hub.is_none() && beta_ctx.vault.is_none() {
        anyhow::bail!("neither hub nor vault configured");
    }

    let hub = beta_ctx.hub.clone().map(|h| {
        new_hub_adapter(h, ipow_headers_address.clone())
    });
    let spoke = beta_ctx.vault.clone().map(|v| {
        new_spoke_adapter(v, ipow_headers_address.clone(), evm_network)
    });

    // Reuse the Conversion engine's own header-relay machinery unchanged,
    // pointed at Beta's `iPoW` instance instead of Conversion's.
    let streaming_evm_cfg = EvmConfig {
        network: evm_network,
        rpc_url: beta_ctx.cfg.rpc_url.clone(),
        operator_private_key: beta_ctx.cfg.operator_private_key.clone(),
        contract_address: ipow_headers_address,
        enable_onchain_lp_topup: "false".to_string(),
        btc_root_xpub: String::new(),
        btc_mnemonic: String::new(),
        min_transaction_limit: 0,
        max_transaction_limit: 0,
    };
    let evm_ctx = Arc::new(EvmContext::init(streaming_evm_cfg).await?);
    let chain_provider: Arc<dyn ChainProviderAdapter> = Arc::new(
        EvmChainProvider::new(evm_ctx.clone(), core_ctx.clone()),
    );
    let streaming: Arc<dyn StreamingAdapter> = Arc::new(EvmStreamingAdapter {
        ctx: evm_ctx,
        core_ctx,
        chain_provider,
    });

    Ok(BetaNetworkHandle {
        network_label: evm_network.string_identifier().to_string(),
        self_network_id,
        hub,
        spoke,
        streaming,
        party_id,
    })
}

/// Solana's `beta-factory` is always a hub, never a spoke — there is no
/// Solana spoke program (`HubCompositionRegistry`'s own EVM-side guard
/// already rejects Solana as a remote leg). `Ok(None)` means no
/// `beta_networks.solana` config exists yet, not an error — mirrors every
/// EVM network's own "silently skip if unconfigured" behavior.
async fn build_solana_handle(
    core_ctx: Arc<CoreContext>,
) -> anyhow::Result<Option<BetaNetworkHandle>> {
    let beta_cfg = match BetaSvmConfig::load() {
        Ok(c) => c,
        Err(_) => return Ok(None),
    };
    let party_id = beta_cfg.party_id;
    let ipow_program_id = beta_cfg.ipow_program_id.clone();
    let rpc_url = beta_cfg.rpc_url.clone();
    let operator_private_key = beta_cfg.operator_private_key.clone();

    let beta_ctx = Arc::new(BetaSvmContext::init(beta_cfg)?);
    let hub: Option<Arc<dyn BetaHubAdapter>> = Some(new_svm_hub_adapter(beta_ctx));

    // Reuse Conversion's own real-Bitcoin-PoW header relay unchanged,
    // pointed at Beta's own `ipow` program id via a synthetic
    // `SolanaConfig` — the exact same trick `build_evm_handle` already
    // uses for `EvmStreamingAdapter`/`EvmConfig`.
    let streaming_svm_cfg = SolanaConfig {
        network: SolanaNetwork::Devnet,
        rpc_url,
        operator_private_key,
        contract_address: ipow_program_id,
        enable_onchain_lp_topup: "false".to_string(),
        btc_root_xpub: String::new(),
        btc_mnemonic: String::new(),
        min_transaction_limit: 0,
        max_transaction_limit: 0,
    };
    let solana_ctx = Arc::new(SolanaContext::init(streaming_svm_cfg).await?);
    let chain_provider: Arc<dyn ChainProviderAdapter> =
        Arc::new(SvmChainProvider::new(solana_ctx.clone(), core_ctx.clone()));
    let streaming: Arc<dyn StreamingAdapter> = Arc::new(SolanaStreamingAdapter {
        ctx: solana_ctx,
        core_ctx,
        chain_provider,
    });

    Ok(Some(BetaNetworkHandle {
        network_label: "solana".to_string(),
        self_network_id: SOLANA_SELF_NETWORK_ID,
        hub,
        spoke: None,
        streaming,
        party_id,
    }))
}
