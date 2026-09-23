use anchor_lang::prelude::*;
use ipow::state::GlobalState as IpowGlobalState;
use sha2::{Digest, Sha256};

use crate::constants::DIFF_PERIOD;
use crate::errors::ConversionError;
use crate::spl_accounts::{SplAccounts, SplAccountsBumps, __client_accounts_spl_accounts, __cpi_client_accounts_spl_accounts};
use crate::state::{Conversion, ConversionGlobalState, ConversionStatus, Pool, SupportedNetwork, TokenAmount};
use crate::utils::transfer_value_in;

/// Permissionless, atomic "open + fully fund" path for `bitcoin->token`
/// conversions that carry a multi-token bundle — see `docs/DESIGN_V2.md`
/// §2. Collapses what would otherwise be `commit_bitcoin_to_token` +
/// `propose_claim_conversion` + `finalize_claim_conversion` into one
/// instruction: the opener self-funds the *entire* fixed bundle (primary +
/// up to 3 extras) right here, becomes `responsible_operator`, and the
/// conversion is immediately `Approved`/`window_started`.
///
/// There is deliberately no auction: `commit_bitcoin_to_token` already
/// covers the single-token, rate-competed case, and a bundle can't be
/// rate-competed at all without a price oracle (comparing "claimant A
/// offers 100 USDC + 5 SOL" against "claimant B offers 90 USDC + 6 SOL"
/// has no natural ordering). Restricted to `network_id != 0` so this stays
/// purely additive: a direct-Bitcoin, single-token `bitcoin->token`
/// conversion already has a perfectly good auctioned path
/// (`commit_bitcoin_to_token`) — this isn't a backdoor around it.
#[allow(clippy::too_many_arguments)]
pub fn handler<'info>(
    ctx: Context<'info, OpenBundleTunnel<'info>>,
    native_amount: u64,
    bitcoin_amount: u64,
    token_mint: Pubkey,
    extra_tokens: Vec<TokenAmount>,
    dest_address: Pubkey,
    network_id: u64,
    network_address: Vec<u8>,
    duty_window_seconds: i64,
    ipow_receive_program: Vec<u8>,
    program_hash: [u8; 32],
) -> Result<()> {
    require!(
        native_amount > 0 && bitcoin_amount > 0,
        ConversionError::ZeroValue
    );
    require!(duty_window_seconds > 0, ConversionError::NeedDutyWindow);

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

    require!(
        !ipow_receive_program.is_empty() && ipow_receive_program.len() <= 80,
        ConversionError::BadBitcoinProgram
    );
    let hash = Sha256::digest(&ipow_receive_program);
    require!(
        hash.as_slice() == program_hash,
        ConversionError::InvalidProgramHash
    );
    require!(
        ctx.accounts.used_program_pda.data_is_empty(),
        ConversionError::ProgramAlreadyUsed
    );
    let used_prog_bump = ctx.bumps.used_program_pda;
    let used_prog_signer_seeds: &[&[&[u8]]] =
        &[&[b"used_prog", program_hash.as_ref(), &[used_prog_bump]]];
    let rent = Rent::get()?;
    let lamports = rent.minimum_balance(1);
    anchor_lang::solana_program::program::invoke_signed(
        &anchor_lang::solana_program::system_instruction::create_account(
            &ctx.accounts.opener.key(),
            &ctx.accounts.used_program_pda.key(),
            lamports,
            1,
            ctx.program_id,
        ),
        &[
            ctx.accounts.opener.to_account_info(),
            ctx.accounts.used_program_pda.to_account_info(),
            ctx.accounts.system_program.to_account_info(),
        ],
        used_prog_signer_seeds,
    )?;

    // Tunnel-only — see the handler doc comment.
    require!(network_id != 0, ConversionError::IncorrectNetwork);
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

    // Self-escrow the primary slot — same call `propose_claim_conversion`
    // makes for its own bitcoin->token self-escrow.
    let pool = &mut ctx.accounts.pool;
    pool.token_mint = token_mint;
    pool.escrow_bump = ctx.bumps.escrow_vault;

    let received = transfer_value_in(
        token_mint,
        native_amount,
        &ctx.accounts.opener.to_account_info(),
        &ctx.accounts.opener_token_account.to_account_info(),
        &ctx.accounts.escrow_vault.to_account_info(),
        &ctx.accounts.escrow_ata.to_account_info(),
        &ctx.accounts.escrow_vault.to_account_info(),
        &ctx.accounts.opener.to_account_info(),
        &ctx.accounts.system_program.to_account_info(),
        &ctx.accounts.spl,
    )?;
    require!(received == native_amount, ConversionError::DepositShortfall);
    pool.total_reserved = pool.total_reserved.checked_add(received).unwrap();

    // Self-escrow the bundle — same `remaining_accounts` shape and manual
    // PDA re-validation as `deposit_conversion.rs`'s own loop, just
    // self-funded by the opener instead of the user.
    const ACCOUNTS_PER_EXTRA: usize = 5;
    require!(
        ctx.remaining_accounts.len() == extra_tokens.len() * ACCOUNTS_PER_EXTRA,
        ConversionError::InvalidRemainingAccount
    );
    let opener_info = ctx.accounts.opener.to_account_info();
    let system_program_info = ctx.accounts.system_program.to_account_info();
    for (i, extra) in extra_tokens.iter().enumerate() {
        let base = i * ACCOUNTS_PER_EXTRA;
        let mint: &AccountInfo = &ctx.remaining_accounts[base];
        let token_program: &AccountInfo = &ctx.remaining_accounts[base + 1];
        let escrow_vault: &AccountInfo = &ctx.remaining_accounts[base + 2];
        let escrow_ata: &AccountInfo = &ctx.remaining_accounts[base + 3];
        let opener_extra_token_account: &AccountInfo = &ctx.remaining_accounts[base + 4];

        require_keys_eq!(mint.key(), extra.mint, ConversionError::InvalidRemainingAccount);
        let (expected_escrow_vault, _) =
            Pubkey::find_program_address(&[b"escrow", extra.mint.as_ref()], ctx.program_id);
        require_keys_eq!(
            escrow_vault.key(),
            expected_escrow_vault,
            ConversionError::InvalidRemainingAccount
        );

        let extra_spl = SplAccounts {
            mint: UncheckedAccount::try_from(mint),
            token_program: UncheckedAccount::try_from(token_program),
            associated_token_program: ctx.accounts.spl.associated_token_program.clone(),
        };
        let received_extra = transfer_value_in(
            extra.mint,
            extra.amount,
            &opener_info,
            opener_extra_token_account,
            escrow_vault,
            escrow_ata,
            escrow_vault,
            &opener_info,
            &system_program_info,
            &extra_spl,
        )?;
        require!(received_extra == extra.amount, ConversionError::DepositShortfall);
    }

    let now = Clock::get()?.unix_timestamp;
    let tip = ctx.accounts.ipow_global_state.global_tip_height;

    let gs = &mut ctx.accounts.conversion_global_state;
    let conversion = &mut ctx.accounts.conversion;
    conversion.tx_id = gs.next_tx_id;
    conversion.user = dest_address;
    conversion.is_native_to_bitcoin = false;
    conversion.token_mint = token_mint;

    conversion.native_amount = native_amount;
    conversion.bitcoin_amount = bitcoin_amount;
    conversion.commit_fee = 0;
    conversion.reserved_native = native_amount;
    conversion.extra_tokens = extra_tokens;

    conversion.user_program = vec![];
    conversion.ipow_receive_program = ipow_receive_program;
    conversion.network_id = network_id;
    conversion.network_address = network_address;

    conversion.created_at = now;
    conversion.status = ConversionStatus::Approved;
    conversion.required_bond = 0;
    conversion.staked_bond = 0;
    conversion.claim_started_at = now;
    conversion.last_claim_at = now;
    conversion.duty_window_seconds = duty_window_seconds;
    conversion.operator_duty_expires_at = now.checked_add(duty_window_seconds).unwrap();
    conversion.responsible_operator = ctx.accounts.opener.key();

    conversion.window_started = true;
    conversion.window_start_height = tip;
    conversion.epoch_start_height = tip - (tip % DIFF_PERIOD);

    gs.next_tx_id = gs.next_tx_id.checked_add(1).unwrap();

    Ok(())
}

#[derive(Accounts)]
#[instruction(
    native_amount: u64,
    bitcoin_amount: u64,
    token_mint: Pubkey,
    extra_tokens: Vec<TokenAmount>,
    dest_address: Pubkey,
    network_id: u64,
    network_address: Vec<u8>,
    duty_window_seconds: i64,
    ipow_receive_program: Vec<u8>,
    program_hash: [u8; 32]
)]
pub struct OpenBundleTunnel<'info> {
    #[account(mut, seeds = [b"conversion_global_state"], bump)]
    pub conversion_global_state: Account<'info, ConversionGlobalState>,

    #[account(
        init,
        payer = opener,
        space = 8 + Conversion::INIT_SPACE,
        seeds = [b"conversion".as_ref(), conversion_global_state.next_tx_id.to_le_bytes().as_ref()],
        bump
    )]
    pub conversion: Account<'info, Conversion>,

    pub network_config: Option<Account<'info, SupportedNetwork>>,

    /// CHECK: pure existence marker, same pattern as `propose_claim_
    /// conversion`'s `used_program_pda`.
    #[account(mut, seeds = [b"used_prog", program_hash.as_ref()], bump)]
    pub used_program_pda: UncheckedAccount<'info>,

    #[account(
        init_if_needed,
        payer = opener,
        space = 8 + Pool::INIT_SPACE,
        seeds = [b"pool", token_mint.as_ref()],
        bump
    )]
    pub pool: Account<'info, Pool>,

    /// CHECK: native-SOL vault when `token_mint == default`, the SPL escrow
    /// authority PDA otherwise — see `transfer_value_in`.
    #[account(mut, seeds = [b"escrow", token_mint.as_ref()], bump)]
    pub escrow_vault: UncheckedAccount<'info>,

    /// CHECK: only touched when `token_mint != default`, created
    /// idempotently by `transfer_value_in`.
    #[account(mut)]
    pub escrow_ata: UncheckedAccount<'info>,

    /// CHECK: the opener's own token account for `token_mint` — only needs
    /// to be valid on the SPL path.
    #[account(mut)]
    pub opener_token_account: UncheckedAccount<'info>,

    pub spl: SplAccounts<'info>,

    #[account(seeds = [b"global_state"], bump, seeds::program = ipow::ID)]
    pub ipow_global_state: Account<'info, IpowGlobalState>,

    #[account(mut)]
    pub opener: Signer<'info>,
    pub system_program: Program<'info, System>,
}
