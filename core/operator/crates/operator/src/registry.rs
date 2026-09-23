use once_cell::sync::Lazy;
use ipow_evm_revm::evm_stack::EvmStack;
use ipow_evm_revm::network::EvmNetwork;
use ipow_svm::network::SolanaNetwork;
use ipow_svm::svm_stack::SvmStack;
use ipow_core::dependencies::context::CoreContext;
use ipow_core::traits::chain_stack::ChainStack;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

static STACK_CACHE: Lazy<RwLock<HashMap<String, Arc<dyn ChainStack>>>> =
    Lazy::new(|| RwLock::new(HashMap::new()));

pub struct Registry;

impl Registry {
    pub async fn get_stack(
        name: &str,
        core_ctx: Arc<CoreContext>,
    ) -> anyhow::Result<Arc<dyn ChainStack>> {
        let name = name.to_lowercase();

        // 1. Fast Path: Return if already initialized
        {
            let cache = STACK_CACHE.read().await;
            if let Some(stack) = cache.get(&name) {
                return Ok(stack.clone());
            }
        }

        // 2. Slow Path: Initialize and cache
        let mut cache = STACK_CACHE.write().await;

        // Double-check inside write lock to prevent race conditions
        if let Some(stack) = cache.get(&name) {
            return Ok(stack.clone());
        }

        let stack: Arc<dyn ChainStack> = match name.as_str() {
            "hedera" => {
                Arc::new(EvmStack::init(EvmNetwork::Hedera, core_ctx).await?)
            },
            "eth" | "ethereum" => Arc::new(
                EvmStack::init(EvmNetwork::EthereumSepolia, core_ctx).await?,
            ),
            "polkadot" => Arc::new(
                EvmStack::init(EvmNetwork::PolkadotHub, core_ctx).await?,
            ),
            "base" => {
                Arc::new(EvmStack::init(EvmNetwork::Base, core_ctx).await?)
            },
            "robinhood" => Arc::new(
                EvmStack::init(EvmNetwork::Robinhood, core_ctx).await?,
            ),
            "hyperliquid" => Arc::new(
                EvmStack::init(EvmNetwork::Hyperliquid, core_ctx).await?,
            ),
            "solana" | "sol" => {
                Arc::new(SvmStack::init(SolanaNetwork::Devnet, core_ctx).await?)
            },
            _ => anyhow::bail!("Unsupported network: {}", name),
        };

        cache.insert(name, stack.clone());
        Ok(stack)
    }
}
