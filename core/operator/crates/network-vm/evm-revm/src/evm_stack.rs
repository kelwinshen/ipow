use crate::approving_adapter::EvmApprovingAdapter;
use crate::converting_adapter::EvmConvertingAdapter;
use crate::dependencies::config::EvmConfig;
use crate::dependencies::context::EvmContext;
use crate::evm_provider::EvmChainProvider;
use crate::network::EvmNetwork;
use crate::streaming_adapter::EvmStreamingAdapter;
use async_trait::async_trait;
use ipow_core::consts::supported_network_enum::SupportedNetwork;
use ipow_core::dependencies::context::CoreContext;
use ipow_core::traits::approving_adapter::ApprovingAdapter;
use ipow_core::traits::chain_provider_adapter::ChainProviderAdapter;
use ipow_core::traits::chain_stack::ChainStack;
use ipow_core::traits::converting_adapter::ConvertingAdapter;
use ipow_core::traits::streaming_adapter::StreamingAdapter;
use std::sync::Arc;

pub struct EvmStack {
    pub network_id: String,
    pub network_enum: SupportedNetwork,
    pub chain_provider: Arc<EvmChainProvider>,
    pub streaming: Arc<EvmStreamingAdapter>,
    pub approving: Arc<EvmApprovingAdapter>,
    pub converting: Arc<EvmConvertingAdapter>,
}

impl EvmStack {
    pub async fn init(
        network: EvmNetwork,
        core_ctx: Arc<CoreContext>,
    ) -> anyhow::Result<Self> {
        let network_name = network.string_identifier().to_string();
        let network_enum: SupportedNetwork = network.into();

        let cfg = EvmConfig::load(network);
        let ctx = Arc::new(EvmContext::init(cfg).await?);

        let provider =
            Arc::new(EvmChainProvider::new(ctx.clone(), core_ctx.clone()));

        let provider_trait: Arc<dyn ChainProviderAdapter> = provider.clone();

        let approving = Arc::new(EvmApprovingAdapter {
            ctx: ctx.clone(),
            core_ctx: core_ctx.clone(),
            chain_provider: provider_trait.clone(),
        });

        let converting = Arc::new(EvmConvertingAdapter {
            ctx: ctx.clone(),
            core_ctx: core_ctx.clone(),
            chain_provider: provider_trait.clone(),
        });

        let streaming = Arc::new(EvmStreamingAdapter {
            ctx: ctx.clone(),
            core_ctx: core_ctx.clone(),
            chain_provider: provider_trait,
        });

        Ok(Self {
            network_id: network_name,
            network_enum,
            chain_provider: provider,
            streaming,
            approving,
            converting,
        })
    }
}

#[async_trait]
impl ChainStack for EvmStack {
    fn converting(&self) -> Arc<dyn ConvertingAdapter> {
        self.converting.clone()
    }

    fn approving(&self) -> Arc<dyn ApprovingAdapter> {
        self.approving.clone()
    }

    fn streaming(&self) -> Arc<dyn StreamingAdapter> {
        self.streaming.clone()
    }

    fn chain_provider(&self) -> Arc<dyn ChainProviderAdapter> {
        self.chain_provider.clone()
    }

    fn network_id(&self) -> &str {
        &self.network_id
    }

    fn network_enum(&self) -> SupportedNetwork {
        self.network_enum
    }

    fn core_context(&self) -> Arc<CoreContext> {
        self.converting.core_ctx.clone()
    }
}
