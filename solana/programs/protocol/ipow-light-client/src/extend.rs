use anchor_lang::prelude::*;

use crate::bitcoin::{retarget, Header, U256, EPOCH_BLOCKS, HEADER_LENGTH};
use crate::constants::{CONFIG_SEED, MAX_EXTEND};
use crate::errors::LightClientError;
use crate::state::{Config, Node};
use crate::utils::{check_header, store_node};

/// Streams blocks on top of a stored block. Each block names the one before
/// it and has the difficulty Bitcoin's rules give it (D72). The new blocks'
/// accounts come as the remaining accounts, in order.
pub fn handler<'info>(ctx: Context<'info, Extend<'info>>, headers: Vec<u8>) -> Result<()> {
    require!(
        !headers.is_empty() && headers.len() % HEADER_LENGTH == 0 && headers.len() / HEADER_LENGTH <= MAX_EXTEND,
        LightClientError::InvalidLength
    );
    let count = headers.len() / HEADER_LENGTH;
    require!(ctx.remaining_accounts.len() == count, LightClientError::InvalidLength);

    let now = Clock::get()?.unix_timestamp;
    let config = &ctx.accounts.config;
    let pow_limit = U256::from_be_bytes(&config.pow_limit);
    let parent = &ctx.accounts.parent;
    let payer = ctx.accounts.payer.to_account_info();
    let system = ctx.accounts.system_program.to_account_info();

    let mut parent_hash = parent.hash;
    let mut parent_bits = parent.bits;
    let mut parent_time = parent.time;
    let mut height = parent.height;
    let mut epoch_time = parent.epoch_time;

    for i in 0..count {
        let header = Header(&headers[i * HEADER_LENGTH..(i + 1) * HEADER_LENGTH]);
        require!(header.prev() == parent_hash, LightClientError::NotLinked);

        height = height.checked_add(1).ok_or(LightClientError::InvalidHeight)?;
        let mut expected = parent_bits;
        if height % EPOCH_BLOCKS == 0 {
            // D72: the first block of a new epoch.
            expected = retarget(parent_bits, epoch_time, parent_time, &pow_limit)
                .map_err(|_| error!(LightClientError::InvalidBits))?;
            epoch_time = header.time();
        }
        require!(header.bits() == expected, LightClientError::DifficultyMismatch);

        let hash = check_header(&header, config, now)?;
        store_node(
            ctx.program_id,
            &ctx.remaining_accounts[i],
            &payer,
            &system,
            &header,
            hash,
            height,
            epoch_time,
            now,
        )?;
        parent_hash = hash;
        parent_bits = expected;
        parent_time = header.time();
    }
    Ok(())
}

#[derive(Accounts)]
pub struct Extend<'info> {
    #[account(seeds = [CONFIG_SEED], bump = config.bump)]
    pub config: Account<'info, Config>,
    /// The stored block the first header names.
    pub parent: Account<'info, Node>,
    #[account(mut)]
    pub payer: Signer<'info>,
    pub system_program: Program<'info, System>,
}
