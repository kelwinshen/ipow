use anchor_lang::prelude::*;

use crate::constants::BPS_DENOM;
use crate::errors::IPoWError;
use crate::state::{Conversion, ConversionStatus, GlobalState};

pub fn handler(
    ctx: Context<CommitBitcoinToNative>,
    bitcoin_amount: u64,
    native_amount: u64,
    network_id: u64,
    network_address: Vec<u8>,
    user_program: Vec<u8>,
) -> Result<()> {
    require!(
        native_amount > 0 && bitcoin_amount > 0,
        IPoWError::ZeroValue
    );

    require!(network_id == 0, IPoWError::NetworkNotAllowed);
    require!(
        network_address.is_empty(),
        IPoWError::NetworkAddressNotAllowed
    );
    require!(
        !user_program.is_empty() && user_program.len() <= 80,
        IPoWError::BadBitcoinProgram
    );

    let global_state = &mut ctx.accounts.global_state;

    let calculated_fee = native_amount
        .checked_mul(global_state.commit_fee_bps as u64)
        .unwrap()
        .checked_div(BPS_DENOM)
        .unwrap();

    let min_fee = Rent::get()?.minimum_balance(0);
    let required_fee = calculated_fee.max(min_fee);

    anchor_lang::solana_program::program::invoke(
        &anchor_lang::solana_program::system_instruction::transfer(
            &ctx.accounts.user.key(),
            &ctx.accounts.escrow_vault.key(),
            required_fee,
        ),
        &[
            ctx.accounts.user.to_account_info(),
            ctx.accounts.escrow_vault.to_account_info(),
            ctx.accounts.system_program.to_account_info(),
        ],
    )?;

    let conversion = &mut ctx.accounts.conversion;
    conversion.tx_id = global_state.next_tx_id;
    conversion.user = ctx.accounts.user.key();
    conversion.is_native_to_bitcoin = false;

    conversion.native_amount = native_amount;
    conversion.bitcoin_amount = bitcoin_amount;
    conversion.commit_fee = required_fee;

    conversion.user_program = user_program;
    conversion.network_id = network_id;
    conversion.network_address = network_address;

    conversion.created_at = Clock::get()?.unix_timestamp;
    conversion.status = ConversionStatus::Committed;

    global_state.total_held_commit_fees = global_state
        .total_held_commit_fees
        .checked_add(required_fee)
        .unwrap();
    global_state.next_tx_id = global_state.next_tx_id.checked_add(1).unwrap();

    Ok(())
}

#[derive(Accounts)]
#[instruction(bitcoin_amount: u64, native_amount: u64, network_id: u64)]
pub struct CommitBitcoinToNative<'info> {
    #[account(mut, seeds = [b"global_state"], bump)]
    pub global_state: Account<'info, GlobalState>,

    #[account(mut, seeds = [b"escrow"], bump = global_state.escrow_bump)]
    pub escrow_vault: SystemAccount<'info>,

    #[account(
        init,
        payer = user,
        space = 8 + Conversion::MAX_SPACE,
        seeds = [b"conversion".as_ref(), global_state.next_tx_id.to_le_bytes().as_ref()],
        bump
    )]
    pub conversion: Account<'info, Conversion>,

    #[account(mut)]
    pub user: Signer<'info>,
    pub system_program: Program<'info, System>,
}
