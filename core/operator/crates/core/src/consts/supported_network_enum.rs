use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum SupportedNetwork {
    BTC = 0,
    HEDERA = 1,
    ETH = 2,
    SOLANA = 3,
    POLKADOT = 4,
    // Base and Robinhood Chain both use ETH as their native gas token
    // (Base: an Ethereum L2; Robinhood Chain: an Arbitrum Orbit L2 with
    // no custom gas token configured) — they reuse Self::ETH rather than
    // getting their own variant. Hyperliquid's HyperEVM genuinely does
    // not: its native gas token is HYPE, a distinct priced asset.
    HYPE = 5,
}

impl SupportedNetwork {
    pub fn from_u8(val: u8) -> Option<Self> {
        match val {
            0 => Some(Self::BTC),
            1 => Some(Self::HEDERA),
            2 => Some(Self::ETH),
            3 => Some(Self::SOLANA),
            4 => Some(Self::POLKADOT),
            5 => Some(Self::HYPE),
            _ => None,
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "btc" | "bitcoin" => Some(Self::BTC),
            "hedera" => Some(Self::HEDERA),
            "ethereum" | "eth" => Some(Self::ETH),
            "solana" | "sol" => Some(Self::SOLANA),
            "polkadot" => Some(Self::POLKADOT),
            "hyperliquid" | "hype" => Some(Self::HYPE),
            _ => None,
        }
    }

    pub fn decimals(&self) -> u32 {
        match self {
            Self::BTC => 8,
            Self::HEDERA => 8,
            Self::ETH => 18,
            Self::SOLANA => 9,
            Self::POLKADOT => 18,
            Self::HYPE => 18,
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Self::BTC => "btc",
            Self::HEDERA => "hedera",
            Self::ETH => "ethereum",
            Self::SOLANA => "solana",
            Self::POLKADOT => "polkadot",
            Self::HYPE => "hyperliquid",
        }
    }
}

impl fmt::Display for SupportedNetwork {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const ALL: &[SupportedNetwork] = &[
        SupportedNetwork::BTC,
        SupportedNetwork::HEDERA,
        SupportedNetwork::ETH,
        SupportedNetwork::SOLANA,
        SupportedNetwork::POLKADOT,
        SupportedNetwork::HYPE,
    ];

    #[test]
    fn from_u8_round_trips_every_known_variant() {
        for &net in ALL {
            assert_eq!(SupportedNetwork::from_u8(net as u8), Some(net));
        }
    }

    #[test]
    fn from_u8_rejects_unknown_values() {
        assert_eq!(SupportedNetwork::from_u8(6), None);
        assert_eq!(SupportedNetwork::from_u8(255), None);
    }

    #[test]
    fn from_str_round_trips_every_known_variant_via_as_str() {
        for &net in ALL {
            assert_eq!(SupportedNetwork::from_str(net.as_str()), Some(net));
        }
    }

    #[test]
    fn from_str_is_case_insensitive_and_accepts_aliases() {
        assert_eq!(SupportedNetwork::from_str("ETH"), Some(SupportedNetwork::ETH));
        assert_eq!(SupportedNetwork::from_str("Ethereum"), Some(SupportedNetwork::ETH));
        assert_eq!(SupportedNetwork::from_str("SOL"), Some(SupportedNetwork::SOLANA));
        assert_eq!(SupportedNetwork::from_str("Hype"), Some(SupportedNetwork::HYPE));
        assert_eq!(SupportedNetwork::from_str("bitcoin"), Some(SupportedNetwork::BTC));
    }

    #[test]
    fn from_str_rejects_unknown_names() {
        assert_eq!(SupportedNetwork::from_str("tempo"), None);
        assert_eq!(SupportedNetwork::from_str(""), None);
    }

    #[test]
    fn decimals_match_each_networks_real_native_unit() {
        // Regression guard: tick_tunneling's market-adjusted payout math
        // (chain_operator.rs) depends on this being right per destination —
        // an 8-vs-18 decimals mixup here silently mis-prices every
        // Native-to-Native payout on that chain.
        assert_eq!(SupportedNetwork::BTC.decimals(), 8);
        assert_eq!(SupportedNetwork::HEDERA.decimals(), 8);
        assert_eq!(SupportedNetwork::ETH.decimals(), 18);
        assert_eq!(SupportedNetwork::SOLANA.decimals(), 9);
        assert_eq!(SupportedNetwork::POLKADOT.decimals(), 18);
        assert_eq!(SupportedNetwork::HYPE.decimals(), 18);
    }
}
