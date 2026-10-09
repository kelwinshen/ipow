use anchor_lang::prelude::*;

use crate::bitcoin::{Header, EPOCH_BLOCKS, U256};
use crate::constants::{COMPARE_STEP, CONFIG_SEED, DAYS_SEED, DAY_SLOTS, MAX_ANCHOR_AGE, MAX_EPOCH_START_AGE};
use crate::errors::LightClientError;
use crate::state::{Config, DayTable, EpochStart};
use crate::utils::{check_header, load_node, save_node, store_node, target};

/// Jumps to an anchor. The guards are D38, D39, D44, D45 and D57.
pub fn handler(ctx: Context<Jump>, header: [u8; 80], height: u32) -> Result<()> {
    let es = &ctx.accounts.epoch_start;
    let now = Clock::get()?.unix_timestamp;
    let h = Header(&header);
    let hash = check_header(&h, &ctx.accounts.config, now)?;
    let time = h.time();

    // D44: the anchor has the difficulty of the epoch start.
    require!(h.bits() == es.bits, LightClientError::DifficultyMismatch);
    // D39.
    require!(now <= time as i64 + MAX_ANCHOR_AGE, LightClientError::AnchorTooOld);
    // D57.
    require!(
        time >= es.first_time && (time - es.first_time) as i64 <= MAX_EPOCH_START_AGE,
        LightClientError::EpochStartOutOfRange
    );
    require!(
        height >= es.height && (height as u64) < es.height as u64 + EPOCH_BLOCKS as u64,
        LightClientError::InvalidHeight
    );
    // The first block of an epoch gives the epoch its time, as in `extend`.
    require!(height != es.height || time == es.first_time, LightClientError::EpochTimeMismatch);
    // D45.
    require!(counts(es, &ctx.accounts.day_table, time)?, LightClientError::EpochStartTooWeak);

    let account = ctx.accounts.node.to_account_info();
    store_node(
        ctx.program_id,
        &account,
        &ctx.accounts.payer.to_account_info(),
        &ctx.accounts.system_program.to_account_info(),
        &h,
        hash,
        height,
        es.first_time,
        now,
    )?;
    let mut node = load_node(ctx.program_id, &account)?.unwrap();
    if node.anchored_at == 0 {
        node.anchored_at = now;
        save_node(&account, &node)?;
    }
    emit!(crate::Jumped { hash, height, epoch_time: es.first_time, by: ctx.accounts.payer.key() });
    Ok(())
}

/// D45: the epoch start counts if its difficulty is at least half of the
/// highest among the epoch starts of the last 4 weeks before the anchor,
/// counted in whole days by the time of their first block.
pub fn counts(es: &EpochStart, table: &DayTable, anchor_time: u32) -> Result<bool> {
    let t = target(es.bits)?;
    let mut lowest = t;
    let last_day = anchor_time as i64 / COMPARE_STEP;
    let first_day = if anchor_time as i64 > MAX_EPOCH_START_AGE {
        (anchor_time as i64 - MAX_EPOCH_START_AGE) / COMPARE_STEP
    } else {
        0
    };
    for day in first_day..=last_day {
        let slot = &table.slots[day as usize % DAY_SLOTS];
        if slot.day as i64 != day || slot.lowest_target == [0u8; 32] {
            continue;
        }
        let other = U256::from_be_bytes(&slot.lowest_target);
        if other.lt(&lowest) {
            lowest = other;
        }
    }
    // Half the difficulty is twice the target.
    Ok(!t.wrapping_sub(&lowest).gt(&lowest))
}

#[derive(Accounts)]
pub struct Jump<'info> {
    #[account(seeds = [CONFIG_SEED], bump = config.bump)]
    pub config: Account<'info, Config>,
    #[account(seeds = [DAYS_SEED], bump = day_table.bump)]
    pub day_table: Account<'info, DayTable>,
    pub epoch_start: Account<'info, EpochStart>,
    /// CHECK: the block's account, at the address its hash, number and epoch
    /// time give; checked and created in `store_node`.
    #[account(mut)]
    pub node: UncheckedAccount<'info>,
    #[account(mut)]
    pub payer: Signer<'info>,
    pub system_program: Program<'info, System>,
}
