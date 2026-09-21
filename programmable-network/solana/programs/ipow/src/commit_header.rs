use anchor_lang::prelude::*;
use sha2::{Digest, Sha256};

use crate::bitcoin::{expected_retarget_bits, target_from_bits, validate_work_le};
use crate::constants::{DIFF_PERIOD, PERMISSIONLESS_HEADER_DELAY_SEC};
use crate::errors::IPoWError;
use crate::state::{GlobalHeader, GlobalState, HeightTracker};

pub fn handler(ctx: Context<CommitHeader>, header_80: [u8; 80], height: u64) -> Result<()> {
    let global_state = &mut ctx.accounts.global_state;
    let header_pda = &mut ctx.accounts.header;
    let now = Clock::get()?.unix_timestamp;

    // Operator-first, permissionless fallback (DESIGN_V2 §6.9). "Extending"
    // = the next height with the tip header account supplied; only that
    // shape is open to non-operators, and only once the tip has sat
    // unextended for `PERMISSIONLESS_HEADER_DELAY_SEC`. Anchoring a fresh
    // relay and jumping ahead stay operator-only.
    let is_operator = ctx.accounts.submitter.key() == global_state.operator;
    let extending = height > 0
        && height == global_state.global_tip_height + 1
        && ctx.accounts.prev_header.is_some();
    if !is_operator {
        require!(extending, IPoWError::Unauthorized);
        let prev = ctx.accounts.prev_header.as_ref().unwrap();
        require!(
            now >= prev.arrival_time + PERMISSIONLESS_HEADER_DELAY_SEC,
            IPoWError::HeaderNotStale
        );
    }
    // The tip header, when it exists, must be supplied — even by the
    // operator — so linkage and difficulty continuity are always checked.
    if height > 0 && height == global_state.global_tip_height + 1 && global_state.global_tip_height != 0 {
        require!(ctx.accounts.prev_header.is_some(), IPoWError::PrevHeaderRequired);
    }

    if height > 0 {
        let (expected_pda, _) = Pubkey::find_program_address(
            &[b"tracker", (height - 1).to_le_bytes().as_ref()],
            ctx.program_id,
        );
        require!(
            ctx.accounts.prev_height_tracker.key() == expected_pda,
            IPoWError::InvalidTrackerAccount
        );

        if !ctx.accounts.prev_height_tracker.data_is_empty() {
            let data = ctx.accounts.prev_height_tracker.try_borrow_data()?;
            let mut data_slice: &[u8] = &data;

            let tracker = HeightTracker::try_deserialize(&mut data_slice)?;

            require!(tracker.count == 0, IPoWError::PendingProofsExist);
        }
    }

    if global_state.global_tip_height != 0 {
        if global_state.active_open_conversions > 0 {
            require!(
                height == global_state.global_tip_height + 1,
                IPoWError::MustBeContiguous
            );
        } else {
            require!(
                height > global_state.global_tip_height,
                IPoWError::InvalidJump
            );
        }
    }

    let hash1 = Sha256::digest(&header_80);
    let hash2 = Sha256::digest(&hash1);
    let mut block_hash_le = [0u8; 32];
    block_hash_le.copy_from_slice(&hash2);

    let mut prev_le = [0u8; 32];
    prev_le.copy_from_slice(&header_80[4..36]);
    let mut merkle_le = [0u8; 32];
    merkle_le.copy_from_slice(&header_80[36..68]);

    let n_bits = u32::from_le_bytes(header_80[72..76].try_into().unwrap());
    let timestamp = u32::from_le_bytes(header_80[68..72].try_into().unwrap());

    let target = target_from_bits(n_bits);
    require!(
        validate_work_le(&block_hash_le, &target),
        IPoWError::LowWork
    );

    if extending {
        let prev = ctx.accounts.prev_header.as_ref().unwrap();
        let (expected_prev, _) = Pubkey::find_program_address(
            &[b"header", (height - 1).to_le_bytes().as_ref()],
            ctx.program_id,
        );
        require!(prev.key() == expected_prev, IPoWError::InvalidHeader);
        require!(prev.height == height - 1, IPoWError::InvalidHeader);
        require!(prev_le == prev.hash_le, IPoWError::PrevAndTipUnmatch);
        // Inside a difficulty epoch nBits is constant; at an epoch boundary
        // the retarget rule below decides instead.
        if height % DIFF_PERIOD != 0 {
            require!(n_bits == prev.n_bits, IPoWError::BitsMismatch);
        }
    }

    if height % DIFF_PERIOD == 0
        && global_state.global_tip_height != 0
        && height == global_state.global_tip_height + 1
    {
        let start_header = ctx.accounts.prev_epoch_start_header.as_ref().unwrap();
        let end_header = ctx.accounts.prev_epoch_end_header.as_ref().unwrap();

        let expected_bits = expected_retarget_bits(
            start_header.timestamp,
            end_header.timestamp,
            start_header.n_bits,
        )?;
        require!(n_bits == expected_bits, IPoWError::InvalidRetarget);
    }

    header_pda.height = height;
    header_pda.hash_le = block_hash_le;
    header_pda.prev_hash_le = prev_le;
    header_pda.merkle_root_le = merkle_le;
    header_pda.n_bits = n_bits;
    header_pda.timestamp = timestamp;
    header_pda.arrival_time = Clock::get()?.unix_timestamp;

    if height > global_state.global_tip_height {
        global_state.global_tip_height = height;
    }

    Ok(())
}

#[derive(Accounts)]
#[instruction(header_80: [u8; 80], height: u64)]
pub struct CommitHeader<'info> {
    #[account(mut, seeds = [b"global_state"], bump)]
    pub global_state: Account<'info, GlobalState>,

    #[account(
        init,
        payer = submitter,
        space = 8 + GlobalHeader::INIT_SPACE,
        seeds = [b"header".as_ref(), height.to_le_bytes().as_ref()],
        bump
    )]
    pub header: Account<'info, GlobalHeader>,

    /// CHECK: address is manually verified inside the handler (only when
    /// `height > 0`) via `Pubkey::find_program_address` against the expected
    /// tracker PDA for `height - 1`, and rejected with `InvalidTrackerAccount` on
    /// mismatch — done manually rather than via a declarative `seeds` constraint
    /// because the seed depends on `height - 1`, which would underflow when
    /// `height == 0`.
    pub prev_height_tracker: UncheckedAccount<'info>,

    pub prev_epoch_start_header: Option<Account<'info, GlobalHeader>>,
    pub prev_epoch_end_header: Option<Account<'info, GlobalHeader>>,

    /// The current tip's header account (`height - 1`). Required whenever
    /// the relay already has a tip and this header extends it; checked for
    /// prev-hash linkage and nBits continuity, and its `arrival_time` gates
    /// the permissionless fallback.
    /// Address is verified in the handler (`prev.height == height - 1`, and
    /// `Account<GlobalHeader>` already proves this program owns it).
    pub prev_header: Option<Account<'info, GlobalHeader>>,

    /// Any signer. Non-operators may only extend a stale tip (see handler).
    #[account(mut)]
    pub submitter: Signer<'info>,
    pub system_program: Program<'info, System>,
}
