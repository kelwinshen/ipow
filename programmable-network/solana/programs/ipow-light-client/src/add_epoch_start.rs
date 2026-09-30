use anchor_lang::prelude::*;

use crate::bitcoin::{Header, EPOCH_BLOCKS, HEADER_LENGTH};
use crate::constants::{
    COMPARE_STEP, CONFIG_SEED, DAYS_SEED, DAY_SLOTS, EPOCH_START_BLOCKS, EPOCH_START_SEED, MAX_ANCHOR_AGE,
    MAX_EPOCH_START_AGE,
};
use crate::errors::LightClientError;
use crate::state::{Config, DayTable, EpochStart};
use crate::utils::{check_header, create_pda, store_node, target};
use crate::bitcoin::U256;

/// Records an epoch start: 6 blocks that name each other in order and have
/// one difficulty (D44). Anyone may call (D45). Recording one that is
/// recorded already changes nothing. The 6 block accounts come as the
/// remaining accounts, in order.
pub fn handler<'info>(
    ctx: Context<'info, AddEpochStart<'info>>,
    headers: Vec<u8>,
    height: u32,
) -> Result<()> {
    require!(headers.len() == EPOCH_START_BLOCKS * HEADER_LENGTH, LightClientError::InvalidLength);
    require!(ctx.remaining_accounts.len() == EPOCH_START_BLOCKS, LightClientError::InvalidLength);
    require!(height % EPOCH_BLOCKS == 0, LightClientError::InvalidHeight);
    // D93. A jump is never below its epoch start, and streaming only goes up.
    require!(height >= ctx.accounts.config.min_height, LightClientError::BelowMinHeight);

    let now = Clock::get()?.unix_timestamp;
    let first = Header(&headers[0..HEADER_LENGTH]);
    let first_time = first.time();
    let first_bits = first.bits();
    // An epoch start older than this can serve no anchor (D39, D57).
    require!(
        now <= first_time as i64 + MAX_EPOCH_START_AGE + MAX_ANCHOR_AGE,
        LightClientError::EpochStartTooOld
    );

    let program_id = ctx.program_id;
    let payer = ctx.accounts.payer.to_account_info();
    let system = ctx.accounts.system_program.to_account_info();
    let mut prev_hash = [0u8; 32];
    let mut first_hash = [0u8; 32];
    for i in 0..EPOCH_START_BLOCKS {
        let header = Header(&headers[i * HEADER_LENGTH..(i + 1) * HEADER_LENGTH]);
        require!(header.bits() == first_bits, LightClientError::DifficultyMismatch);
        if i > 0 {
            require!(header.prev() == prev_hash, LightClientError::NotLinked);
        }
        let hash = check_header(&header, &ctx.accounts.config, now)?;
        store_node(
            program_id,
            &ctx.remaining_accounts[i],
            &payer,
            &system,
            &header,
            hash,
            height + i as u32,
            first_time,
            now,
        )?;
        if i == 0 {
            first_hash = hash;
        }
        prev_hash = hash;
    }

    // The record of the epoch start, unless it exists.
    let account = &ctx.accounts.epoch_start;
    let height_bytes = height.to_le_bytes();
    let time_bytes = first_time.to_le_bytes();
    let (address, bump) = Pubkey::find_program_address(
        &[EPOCH_START_SEED, &first_hash, &height_bytes, &time_bytes],
        program_id,
    );
    require_keys_eq!(account.key(), address, LightClientError::WrongAccount);
    if account.owner == program_id && !account.data_is_empty() {
        return Ok(());
    }
    let space = 8 + EpochStart::INIT_SPACE;
    let seeds: &[&[u8]] = &[EPOCH_START_SEED, &first_hash, &height_bytes, &time_bytes, &[bump]];
    create_pda(program_id, &account.to_account_info(), &payer, &system, space, seeds)?;
    let record = EpochStart { bits: first_bits, first_time, height, recorded_at: now, payer: payer.key(), bump };
    {
        let info = account.to_account_info();
        let mut data = info.try_borrow_mut_data()?;
        record.try_serialize(&mut &mut data[..])?;
    }

    // D45: the lowest target of the day of its first block.
    let t = target(first_bits)?;
    let day = (first_time as i64 / COMPARE_STEP) as u32;
    let slot = &mut ctx.accounts.day_table.slots[day as usize % DAY_SLOTS];
    let empty = slot.day != day || slot.lowest_target == [0u8; 32];
    if empty || t.lt(&U256::from_be_bytes(&slot.lowest_target)) {
        slot.day = day;
        slot.lowest_target = t.to_be_bytes();
    }

    emit!(crate::EpochStartRecorded { first_hash, bits: first_bits, height, first_time });
    Ok(())
}

#[derive(Accounts)]
pub struct AddEpochStart<'info> {
    #[account(seeds = [CONFIG_SEED], bump = config.bump)]
    pub config: Account<'info, Config>,
    #[account(mut, seeds = [DAYS_SEED], bump = day_table.bump)]
    pub day_table: Account<'info, DayTable>,
    /// CHECK: the record of this epoch start, at the address its first block
    /// gives; checked and created in the handler.
    #[account(mut)]
    pub epoch_start: UncheckedAccount<'info>,
    #[account(mut)]
    pub payer: Signer<'info>,
    pub system_program: Program<'info, System>,
}
