use anchor_lang::prelude::*;

use crate::constants::BPS_DENOM;
use crate::errors::ConversionError;
use crate::state::{Conversion, ConversionGlobalState, ConversionStatus, Pool, TokenAmount};

/// Renamed from `commit_bitcoin_to_native` — "native" only ever meant
/// "the default case" (`token_mint == Pubkey::default()`); this already
/// accepted any SPL mint. Direct-Bitcoin only (`network_id == 0`).
///
/// Also accepts an optional bundle: up to 3 `extra_tokens` beyond the
/// primary `token_mint`/`native_amount` slot. A single-token `bitcoin->
/// token` conversion (`extra_tokens` empty) keeps the existing auction
/// shape unchanged — `native_amount` is what claimants compete on
/// (higher wins), `bitcoin_amount` is the fixed ask. A bundle flips that:
/// comparing "claimant A offers 100 USDC + 5 SOL" against "claimant B
/// offers 90 USDC + 6 SOL" has no natural ordering without a price
/// oracle, so once a bundle is present, the *token* side (this whole
/// wishlist) becomes the fixed ask instead, and claimants compete on
/// `bitcoin_amount` — lower wins, since they're underbidding how little
/// BTC the user needs to pay for the same fixed bundle. See
/// `propose_claim_conversion`'s own doc comment for the auction-side
/// logic this drives.
///
/// No `required_bond`/stake for this direction: the claimant self-
/// escrows real collateral in `propose_claim_conversion` before any
/// proof exists, and that collateral's destination is identical whether
/// they complete the duty or abandon it (always ends up with the user,
/// via proof or force-claim) — a separate stake would be redundant with
/// that already-real exposure. `token->bitcoin` is different (the
/// claimant never self-escrows anything on-chain), so it still requires
/// one — see `commit_token_to_bitcoin`.
pub fn handler(
    ctx: Context<CommitBitcoinToToken>,
    bitcoin_amount: u64,
    native_amount: u64,
    network_id: u64,
    network_address: Vec<u8>,
    user_program: Vec<u8>,
    token_mint: Pubkey,
    extra_tokens: Vec<TokenAmount>,
) -> Result<()> {
    require!(
        native_amount > 0 && bitcoin_amount > 0,
        ConversionError::ZeroValue
    );

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

    require!(network_id == 0, ConversionError::IncorrectNetwork);
    require!(
        network_address.is_empty(),
        ConversionError::NetworkAddressNotAllowed
    );
    require!(
        !user_program.is_empty() && user_program.len() <= 80,
        ConversionError::BadBitcoinProgram
    );

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
    conversion.is_native_to_bitcoin = false;
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
    conversion.claim_started_at = conversion.created_at;
    conversion.last_claim_at = conversion.created_at;

    gs.next_tx_id = gs.next_tx_id.checked_add(1).unwrap();

    Ok(())
}

#[derive(Accounts)]
#[instruction(bitcoin_amount: u64, native_amount: u64, network_id: u64)]
pub struct CommitBitcoinToToken<'info> {
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

    #[account(mut)]
    pub user: Signer<'info>,
    pub system_program: Program<'info, System>,
}
