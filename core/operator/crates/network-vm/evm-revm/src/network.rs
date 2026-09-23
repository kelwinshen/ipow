use ipow_core::consts::supported_network_enum::SupportedNetwork;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EvmNetwork {
    EthereumSepolia,
    Hedera,
    PolkadotHub,
    Base,
    Robinhood,
    Hyperliquid,
    Tempo,
}

impl From<EvmNetwork> for SupportedNetwork {
    fn from(network: EvmNetwork) -> Self {
        match network {
            EvmNetwork::EthereumSepolia => SupportedNetwork::ETH,
            EvmNetwork::Hedera => SupportedNetwork::HEDERA,
            EvmNetwork::PolkadotHub => SupportedNetwork::POLKADOT,
            // Base and Robinhood Chain both gas in real ETH (Base is an
            // Ethereum L2; Robinhood Chain is an Arbitrum Orbit L2 with no
            // custom gas token) — same priced asset as EthereumSepolia.
            EvmNetwork::Base => SupportedNetwork::ETH,
            EvmNetwork::Robinhood => SupportedNetwork::ETH,
            // HyperEVM's native gas token is HYPE, not ETH — confirmed via
            // eth_estimateGas (see DESIGN_V2.md §8.16) — a genuinely
            // distinct priced asset, not a relabeling of ETH.
            EvmNetwork::Hyperliquid => SupportedNetwork::HYPE,
            // Tempo has no native-value gas token at all — every real
            // value movement goes through its ERC20 feeToken, PathUSD
            // (DESIGN_V2.md §8.21). There's no `SupportedNetwork` variant
            // for a stablecoin, and nothing wires Tempo into the
            // Conversion/`networks:` pricing role this pass (only Beta's
            // spoke role, which never reads this conversion) — ETH here
            // is an unused placeholder to satisfy the exhaustive match,
            // not a real pricing claim.
            EvmNetwork::Tempo => SupportedNetwork::ETH,
        }
    }
}

impl EvmNetwork {
    pub fn chain_id(&self) -> u64 {
        match self {
            Self::EthereumSepolia => 11155111,
            Self::Hedera => 296,
            Self::PolkadotHub => 420420417,
            Self::Base => 84532,
            Self::Robinhood => 46630,
            Self::Hyperliquid => 998,
            Self::Tempo => 42431,
        }
    }

    /// iPoW's own protocol-wide network id registry (DESIGN_V2.md §8.19:
    /// 1=Hedera, 2=Ethereum, 3=Solana (reserved), 4=Polkadot, 5=Base,
    /// 6=Robinhood, 7=Tempo, 8=Hyperliquid). Fixed by convention, not read
    /// from any contract — `BetaVault.sol` (a spoke) doesn't even expose
    /// its own network id, only `BetaHub.sol`'s composition registry needs
    /// one per component.
    pub fn beta_self_network_id(&self) -> u64 {
        match self {
            Self::Hedera => 1,
            Self::EthereumSepolia => 2,
            Self::PolkadotHub => 4,
            Self::Base => 5,
            Self::Robinhood => 6,
            Self::Hyperliquid => 8,
            Self::Tempo => 7,
        }
    }

    pub fn string_identifier(&self) -> &'static str {
        match self {
            Self::EthereumSepolia => "ethereum",
            Self::Hedera => "hedera",
            Self::PolkadotHub => "polkadot",
            Self::Base => "base",
            Self::Robinhood => "robinhood",
            Self::Hyperliquid => "hyperliquid",
            Self::Tempo => "tempo",
        }
    }

    // --- config.yml PATHS ---

    pub fn rpc_config_path(&self) -> &'static str {
        match self {
            Self::EthereumSepolia => "networks.eth_sepolia.rpc_url",
            Self::Hedera => "networks.hedera.rpc_url",
            Self::PolkadotHub => "networks.polkadot.rpc_url",
            Self::Base => "networks.base.rpc_url",
            Self::Robinhood => "networks.robinhood.rpc_url",
            Self::Hyperliquid => "networks.hyperliquid.rpc_url",
            Self::Tempo => "networks.tempo.rpc_url",
        }
    }

    pub fn min_limit_config_path(&self) -> &'static str {
        match self {
            Self::EthereumSepolia => {
                "networks.eth_sepolia.min_transaction_limit"
            },
            Self::Hedera => "networks.hedera.min_transaction_limit",
            Self::PolkadotHub => "networks.polkadot.min_transaction_limit",
            Self::Base => "networks.base.min_transaction_limit",
            Self::Robinhood => "networks.robinhood.min_transaction_limit",
            Self::Hyperliquid => "networks.hyperliquid.min_transaction_limit",
            Self::Tempo => "networks.tempo.min_transaction_limit",
        }
    }

    pub fn max_limit_config_path(&self) -> &'static str {
        match self {
            Self::EthereumSepolia => {
                "networks.eth_sepolia.max_transaction_limit"
            },
            Self::Hedera => "networks.hedera.max_transaction_limit",
            Self::PolkadotHub => "networks.polkadot.max_transaction_limit",
            Self::Base => "networks.base.max_transaction_limit",
            Self::Robinhood => "networks.robinhood.max_transaction_limit",
            Self::Hyperliquid => "networks.hyperliquid.max_transaction_limit",
            Self::Tempo => "networks.tempo.max_transaction_limit",
        }
    }

    // --- .env KEYS ---

    pub fn contract_env(&self) -> &'static str {
        match self {
            Self::EthereumSepolia => "ETH_SEPOLIA_CONTRACT",
            Self::Hedera => "HEDERA_CONTRACT",
            Self::PolkadotHub => "POLKADOT_CONTRACT",
            Self::Base => "BASE_CONTRACT",
            Self::Robinhood => "ROBINHOOD_CONTRACT",
            Self::Hyperliquid => "HYPERLIQUID_CONTRACT",
            Self::Tempo => "TEMPO_CONTRACT",
        }
    }

    pub fn operator_private_key(&self) -> &'static str {
        match self {
            Self::EthereumSepolia => "ETH_SEPOLIA_OPERATOR_PRIVATE_KEY",
            Self::Hedera => "HEDERA_OPERATOR_PRIVATE_KEY",
            Self::PolkadotHub => "POLKADOT_OPERATOR_PRIVATE_KEY",
            Self::Base => "BASE_OPERATOR_PRIVATE_KEY",
            Self::Robinhood => "ROBINHOOD_OPERATOR_PRIVATE_KEY",
            Self::Hyperliquid => "HYPERLIQUID_OPERATOR_PRIVATE_KEY",
            Self::Tempo => "TEMPO_OPERATOR_PRIVATE_KEY",
        }
    }

    pub fn btc_root_xpub_env(&self) -> &'static str {
        match self {
            Self::EthereumSepolia => "ETH_SEPOLIA_BTC_ROOT_XPUB",
            Self::Hedera => "HEDERA_BTC_ROOT_XPUB",
            Self::PolkadotHub => "POLKADOT_BTC_ROOT_XPUB",
            Self::Base => "BASE_BTC_ROOT_XPUB",
            Self::Robinhood => "ROBINHOOD_BTC_ROOT_XPUB",
            Self::Hyperliquid => "HYPERLIQUID_BTC_ROOT_XPUB",
            Self::Tempo => "TEMPO_BTC_ROOT_XPUB",
        }
    }

    pub fn btc_mnemonic_env(&self) -> &'static str {
        match self {
            Self::EthereumSepolia => "ETH_SEPOLIA_BTC_MNEMONIC",
            Self::Hedera => "HEDERA_BTC_MNEMONIC",
            Self::PolkadotHub => "POLKADOT_BTC_MNEMONIC",
            Self::Base => "BASE_BTC_MNEMONIC",
            Self::Robinhood => "ROBINHOOD_BTC_MNEMONIC",
            Self::Hyperliquid => "HYPERLIQUID_BTC_MNEMONIC",
            Self::Tempo => "TEMPO_BTC_MNEMONIC",
        }
    }
}
