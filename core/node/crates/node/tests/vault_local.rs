//! The vault roles on Ethereum and Solana at once, with one Bitcoin: an
//! operator registers its pair chain, states its bond, carries a lock from
//! Ethereum, and the user receives vETH on Solana (spec section 11).
//! Real blocks come from checkpoint jobs, taken and proven by the
//! operator's protocol role.

use std::sync::Arc;

use ipow_bitcoin::memory::MemoryBitcoin;
use ipow_bitcoin::view::BitcoinView;
use ipow_bitcoin::tx;
use ipow_bitcoin::wallet::Wallet;
use ipow_bitcoin::{merkle, sha256d};
use ipow_network_evm::testing::EvmWorld;
use ipow_network_svm::testing::SvmWorld;
use ipow_node::operator::Operator;
use ipow_node::supervisor::Worker;
use ipow_node::vault::{Side, VaultGuardian, VaultOperator, VaultOperatorSettings};
use ipow_node::wallet::{head_spend, SharedWallet};
use ipow_protocol_core::vault::{encode, message_payload, Record, TxProof, VaultApp};
use ipow_testing::{World, mine};
use solana_sdk::signer::Signer;

const WIF: &str = "KwDiBf89QgGbjEhKnhXJuH7LrciVrZi3qYjgd9M7rFU73sVHnoWn";
const ETH: u128 = 1_000_000_000_000_000_000;
const SOL: u128 = 1_000_000_000;
const DAY: u64 = 24 * 3600;

struct Setup {
    eth: EvmWorld,
    sol: SvmWorld,
    btc: Arc<MemoryBitcoin>,
    on_eth: Operator,
    on_sol: Operator,
    vault_op: VaultOperator,
    guardian: VaultGuardian,
    eth_vault: Arc<dyn VaultApp>,
    sol_vault: Arc<dyn VaultApp>,
    eth_vault_address: alloy::primitives::Address,
    mallory: VaultOperator,
    mallory_wallet: Arc<SharedWallet>,
    m_eth: Arc<dyn VaultApp>,
    m_sol: Arc<dyn VaultApp>,
    /// Whether Mallory's node runs in each round: stopped before she lies.
    mallory_runs: std::sync::atomic::AtomicBool,
    _journal: tempfile::TempDir,
}

impl Setup {
    async fn new() -> Self {
        let _ = tracing_subscriber::fmt().with_test_writer().with_env_filter("warn,ipow_node::vault=info,ipow_node::operator=info").try_init();
        let eth = EvmWorld::new().await;
        let sol = SvmWorld::new().await;
        let now = eth.latest_time().await;
        sol.chain.set_time(now as i64);
        let wallet = Wallet::from_wif(WIF).unwrap();
        let btc = Arc::new(MemoryBitcoin::default());
        let mut prev = [0u8; 32];
        for i in 0..6u32 {
            let h = mine(prev, sha256d(&i.to_le_bytes()), now - 3600 + i * 600);
            btc.add(i, h, &[]);
            prev = sha256d(&h);
        }
        let mallory_wallet = wallet.derive(&[66]).unwrap();
        let funding = tx::build(
            &[([7; 32], 0)],
            &[
                tx::Output { value: 10_000_000, script: wallet.script().to_vec() },
                tx::Output { value: 10_000_000, script: mallory_wallet.script().to_vec() },
            ],
        );
        btc.add(6, mine(prev, merkle::root_and_proof(&[tx::txid(&funding)], 0).0, now - 60), &[funding]);
        let shared = Arc::new(SharedWallet::new(wallet, btc.clone(), 200.0));
        let mallory_shared = Arc::new(SharedWallet::new(mallory_wallet, btc.clone(), 200.0));

        // The vaults: each names the other; 1 ETH and 1 SOL certify.
        let program = ipow_network_svm::programs::ipow_vault::ID.to_bytes();
        let address = eth.deploy_vault(program, ETH / 100, ETH).await;
        sol.init_vault(address.0.0, (SOL / 20) as u64, SOL as u64);

        let (eth_vault, eth_net) = eth.vault_node(address, 0).await;
        let (sol_vault, sol_net) = sol.vault_node(&sol.operator_key);
        let journal = tempfile::tempdir().unwrap();
        let vault_op = VaultOperator::new(
            Side { vault: eth_vault.clone(), net: eth_net },
            Side { vault: sol_vault.clone(), net: sol_net },
            btc.clone(),
            shared.clone(),
            VaultOperatorSettings {
                eth_bond: 2 * ETH,
                veth_bond: 0,
                deposits: 5,
                min_fee_gwei: 0,
                checkpoint_paid: [Some(ETH / 10), Some(SOL / 10)],
                journal: journal.path().join("vault.journal"),
            },
        );
        // Mallory: another operator, with its own keys and journal.
        let (m_eth, m_eth_net) = eth.vault_node(address, 3).await;
        let m_key = sol.chain.funded(100);
        let (m_sol, m_sol_net) = sol.vault_node(&m_key);
        let mallory = VaultOperator::new(
            Side { vault: m_eth.clone(), net: m_eth_net },
            Side { vault: m_sol.clone(), net: m_sol_net },
            btc.clone(),
            mallory_shared.clone(),
            VaultOperatorSettings {
                eth_bond: ETH,
                veth_bond: 0,
                deposits: 5,
                min_fee_gwei: 0,
                checkpoint_paid: [None, None],
                journal: journal.path().join("mallory.journal"),
            },
        );
        let (g_eth, g_eth_net) = eth.vault_node(address, 2).await;
        let g_key = sol.chain.funded(100);
        let (g_sol, g_sol_net) = sol.vault_node(&g_key);
        let guardian = VaultGuardian::new(Side { vault: g_eth, net: g_eth_net }, Side { vault: g_sol, net: g_sol_net });

        // The protocol operators take the checkpoint jobs.
        eth.operator().lock_bond(10 * ETH).await.unwrap();
        sol.operator().lock_bond(10 * SOL).await.unwrap();
        let on_eth = Operator::new(shared.clone(), 2 * ETH);
        let on_sol = Operator::new(shared.clone(), 2 * SOL);
        Setup {
            eth,
            sol,
            btc,
            on_eth,
            on_sol,
            vault_op,
            guardian,
            eth_vault,
            sol_vault,
            eth_vault_address: address,
            mallory,
            mallory_wallet: mallory_shared,
            m_eth,
            m_sol,
            mallory_runs: std::sync::atomic::AtomicBool::new(false),
            _journal: journal,
        }
    }

    async fn now(&self) -> u32 {
        self.eth.latest_time().await
    }

    async fn round(&self) {
        self.on_eth.round(self.eth.operator()).await.unwrap();
        self.on_sol.round(self.sol.operator()).await.unwrap();
        self.vault_op.round(self.eth.operator()).await.unwrap();
        if self.mallory_runs.load(std::sync::atomic::Ordering::SeqCst) {
            self.mallory.round(self.eth.operator()).await.unwrap();
        }
        self.guardian.round(self.eth.operator()).await.unwrap();
    }

    async fn tick(&self, s: u64) {
        self.eth.increase_time(s).await;
        self.sol.increase_time(s).await;
    }

    /// Mines `n` blocks, a round after each.
    async fn blocks(&self, n: usize) {
        for _ in 0..n {
            self.tick(600).await;
            self.btc.mine(self.now().await);
            self.round().await;
        }
    }

    /// Rounds and blocks until `done`, with time passing: checkpoint jobs
    /// are won, proven, and their locks end.
    async fn until(&self, what: &str, mut done: impl AsyncFnMut(&Self) -> bool) {
        for _ in 0..60 {
            if done(self).await {
                return;
            }
            // A fresh block before the auction ends: an anchor may be at
            // most 2 hours old.
            self.blocks(1).await;
            self.tick(61).await;
            self.round().await;
            // Enough blocks for a job's proof and its confirmations before
            // time jumps, well inside its deadline of a day.
            self.blocks(12).await;
            self.tick(DAY / 2).await;
            self.round().await;
        }
        panic!("{what} never happened");
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn carries_a_lock_from_ethereum_to_veth_on_solana() {
    let s = Setup::new().await;
    let eth_me = s.eth_vault.me();
    let sol_me = s.sol_vault.me();

    s.until("registration", async |s| {
        s.eth_vault.chain(&eth_me).await.unwrap().is_some() && s.sol_vault.chain(&sol_me).await.unwrap().is_some()
    })
    .await;
    // The bond on Ethereum, stated and counted on Solana after 7 days.
    s.until("the bond counted on Solana", async |s| s.sol_vault.chain(&sol_me).await.unwrap().unwrap().peer_bond == 2_000_000_000).await;
    let eth_chain = s.eth_vault.chain(&eth_me).await.unwrap().unwrap();
    assert_eq!(eth_chain.stated, 2 * ETH);

    // Bob locks 5 ETH: more than 80% of the operator's 2 ETH bond, so it
    // never carries it. Alice then locks 1 ETH for vETH on Solana: the lock
    // too large does not hold hers up.
    let alice = s.sol.user.pubkey();
    s.eth.user_lock(s.eth_vault_address, [5; 32], 5 * ETH, 1_000_000_000).await;
    s.eth.user_lock(s.eth_vault_address, alice.to_bytes(), ETH, 1_000_000_000).await;
    s.until("the lock's claim accepted", async |s| {
        let n = s.sol_vault.claim_count().await.unwrap();
        n >= 2 && s.sol_vault.claim(2).await.unwrap().accepted
    })
    .await;
    s.sol.issue(2, 2, &alice).await;
    assert_eq!(s.sol.veth(&alice).await, 1_000_000_000);
    // The operator earned the lock's fee on Ethereum, and nobody objected.
    assert!(s.eth_vault.lock(2).await.unwrap().unwrap().fee_paid);
    assert!(!s.eth_vault.lock(1).await.unwrap().unwrap().fee_paid);
    assert!(!s.sol_vault.chain(&sol_me).await.unwrap().unwrap().refused);
}

/// Mallory, with a bond counted on Solana, writes a LOCK record for a lock
/// that does not exist and shows it to Solana only. The guardian reads
/// Ethereum, objects, and the claim is refused: nothing is issued.
#[tokio::test(flavor = "multi_thread")]
async fn a_guardian_objects_to_a_made_up_lock_and_the_claim_is_refused() {
    let s = Setup::new().await;
    s.mallory_runs.store(true, std::sync::atomic::Ordering::SeqCst);
    let m_sol_me = s.m_sol.me();
    // Mallory's checkpoints come from the honest operator's.
    s.until("Mallory's bond counted on Solana", async |s| {
        // The honest operator makes the real blocks both need.
        s.m_sol.chain(&m_sol_me).await.unwrap().is_some_and(|c| c.peer_bond == 1_000_000_000)
    })
    .await;
    s.mallory_runs.store(false, std::sync::atomic::Ordering::SeqCst);

    // The lie: lock #99 does not exist.
    let chain = s.m_sol.chain(&m_sol_me).await.unwrap().unwrap();
    let batch = encode(&[Record::Lock { id: 99, amount: 500_000_000, recipient: [9; 32], fee: 0 }]);
    let sent = s.mallory_wallet.send(&[head_spend(chain.coin.0, chain.coin.1)], &message_payload(&batch)).await.unwrap();
    s.blocks(1).await;
    // A real block above it on Solana: a checkpoint the honest operator
    // proves.
    let job = s.sol_vault.open_checkpoint(6, SOL / 10).await.unwrap();
    let real = loop {
        s.blocks(1).await;
        s.tick(61).await;
        s.round().await;
        s.blocks(12).await;
        s.tick(DAY / 2).await;
        s.round().await;
        let j = s.sol.operator().job(job).await.unwrap();
        if j.status == ipow_protocol_core::types::JobStatus::Settled
            || (j.status == ipow_protocol_core::types::JobStatus::Proven && s.sol.operator().now().await.unwrap() >= j.lock_end)
        {
            if !s.sol_vault.is_real(&j.proof_block.unwrap()).await.unwrap() {
                s.sol_vault.record_real_from_job(job).await.unwrap();
            }
            break j.proof_block.unwrap();
        }
    };
    let (block, height) = match s.btc.tx_status(&sent.txid).await.unwrap().unwrap() {
        ipow_bitcoin::view::TxStatus::Confirmed { block, height } => (block, height),
        _ => panic!("Mallory's message was not mined"),
    };
    let block = ipow_protocol_core::types::BlockRef { hash: block, height, epoch_time: real.epoch_time };
    let raw = s.btc.raw_tx(&sent.txid).await.unwrap().unwrap();
    let (siblings, tx_index) = ipow_node::wallet::merkle_proof(s.btc.as_ref(), &block.hash, &sent.txid).await.unwrap();
    let before = s.sol_vault.claim_count().await.unwrap();
    s.m_sol.submit_message(&m_sol_me, &TxProof { block, raw_tx: raw, siblings, tx_index, real }, 0, 1, &batch).await.unwrap();
    let claim = before + 1;
    assert_eq!(s.sol_vault.claim_count().await.unwrap(), claim);

    // The guardian objects at once; nobody answers for Mallory.
    s.round().await;
    assert!(s.sol_vault.claim(claim).await.unwrap().held);
    s.tick(8 * DAY).await;
    s.round().await;
    s.round().await;
    let c = s.sol_vault.claim(claim).await.unwrap();
    assert!(c.decided && !c.accepted);
    assert!(s.m_sol.chain(&m_sol_me).await.unwrap().unwrap().refused);
    // The honest operator's chain is untouched.
    assert!(!s.sol_vault.chain(&s.sol_vault.me()).await.unwrap().unwrap().refused);
    let _ = &s.m_eth;
}
