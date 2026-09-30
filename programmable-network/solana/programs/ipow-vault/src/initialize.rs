use anchor_lang::prelude::*;
use anchor_spl::token::{Mint, Token, TokenAccount};

use crate::constants::*;
use crate::errors::VaultError;
use crate::state::Config;

/// The loader that holds upgradeable programs.
const UPGRADEABLE_LOADER: Pubkey = pubkey!("BPFLoaderUpgradeab1e11111111111111111111111");

/// Sets what is fixed at deployment, once: the vault on Ethereum, the flat
/// deposit (V1) and the least certifying escrow (D118), as Ethereum's
/// constructor does. Only the program's upgrade authority may call it, so
/// nobody else can set them first; the authority is then removed and
/// nothing can change (D59). Creates vETH and the vault's vETH account, and
/// registers the vault with the protocol as an application with no claims,
/// so that it can open checkpoint jobs (D116).
pub fn handler(ctx: Context<Initialize>, ethereum_vault: [u8; 20], deposit: u64, min_certifying_escrow: u64) -> Result<()> {
    require!(ethereum_vault != [0u8; 20] && deposit > 0 && min_certifying_escrow > 0, VaultError::ZeroAmount);
    #[cfg(not(feature = "test-limits"))]
    require_upgrade_authority(ctx.program_id, &ctx.accounts.program_data, &ctx.accounts.payer.key())?;
    let c = &mut ctx.accounts.config;
    c.ethereum_vault = ethereum_vault;
    c.deposit = deposit;
    c.min_certifying_escrow = min_certifying_escrow;
    c.bump = ctx.bumps.config;
    let seeds: &[&[u8]] = &[CONFIG_SEED, &[ctx.bumps.config]];
    ipow_protocol::cpi::register_application(
        CpiContext::new_with_signer(
            ipow_protocol::ID,
            ipow_protocol::cpi::accounts::RegisterApplication {
                application: ctx.accounts.application.to_account_info(),
                key: ctx.accounts.config.to_account_info(),
                funder: ctx.accounts.payer.to_account_info(),
                system_program: ctx.accounts.system_program.to_account_info(),
            },
            &[seeds],
        ),
        vec![],
    )
}

#[cfg_attr(feature = "test-limits", allow(dead_code))]
fn require_upgrade_authority(program_id: &Pubkey, program_data: &AccountInfo, authority: &Pubkey) -> Result<()> {
    let (expected, _) = Pubkey::find_program_address(&[program_id.as_ref()], &UPGRADEABLE_LOADER);
    require_keys_eq!(program_data.key(), expected, VaultError::NotAuthority);
    require_keys_eq!(*program_data.owner, UPGRADEABLE_LOADER, VaultError::NotAuthority);
    let data = program_data.try_borrow_data()?;
    // ProgramData: a 4-byte tag (3), an 8-byte slot, then an optional key.
    require!(data.len() >= 45, VaultError::NotAuthority);
    require!(data[0..4] == [3, 0, 0, 0], VaultError::NotAuthority);
    require!(data[12] == 1, VaultError::NotAuthority);
    require!(data[13..45] == authority.to_bytes(), VaultError::NotAuthority);
    Ok(())
}

#[derive(Accounts)]
pub struct Initialize<'info> {
    #[account(init, payer = payer, space = 8 + Config::INIT_SPACE, seeds = [CONFIG_SEED], bump)]
    pub config: Account<'info, Config>,
    #[account(init, payer = payer, seeds = [MINT_SEED], bump, mint::decimals = VETH_DECIMALS, mint::authority = config)]
    pub mint: Account<'info, Mint>,
    #[account(init, payer = payer, seeds = [HOLDING_SEED], bump, token::mint = mint, token::authority = config)]
    pub holding: Account<'info, TokenAccount>,
    /// CHECK: the application's record in the protocol, created by it.
    #[account(mut)]
    pub application: UncheckedAccount<'info>,
    /// The program's upgrade authority.
    #[account(mut)]
    pub payer: Signer<'info>,
    /// CHECK: the program data account of this program, checked in the
    /// handler against the upgradeable loader's layout.
    pub program_data: UncheckedAccount<'info>,
    /// CHECK: the protocol program.
    #[account(address = ipow_protocol::ID)]
    pub protocol_program: UncheckedAccount<'info>,
    pub token_program: Program<'info, Token>,
    pub system_program: Program<'info, System>,
}
