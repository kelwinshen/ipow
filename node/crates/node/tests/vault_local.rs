//! The vault roles on Ethereum and Solana at once, with one Bitcoin: an
//! operator registers its pair chain, states its bonds, carries locks and
//! burns in both directions, and users receive vETH on Solana and vSOL on
//! Ethereum (spec section 11). Real blocks come from checkpoint jobs, taken
//! and proven by the operator's protocol role.

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
use ipow_node::vault::CarriedAsset;
use ipow_protocol_core::settings::FastSettings;
use ipow_protocol_core::vault::{eth_address32, encode, message_payload, Record, TxProof, VaultApp, BASE, ETHEREUM, SOLANA};
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
    g_eth: Arc<dyn VaultApp>,
    /// Whether Mallory's node runs in each round: stopped before she lies.
    mallory_runs: std::sync::atomic::AtomicBool,
    /// A second pair, of two EVM networks, when a test asks for it.
    evm_pair: Option<EvmPair>,
    _journal: tempfile::TempDir,
}

/// A pair of two EVM networks (D132), Ethereum (1) and Base (3), both
/// vaults on the local chain, with the same operator and guardian.
struct EvmPair {
    op: VaultOperator,
    guardian: VaultGuardian,
    a: Arc<dyn VaultApp>,
    b: Arc<dyn VaultApp>,
    a_address: alloy::primitives::Address,
    b_address: alloy::primitives::Address,
}

/// ETH, asset 0 of Ethereum, carried with a bond of `bond` gwei there, and
/// the fast paths for a fast fee of 1 gwei or more.
fn eth_asset(bond: u64, fast: bool) -> CarriedAsset {
    CarriedAsset {
        home: ETHEREUM,
        asset: 0,
        bond_home: bond,
        bond_receipt: 0,
        min_fee: 0,
        fast: fast.then_some(FastSettings { min_fee: 1, max: 10_000_000_000 }),
    }
}

/// An accepted claim on `v` carrying `r`, if any.
async fn accepted_carrying(v: &dyn VaultApp, r: &Record) -> Option<u64> {
    for id in 1..=v.claim_count().await.unwrap() {
        let c = v.claim(id).await.unwrap();
        if c.accepted && c.records.contains(r) {
            return Some(id);
        }
    }
    None
}

impl Setup {
    /// The honest operator carries `assets`.
    async fn new(assets: Vec<CarriedAsset>) -> Self {
        Self::build(assets, None).await
    }

    /// The same, with also a pair of two EVM networks whose operator carries
    /// `evm_assets`.
    async fn build(assets: Vec<CarriedAsset>, evm_assets: Option<Vec<CarriedAsset>>) -> Self {
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

        // The pair Ethereum and Solana (D132): each vault names the other,
        // Solana's being the pair's configuration; 1 ETH and 1 SOL certify.
        let pair_config = ipow_network_svm::vault::Pdas::new(ETHEREUM).config.to_bytes();
        let address = eth.deploy_vault(ETHEREUM, SOLANA, pair_config, ETH / 100, ETH).await;
        sol.init_vault(address.0.0, (SOL / 20) as u64, SOL as u64);

        let (eth_vault, eth_net) = eth.vault_node(address, 0).await;
        let (sol_vault, sol_net) = sol.vault_node(&sol.operator_key);
        // The two vaults name each other (D132).
        ipow_node::vault::check_pair(&Side { vault: eth_vault.clone(), net: eth_net.clone() }, &Side { vault: sol_vault.clone(), net: sol_net.clone() })
            .await
            .unwrap();
        let journal = tempfile::tempdir().unwrap();
        let vault_op = VaultOperator::new(
            Side { vault: eth_vault.clone(), net: eth_net },
            Side { vault: sol_vault.clone(), net: sol_net },
            btc.clone(),
            shared.clone(),
            VaultOperatorSettings {
                assets,
                deposits: 5,
                checkpoint_paid: [Some(ETH / 10), Some(SOL / 10)],
                journal: journal.path().join("vault.journal"),
            },
        )
        .unwrap();
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
                assets: vec![eth_asset(1_000_000_000, false)],
                deposits: 5,
                checkpoint_paid: [None, None],
                journal: journal.path().join("mallory.journal"),
            },
        )
        .unwrap();
        let (g_eth, g_eth_net) = eth.vault_node(address, 2).await;
        let g_key = sol.chain.funded(100);
        let (g_sol, g_sol_net) = sol.vault_node(&g_key);
        let guardian = VaultGuardian::new(Side { vault: g_eth.clone(), net: g_eth_net }, Side { vault: g_sol, net: g_sol_net }, btc.clone(), [Some(ETH / 10), None]).unwrap();

        let evm_pair = match evm_assets {
            None => None,
            Some(evm_assets) => {
                let (a_address, b_address) = eth.deploy_evm_pair(ETHEREUM, BASE, ETH / 100, ETH).await;
                let (a, a_net) = eth.vault_node(a_address, 0).await;
                let (b, b_net) = eth.vault_node(b_address, 0).await;
                ipow_node::vault::check_pair(&Side { vault: a.clone(), net: a_net.clone() }, &Side { vault: b.clone(), net: b_net.clone() }).await.unwrap();
                let op = VaultOperator::new(
                    Side { vault: a.clone(), net: a_net },
                    Side { vault: b.clone(), net: b_net },
                    btc.clone(),
                    shared.clone(),
                    VaultOperatorSettings {
                        assets: evm_assets,
                        deposits: 5,
                        checkpoint_paid: [Some(ETH / 10), Some(ETH / 10)],
                        journal: journal.path().join("evm.journal"),
                    },
                )
                .unwrap();
                let (ga, ga_net) = eth.vault_node(a_address, 2).await;
                let (gb, gb_net) = eth.vault_node(b_address, 2).await;
                let guardian = VaultGuardian::new(Side { vault: ga, net: ga_net }, Side { vault: gb, net: gb_net }, btc.clone(), [Some(ETH / 10), Some(ETH / 10)]).unwrap();
                Some(EvmPair { op, guardian, a, b, a_address, b_address })
            }
        };

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
            g_eth,
            mallory_runs: std::sync::atomic::AtomicBool::new(false),
            evm_pair,
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
        if let Some(p) = &self.evm_pair {
            p.op.round(self.eth.operator()).await.unwrap();
            p.guardian.round(self.eth.operator()).await.unwrap();
        }
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

    /// Mines at once to 6 blocks into the next difficulty epoch, as real
    /// Bitcoin starts one every two weeks: the light client takes no epoch
    /// start older than 4 weeks (D39, D57), and a test longer than that
    /// needs a new one. The epoch that ends must last between 2 and 4 weeks:
    /// longer than 2 keeps the test difficulty the same, and its target,
    /// near 2^255, cannot grow by 2 times without passing 256 bits.
    async fn new_epoch(&self) {
        let first = self.btc.header(&self.btc.block_hash(0).await.unwrap()).await.unwrap();
        let first_time = u32::from_le_bytes(first[68..72].try_into().unwrap());
        let lasted = self.now().await - first_time;
        assert!((14 * DAY as u32..28 * DAY as u32).contains(&lasted), "the epoch lasted {lasted} seconds");
        let tip = self.btc.tip_height().await.unwrap();
        let n = (tip / 2016 + 1) * 2016 + 6 - tip;
        self.tick(n as u64 + 1).await;
        let start = self.now().await - n;
        for k in 0..n {
            self.btc.mine(start + k);
        }
        self.round().await;
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
    let s = Setup::new(vec![eth_asset(2_000_000_000, false)]).await;
    let eth_me = s.eth_vault.me();
    let sol_me = s.sol_vault.me();

    s.until("registration", async |s| {
        s.eth_vault.chain(&eth_me).await.unwrap().is_some() && s.sol_vault.chain(&sol_me).await.unwrap().is_some()
    })
    .await;
    // The bond on Ethereum, stated and counted on Solana after 7 days.
    // ETH's ASSET record comes with it, and the operator makes vETH.
    s.until("the bond counted on Solana, and vETH made", async |s| {
        s.sol_vault.position(&sol_me, ETHEREUM, 0).await.unwrap().peer_bond == 2_000_000_000 && s.sol_vault.has_receipt(0).await.unwrap()
    })
    .await;
    let eth_chain = s.eth_vault.chain(&eth_me).await.unwrap().unwrap();
    assert_eq!(eth_chain.position(ETHEREUM, 0).stated, 2_000_000_000);

    // Bob locks 5 ETH: more than 80% of the operator's 2 ETH bond, so it
    // never carries it. Alice then locks 1 ETH for vETH on Solana: the lock
    // too large does not hold hers up.
    let alice = s.sol.user.pubkey();
    s.eth.user_lock(s.eth_vault_address, [5; 32], 5 * ETH, 1_000_000_000, 0).await;
    s.eth.user_lock(s.eth_vault_address, alice.to_bytes(), ETH, 1_000_000_000, 0).await;
    let record = s.eth_vault.lock(2).await.unwrap().unwrap().record(ETHEREUM);
    s.until("the lock's claim accepted", async |s| accepted_carrying(s.sol_vault.as_ref(), &record).await.is_some()).await;
    let claim = accepted_carrying(s.sol_vault.as_ref(), &record).await.unwrap();
    s.sol.issue(claim, &record.bytes()).await;
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
    let s = Setup::new(vec![eth_asset(2_000_000_000, false)]).await;
    s.mallory_runs.store(true, std::sync::atomic::Ordering::SeqCst);
    let m_sol_me = s.m_sol.me();
    // Mallory's checkpoints come from the honest operator's.
    s.until("Mallory's bond counted on Solana", async |s| {
        // The honest operator makes the real blocks both need.
        s.m_sol.chain(&m_sol_me).await.unwrap().is_some() && s.m_sol.position(&m_sol_me, ETHEREUM, 0).await.unwrap().peer_bond == 1_000_000_000
    })
    .await;
    s.mallory_runs.store(false, std::sync::atomic::Ordering::SeqCst);

    // The lie: lock #99 does not exist.
    let chain = s.m_sol.chain(&m_sol_me).await.unwrap().unwrap();
    let batch = encode(&[Record::Lock { home: ETHEREUM, asset: 0, id: 99, amount: 500_000_000, recipient: [9; 32], fee: 0, fast_fee: 0, at: 0 }]);
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

    // Mallory never showed the lie to Ethereum. The guardian brings it there
    // once a real block is above it on Ethereum, opening a checkpoint job
    // itself when none is: lock #99 does not exist, and her ETH bond is
    // slashed.
    let m_eth_me = s.m_eth.me();
    assert!(!s.m_eth.chain(&m_eth_me).await.unwrap().unwrap().slashed);
    let g_me: alloy::primitives::Address = s.g_eth.me().parse().unwrap();
    let before = s.eth.balance(g_me).await;
    s.until("Mallory's lie brought to Ethereum and slashed", async |s| s.m_eth.chain(&m_eth_me).await.unwrap().unwrap().slashed).await;
    let c = s.m_eth.chain(&m_eth_me).await.unwrap().unwrap();
    assert_eq!(c.position(ETHEREUM, 0).bond, 0);
    assert_eq!(c.messages, s.m_sol.chain(&m_sol_me).await.unwrap().unwrap().messages);
    // The guardian settled the slashed bond into ETH's backing and took its
    // 20%: 0.2 ETH, less what it spent on the way.
    s.round().await;
    assert_eq!(s.m_eth.slash_pending(&m_eth_me, ETHEREUM, 0).await.unwrap(), 0);
    let gained = s.eth.balance(g_me).await as i128 - before as i128;
    assert!(gained > (ETH / 10) as i128, "the guardian gained {gained} wei");
}

/// The fast paths (section 11.7). The operator holds vETH from a lock of its
/// own. Alice locks 1 ETH with a fast fee: the operator issues her receipt
/// at once, then carries the lock, links its attest to the claim, and gets
/// its vETH back with the fast fee once the claim is accepted. Alice burns
/// vETH with a fast fee: the operator pays her ETH on Ethereum at once.
#[tokio::test(flavor = "multi_thread")]
async fn issues_a_receipt_at_once_and_pays_a_burn_at_once() {
    // With 0.3 vETH bonded on Solana, so that burns of vETH are carried.
    let s = Setup::new(vec![CarriedAsset { bond_receipt: 300_000_000, ..eth_asset(2_000_000_000, true) }]).await;
    let sol_me = s.sol_vault.me();
    let op = s.sol.operator_key.pubkey();
    s.until("the bond counted on Solana, and vETH made", async |s| {
        s.sol_vault.position(&sol_me, ETHEREUM, 0).await.unwrap().peer_bond == 2_000_000_000 && s.sol_vault.has_receipt(0).await.unwrap()
    })
    .await;
    // The operator's own vETH, the slow way.
    s.eth.user_lock(s.eth_vault_address, op.to_bytes(), ETH, 0, 0).await;
    let record = s.eth_vault.lock(1).await.unwrap().unwrap().record(ETHEREUM);
    s.until("the operator's lock accepted", async |s| accepted_carrying(s.sol_vault.as_ref(), &record).await.is_some()).await;
    let claim = accepted_carrying(s.sol_vault.as_ref(), &record).await.unwrap();
    s.sol.issue(claim, &record.bytes()).await;
    assert_eq!(s.sol.veth(&op).await, 1_000_000_000);
    // Over two weeks have passed: Bitcoin starts a new epoch.
    s.new_epoch().await;

    // Alice's lock, with a fast fee of 0.001 ETH: vETH within a few rounds,
    // long before any claim could be accepted.
    let alice = s.sol.user.pubkey();
    let fast_fee = 1_000_000u64;
    s.eth.user_lock(s.eth_vault_address, alice.to_bytes(), ETH / 2, 0, fast_fee as u128 * 1_000_000_000).await;
    let start = s.now().await;
    for _ in 0..5 {
        if s.sol.veth(&alice).await > 0 {
            break;
        }
        s.blocks(1).await;
    }
    assert_eq!(s.sol.veth(&alice).await, 500_000_000);
    assert!(s.now().await - start < 2 * 3600, "the receipt came at once");
    let f = s.sol_vault.fast_lock(1).await.unwrap().unwrap();
    assert_eq!(f.attester, sol_me);
    assert_eq!(f.collateral, 625_000_000);
    // It bonded 0.3 vETH as soon as it held them, then locked 0.625.
    assert_eq!(s.sol.veth(&op).await, 1_000_000_000 - 300_000_000 - 625_000_000);

    // The claim carrying the lock is linked, accepted, and the attest
    // settled: the operator's vETH back, with the fast fee.
    s.until("the attest settled", async |s| s.sol_vault.fast_lock(1).await.unwrap().is_none()).await;
    // Its share of the fast fee: nearly all of it, for an attest within
    // hours of the lock (D124); Alice gets the rest.
    // The operator withdraws the credit in its next round: its 0.625 vETH
    // back with the share.
    s.round().await;
    assert_eq!(s.sol_vault.credit(ETHEREUM, 0).await.unwrap(), 0);
    let share = s.sol.veth(&op).await - 700_000_000;
    assert!(share > fast_fee * 99 / 100 && share < fast_fee, "share {share}");
    assert_eq!(s.sol.veth(&alice).await, 500_000_000 + fast_fee - share);

    // Alice burns 0.2 vETH with a fast fee: the operator pays her ETH on
    // Ethereum at once, from its own.
    let to = [0x42u8; 20];
    let id = s.sol.user_burn(200_000_000, to, 0, 1_000).await;
    let before = s.eth.balance(alloy::primitives::Address::from(to)).await;
    let r = s.sol_vault.request(id).await.unwrap().unwrap().record(SOLANA);
    for _ in 0..5 {
        if s.eth_vault.fast_paid_by(&r).await.unwrap().is_some() {
            break;
        }
        s.blocks(1).await;
    }
    assert_eq!(s.eth_vault.fast_paid_by(&r).await.unwrap(), Some(s.eth_vault.me()));
    assert_eq!(s.eth.balance(alloy::primitives::Address::from(to)).await, before + ETH / 5);

    // The claim carrying the burn is accepted at the end of a round; the
    // operator would take the repayment in its next. It restarts first,
    // forgetting the burn it paid, and still takes it.
    s.until("the burn's claim accepted", async |s| accepted_carrying(s.eth_vault.as_ref(), &r).await.is_some()).await;
    assert!(!s.eth_vault.request_paid(id).await.unwrap());
    s.vault_op.restart().await;
    s.round().await;
    s.round().await;
    assert!(s.eth_vault.request_paid(id).await.unwrap(), "the operator took its repayment");
}

/// The other direction (section 11.9): SOL locked on Solana for vSOL on
/// Ethereum, and vSOL burned for SOL back on Solana. The operator bonds SOL
/// on Solana for the locks, and vSOL on Ethereum for the burns, which it
/// gets from a lock of its own. One message carries 25 locks of Solana:
/// Solana reads an account for each. A lock with a fast fee is attested on
/// Ethereum, with vSOL as collateral.
#[tokio::test(flavor = "multi_thread")]
async fn carries_sol_to_vsol_on_ethereum_and_back() {
    let sol = CarriedAsset {
        home: SOLANA,
        asset: 0,
        bond_home: 5 * SOL as u64,
        bond_receipt: SOL as u64,
        min_fee: 0,
        fast: Some(FastSettings { min_fee: 1, max: SOL as u64 }),
    };
    let s = Setup::new(vec![sol]).await;
    let eth_me = s.eth_vault.me();
    s.until("the SOL bond counted on Ethereum, and vSOL made", async |s| {
        s.eth_vault.chain(&eth_me).await.unwrap().is_some() && s.eth_vault.position(&eth_me, SOLANA, 0).await.unwrap().peer_bond == 5 * SOL as u64 && s.eth_vault.has_receipt(0).await.unwrap()
    })
    .await;

    // Alice locks 1 SOL for vSOL to her Ethereum address; she also locks 2
    // SOL for the operator, who needs vSOL for its bond there.
    let alice: [u8; 20] = s.eth.user_address().await.0.0;
    let op: [u8; 20] = s.eth_vault.me_bytes().try_into().unwrap();
    let a = s.sol.user_lock_sol(SOL as u64, alice, 1_000, 0).await;
    let o = s.sol.user_lock_sol(2 * SOL as u64, op, 0, 0).await;
    // And 23 small ones: all 25 fit one message.
    let mut small = vec![];
    for _ in 0..23 {
        small.push(s.sol.user_lock_sol(1_000_000, alice, 1, 0).await);
    }
    let (ra, ro) = (
        s.sol_vault.lock(a).await.unwrap().unwrap().record(SOLANA),
        s.sol_vault.lock(o).await.unwrap().unwrap().record(SOLANA),
    );
    s.until("the SOL locks accepted on Ethereum", async |s| {
        accepted_carrying(s.eth_vault.as_ref(), &ra).await.is_some() && accepted_carrying(s.eth_vault.as_ref(), &ro).await.is_some()
    })
    .await;
    for r in [&ra, &ro] {
        let claim = accepted_carrying(s.eth_vault.as_ref(), r).await.unwrap();
        s.eth.issue_receipt(s.eth_vault_address, claim, &r.bytes()).await;
    }
    let alice_address = alloy::primitives::Address::from(alice);
    assert_eq!(s.eth.receipt_balance(s.eth_vault_address, 0, alice_address).await, SOL);
    // Solana processed the message carrying all 25: each fee was earned.
    let sol_me = s.sol_vault.me();
    assert_eq!(s.sol_vault.chain(&sol_me).await.unwrap().unwrap().messages, s.eth_vault.chain(&eth_me).await.unwrap().unwrap().messages);
    for id in small {
        assert!(s.sol_vault.lock(id).await.unwrap().unwrap().fee_paid, "lock {id}");
    }
    // The operator earned Alice's fee on Solana, and took it.
    assert!(s.sol_vault.lock(a).await.unwrap().unwrap().fee_paid);
    assert_eq!(s.sol.lock_fee_taken_by(a).await, Some(s.sol.operator_key.pubkey()));

    // Over two weeks have passed: Bitcoin starts a new epoch.
    s.new_epoch().await;

    // The operator bonds 1 vSOL on Ethereum, and Solana counts it.
    s.until("the vSOL bond counted on Solana", async |s| {
        s.sol_vault.position(&sol_me, SOLANA, 0).await.unwrap().peer_bond == SOL as u64
    })
    .await;
    assert_eq!(s.eth_vault.chain(&eth_me).await.unwrap().unwrap().position(SOLANA, 0).bond, SOL as u64);

    // Alice locks 0.2 SOL with a fast fee: the operator issues her vSOL on
    // Ethereum at once, locking 0.25 vSOL of its own.
    let held = s.eth.receipt_balance(s.eth_vault_address, 0, alice_address).await;
    let f = s.sol.user_lock_sol(SOL as u64 / 5, alice, 0, 1_000).await;
    for _ in 0..5 {
        if s.eth.receipt_balance(s.eth_vault_address, 0, alice_address).await > held {
            break;
        }
        s.blocks(1).await;
    }
    assert_eq!(s.eth.receipt_balance(s.eth_vault_address, 0, alice_address).await, held + SOL / 5);
    let attest = s.eth_vault.fast_lock(1).await.unwrap().unwrap();
    assert_eq!((attest.attester, attest.lock_id, attest.collateral), (eth_me.clone(), f, SOL as u64 / 4));

    // Alice burns 0.5 vSOL for SOL to her Solana key.
    let to = s.sol.user.pubkey();
    let id = s.eth.user_burn_receipt(s.eth_vault_address, 0, to.to_bytes(), SOL as u64 / 2, 0, 0).await;
    let r = s.eth_vault.request(id).await.unwrap().unwrap().record(ETHEREUM);
    s.until("the burn accepted on Solana", async |s| accepted_carrying(s.sol_vault.as_ref(), &r).await.is_some()).await;
    let claim = accepted_carrying(s.sol_vault.as_ref(), &r).await.unwrap();
    let before = s.sol.lamports(&to).await;
    s.sol_vault.pay_request(claim, &r).await.unwrap();
    assert_eq!(s.sol.lamports(&to).await, before + SOL as u64 / 2);
    assert!(s.sol_vault.request_paid(id).await.unwrap());
    // The attest is settled once its claim is accepted.
    s.until("the attest settled on Ethereum", async |s| s.eth_vault.fast_lock(1).await.unwrap().is_none()).await;
}

/// A pair of two EVM networks (D132): ETH locked in Ethereum's vault for
/// Base is issued as its receipt by Base's vault, through the same node,
/// with no Solana in the pair.
#[tokio::test(flavor = "multi_thread")]
async fn carries_a_lock_between_two_evm_networks() {
    let s = Setup::build(vec![], Some(vec![eth_asset(2_000_000_000, false)])).await;
    let p = s.evm_pair.as_ref().unwrap();
    assert_eq!((p.a.network_id(), p.a.peer_id(), p.b.network_id(), p.b.peer_id()), (ETHEREUM, BASE, BASE, ETHEREUM));
    let me = p.b.me();
    s.until("ETH's bond counted on Base, and its receipt made", async |s| {
        let p = s.evm_pair.as_ref().unwrap();
        p.b.chain(&me).await.unwrap().is_some() && p.b.position(&me, ETHEREUM, 0).await.unwrap().peer_bond == 2_000_000_000 && p.b.has_receipt(0).await.unwrap()
    })
    .await;

    // Alice locks 1 ETH in Ethereum's vault for Base, for its receipt there.
    let alice = s.eth.user_address().await;
    s.eth.user_lock(p.a_address, eth_address32(&alice.0.0), ETH, 1_000_000_000, 0).await;
    let record = p.a.lock(1).await.unwrap().unwrap().record(ETHEREUM);
    s.until("the lock's claim accepted on Base", async |s| accepted_carrying(s.evm_pair.as_ref().unwrap().b.as_ref(), &record).await.is_some()).await;
    let claim = accepted_carrying(p.b.as_ref(), &record).await.unwrap();
    s.eth.issue_receipt(p.b_address, claim, &record.bytes()).await;
    assert_eq!(s.eth.receipt_balance(p.b_address, 0, alice).await, 1_000_000_000);
    // The operator earned the lock's fee on Ethereum.
    assert!(p.a.lock(1).await.unwrap().unwrap().fee_paid);
}
