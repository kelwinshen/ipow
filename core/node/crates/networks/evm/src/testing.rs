//! For tests: a real local network (Hardhat's, started by the test) with
//! the new contracts deployed from their compiled code (the test versions,
//! which keep every rule of the real ones), and Bitcoin blocks mined at a
//! difficulty only those test versions accept.

use std::io::{BufRead, BufReader};
use std::process::{Child, Command, Stdio};
use std::time::Duration;

use alloy::network::{EthereumWallet, TransactionBuilder};
use alloy::primitives::{Address, Bytes, FixedBytes, U256};
use alloy::providers::{DynProvider, Provider, ProviderBuilder};
use alloy::rpc::types::TransactionRequest;
use alloy::signers::local::PrivateKeySigner;
use crate::contracts::IPoWProtocol;
use crate::network::EvmNetwork;
use ipow_protocol_core::network::ProtocolNetwork;
use ipow_protocol_core::types::Amount;
pub use ipow_testing::{EASY, TestChain, World, mine, proven_job, tagged_tx};

pub const ETH: u128 = 1_000_000_000_000_000_000;


alloy::sol! {
    #[sol(rpc)]
    interface LightClientHarness {
        function setLimits(uint256 maxTarget, uint256 powLimit) external;
    }
}

/// A Hardhat network for one test. Stopped when dropped.
pub struct LocalNetwork {
    child: Child,
    pub url: String,
    pub keys: Vec<String>,
}

impl Drop for LocalNetwork {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

/// Starts Hardhat on a free port.
pub fn start() -> LocalNetwork {
    let port = std::net::TcpListener::bind("127.0.0.1:0").unwrap().local_addr().unwrap().port();
    let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/../../../../../programmable-network/ethereum");
    // Hardhat itself, not through pnpm: stopping pnpm would leave it running.
    let mut child = Command::new(format!("{dir}/node_modules/.bin/hardhat"))
        .args(["node", "--port", &port.to_string()])
        .current_dir(dir)
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .expect("the ethereum package must be installed: pnpm install in programmable-network/ethereum");
    // Hardhat prints its funded test accounts, then listens. The keys are
    // Hardhat's well-known test keys, never used anywhere real.
    // The rest of its output is read, and dropped, by a thread of its own:
    // a closed pipe would stop Hardhat at its next line of log.
    let mut out = BufReader::new(child.stdout.take().unwrap()).lines();
    let mut keys = vec![];
    for line in out.by_ref() {
        let line = line.unwrap();
        if let Some(k) = line.trim().strip_prefix("Private Key: ") {
            keys.push(k.to_string());
            if keys.len() == 5 {
                break;
            }
        }
    }
    std::thread::spawn(move || for _ in out {});
    assert_eq!(keys.len(), 5, "Hardhat did not print its test accounts");
    LocalNetwork { child, url: format!("http://127.0.0.1:{port}"), keys }
}

/// Waits until the network answers.
pub async fn ready(url: &str) {
    let p = ProviderBuilder::new().connect_http(url.parse().unwrap());
    for _ in 0..100 {
        if p.get_chain_id().await.is_ok() {
            return;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    panic!("the local network did not start");
}

pub fn provider(url: &str, key: &str) -> DynProvider {
    let signer: PrivateKeySigner = key.parse().unwrap();
    ProviderBuilder::new().wallet(EthereumWallet::from(signer)).connect_http(url.parse().unwrap()).erased()
}

pub async fn deploy(p: &DynProvider, bin: &str, arg: [u8; 32]) -> Address {
    deploy_with(p, bin, &[arg]).await
}

/// Deploys compiled code with its constructor's arguments, each one word.
pub async fn deploy_with(p: &DynProvider, bin: &str, args: &[[u8; 32]]) -> Address {
    let mut code = alloy::hex::decode(bin.trim()).unwrap();
    for a in args {
        code.extend_from_slice(a);
    }
    let tx = TransactionRequest::default().with_deploy_code(Bytes::from(code));
    let receipt = p.send_transaction(tx).await.unwrap().get_receipt().await.unwrap();
    receipt.contract_address.expect("no contract created")
}

pub async fn increase_time(p: &DynProvider, seconds: u64) {
    let _: serde_json::Value = p.raw_request("evm_increaseTime".into(), (seconds,)).await.unwrap();
    let _: serde_json::Value = p.raw_request("evm_mine".into(), ()).await.unwrap();
}

pub async fn latest_time(p: &DynProvider) -> u32 {
    p.get_block_by_number(alloy::eips::BlockNumberOrTag::Latest).await.unwrap().unwrap().header.timestamp as u32
}

/// The contracts on a fresh local network: the light client, at the easy
/// test difficulty and with lowest block number 0, and the protocol on top.
/// Returns the application's connection, and the nodes of accounts 0 and 2.
pub async fn contracts(net: &LocalNetwork) -> (DynProvider, Address, EvmNetwork, EvmNetwork) {
    ready(&net.url).await;
    let app = provider(&net.url, &net.keys[1]);
    let lc = deploy(&app, include_str!("../tests/iPoWLightClientHarness.bin"), [0u8; 32]).await;
    let easy = U256::from(0x7fffffu64) << (8 * (0x20 - 3));
    LightClientHarness::new(lc, &app).setLimits(easy, easy).send().await.unwrap().get_receipt().await.unwrap();
    let mut arg = [0u8; 32];
    arg[12..].copy_from_slice(lc.as_slice());
    let protocol = deploy(&app, include_str!("../tests/iPoWProtocolHarness.bin"), arg).await;
    let node = |key: &str| EvmNetwork::connect("hardhat", &net.url, key, &lc.to_string(), &protocol.to_string()).unwrap();
    (app.clone(), protocol, node(&net.keys[0]), node(&net.keys[2]))
}

/// An application registers with no claims and opens a settlement job for
/// 1 ETH with 6 confirmations. Once per test.
pub async fn open_job(app: &DynProvider, protocol: Address) {
    let p = IPoWProtocol::new(protocol, app);
    p.registerApplication(vec![]).send().await.unwrap().get_receipt().await.unwrap();
    let user: Address = PrivateKeySigner::from_bytes(&FixedBytes::from([7u8; 32])).unwrap().address();
    p.openJob(FixedBytes::from([1u8; 32]), U256::from(ETH), 50, 6, 0, user)
        .value(U256::from(ETH / 10))
        .gas(1_000_000)
        .send()
        .await
        .unwrap()
        .get_receipt()
        .await
        .unwrap();
}

/// A fresh local network with the contracts, two applications, and the
/// nodes of an operator (account 0) and a guardian (account 2).
pub struct EvmWorld {
    pub net: LocalNetwork,
    /// Accounts 1 and 3.
    pub apps: [DynProvider; 2],
    pub protocol: Address,
    pub operator: EvmNetwork,
    pub guardian: EvmNetwork,
    /// Conversion, with a limit of 0.1 BTC per buy.
    pub conversion: Address,
    /// Account 4: a user of Conversion.
    pub user: DynProvider,
}

impl EvmWorld {
    pub async fn new() -> Self {
        let net = start();
        let (app, protocol, operator, guardian) = contracts(&net).await;
        let other = provider(&net.url, &net.keys[3]);
        let mut p = [0u8; 32];
        p[12..].copy_from_slice(protocol.as_slice());
        let mut max_sats = [0u8; 32];
        max_sats[24..].copy_from_slice(&10_000_000u64.to_be_bytes());
        let conversion = deploy_with(&app, include_str!("../tests/Conversion.bin"), &[p, max_sats]).await;
        let user = provider(&net.url, &net.keys[4]);
        EvmWorld { net, apps: [app, other], protocol, operator, guardian, conversion, user }
    }
}

impl EvmWorld {
    /// Deploys the protocol's vault (section 11), naming the Solana vault
    /// program, with its deposit and least certifying escrow in wei.
    pub async fn deploy_vault(&self, solana_vault: [u8; 32], deposit: u128, min_certifying_escrow: u128) -> Address {
        let mut p = [0u8; 32];
        p[12..].copy_from_slice(self.protocol.as_slice());
        let d: [u8; 32] = U256::from(deposit).to_be_bytes();
        let m: [u8; 32] = U256::from(min_certifying_escrow).to_be_bytes();
        deploy_with(&self.apps[0], include_str!("../tests/iPoWVault.bin"), &[p, solana_vault, d, m]).await
    }

    /// The vault as the node of account `key` (0 the operator, 2 the
    /// guardian) sees it.
    pub async fn vault_node(&self, vault: Address, key: usize) -> (std::sync::Arc<crate::vault::EvmVault>, std::sync::Arc<EvmNetwork>) {
        let lc = IPoWProtocol::new(self.protocol, &self.apps[0]).lightClient().call().await.unwrap();
        let net = std::sync::Arc::new(EvmNetwork::connect("hardhat", &self.net.url, &self.net.keys[key], &lc.to_string(), &self.protocol.to_string()).unwrap());
        (std::sync::Arc::new(crate::vault::EvmVault::new(net.clone(), &vault.to_string()).unwrap()), net)
    }

    /// The balance of `who`, in wei.
    pub async fn balance(&self, who: Address) -> u128 {
        self.user.get_balance(who).await.unwrap().to::<u128>()
    }

    /// The user locks `amount` wei for vETH to `recipient` on Solana, with
    /// `fee` wei for the operator and `fast_fee` wei for an attester.
    pub async fn user_lock(&self, vault: Address, recipient: [u8; 32], amount: u128, fee: u128, fast_fee: u128) {
        crate::contracts::IPoWVault::new(vault, &self.user)
            .lock(FixedBytes(recipient), U256::from(fee), U256::from(fast_fee))
            .value(U256::from(amount + fee + fast_fee))
            .gas(1_000_000)
            .send()
            .await
            .unwrap()
            .get_receipt()
            .await
            .unwrap();
    }
}

#[async_trait::async_trait]
impl World for EvmWorld {
    fn unit(&self) -> Amount {
        ETH
    }
    fn operator(&self) -> &dyn ProtocolNetwork {
        &self.operator
    }
    fn guardian(&self) -> &dyn ProtocolNetwork {
        &self.guardian
    }
    async fn open_job(&self, app: u8) {
        open_job(&self.apps[app as usize], self.protocol).await
    }
    async fn increase_time(&self, seconds: u64) {
        increase_time(&self.apps[0], seconds).await
    }
    async fn latest_time(&self) -> u32 {
        latest_time(&self.apps[0]).await
    }

    async fn conversion(&self) -> std::sync::Arc<dyn ipow_protocol_core::conversion::ConversionApp> {
        // The operator's node on its own connection, with the operator's key.
        let lc = IPoWProtocol::new(self.protocol, &self.apps[0]).lightClient().call().await.unwrap();
        let net = EvmNetwork::connect("hardhat", &self.net.url, &self.net.keys[0], &lc.to_string(), &self.protocol.to_string()).unwrap();
        std::sync::Arc::new(crate::conversion::EvmConversion::new(std::sync::Arc::new(net), &self.conversion.to_string()).unwrap())
    }

    async fn user_sell(&self, amount: Amount, sats: u64, script: &[u8]) {
        let c = crate::contracts::Conversion::new(self.conversion, &self.user);
        c.sell(Address::ZERO, U256::from(amount), sats, Bytes::copy_from_slice(script), 6)
            .value(U256::from(amount + ETH / 10))
            .gas(2_000_000)
            .send()
            .await
            .unwrap()
            .get_receipt()
            .await
            .unwrap();
    }

    async fn user_buy(&self, amount: Amount, sats: u64) {
        let c = crate::contracts::Conversion::new(self.conversion, &self.user);
        c.buy(Address::ZERO, U256::from(amount), sats, 6).value(U256::from(ETH / 10)).gas(2_000_000).send().await.unwrap().get_receipt().await.unwrap();
    }
}
