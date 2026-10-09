//! Runs each role test on every network: once on Hardhat, once on an
//! in-process Solana.

/// For each named test in the current file, one test per network.
#[macro_export]
macro_rules! on_every_network {
    ($($name:ident),* $(,)?) => {
        mod on_hardhat {
            $(
                #[tokio::test]
                async fn $name() {
                    let _ = tracing_subscriber::fmt().with_test_writer().with_env_filter(tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "warn".into())).try_init();
                    super::$name(&ipow_network_evm::testing::EvmWorld::new().await).await;
                }
            )*
        }
        mod on_solana {
            $(
                #[tokio::test]
                async fn $name() {
                    let _ = tracing_subscriber::fmt().with_test_writer().with_env_filter(tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "warn".into())).try_init();
                    super::$name(&ipow_network_svm::testing::SvmWorld::new().await).await;
                }
            )*
        }
    };
}
