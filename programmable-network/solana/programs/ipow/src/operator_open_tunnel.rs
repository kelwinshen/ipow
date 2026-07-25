use anchor_lang::prelude::*;
use sha2::{Digest, Sha256};

use crate::constants::{BPS_DENOM, DIFF_PERIOD, RESERVE_MARGIN_BPS};
use crate::errors::IPoWError;
use crate::state::{Conversion, ConversionStatus, GlobalState, SupportedNetwork};

pub fn handler(
    ctx: Context<OperatorOpenTunnel>,
    bitcoin_amount: u64,
    native_amount: u64,
    network_id: u64,
    dest_address: Pubkey,
    network_address: Vec<u8>,
    duty_window_seconds: i64,
    ipow_receive_program: Vec<u8>,
    locked_anchor_height: u64,
    program_hash: [u8; 32],
) -> Result<()> {
    require!(
        native_amount > 0 && bitcoin_amount > 0,
        IPoWError::ZeroValue
    );
    require!(
        !ipow_receive_program.is_empty() && ipow_receive_program.len() <= 80,
        IPoWError::BadBitcoinProgram
    );

    let global_state = &mut ctx.accounts.global_state;

    let hash = Sha256::digest(&ipow_receive_program);
    require!(
        hash.as_slice() == program_hash,
        IPoWError::InvalidProgramHash
    );
    require!(
        ctx.accounts.used_program_pda.data_is_empty(),
        IPoWError::ProgramAlreadyUsed
    );

    let bump = ctx.bumps.used_program_pda;
    let signer_seeds: &[&[&[u8]]] = &[&[b"used_prog", program_hash.as_ref(), &[bump]]];

    let rent = Rent::get()?;
    let space = 1;
    let lamports = rent.minimum_balance(space);

    anchor_lang::solana_program::program::invoke_signed(
        &anchor_lang::solana_program::system_instruction::create_account(
            &ctx.accounts.operator.key(),
            &ctx.accounts.used_program_pda.key(),
            lamports,
            space as u64,
            ctx.program_id,
        ),
        &[
            ctx.accounts.operator.to_account_info(),
            ctx.accounts.used_program_pda.to_account_info(),
            ctx.accounts.system_program.to_account_info(),
        ],
        signer_seeds,
    )?;

    require!(
        locked_anchor_height >= global_state.min_anchor_height
            && locked_anchor_height <= global_state.global_tip_height,
        IPoWError::InvalidAnchorHeight
    );
    require!(network_id != 0, IPoWError::IncorrectNetwork);
    require!(duty_window_seconds > 0, IPoWError::NeedDutyWindow);

    let net_config = &ctx.accounts.network_config;
    require!(net_config.is_active, IPoWError::Unauthorized);
    let addr_len = network_address.len() as u16;
    require!(
        addr_len >= net_config.min_addr_len && addr_len <= net_config.max_addr_len,
        IPoWError::InvalidAddressLength
    );

    let reserve = native_amount.checked_mul(RESERVE_MARGIN_BPS).unwrap() / BPS_DENOM;

    let current_balance = ctx.accounts.escrow_vault.lamports();
    let locked_funds = global_state
        .total_locked_deposits
        .checked_add(global_state.total_reserved_native)
        .unwrap()
        .checked_add(global_state.total_held_commit_fees)
        .unwrap();

    let rent_minimum = Rent::get()?.minimum_balance(0);
    let total_unavailable = locked_funds.checked_add(rent_minimum).unwrap();

    let available_liquidity = current_balance.saturating_sub(total_unavailable);
    require!(
        available_liquidity >= reserve,
        IPoWError::InsufficientLiquidity
    );

    global_state.total_reserved_native = global_state
        .total_reserved_native
        .checked_add(reserve)
        .unwrap();

    let conversion = &mut ctx.accounts.conversion;
    conversion.tx_id = global_state.next_tx_id;
    conversion.user = dest_address;
    conversion.is_native_to_bitcoin = false;

    conversion.native_amount = native_amount;
    conversion.bitcoin_amount = bitcoin_amount;
    conversion.commit_fee = 0;

    conversion.user_program = vec![];
    conversion.ipow_receive_program = ipow_receive_program;
    conversion.network_id = network_id;
    conversion.network_address = network_address;

    let now = Clock::get()?.unix_timestamp;
    conversion.created_at = now;

    conversion.approved_at = now;
    conversion.operator_duty_expires_at = now.checked_add(duty_window_seconds).unwrap();
    conversion.status = ConversionStatus::Approved;
    conversion.reserved_native = reserve;

    let first_height = locked_anchor_height - (locked_anchor_height % DIFF_PERIOD);
    conversion.window_started = true;
    conversion.window_start_height = locked_anchor_height;
    conversion.epoch_start_height = first_height;

    global_state.active_open_conversions =
        global_state.active_open_conversions.checked_add(1).unwrap();
    global_state.next_tx_id = global_state.next_tx_id.checked_add(1).unwrap();

    Ok(())
}

#[derive(Accounts)]
#[instruction(
    bitcoin_amount: u64,
    native_amount: u64,
    network_id: u64,
    dest_address: Pubkey,
    network_address: Vec<u8>,
    duty_window_seconds: i64,
    ipow_receive_program: Vec<u8>,
    locked_anchor_height: u64,
    program_hash: [u8; 32]
)]
pub struct OperatorOpenTunnel<'info> {
    #[account(mut, seeds = [b"global_state"], bump)]
    pub global_state: Account<'info, GlobalState>,

    /// CHECK: address is fully constrained by the `seeds`/`bump` PDA derivation
    /// above; only ever read (`.lamports()`) for the liquidity-availability check,
    /// never written here.
    #[account(seeds = [b"escrow"], bump = global_state.escrow_bump)]
    pub escrow_vault: UncheckedAccount<'info>,

    #[account(
        init,
        payer = operator,
        space = 8 + Conversion::MAX_SPACE,
        seeds = [b"conversion".as_ref(), global_state.next_tx_id.to_le_bytes().as_ref()],
        bump
    )]
    pub conversion: Account<'info, Conversion>,

    /// CHECK: address is fully constrained by the `seeds`/`bump` PDA derivation
    /// above; the handler explicitly checks `data_is_empty()` before manually
    /// creating this account via CPI, preventing reuse of the same Bitcoin
    /// program hash across conversions.
    #[account(
        mut,
        seeds = [b"used_prog", program_hash.as_ref()],
        bump
    )]
    pub used_program_pda: UncheckedAccount<'info>,

    #[account(seeds = [b"network", network_id.to_le_bytes().as_ref()], bump)]
    pub network_config: Account<'info, SupportedNetwork>,

    #[account(mut, address = global_state.operator @ IPoWError::Unauthorized)]
    pub operator: Signer<'info>,

    pub system_program: Program<'info, System>,
}
