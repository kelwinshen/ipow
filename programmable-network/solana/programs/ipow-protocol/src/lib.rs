//! The iPoW protocol on Solana: operators, jobs, fees, the auction, the
//! duty, punishment, claims and attest. Spec: docs/design/ipow-protocol.md.
//! The same rules as `contracts/protocol/iPoWProtocol.sol` on Ethereum. The
//! `D` numbers are the decisions of the spec.
//!
//! No key controls it: after deployment the upgrade authority is removed
//! (D59). All money is the network's own coin, lamports (D84), held in one
//! vault; records say whose it is.
//!
//! Built: the bond, registration of an application, opening a job, the
//! fees, the auction, the chain head, the anchor, the proof, the payment,
//! the guardian's note, the slash of a missed duty, the challenge of a
//! proof, attest, and the check that a message is official.

use anchor_lang::prelude::*;

pub mod auction;
pub mod bitcoin_tx;
pub mod duty;
pub mod constants;
pub mod errors;
pub mod fees;
pub mod money;
pub mod state;

// Instruction modules sit at the crate root and are named after their
// `Accounts` struct, to work around the anchor-lang 1.0 `#[program]` macro
// bug described in `programs/ipow/src/lib.rs`.
pub mod anchor_job;
pub mod attest;
pub mod challenge;
pub mod bid;
pub mod notes;
pub mod prove_job;
pub mod register_chain_head;
pub mod settle;
pub mod expire;
pub mod initialize_protocol;
pub mod lock_bond;
pub mod open_job;
pub mod register_application;
pub mod withdraw_bond;
pub mod withdraw_credit;

pub use anchor_job::AnchorJob;
pub(crate) use anchor_job::__client_accounts_anchor_job;
pub use attest::{Attest, RequireOfficial};
pub(crate) use attest::{__client_accounts_attest, __client_accounts_require_official};
pub use bid::Bid;
pub use challenge::{AskParent, ChallengeFork, ExtendBranch, ForkArgs, ResolveChallenge, ShowParent};
pub(crate) use challenge::{
    __client_accounts_ask_parent, __client_accounts_challenge_fork, __client_accounts_extend_branch,
    __client_accounts_resolve_challenge, __client_accounts_show_parent,
};
pub use notes::{ReportMissedDuty, SealNote};
pub(crate) use notes::{__client_accounts_report_missed_duty, __client_accounts_seal_note};
pub use prove_job::{Proof, ProveJob};
pub(crate) use prove_job::__client_accounts_prove_job;
pub use register_chain_head::RegisterChainHead;
pub(crate) use register_chain_head::__client_accounts_register_chain_head;
pub use settle::Settle;
pub(crate) use settle::__client_accounts_settle;
pub(crate) use bid::__client_accounts_bid;
pub use expire::Expire;
pub(crate) use expire::__client_accounts_expire;
pub use initialize_protocol::InitializeProtocol;
pub(crate) use initialize_protocol::__client_accounts_initialize_protocol;
pub use lock_bond::LockBond;
pub(crate) use lock_bond::__client_accounts_lock_bond;
pub use open_job::OpenJob;
pub(crate) use open_job::__client_accounts_open_job;
pub use register_application::RegisterApplication;
pub(crate) use register_application::__client_accounts_register_application;
pub use withdraw_bond::WithdrawBond;
pub(crate) use withdraw_bond::__client_accounts_withdraw_bond;
pub use withdraw_credit::WithdrawCredit;
pub(crate) use withdraw_credit::__client_accounts_withdraw_credit;

// The same bug hides the accounts the generated CPI helpers use.
#[cfg(feature = "cpi")]
pub(crate) use anchor_job::__cpi_client_accounts_anchor_job;
#[cfg(feature = "cpi")]
pub(crate) use attest::__cpi_client_accounts_attest;
#[cfg(feature = "cpi")]
pub(crate) use attest::__cpi_client_accounts_require_official;
#[cfg(feature = "cpi")]
pub(crate) use challenge::__cpi_client_accounts_ask_parent;
#[cfg(feature = "cpi")]
pub(crate) use challenge::__cpi_client_accounts_challenge_fork;
#[cfg(feature = "cpi")]
pub(crate) use challenge::__cpi_client_accounts_extend_branch;
#[cfg(feature = "cpi")]
pub(crate) use challenge::__cpi_client_accounts_resolve_challenge;
#[cfg(feature = "cpi")]
pub(crate) use challenge::__cpi_client_accounts_show_parent;
#[cfg(feature = "cpi")]
pub(crate) use notes::__cpi_client_accounts_report_missed_duty;
#[cfg(feature = "cpi")]
pub(crate) use notes::__cpi_client_accounts_seal_note;
#[cfg(feature = "cpi")]
pub(crate) use prove_job::__cpi_client_accounts_prove_job;
#[cfg(feature = "cpi")]
pub(crate) use register_chain_head::__cpi_client_accounts_register_chain_head;
#[cfg(feature = "cpi")]
pub(crate) use settle::__cpi_client_accounts_settle;
#[cfg(feature = "cpi")]
pub(crate) use bid::__cpi_client_accounts_bid;
#[cfg(feature = "cpi")]
pub(crate) use expire::__cpi_client_accounts_expire;
#[cfg(feature = "cpi")]
pub(crate) use initialize_protocol::__cpi_client_accounts_initialize_protocol;
#[cfg(feature = "cpi")]
pub(crate) use lock_bond::__cpi_client_accounts_lock_bond;
#[cfg(feature = "cpi")]
pub(crate) use open_job::__cpi_client_accounts_open_job;
#[cfg(feature = "cpi")]
pub(crate) use register_application::__cpi_client_accounts_register_application;
#[cfg(feature = "cpi")]
pub(crate) use withdraw_bond::__cpi_client_accounts_withdraw_bond;
#[cfg(feature = "cpi")]
pub(crate) use withdraw_credit::__cpi_client_accounts_withdraw_credit;

declare_id!("ChYhovM8vm2tuMRaRFn4m6etG979bjixVa71fXBDwjPL");

#[event]
pub struct BondLocked {
    pub operator: Pubkey,
    pub amount: u64,
}

#[event]
pub struct BondWithdrawn {
    pub operator: Pubkey,
    pub amount: u64,
}

#[event]
pub struct ApplicationRegistered {
    pub application: Pubkey,
}

#[event]
pub struct JobOpened {
    pub job_id: u64,
    pub application: Pubkey,
    pub tag: [u8; 32],
    pub escrow: u64,
    pub commitment_fee: u64,
    pub escrow_fee: u64,
    pub confirmations: u16,
    pub claim_kind: u16,
    pub payer: Pubkey,
}

#[event]
pub struct BidPlaced {
    pub job_id: u64,
    pub operator: Pubkey,
    pub amount: u64,
}

#[event]
pub struct JobExpired {
    pub job_id: u64,
}

#[event]
pub struct ChainHeadSet {
    pub operator: Pubkey,
    pub txid: [u8; 32],
    pub vout: u32,
}

#[event]
pub struct JobAnchored {
    pub job_id: u64,
    pub anchor_hash: [u8; 32],
    pub height: u32,
}

#[event]
pub struct JobProven {
    pub job_id: u64,
    pub txid: [u8; 32],
    pub lock_end: i64,
}

#[event]
pub struct JobSettled {
    pub job_id: u64,
    pub operator: Pubkey,
    pub paid: u64,
}

#[event]
pub struct JobSlashed {
    pub job_id: u64,
    pub operator: Pubkey,
    pub guardian: Pubkey,
    pub to_application: u64,
    pub to_guardian: u64,
}

#[event]
pub struct ParentAsked {
    pub challenge_id: u64,
    pub job_id: u64,
    pub guardian: Pubkey,
    pub child_hash: [u8; 32],
}

#[event]
pub struct ParentShown {
    pub challenge_id: u64,
    pub parent_hash: [u8; 32],
}

#[event]
pub struct ForkChallenged {
    pub challenge_id: u64,
    pub job_id: u64,
    pub guardian: Pubkey,
}

#[event]
pub struct BranchExtended {
    pub challenge_id: u64,
    pub guardian_side: bool,
    pub tip_hash: [u8; 32],
}

#[event]
pub struct ChallengeFailed {
    pub challenge_id: u64,
    pub guardian: Pubkey,
}

#[event]
pub struct ChallengeWon {
    pub challenge_id: u64,
    pub guardian: Pubkey,
}

#[event]
pub struct ChallengeRefunded {
    pub challenge_id: u64,
    pub guardian: Pubkey,
}

#[event]
pub struct Attested {
    pub job_id: u64,
    pub attester: Pubkey,
    pub amount: u64,
}

#[event]
pub struct Credited {
    pub to: Pubkey,
    pub amount: u64,
}

#[event]
pub struct CreditWithdrawn {
    pub to: Pubkey,
    pub amount: u64,
}

#[program]
pub mod ipow_protocol {
    use super::*;

    pub fn initialize_protocol(ctx: Context<InitializeProtocol>) -> Result<()> {
        initialize_protocol::handler(ctx)
    }

    pub fn lock_bond(ctx: Context<LockBond>, amount: u64) -> Result<()> {
        lock_bond::handler(ctx, amount)
    }

    pub fn withdraw_bond(ctx: Context<WithdrawBond>, amount: u64) -> Result<()> {
        withdraw_bond::handler(ctx, amount)
    }

    pub fn register_application(ctx: Context<RegisterApplication>, challenge_periods: Vec<u32>) -> Result<()> {
        register_application::handler(ctx, challenge_periods)
    }

    #[allow(clippy::too_many_arguments)]
    pub fn open_job(
        ctx: Context<OpenJob>,
        tag: [u8; 32],
        escrow: u64,
        escrow_fee_bps: u16,
        confirmations: u16,
        claim_kind: u16,
        payer: Pubkey,
        paid: u64,
    ) -> Result<()> {
        open_job::handler(ctx, tag, escrow, escrow_fee_bps, confirmations, claim_kind, payer, paid)
    }

    pub fn bid(ctx: Context<Bid>, amount: u64) -> Result<()> {
        bid::handler(ctx, amount)
    }

    pub fn expire(ctx: Context<Expire>) -> Result<()> {
        expire::handler(ctx)
    }

    pub fn withdraw_credit(ctx: Context<WithdrawCredit>) -> Result<()> {
        withdraw_credit::handler(ctx)
    }

    #[allow(clippy::too_many_arguments)]
    pub fn register_chain_head(
        ctx: Context<RegisterChainHead>,
        txid: [u8; 32],
        block: state::BlockRef,
        raw_tx: Vec<u8>,
        siblings: Vec<[u8; 32]>,
        tx_index: u64,
        coin_index: u32,
        tag_index: u32,
    ) -> Result<()> {
        register_chain_head::register(ctx, txid, block, raw_tx, siblings, tx_index, coin_index, tag_index)
    }

    pub fn advance_chain_head(
        ctx: Context<RegisterChainHead>,
        txid: [u8; 32],
        block: state::BlockRef,
        raw_tx: Vec<u8>,
        siblings: Vec<[u8; 32]>,
        tx_index: u64,
        head_index: u32,
    ) -> Result<()> {
        register_chain_head::advance(ctx, txid, block, raw_tx, siblings, tx_index, head_index)
    }

    pub fn anchor_job(ctx: Context<AnchorJob>, anchor: state::BlockRef) -> Result<()> {
        anchor_job::handler(ctx, anchor)
    }

    pub fn prove_job(ctx: Context<ProveJob>, proof: Proof) -> Result<()> {
        prove_job::handler(ctx, proof)
    }

    pub fn settle(ctx: Context<Settle>) -> Result<()> {
        settle::handler(ctx)
    }

    pub fn seal_note(ctx: Context<SealNote>, note: [u8; 32]) -> Result<()> {
        notes::seal(ctx, note)
    }

    pub fn report_missed_duty(ctx: Context<ReportMissedDuty>, note: [u8; 32], salt: [u8; 32]) -> Result<()> {
        notes::report_missed_duty(ctx, note, salt)
    }

    pub fn ask_parent(ctx: Context<AskParent>, note: [u8; 32], salt: [u8; 32], paid: u64) -> Result<()> {
        challenge::ask_parent(ctx, note, salt, paid)
    }

    pub fn show_parent(ctx: Context<ShowParent>, prev_epoch_time: u32) -> Result<()> {
        challenge::show_parent(ctx, prev_epoch_time)
    }

    pub fn challenge_fork(ctx: Context<ChallengeFork>, note: [u8; 32], salt: [u8; 32], paid: u64, args: ForkArgs) -> Result<()> {
        challenge::challenge_fork(ctx, note, salt, paid, args)
    }

    pub fn extend_branch(ctx: Context<ExtendBranch>, guardian_side: bool, from: state::BlockRef, new_tip: state::BlockRef) -> Result<()> {
        challenge::extend_branch(ctx, guardian_side, from, new_tip)
    }

    pub fn resolve_challenge(ctx: Context<ResolveChallenge>) -> Result<()> {
        challenge::resolve(ctx)
    }

    pub fn attest(ctx: Context<Attest>) -> Result<()> {
        attest::attest(ctx)
    }

    pub fn require_official(ctx: Context<RequireOfficial>) -> Result<()> {
        attest::require_official(ctx)
    }
}
