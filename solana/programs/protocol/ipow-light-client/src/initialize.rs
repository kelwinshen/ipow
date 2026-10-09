use anchor_lang::prelude::*;

use crate::constants::{CONFIG_SEED, DAYS_SEED, MIN_DIFFICULTY_TARGET, POW_LIMIT};
use crate::errors::LightClientError;
use crate::state::{Config, DayTable};

/// The loader that holds upgradeable programs.
const UPGRADEABLE_LOADER: Pubkey = pubkey!("BPFLoaderUpgradeab1e11111111111111111111111");

/// Sets the lowest allowed block number (D93), once. Only the program's
/// upgrade authority may call it, so nobody else can set it first. After
/// that the authority is removed and nothing can change (D59).
pub fn handler(
    ctx: Context<Initialize>,
    min_height: u32,
    max_target: [u8; 32],
    pow_limit: [u8; 32],
) -> Result<()> {
    let config = &mut ctx.accounts.config;
    config.min_height = min_height;
    config.bump = ctx.bumps.config;
    ctx.accounts.day_table.bump = ctx.bumps.day_table;

    #[cfg(feature = "test-limits")]
    {
        config.max_target = max_target;
        config.pow_limit = pow_limit;
    }
    #[cfg(not(feature = "test-limits"))]
    {
        let _ = (max_target, pow_limit);
        config.max_target = MIN_DIFFICULTY_TARGET;
        config.pow_limit = POW_LIMIT;
        require_upgrade_authority(
            ctx.program_id,
            &ctx.accounts.program_data,
            &ctx.accounts.authority.key(),
        )?;
    }
    let _ = (MIN_DIFFICULTY_TARGET, POW_LIMIT);
    Ok(())
}

#[cfg_attr(feature = "test-limits", allow(dead_code))]
fn require_upgrade_authority(program_id: &Pubkey, program_data: &AccountInfo, authority: &Pubkey) -> Result<()> {
    let (expected, _) = Pubkey::find_program_address(&[program_id.as_ref()], &UPGRADEABLE_LOADER);
    require_keys_eq!(program_data.key(), expected, LightClientError::NotAuthority);
    require_keys_eq!(*program_data.owner, UPGRADEABLE_LOADER, LightClientError::NotAuthority);
    let data = program_data.try_borrow_data()?;
    // ProgramData: a 4-byte tag (3), an 8-byte slot, then an optional key.
    require!(data.len() >= 45, LightClientError::NotAuthority);
    require!(data[0..4] == [3, 0, 0, 0], LightClientError::NotAuthority);
    require!(data[12] == 1, LightClientError::NotAuthority);
    require!(data[13..45] == authority.to_bytes(), LightClientError::NotAuthority);
    Ok(())
}

#[derive(Accounts)]
pub struct Initialize<'info> {
    #[account(init, payer = authority, space = 8 + Config::INIT_SPACE, seeds = [CONFIG_SEED], bump)]
    pub config: Account<'info, Config>,
    #[account(init, payer = authority, space = 8 + DayTable::INIT_SPACE, seeds = [DAYS_SEED], bump)]
    pub day_table: Account<'info, DayTable>,
    #[account(mut)]
    pub authority: Signer<'info>,
    /// CHECK: the program data account of this program, checked in the
    /// handler against the upgradeable loader's layout.
    pub program_data: UncheckedAccount<'info>,
    pub system_program: Program<'info, System>,
}
