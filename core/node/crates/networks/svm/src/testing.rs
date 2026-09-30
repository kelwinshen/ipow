//! For tests: an in-process Solana (LiteSVM) with the two programs loaded
//! from their compiled code and set up: the light client's test build,
//! which accepts blocks mined at a low difficulty, and the protocol.

use std::sync::{Arc, Mutex};

use anchor_lang::prelude::Pubkey;
use anchor_lang::solana_program::instruction::Instruction;
use anchor_lang::{InstructionData, ToAccountMetas};
use async_trait::async_trait;
use ipow_protocol_core::network::ProtocolNetwork;
use ipow_protocol_core::types::Amount;
use litesvm::LiteSVM;
use solana_sdk::clock::Clock;
use solana_sdk::signature::Keypair;
use solana_sdk::signer::Signer;
use solana_sdk::message::AddressLookupTableAccount;
use solana_sdk::slot_hashes::SlotHashes;

use crate::chain::{Chain, PACKET_DATA_SIZE, build, error_name, size};
use crate::network::SvmNetwork;
use crate::programs::{ipow_light_client as lc, ipow_protocol as pr};

pub use ipow_testing::{EASY, TestChain, World, mine, proven_job, tagged_tx};

pub const SOL: u128 = 1_000_000_000;
const SYSTEM: Pubkey = anchor_lang::solana_program::system_program::ID;
/// A time in 2027, so that Bitcoin times fit in 32 bits for decades.
const START: i64 = 1_800_000_000;

/// Solana in the test's own process. Each transaction that succeeds moves
/// to the next slot, as a real network would, so a note sealed in one
/// transaction counts in the next.
pub struct LocalSolana(Mutex<LiteSVM>);

impl LocalSolana {
    pub fn new() -> Arc<Self> {
        let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/../../../../../programmable-network/solana/target");
        let read = |path: &str| std::fs::read(format!("{dir}/{path}")).unwrap_or_else(|_| panic!("build the Solana programs first: {dir}/{path} is missing"));
        let mut svm = LiteSVM::new();
        svm.add_program(lc::ID, &read("deploy-test/ipow_light_client.so")).unwrap();
        svm.add_program(pr::ID, &read("deploy/ipow_protocol.so")).unwrap();
        svm.add_program(crate::programs::conversion::ID, &read("deploy/conversion.so")).unwrap();
        // The vault's test build: its initialize needs no upgrade authority.
        svm.add_program(crate::programs::ipow_vault::ID, &read("deploy-test/ipow_vault.so")).unwrap();
        let chain = Arc::new(LocalSolana(Mutex::new(svm)));
        chain.set_time(START);

        let admin = chain.funded(10);
        // The easy test difficulty as a big-endian target.
        let mut easy = [0u8; 32];
        easy[..3].copy_from_slice(&[0x7f, 0xff, 0xff]);
        chain
            .run(
                &[ix(
                    lc::ID,
                    lc::client::accounts::Initialize {
                        config: SvmNetwork::lc_pda(&[b"config"]),
                        day_table: SvmNetwork::lc_pda(&[b"days"]),
                        authority: admin.pubkey(),
                        program_data: SYSTEM,
                        system_program: SYSTEM,
                    },
                    lc::client::args::Initialize { min_height: 0, max_target: easy, pow_limit: easy },
                )],
                &admin,
            )
            .unwrap();
        chain
            .run(
                &[ix(
                    pr::ID,
                    pr::client::accounts::InitializeProtocol {
                        protocol: SvmNetwork::pr_pda(&[b"protocol"]),
                        vault: SvmNetwork::pr_pda(&[b"vault"]),
                        payer: admin.pubkey(),
                        system_program: SYSTEM,
                    },
                    pr::client::args::InitializeProtocol {},
                )],
                &admin,
            )
            .unwrap();
        chain
    }

    /// A new key holding `sol` coins.
    pub fn funded(&self, sol: u64) -> Keypair {
        let k = Keypair::new();
        self.0.lock().unwrap().airdrop(&k.pubkey(), sol * SOL as u64).unwrap();
        k
    }

    pub fn set_time(&self, t: i64) {
        let mut svm = self.0.lock().unwrap();
        let mut clock: Clock = svm.get_sysvar();
        clock.unix_timestamp = t;
        svm.set_sysvar(&clock);
    }

    pub fn time(&self) -> i64 {
        self.0.lock().unwrap().get_sysvar::<Clock>().unix_timestamp
    }

    fn run(&self, ixs: &[Instruction], payer: &Keypair) -> anyhow::Result<()> {
        self.run_with(ixs, payer, None)
    }

    fn run_with(&self, ixs: &[Instruction], payer: &Keypair, table: Option<&AddressLookupTableAccount>) -> anyhow::Result<()> {
        let mut svm = self.0.lock().unwrap();
        svm.expire_blockhash();
        let tx = build(ixs, payer, svm.latest_blockhash(), table)?;
        // A real network refuses a transaction above 1,232 bytes; LiteSVM
        // does not check.
        let size = size(&tx)?;
        anyhow::ensure!(size <= PACKET_DATA_SIZE, "the transaction is {size} bytes, above Solana's {PACKET_DATA_SIZE}");
        match svm.send_transaction(tx) {
            Ok(_) => {
                next_slot(&mut svm);
                Ok(())
            }
            Err(f) => match error_name(&f.meta.logs) {
                Some(name) => anyhow::bail!("rejected by the program: {name}"),
                None => anyhow::bail!("the transaction failed: {:?}\n{}", f.err, f.meta.logs.join("\n")),
            },
        }
    }
}

/// Moves to the next slot. As on a real network, the slot just left joins
/// the recent slots (SlotHashes); the current one is never among them.
fn next_slot(svm: &mut LiteSVM) {
    let mut clock: Clock = svm.get_sysvar();
    let left = clock.slot;
    clock.slot += 1;
    svm.set_sysvar(&clock);
    let hashes: SlotHashes = svm.get_sysvar();
    let mut entries: Vec<(u64, solana_sdk::hash::Hash)> = hashes.iter().copied().filter(|(s, _)| *s != left).collect();
    entries.insert(0, (left, svm.latest_blockhash()));
    entries.truncate(512);
    svm.set_sysvar(&SlotHashes::new(&entries));
}

fn ix(program_id: Pubkey, accounts: impl ToAccountMetas, data: impl InstructionData) -> Instruction {
    Instruction { program_id, accounts: accounts.to_account_metas(None), data: data.data() }
}

#[async_trait]
impl Chain for LocalSolana {
    async fn account(&self, key: &Pubkey) -> anyhow::Result<Option<Vec<u8>>> {
        Ok(self.0.lock().unwrap().get_account(key).filter(|a| a.lamports > 0).map(|a| a.data))
    }

    async fn send(&self, instructions: &[Instruction], payer: &Keypair, table: Option<&AddressLookupTableAccount>) -> anyhow::Result<()> {
        self.run_with(instructions, payer, table)
    }

    async fn clock(&self) -> anyhow::Result<Clock> {
        Ok(self.0.lock().unwrap().get_sysvar())
    }

    async fn owner_and_lamports(&self, key: &Pubkey) -> anyhow::Result<Option<(Pubkey, u64)>> {
        Ok(self.0.lock().unwrap().get_account(key).filter(|a| a.lamports > 0).map(|a| (a.owner, a.lamports)))
    }

    async fn wait_past(&self, slot: u64) -> anyhow::Result<()> {
        // Time passes: the next slot.
        let mut svm = self.0.lock().unwrap();
        while svm.get_sysvar::<Clock>().slot <= slot {
            next_slot(&mut svm);
        }
        Ok(())
    }

    async fn tables_of(&self, authority: &Pubkey) -> anyhow::Result<Vec<Pubkey>> {
        // LiteSVM cannot list accounts by program; tests keep their tables
        // in the adapter's own list.
        let _ = authority;
        Ok(vec![])
    }
}

/// A fresh Solana with the programs, two applications, and the nodes of an
/// operator and a guardian.
pub struct SvmWorld {
    pub chain: Arc<LocalSolana>,
    pub apps: [Keypair; 2],
    pub operator: SvmNetwork,
    pub guardian: SvmNetwork,
    /// The operator's key, for a second connection of its node.
    pub operator_key: Keypair,
    /// A user of Conversion.
    pub user: Keypair,
}

fn cv_pda(seeds: &[&[u8]]) -> Pubkey {
    Pubkey::find_program_address(seeds, &crate::programs::conversion::ID).0
}

fn vt_pda(seeds: &[&[u8]]) -> Pubkey {
    Pubkey::find_program_address(seeds, &crate::programs::ipow_vault::ID).0
}

impl SvmWorld {
    /// Initializes the protocol's vault (section 11), naming the Ethereum
    /// vault, with its deposit and least certifying escrow in lamports.
    pub fn init_vault(&self, ethereum_vault: [u8; 20], deposit: u64, min_certifying_escrow: u64) {
        use crate::programs::ipow_vault as vt;
        let config = vt_pda(&[b"config"]);
        self.chain
            .run(
                &[ix(
                    vt::ID,
                    vt::client::accounts::Initialize {
                        config,
                        mint: vt_pda(&[b"veth"]),
                        holding: vt_pda(&[b"holding"]),
                        application: SvmNetwork::pr_pda(&[b"application", config.as_ref()]),
                        payer: self.user.pubkey(),
                        program_data: SYSTEM,
                        protocol_program: pr::ID,
                        token_program: anchor_spl::token::ID,
                        system_program: SYSTEM,
                    },
                    vt::client::args::Initialize { ethereum_vault, deposit, min_certifying_escrow },
                )],
                &self.user,
            )
            .unwrap();
    }

    /// The vault as a node with `key` sees it.
    pub fn vault_node(&self, key: &Keypair) -> (Arc<crate::vault::SvmVault>, Arc<SvmNetwork>) {
        let net = Arc::new(SvmNetwork::connect("solana", self.chain.clone(), key.insecure_clone(), &lc::ID.to_string(), &pr::ID.to_string()).unwrap());
        (Arc::new(crate::vault::SvmVault::new(net.clone(), &crate::programs::ipow_vault::ID.to_string()).unwrap()), net)
    }

    /// vETH held by `owner` in its associated account.
    pub async fn veth(&self, owner: &Pubkey) -> u64 {
        let account = anchor_spl::associated_token::get_associated_token_address(owner, &vt_pda(&[b"veth"]));
        match self.chain.account(&account).await.unwrap() {
            Some(data) if !data.is_empty() => {
                <anchor_spl::token::TokenAccount as anchor_lang::AccountDeserialize>::try_deserialize(&mut data.as_slice()).unwrap().amount
            }
            _ => 0,
        }
    }

    /// Anyone (the user here) issues the vETH of lock `lock_id`, carried by
    /// accepted claim `claim_id`, to `recipient`'s associated account.
    pub async fn issue(&self, claim_id: u64, lock_id: u64, recipient: &Pubkey) {
        use crate::programs::ipow_vault as vt;
        let mint = vt_pda(&[b"veth"]);
        let to = anchor_spl::associated_token::get_associated_token_address(recipient, &mint);
        let create = anchor_spl::associated_token::spl_associated_token_account::instruction::create_associated_token_account_idempotent(
            &self.user.pubkey(),
            recipient,
            &mint,
            &anchor_spl::token::ID,
        );
        self.chain.run(&[create], &self.user).unwrap();
        self.chain
            .run(
                &[ix(
                    vt::ID,
                    vt::client::accounts::Issue {
                        claim: vt_pda(&[b"claim", &claim_id.to_le_bytes()]),
                        mark: vt_pda(&[b"lock", &lock_id.to_le_bytes()]),
                        config: vt_pda(&[b"config"]),
                        mint,
                        holding: vt_pda(&[b"holding"]),
                        to,
                        payer: self.user.pubkey(),
                        token_program: anchor_spl::token::ID,
                        system_program: SYSTEM,
                    },
                    vt::client::args::Issue { claim_id, lock_id },
                )],
                &self.user,
            )
            .unwrap();
    }

    pub async fn new() -> Self {
        let chain = LocalSolana::new();
        let node = |k: Keypair| SvmNetwork::connect("solana", chain.clone(), k, &lc::ID.to_string(), &pr::ID.to_string()).unwrap();
        let operator_key = chain.funded(100);
        let (operator, guardian) = (node(operator_key.insecure_clone()), node(chain.funded(100)));
        let apps = [chain.funded(100), chain.funded(100)];
        let user = chain.funded(100);
        // Conversion, registered with the protocol.
        let cv = crate::programs::conversion::ID;
        let config = cv_pda(&[b"config"]);
        chain
            .run(
                &[ix(
                    cv,
                    crate::programs::conversion::client::accounts::Initialize {
                        config,
                        application: SvmNetwork::pr_pda(&[b"application", config.as_ref()]),
                        payer: user.pubkey(),
                        protocol_program: pr::ID,
                        system_program: SYSTEM,
                    },
                    crate::programs::conversion::client::args::Initialize {},
                )],
                &user,
            )
            .unwrap();
        SvmWorld { chain, apps, operator, guardian, operator_key, user }
    }

    /// The accounts a user's sell or buy opens: the swap, the job, and its
    /// tag record.
    async fn open_accounts(&self) -> (Pubkey, Pubkey, Pubkey, u64) {
        let config: crate::programs::conversion::accounts::Config = {
            let data = self.chain.account(&cv_pda(&[b"config"])).await.unwrap().unwrap();
            anchor_lang::AccountDeserialize::try_deserialize(&mut data.as_slice()).unwrap()
        };
        let id = config.swap_count + 1;
        let job_count = self.operator.jobs_after(0, usize::MAX).await.unwrap().len() as u64;
        let mut h = <sha2::Sha256 as sha2::Digest>::new();
        sha2::Digest::update(&mut h, b"iPoW conversion");
        sha2::Digest::update(&mut h, crate::programs::conversion::ID.as_ref());
        sha2::Digest::update(&mut h, id.to_le_bytes());
        let tag: [u8; 32] = sha2::Digest::finalize(h).into();
        (
            cv_pda(&[b"swap", &id.to_le_bytes()]),
            SvmNetwork::job_pda(job_count + 1),
            SvmNetwork::pr_pda(&[b"tag", cv_pda(&[b"config"]).as_ref(), &tag]),
            id,
        )
    }
}

#[async_trait]
impl World for SvmWorld {
    fn unit(&self) -> Amount {
        SOL
    }
    fn operator(&self) -> &dyn ProtocolNetwork {
        &self.operator
    }
    fn guardian(&self) -> &dyn ProtocolNetwork {
        &self.guardian
    }

    async fn open_job(&self, app: u8) {
        let key = &self.apps[app as usize];
        let application = SvmNetwork::pr_pda(&[b"application", key.pubkey().as_ref()]);
        self.chain
            .run(
                &[ix(
                    pr::ID,
                    pr::client::accounts::RegisterApplication { application, key: key.pubkey(), funder: key.pubkey(), system_program: SYSTEM },
                    pr::client::args::RegisterApplication { challenge_periods: vec![] },
                )],
                key,
            )
            .unwrap();
        let count = self.operator.jobs_after(0, usize::MAX).await.unwrap().len() as u64;
        let tag = [1u8; 32];
        // The commitment fee of 6 confirmations (`fees.rs`) and 0.5% of x.
        let fee = (24 + 6 + 20) * 5_000 * 3 / 2;
        self.chain
            .run(
                &[ix(
                    pr::ID,
                    pr::client::accounts::OpenJob {
                        protocol: SvmNetwork::pr_pda(&[b"protocol"]),
                        application,
                        job: SvmNetwork::job_pda(count + 1),
                        tag_record: SvmNetwork::pr_pda(&[b"tag", key.pubkey().as_ref(), &tag]),
                        vault: SvmNetwork::pr_pda(&[b"vault"]),
                        key: key.pubkey(),
                        funder: key.pubkey(),
                        system_program: SYSTEM,
                    },
                    pr::client::args::OpenJob {
                        tag,
                        escrow: SOL as u64,
                        escrow_fee_bps: 50,
                        confirmations: 6,
                        claim_kind: 0,
                        payer: Pubkey::new_unique(),
                        paid: fee + SOL as u64 / 200,
                    },
                )],
                key,
            )
            .unwrap();
    }

    async fn increase_time(&self, seconds: u64) {
        self.chain.set_time(self.chain.time() + seconds as i64);
    }

    async fn latest_time(&self) -> u32 {
        self.chain.time() as u32
    }

    async fn conversion(&self) -> Arc<dyn ipow_protocol_core::conversion::ConversionApp> {
        let net = SvmNetwork::connect("solana", self.chain.clone(), self.operator_key.insecure_clone(), &lc::ID.to_string(), &pr::ID.to_string()).unwrap();
        Arc::new(crate::conversion::SvmConversion::new(Arc::new(net), &crate::programs::conversion::ID.to_string()).unwrap())
    }

    async fn user_sell(&self, amount: Amount, sats: u64, script: &[u8]) {
        use crate::programs::conversion as cvp;
        let (swap, job, tag_record, _) = self.open_accounts().await;
        let config = cv_pda(&[b"config"]);
        self.chain
            .run(
                &[ix(
                    cvp::ID,
                    cvp::client::accounts::Sell {
                        config,
                        swap,
                        user: self.user.pubkey(),
                        protocol: SvmNetwork::pr_pda(&[b"protocol"]),
                        application: SvmNetwork::pr_pda(&[b"application", config.as_ref()]),
                        job,
                        tag_record,
                        protocol_vault: SvmNetwork::pr_pda(&[b"vault"]),
                        protocol_program: pr::ID,
                        system_program: SYSTEM,
                        mint: None,
                        escrow: None,
                        from: None,
                        token_program: None,
                        associated_token_program: None,
                    },
                    cvp::client::args::Sell { amount: amount as u64, sats, script: script.to_vec(), confirmations: 6, paid: SOL as u64 / 10 },
                )],
                &self.user,
            )
            .unwrap();
    }

    async fn user_buy(&self, amount: Amount, sats: u64) {
        use crate::programs::conversion as cvp;
        let (swap, job, tag_record, _) = self.open_accounts().await;
        let config = cv_pda(&[b"config"]);
        self.chain
            .run(
                &[ix(
                    cvp::ID,
                    cvp::client::accounts::Buy {
                        config,
                        swap,
                        user: self.user.pubkey(),
                        protocol: SvmNetwork::pr_pda(&[b"protocol"]),
                        application: SvmNetwork::pr_pda(&[b"application", config.as_ref()]),
                        job,
                        tag_record,
                        protocol_vault: SvmNetwork::pr_pda(&[b"vault"]),
                        protocol_program: pr::ID,
                        system_program: SYSTEM,
                        mint: None,
                    },
                    cvp::client::args::Buy { amount: amount as u64, sats, confirmations: 6, paid: SOL as u64 / 10 },
                )],
                &self.user,
            )
            .unwrap();
    }
}
