use anchor_lang::prelude::*;

use crate::bitcoin::{retarget, Header, U256, EPOCH_BLOCKS};
use crate::constants::CONFIG_SEED;
use crate::errors::LightClientError;
use crate::state::{Config, Node};
use crate::utils::{check_header, store_node};

/// Stores the parent of a stored block: the block whose hash the stored
/// block names as the one before it. Anyone may call (D64). This is how the
/// parent of an anchor is shown when a guardian asks for it (D81).
/// `prev_epoch_time` is only used when the stored block is the first of its
/// epoch: the epoch time of the epoch before. Zero otherwise.
pub fn handler(ctx: Context<ExtendBack>, header: [u8; 80], prev_epoch_time: u32) -> Result<()> {
    let child = &ctx.accounts.child;
    let config = &ctx.accounts.config;
    require!(child.height > 0, LightClientError::InvalidHeight);
    require!(child.height - 1 >= config.min_height, LightClientError::BelowMinHeight);

    let now = Clock::get()?.unix_timestamp;
    let h = Header(&header);
    let hash = check_header(&h, config, now)?;
    require!(hash == child.prev_hash, LightClientError::NotLinked);

    let mut epoch_time = child.epoch_time;
    if child.height % EPOCH_BLOCKS == 0 {
        // D72, read backward: the stored block is the first of its epoch.
        epoch_time = prev_epoch_time;
        let expected = retarget(h.bits(), prev_epoch_time, h.time(), &U256::from_be_bytes(&config.pow_limit))
            .map_err(|_| error!(LightClientError::InvalidBits))?;
        require!(expected == child.bits, LightClientError::DifficultyMismatch);
    } else {
        require!(h.bits() == child.bits, LightClientError::DifficultyMismatch);
    }
    // The first block of an epoch gives the epoch its time.
    require!(
        (child.height - 1) % EPOCH_BLOCKS != 0 || epoch_time == h.time(),
        LightClientError::EpochTimeMismatch
    );

    store_node(
        ctx.program_id,
        &ctx.accounts.parent.to_account_info(),
        &ctx.accounts.payer.to_account_info(),
        &ctx.accounts.system_program.to_account_info(),
        &h,
        hash,
        child.height - 1,
        epoch_time,
        now,
    )?;
    Ok(())
}

#[derive(Accounts)]
pub struct ExtendBack<'info> {
    #[account(seeds = [CONFIG_SEED], bump = config.bump)]
    pub config: Account<'info, Config>,
    pub child: Account<'info, Node>,
    /// CHECK: the parent's account, at the address its hash, number and
    /// epoch time give; checked and created in `store_node`.
    #[account(mut)]
    pub parent: UncheckedAccount<'info>,
    #[account(mut)]
    pub payer: Signer<'info>,
    pub system_program: Program<'info, System>,
}
