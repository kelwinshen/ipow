use crate::{
    beta::config::BetaConfig,
    bindings::{beta_hub::BetaHub, beta_vault::BetaVault},
};
use ethers::{
    middleware::SignerMiddleware,
    providers::{Http, HttpRateLimitRetryPolicy, Provider, RetryClient},
    signers::{LocalWallet, Signer},
    types::Address,
};
use std::sync::Arc;

pub type ResilientProvider = Provider<RetryClient<Http>>;
pub type ResilientSigner = SignerMiddleware<Arc<ResilientProvider>, LocalWallet>;

#[derive(Clone)]
pub struct BetaContext {
    pub provider: Arc<ResilientProvider>,
    pub cfg: Arc<BetaConfig>,
    pub hub: Option<HubHandles>,
    pub vault: Option<VaultHandles>,
}

#[derive(Clone)]
pub struct HubHandles {
    pub read: Arc<BetaHub<ResilientProvider>>,
    pub write: Arc<BetaHub<ResilientSigner>>,
}

#[derive(Clone)]
pub struct VaultHandles {
    pub read: Arc<BetaVault<ResilientProvider>>,
    pub write: Arc<BetaVault<ResilientSigner>>,
}

impl BetaContext {
    pub async fn init(cfg: BetaConfig) -> anyhow::Result<Self> {
        let cfg = Arc::new(cfg);

        let client = cfg.rpc_url.parse::<Http>()?;
        let retry_client =
            RetryClient::new(client, Box::new(HttpRateLimitRetryPolicy), 5, 2000);
        let provider = Arc::new(Provider::new(retry_client));

        let wallet: LocalWallet = cfg.operator_private_key.parse()?;
        let wallet = wallet.with_chain_id(cfg.network.chain_id());
        let signer =
            Arc::new(SignerMiddleware::new(provider.clone(), wallet));

        let hub = match &cfg.hub_address {
            Some(addr) => {
                let address: Address = addr.parse()?;
                Some(HubHandles {
                    read: Arc::new(BetaHub::new(address, provider.clone())),
                    write: Arc::new(BetaHub::new(address, signer.clone())),
                })
            },
            None => None,
        };

        let vault = match &cfg.vault_address {
            Some(addr) => {
                let address: Address = addr.parse()?;
                Some(VaultHandles {
                    read: Arc::new(BetaVault::new(address, provider.clone())),
                    write: Arc::new(BetaVault::new(address, signer.clone())),
                })
            },
            None => None,
        };

        Ok(Self { provider, cfg, hub, vault })
    }
}
