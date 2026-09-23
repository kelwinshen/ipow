use anchor_lang::prelude::*;

use crate::constants::BPS_DENOM;
use crate::errors::ConversionError;
use crate::state::{Conversion, ConversionGlobalState, ConversionStatus, Pool, SupportedNetwork, TokenAmount};

/// Renamed from `commit_native_to_bitcoin` — "native" only ever meant
/// "the default case" (`token_mint == Pubkey::default()`); this already
/// accepted any SPL mint. Also accepts an optional bundle: up to 3
/// `extra_tokens` beyond the primary `token_mint`/`native_amount` slot,
/// all locked by this same conversion and settled by the same single
/// `bitcoin_amount` proof. Every existing single-token caller is
/// unaffected — pass an empty `extra_tokens` and behavior is identical
/// to before the rename.
#[allow(clippy::too_many_arguments)]
pub fn handler(
    ctx: Context<CommitTokenToBitcoin>,
    native_amount: u64,
    bitcoin_amount: u64,
    network_id: u64,
    network_address: Vec<u8>,
    user_program: Vec<u8>,
    required_bond: u64,
    token_mint: Pubkey,
    extra_tokens: Vec<TokenAmount>,
) -> Result<()> {
    require!(
        native_amount > 0 && bitcoin_amount > 0,
        ConversionError::ZeroValue
    );
    // The user, not an app or a fixed operator, decides how much stake must
    // back their own conversion — same rationale `ipow-message-relay`'s
    // `required_bond` gives, applied to whoever actually bears the risk here.
    require!(required_bond > 0, ConversionError::ZeroValue);

    require!(extra_tokens.len() <= 3, ConversionError::TooManyTokens);
    for extra in &extra_tokens {
        require!(extra.amount > 0, ConversionError::ZeroValue);
        require!(extra.mint != Pubkey::default(), ConversionError::InvalidTokenMint);
        require!(extra.mint != token_mint, ConversionError::DuplicateTokenMint);
    }
    for i in 0..extra_tokens.len() {
        for j in (i + 1)..extra_tokens.len() {
            require!(
                extra_tokens[i].mint != extra_tokens[j].mint,
                ConversionError::DuplicateTokenMint
            );
        }
    }

    if network_id == 0 {
        require!(
            !user_program.is_empty() && user_program.len() <= 80,
            ConversionError::BadBitcoinProgram
        );
        require!(
            network_address.is_empty(),
            ConversionError::NetworkAddressNotAllowed
        );
    } else {
        require!(
            user_program.is_empty(),
            ConversionError::UserBitcoinProgramNotAllowed
        );

        let net_config = ctx
            .accounts
            .network_config
            .as_ref()
            .ok_or(ConversionError::IncorrectNetwork)?;
        require!(net_config.is_active, ConversionError::Unauthorized);

        let addr_len = network_address.len() as u16;
        require!(
            addr_len >= net_config.min_addr_len && addr_len <= net_config.max_addr_len,
            ConversionError::InvalidAddressLength
        );
    }

    let gs = &mut ctx.accounts.conversion_global_state;

    let calculated_fee = native_amount
        .checked_mul(gs.commit_fee_bps as u64)
        .unwrap()
        .checked_div(BPS_DENOM)
        .unwrap();
    let min_fee = Rent::get()?.minimum_balance(0);
    let required_fee = calculated_fee.max(min_fee);

    anchor_lang::solana_program::program::invoke(
        &anchor_lang::solana_program::system_instruction::transfer(
            &ctx.accounts.user.key(),
            &ctx.accounts.fee_escrow.key(),
            required_fee,
        ),
        &[
            ctx.accounts.user.to_account_info(),
            ctx.accounts.fee_escrow.to_account_info(),
            ctx.accounts.system_program.to_account_info(),
        ],
    )?;
    let fee_pool = &mut ctx.accounts.fee_pool;
    fee_pool.token_mint = Pubkey::default();
    fee_pool.escrow_bump = ctx.bumps.fee_escrow;
    fee_pool.total_held_commit_fees =
        fee_pool.total_held_commit_fees.checked_add(required_fee).unwrap();

    let conversion = &mut ctx.accounts.conversion;
    conversion.tx_id = gs.next_tx_id;
    conversion.user = ctx.accounts.user.key();
    conversion.is_native_to_bitcoin = true;
    conversion.token_mint = token_mint;

    conversion.native_amount = native_amount;
    conversion.bitcoin_amount = bitcoin_amount;
    conversion.commit_fee = required_fee;
    conversion.extra_tokens = extra_tokens;

    conversion.user_program = user_program;
    conversion.network_id = network_id;
    conversion.network_address = network_address;

    conversion.created_at = Clock::get()?.unix_timestamp;
    conversion.status = ConversionStatus::Committed;
    conversion.required_bond = required_bond;
    conversion.claim_started_at = conversion.created_at;
    conversion.last_claim_at = conversion.created_at;

    gs.next_tx_id = gs.next_tx_id.checked_add(1).unwrap();

    Ok(())
}

#[derive(Accounts)]
#[instruction(native_amount: u64, bitcoin_amount: u64, network_id: u64)]
pub struct CommitTokenToBitcoin<'info> {
    #[account(mut, seeds = [b"conversion_global_state"], bump)]
    pub conversion_global_state: Account<'info, ConversionGlobalState>,

    #[account(
        init_if_needed,
        payer = user,
        space = 8 + Pool::INIT_SPACE,
        seeds = [b"pool", Pubkey::default().as_ref()],
        bump
    )]
    pub fee_pool: Account<'info, Pool>,

    #[account(mut, seeds = [b"escrow", Pubkey::default().as_ref()], bump)]
    pub fee_escrow: SystemAccount<'info>,

    #[account(
        init,
        payer = user,
        space = 8 + Conversion::INIT_SPACE,
        seeds = [b"conversion".as_ref(), conversion_global_state.next_tx_id.to_le_bytes().as_ref()],
        bump
    )]
    pub conversion: Account<'info, Conversion>,

    pub network_config: Option<Account<'info, SupportedNetwork>>,

    #[account(mut)]
    pub user: Signer<'info>,
    pub system_program: Program<'info, System>,
}
