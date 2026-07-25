use anchor_lang::prelude::*;

use crate::constants::BPS_DENOM;
use crate::errors::IPoWError;
use crate::state::{Conversion, ConversionStatus, GlobalState, SupportedNetwork};

pub fn handler(
    ctx: Context<CommitNativeToBitcoin>,
    native_amount: u64,
    bitcoin_amount: u64,
    network_id: u64,
    network_address: Vec<u8>,
    user_program: Vec<u8>,
) -> Result<()> {
    require!(
        native_amount > 0 && bitcoin_amount > 0,
        IPoWError::ZeroValue
    );

    if network_id == 0 {
        require!(
            !user_program.is_empty() && user_program.len() <= 80,
            IPoWError::BadBitcoinProgram
        );
        require!(
            network_address.is_empty(),
            IPoWError::NetworkAddressNotAllowed
        );
    } else {
        require!(
            user_program.is_empty(),
            IPoWError::UserBitcoinProgramNotAllowed
        );

        let net_config = ctx
            .accounts
            .network_config
            .as_ref()
            .ok_or(IPoWError::IncorrectNetwork)?;
        require!(net_config.is_active, IPoWError::Unauthorized);

        let addr_len = network_address.len() as u16;
        require!(
            addr_len >= net_config.min_addr_len && addr_len <= net_config.max_addr_len,
            IPoWError::InvalidAddressLength
        );
    }

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
    conversion.is_native_to_bitcoin = true;

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
#[instruction(native_amount: u64, bitcoin_amount: u64, network_id: u64)]
pub struct CommitNativeToBitcoin<'info> {
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

    pub network_config: Option<Account<'info, SupportedNetwork>>,

    #[account(mut)]
    pub user: Signer<'info>,
    pub system_program: Program<'info, System>,
}
