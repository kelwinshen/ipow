//! The iPoW light client on Solana: the store of Bitcoin blocks, shared by
//! everyone. Spec: docs/design/ipow-protocol.md, section 2. The same rules as
//! `contracts/protocol/iPoWLightClient.sol` on Ethereum. The `D` numbers are
//! the decisions of the spec.
//!
//! Anyone can add a block that passes the checks (D64). After deployment the
//! upgrade authority is removed, so nothing can change (D59).
//!
//! One difference from Ethereum: a Solana transaction cannot hold the 100
//! blocks of a long walk, so a walk is done in steps (`begin_walk`,
//! `walk_step`), with the same checks.

use anchor_lang::prelude::*;

pub mod bitcoin;
pub mod constants;
pub mod errors;
pub mod state;
pub mod utils;

// Instruction modules sit at the crate root and are named after their
// `Accounts` struct, to work around the anchor-lang 1.0 `#[program]` macro
// bug: it expects each `__client_accounts_<snake_case(struct)>` module at
// `crate::<that name>` (https://github.com/solana-foundation/anchor/issues/3690).
pub mod add_epoch_start;
pub mod check_tx;
pub mod close;
pub mod extend;
pub mod extend_back;
pub mod initialize;
pub mod jump;
pub mod walk;

pub use add_epoch_start::AddEpochStart;
pub(crate) use add_epoch_start::__client_accounts_add_epoch_start;
pub use check_tx::CheckTx;
pub use close::{CloseEpochStart, CloseNode};
pub(crate) use close::{__client_accounts_close_epoch_start, __client_accounts_close_node};
pub(crate) use check_tx::__client_accounts_check_tx;
pub use extend::Extend;
pub(crate) use extend::__client_accounts_extend;
pub use extend_back::ExtendBack;
pub(crate) use extend_back::__client_accounts_extend_back;
pub use initialize::Initialize;
pub(crate) use initialize::__client_accounts_initialize;
pub use jump::Jump;
pub(crate) use jump::__client_accounts_jump;
pub use walk::{BeginWalk, CloseWalk, WalkStep};
pub(crate) use walk::{__client_accounts_begin_walk, __client_accounts_close_walk, __client_accounts_walk_step};

use state::BlockRef;

declare_id!("Ctotq1SSaDJ2EUSe4sMdau92qBesyutH7GPaTRiYp4vJ");

#[event]
pub struct BlockStored {
    pub hash: [u8; 32],
    pub height: u32,
    pub epoch_time: u32,
    pub prev_hash: [u8; 32],
}

#[event]
pub struct EpochStartRecorded {
    pub first_hash: [u8; 32],
    pub bits: u32,
    pub height: u32,
    pub first_time: u32,
}

#[event]
pub struct Jumped {
    pub hash: [u8; 32],
    pub height: u32,
    pub epoch_time: u32,
    pub by: Pubkey,
}

#[program]
pub mod ipow_light_client {
    use super::*;

    pub fn initialize(
        ctx: Context<Initialize>,
        min_height: u32,
        max_target: [u8; 32],
        pow_limit: [u8; 32],
    ) -> Result<()> {
        initialize::handler(ctx, min_height, max_target, pow_limit)
    }

    pub fn add_epoch_start<'info>(
        ctx: Context<'info, AddEpochStart<'info>>,
        headers: Vec<u8>,
        height: u32,
    ) -> Result<()> {
        add_epoch_start::handler(ctx, headers, height)
    }

    pub fn jump(ctx: Context<Jump>, header: [u8; 80], height: u32) -> Result<()> {
        jump::handler(ctx, header, height)
    }

    pub fn extend<'info>(ctx: Context<'info, Extend<'info>>, headers: Vec<u8>) -> Result<()> {
        extend::handler(ctx, headers)
    }

    pub fn extend_back(ctx: Context<ExtendBack>, header: [u8; 80], prev_epoch_time: u32) -> Result<()> {
        extend_back::handler(ctx, header, prev_epoch_time)
    }

    pub fn begin_walk(
        ctx: Context<BeginWalk>,
        walk_id: u64,
        descendant: BlockRef,
        ancestor: BlockRef,
        prev_epoch_time: u32,
    ) -> Result<()> {
        walk::begin(ctx, walk_id, descendant, ancestor, prev_epoch_time)
    }

    pub fn walk_step<'info>(ctx: Context<'info, WalkStep<'info>>) -> Result<()> {
        walk::step(ctx)
    }

    pub fn close_walk(_ctx: Context<CloseWalk>) -> Result<()> {
        Ok(())
    }

    pub fn close_node(ctx: Context<CloseNode>) -> Result<()> {
        close::close_node(ctx)
    }

    pub fn close_epoch_start(ctx: Context<CloseEpochStart>) -> Result<()> {
        close::close_epoch_start(ctx)
    }

    pub fn check_tx(ctx: Context<CheckTx>, raw_tx: Vec<u8>, siblings: Vec<[u8; 32]>, index: u64) -> Result<()> {
        check_tx::handler(ctx, raw_tx, siblings, index)
    }
}
