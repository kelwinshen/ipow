//! The iPoW protocol's vault on Solana, for the pair Ethereum and Solana
//! (D104). Spec: docs/design/ipow-protocol.md, section 11, decisions D104 to
//! D118. The same rules as `contracts/protocol/iPoWVault.sol` on Ethereum,
//! with the two networks' parts swapped.
//!
//! Here it issues vETH, the receipt for ETH locked on Ethereum, for LOCK
//! records from Ethereum after 7 days of objections (D111). It judges the
//! records about Solana, REQUEST, CANCEL and BOND for Solana, and slashes an
//! operator's vETH bond for a false one (D109). No key can change the program
//! once its upgrade authority is removed (D59).

use anchor_lang::prelude::*;
pub use ipow_protocol::state::BlockRef;

pub mod constants;
pub mod errors;
pub mod state;
pub mod util;

// Instruction modules sit at the crate root and are named after their
// `Accounts` struct, to work around the anchor-lang 1.0 `#[program]` macro
// bug described in `programs/ipow/src/lib.rs`.
pub mod add_bond;
pub mod add_deposits;
pub mod answer;
pub mod close_buffer;
pub mod collect;
pub mod decide;
pub mod give_up;
pub mod initialize;
pub mod issue;
pub mod object;
pub mod open_buffer;
pub mod open_checkpoint;
pub mod record_real;
pub mod record_real_from_job;
pub mod register_chain;
pub mod request;
pub mod submit_message;
pub mod withdraw_bond;
pub mod withdraw_credit;
pub mod withdraw_deposits;
pub mod write_buffer;

pub use add_bond::AddBond;
pub(crate) use add_bond::__client_accounts_add_bond;
pub use add_deposits::AddDeposits;
pub(crate) use add_deposits::__client_accounts_add_deposits;
pub use answer::Answer;
pub use close_buffer::CloseBuffer;
pub(crate) use close_buffer::__client_accounts_close_buffer;
pub(crate) use answer::__client_accounts_answer;
pub use collect::Collect;
pub(crate) use collect::__client_accounts_collect;
pub use decide::Decide;
pub(crate) use decide::__client_accounts_decide;
pub use give_up::GiveUp;
pub(crate) use give_up::__client_accounts_give_up;
pub use initialize::Initialize;
pub(crate) use initialize::__client_accounts_initialize;
pub use issue::Issue;
pub(crate) use issue::__client_accounts_issue;
pub use object::Object;
pub use open_buffer::OpenBuffer;
pub(crate) use open_buffer::__client_accounts_open_buffer;
pub(crate) use object::__client_accounts_object;
pub use open_checkpoint::OpenCheckpoint;
pub(crate) use open_checkpoint::__client_accounts_open_checkpoint;
pub use record_real::RecordReal;
pub(crate) use record_real::__client_accounts_record_real;
pub use record_real_from_job::RecordRealFromJob;
pub(crate) use record_real_from_job::__client_accounts_record_real_from_job;
pub use register_chain::RegisterChain;
pub(crate) use register_chain::__client_accounts_register_chain;
pub use request::MakeRequest;
pub(crate) use request::__client_accounts_make_request;
pub use submit_message::SubmitMessage;
pub(crate) use submit_message::__client_accounts_submit_message;
pub use withdraw_bond::WithdrawBond;
pub(crate) use withdraw_bond::__client_accounts_withdraw_bond;
pub use withdraw_credit::WithdrawCredit;
pub(crate) use withdraw_credit::__client_accounts_withdraw_credit;
pub use withdraw_deposits::WithdrawDeposits;
pub use write_buffer::WriteBuffer;
pub(crate) use write_buffer::__client_accounts_write_buffer;
pub(crate) use withdraw_deposits::__client_accounts_withdraw_deposits;

use state::Btc;

declare_id!("2e1ZUB5eqA6bQfH3f9pbfvibfie7WnvEJeKif53VAm7p");

#[program]
pub mod ipow_vault {
    use super::*;

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

    pub fn register_chain(ctx: Context<RegisterChain>, peer_operator: [u8; 20], btc: Btc, coin_index: u32, tag_index: u32) -> Result<()> {
        register_chain::handler(ctx, peer_operator, btc, coin_index, tag_index)
    }

    pub fn add_bond(ctx: Context<AddBond>, amount: u64) -> Result<()> {
        add_bond::handler(ctx, amount)
    }

    pub fn withdraw_bond(ctx: Context<WithdrawBond>, amount: u64) -> Result<()> {
        withdraw_bond::handler(ctx, amount)
    }

    pub fn add_deposits(ctx: Context<AddDeposits>, amount: u64) -> Result<()> {
        add_deposits::handler(ctx, amount)
    }

    pub fn withdraw_deposits(ctx: Context<WithdrawDeposits>, amount: u64) -> Result<()> {
        withdraw_deposits::handler(ctx, amount)
    }

    pub fn make_request(ctx: Context<MakeRequest>, amount: u64, to: [u8; 20], fee: u64) -> Result<()> {
        request::handler(ctx, amount, to, fee)
    }

    pub fn give_up(ctx: Context<GiveUp>, claim_id: u64, lock_id: u64) -> Result<()> {
        give_up::handler(ctx, claim_id, lock_id)
    }

    pub fn issue(ctx: Context<Issue>, claim_id: u64, lock_id: u64) -> Result<()> {
        issue::handler(ctx, claim_id, lock_id)
    }

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
        collect::handler(ctx, claim_id)
    }

    pub fn withdraw_credit(ctx: Context<WithdrawCredit>) -> Result<()> {
        withdraw_credit::handler(ctx)
    }

    pub fn open_buffer(ctx: Context<OpenBuffer>) -> Result<()> {
        open_buffer::handler(ctx)
    }

    pub fn write_buffer(ctx: Context<WriteBuffer>, raw: bool, data: Vec<u8>) -> Result<()> {
        write_buffer::handler(ctx, raw, data)
    }

    pub fn close_buffer(ctx: Context<CloseBuffer>) -> Result<()> {
        close_buffer::handler(ctx)
    }
}
