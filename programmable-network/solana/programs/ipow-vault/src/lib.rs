//! The iPoW protocol's vault on Solana, for the pair Ethereum and Solana
//! (D104). Spec: docs/design/ipow-protocol.md, section 11, decisions D104 to
//! D131. The same rules as `contracts/protocol/iPoWVault.sol` and its parts
//! on Ethereum, with the two networks' parts swapped.
//!
//! Solana is the home of SOL and of any SPL token anyone registers: they are
//! locked here for receipts on Ethereum, and paid out for burns of those
//! receipts there. It holds receipts of Ethereum's assets, vETH first,
//! issued for LOCK records from Ethereum after 7 days of objections (D111),
//! or at once when an attester locks 1.25 times the amount in receipts
//! (section 11.7). It judges the records about Solana and slashes every
//! bond of an operator for a false one (D109, D129). No key can change the
//! program once its upgrade authority is removed (D59).

use anchor_lang::prelude::*;
pub use ipow_protocol::state::BlockRef;

pub mod constants;
pub mod errors;
pub mod state;
pub mod util;

// Instruction modules sit at the crate root, and each `Accounts` struct's
// client module is re-exported, to work around the anchor-lang 1.0
// `#[program]` macro bug described in `programs/ipow/src/lib.rs`.
pub mod add_deposits;
pub mod answer;
pub mod attest_lock;
pub mod bond;
pub mod burn_fast;
pub mod close_buffer;
pub mod close_message;
pub mod credit;
pub mod decide;
pub mod fast_pay;
pub mod give_up;
pub mod home_lock;
pub mod initialize;
pub mod issue;
pub mod link_fast;
pub mod make_receipt;
pub mod object;
pub mod open_buffer;
pub mod open_checkpoint;
pub mod pay_request;
pub mod record_real;
pub mod record_real_from_job;
pub mod register_asset;
pub mod register_chain;
pub mod request;
pub mod return_lock;
pub mod settle_fast;
pub mod settle_slash;
pub mod submit_message;
pub mod take_lock_fee;
pub mod take_request_fee;
pub mod withdraw_deposits;
pub mod write_buffer;

pub use add_deposits::AddDeposits;
pub(crate) use add_deposits::__client_accounts_add_deposits;
pub use answer::Answer;
pub(crate) use answer::__client_accounts_answer;
pub use attest_lock::AttestLock;
pub(crate) use attest_lock::__client_accounts_attest_lock;
pub use bond::{AddBondHome, AddBondReceipt, WithdrawBondHome, WithdrawBondReceipt};
pub(crate) use bond::{
    __client_accounts_add_bond_home, __client_accounts_add_bond_receipt, __client_accounts_withdraw_bond_home,
    __client_accounts_withdraw_bond_receipt,
};
pub use burn_fast::BurnFast;
pub(crate) use burn_fast::__client_accounts_burn_fast;
pub use close_buffer::CloseBuffer;
pub(crate) use close_buffer::__client_accounts_close_buffer;
pub use close_message::CloseMessage;
pub(crate) use close_message::__client_accounts_close_message;
pub use credit::{Collect, WithdrawCreditHome, WithdrawCreditReceipt};
pub(crate) use credit::{__client_accounts_collect, __client_accounts_withdraw_credit_home, __client_accounts_withdraw_credit_receipt};
pub use decide::Decide;
pub(crate) use decide::__client_accounts_decide;
pub use fast_pay::FastPayBurn;
pub(crate) use fast_pay::__client_accounts_fast_pay_burn;
pub use give_up::GiveUp;
pub(crate) use give_up::__client_accounts_give_up;
pub use home_lock::LockHome;
pub(crate) use home_lock::__client_accounts_lock_home;
pub use initialize::Initialize;
pub(crate) use initialize::__client_accounts_initialize;
pub use issue::Issue;
pub(crate) use issue::__client_accounts_issue;
pub use link_fast::LinkFast;
pub(crate) use link_fast::__client_accounts_link_fast;
pub use make_receipt::MakeReceipt;
pub(crate) use make_receipt::__client_accounts_make_receipt;
pub use object::Object;
pub(crate) use object::__client_accounts_object;
pub use open_buffer::OpenBuffer;
pub(crate) use open_buffer::__client_accounts_open_buffer;
pub use open_checkpoint::OpenCheckpoint;
pub(crate) use open_checkpoint::__client_accounts_open_checkpoint;
pub use pay_request::PayRequest;
pub(crate) use pay_request::__client_accounts_pay_request;
pub use record_real::RecordReal;
pub(crate) use record_real::__client_accounts_record_real;
pub use record_real_from_job::RecordRealFromJob;
pub(crate) use record_real_from_job::__client_accounts_record_real_from_job;
pub use register_asset::RegisterAsset;
pub(crate) use register_asset::__client_accounts_register_asset;
pub use register_chain::RegisterChain;
pub(crate) use register_chain::__client_accounts_register_chain;
pub use request::MakeRequest;
pub(crate) use request::__client_accounts_make_request;
pub use return_lock::ReturnLock;
pub(crate) use return_lock::__client_accounts_return_lock;
pub use settle_fast::SettleFast;
pub(crate) use settle_fast::__client_accounts_settle_fast;
pub use settle_slash::SettleSlash;
pub(crate) use settle_slash::__client_accounts_settle_slash;
pub use submit_message::SubmitMessage;
pub(crate) use submit_message::__client_accounts_submit_message;
pub use take_lock_fee::TakeLockFee;
pub(crate) use take_lock_fee::__client_accounts_take_lock_fee;
pub use take_request_fee::TakeRequestFee;
pub(crate) use take_request_fee::__client_accounts_take_request_fee;
pub use withdraw_deposits::WithdrawDeposits;
pub(crate) use withdraw_deposits::__client_accounts_withdraw_deposits;
pub use write_buffer::WriteBuffer;
pub(crate) use write_buffer::__client_accounts_write_buffer;

use state::Btc;

declare_id!("2e1ZUB5eqA6bQfH3f9pbfvibfie7WnvEJeKif53VAm7p");

#[program]
pub mod ipow_vault {
    use super::*;

    // Settings and real Bitcoin (D108, D116, D118)

    pub fn initialize(ctx: Context<Initialize>, ethereum_vault: [u8; 20], deposit: u64, min_certifying_escrow: u64) -> Result<()> {
        initialize::handler(ctx, ethereum_vault, deposit, min_certifying_escrow)
    }

    pub fn record_real_from_job(ctx: Context<RecordRealFromJob>, job_id: u64) -> Result<()> {
        record_real_from_job::handler(ctx, job_id)
    }

    pub fn record_real(ctx: Context<RecordReal>, low: BlockRef, high: BlockRef) -> Result<()> {
        record_real::handler(ctx, low, high)
    }

    pub fn open_checkpoint(ctx: Context<OpenCheckpoint>, confirmations: u16, paid: u64) -> Result<()> {
        open_checkpoint::handler(ctx, confirmations, paid)
    }

    // Pair chains, bonds and deposits (D106, D110, D129)

    pub fn register_chain(ctx: Context<RegisterChain>, peer_operator: [u8; 20], btc: Btc, coin_index: u32, tag_index: u32) -> Result<()> {
        register_chain::handler(ctx, peer_operator, btc, coin_index, tag_index)
    }

    pub fn add_bond_home<'info>(ctx: Context<'info, AddBondHome<'info>>, asset: u32, amount: u64) -> Result<()> {
        bond::add_home(ctx, asset, amount)
    }

    pub fn add_bond_receipt(ctx: Context<AddBondReceipt>, asset: u32, amount: u64) -> Result<()> {
        bond::add_receipt(ctx, asset, amount)
    }

    pub fn withdraw_bond_home<'info>(ctx: Context<'info, WithdrawBondHome<'info>>, asset: u32, amount: u64) -> Result<()> {
        bond::withdraw_home(ctx, asset, amount)
    }

    pub fn withdraw_bond_receipt(ctx: Context<WithdrawBondReceipt>, asset: u32, amount: u64) -> Result<()> {
        bond::withdraw_receipt(ctx, asset, amount)
    }

    pub fn add_deposits(ctx: Context<AddDeposits>, amount: u64) -> Result<()> {
        add_deposits::handler(ctx, amount)
    }

    pub fn withdraw_deposits(ctx: Context<WithdrawDeposits>, amount: u64) -> Result<()> {
        withdraw_deposits::handler(ctx, amount)
    }

    pub fn settle_slash<'info>(ctx: Context<'info, SettleSlash<'info>>, home: u8, asset: u32) -> Result<()> {
        settle_slash::handler(ctx, home, asset)
    }

    // Assets whose home is Solana (section 11.9)

    pub fn register_asset(ctx: Context<RegisterAsset>) -> Result<()> {
        register_asset::handler(ctx)
    }

    pub fn lock<'info>(
        ctx: Context<'info, LockHome<'info>>,
        asset: u32,
        recipient: [u8; 32],
        amount: u64,
        fee: u64,
        fast_fee: u64,
    ) -> Result<()> {
        home_lock::handler(ctx, asset, recipient, amount, fee, fast_fee)
    }

    pub fn take_lock_fee<'info>(ctx: Context<'info, TakeLockFee<'info>>, lock_id: u64) -> Result<()> {
        take_lock_fee::handler(ctx, lock_id)
    }

    pub fn return_lock<'info>(ctx: Context<'info, ReturnLock<'info>>, claim_id: u64, lock_id: u64, record: Vec<u8>) -> Result<()> {
        return_lock::handler(ctx, claim_id, lock_id, record)
    }

    pub fn fast_pay<'info>(
        ctx: Context<'info, FastPayBurn<'info>>,
        request_id: u64,
        asset: u32,
        record_hash: [u8; 32],
        record: Vec<u8>,
    ) -> Result<()> {
        fast_pay::handler(ctx, request_id, asset, record_hash, record)
    }

    pub fn pay_request<'info>(
        ctx: Context<'info, PayRequest<'info>>,
        claim_id: u64,
        request_id: u64,
        asset: u32,
        record: Vec<u8>,
    ) -> Result<()> {
        pay_request::handler(ctx, claim_id, request_id, asset, record)
    }

    // Receipts of Ethereum's assets (section 11.9)

    pub fn make_receipt(ctx: Context<MakeReceipt>, claim_id: u64, asset: u32, record: Vec<u8>) -> Result<()> {
        make_receipt::handler(ctx, claim_id, asset, record)
    }

    pub fn issue(ctx: Context<Issue>, claim_id: u64, lock_id: u64, asset: u32, record: Vec<u8>) -> Result<()> {
        issue::handler(ctx, claim_id, lock_id, asset, record)
    }

    pub fn give_up(ctx: Context<GiveUp>, claim_id: u64, lock_id: u64, record: Vec<u8>) -> Result<()> {
        give_up::handler(ctx, claim_id, lock_id, record)
    }

    pub fn make_request(ctx: Context<MakeRequest>, asset: u32, amount: u64, to: [u8; 32], fee: u64, fast_fee: u64) -> Result<()> {
        request::handler(ctx, asset, amount, to, fee, fast_fee)
    }

    pub fn take_request_fee(ctx: Context<TakeRequestFee>, request_id: u64) -> Result<()> {
        take_request_fee::handler(ctx, request_id)
    }

    // The fast path of locks on Ethereum (section 11.7)

    pub fn attest_lock(ctx: Context<AttestLock>, asset: u32, record: Vec<u8>) -> Result<()> {
        attest_lock::handler(ctx, asset, record)
    }

    pub fn link_fast(ctx: Context<LinkFast>, claim_id: u64, attest: u64) -> Result<()> {
        link_fast::handler(ctx, claim_id, attest)
    }

    pub fn burn_fast(ctx: Context<BurnFast>, attest: u64) -> Result<()> {
        burn_fast::handler(ctx, attest)
    }

    pub fn settle_fast(ctx: Context<SettleFast>, claim_id: u64, attest: u64) -> Result<()> {
        settle_fast::handler(ctx, claim_id, attest)
    }

    // Messages and claims (D107, D108, D109, D111)

    pub fn submit_message<'info>(
        ctx: Context<'info, SubmitMessage<'info>>,
        operator: Pubkey,
        btc: Btc,
        input_index: u32,
        tag_index: u32,
        batch: Vec<u8>,
    ) -> Result<()> {
        submit_message::handler(ctx, operator, btc, input_index, tag_index, batch)
    }

    pub fn object(ctx: Context<Object>, claim_id: u64) -> Result<()> {
        object::handler(ctx, claim_id)
    }

    pub fn answer(ctx: Context<Answer>, claim_id: u64) -> Result<()> {
        answer::handler(ctx, claim_id)
    }

    pub fn decide(ctx: Context<Decide>, claim_id: u64) -> Result<()> {
        decide::handler(ctx, claim_id)
    }

    pub fn collect(ctx: Context<Collect>, claim_id: u64) -> Result<()> {
        credit::collect(ctx, claim_id)
    }

    pub fn withdraw_credit_home<'info>(ctx: Context<'info, WithdrawCreditHome<'info>>, asset: u32) -> Result<()> {
        credit::withdraw_home(ctx, asset)
    }

    pub fn withdraw_credit_receipt(ctx: Context<WithdrawCreditReceipt>, asset: u32) -> Result<()> {
        credit::withdraw_receipt(ctx, asset)
    }

    // Large messages, and published batches (D119, D120)

    pub fn open_buffer(ctx: Context<OpenBuffer>) -> Result<()> {
        open_buffer::handler(ctx)
    }

    pub fn write_buffer(ctx: Context<WriteBuffer>, raw: bool, data: Vec<u8>) -> Result<()> {
        write_buffer::handler(ctx, raw, data)
    }

    pub fn close_buffer(ctx: Context<CloseBuffer>) -> Result<()> {
        close_buffer::handler(ctx)
    }

    pub fn close_message(ctx: Context<CloseMessage>) -> Result<()> {
        close_message::handler(ctx)
    }
}
