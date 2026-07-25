use anchor_lang::prelude::*;
use sha2::{Digest, Sha256};

use crate::constants::{APPROVAL_WINDOW_SEC, BPS_DENOM, DIFF_PERIOD, RESERVE_MARGIN_BPS};
use crate::errors::IPoWError;
use crate::state::{Conversion, ConversionStatus, GlobalState};

pub fn handler(
    ctx: Context<ApproveConversion>,
    duty_window_seconds: i64,
    ipow_receive_program: Vec<u8>,
    program_hash: [u8; 32],
) -> Result<()> {
    let conversion = &mut ctx.accounts.conversion;
    let global_state = &mut ctx.accounts.global_state;

    if !ipow_receive_program.is_empty() {
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
    }

    require!(
        conversion.status == ConversionStatus::Committed,
        IPoWError::BadState
    );
    require!(duty_window_seconds > 0, IPoWError::NeedDutyWindow);

    let now = Clock::get()?.unix_timestamp;
    require!(
        now <= conversion.created_at + APPROVAL_WINDOW_SEC,
        IPoWError::ApproveWindowOver
    );

    conversion.status = ConversionStatus::Approved;
    conversion.approved_at = now;
    conversion.operator_duty_expires_at = now.checked_add(duty_window_seconds).unwrap();

    if !conversion.is_native_to_bitcoin {
        let reserve = conversion
            .native_amount
            .checked_mul(RESERVE_MARGIN_BPS)
            .unwrap()
            / BPS_DENOM;
        conversion.reserved_native = reserve;
        global_state.total_reserved_native = global_state
            .total_reserved_native
            .checked_add(reserve)
            .unwrap();

        require!(
            !ipow_receive_program.is_empty() && ipow_receive_program.len() <= 80,
            IPoWError::BadBitcoinProgram
        );
        conversion.ipow_receive_program = ipow_receive_program.clone();
    } else if conversion.is_native_to_bitcoin && conversion.user_program.is_empty() {
        require!(
            !ipow_receive_program.is_empty() && ipow_receive_program.len() <= 80,
            IPoWError::BadBitcoinProgram
        );
        conversion.user_program = ipow_receive_program;
    }

    let first_height =
        global_state.global_tip_height - (global_state.global_tip_height % DIFF_PERIOD);
    conversion.window_started = true;
    conversion.window_start_height = global_state.global_tip_height;
    conversion.epoch_start_height = first_height;

    global_state.active_open_conversions =
        global_state.active_open_conversions.checked_add(1).unwrap();

    Ok(())
}

#[derive(Accounts)]
#[instruction(duty_window_seconds: i64, ipow_receive_program: Vec<u8>, program_hash: [u8; 32])]
pub struct ApproveConversion<'info> {
    #[account(mut, seeds = [b"global_state"], bump)]
    pub global_state: Account<'info, GlobalState>,

    /// CHECK: address is fully constrained by the `seeds`/`bump` PDA derivation
    /// above; not read or written by this instruction's handler.
    #[account(seeds = [b"escrow"], bump = global_state.escrow_bump)]
    pub escrow_vault: UncheckedAccount<'info>,

    #[account(mut)]
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

    #[account(mut, address = global_state.operator @ IPoWError::Unauthorized)]
    pub operator: Signer<'info>,

    pub system_program: Program<'info, System>,
}
