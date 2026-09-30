use anchor_lang::prelude::*;
use ipow::bitcoin::parse_output_at;
use ipow::state::GlobalHeader;
use sha2::{Digest, Sha256};

use crate::constants::PROOF_BLOCKS_WINDOW;
use crate::errors::ConversionError;
use crate::state::{Conversion, ConversionStatus, Pool};
use crate::spl_accounts::{SplAccounts, SplAccountsBumps, __client_accounts_spl_accounts, __cpi_client_accounts_spl_accounts};
use crate::utils::{transfer_native_from_escrow, transfer_value_out};

/// Ported from `ipow`'s own `submit_proof_cache`, with four real changes:
/// 1. Pays `conversion.responsible_operator` (the auction winner), not a
///    fixed `global_state.operator`.
/// 2. `header` is required, not `Option` — this pass drops the old
///    "cache an attempt before the header exists, complete it later"
///    two-phase flow for simplicity; the claiming operator already has to
///    relay headers for their own light client duty, so requiring the
///    header up front isn't new friction.
/// 3. Cross-checks `header.height == proof_block_height` — the old version
///    never verified the *passed-in* header account actually corresponds to
///    the height being proven, only that its `merkle_root_le` validates
///    *some* merkle branch. Harmless when the caller was a single trusted
///    operator; worth closing now that any staked claimant can call this.
/// 4. `native_amount` (the value the conversion actually moves) pays out via
///    `transfer_value_out`, native or SPL depending on `conversion.token_
///    mint` — `commit_fee`/auction stake/bounty stay native SOL regardless,
///    paid from the always-native `fee_pool`/`stake_escrow`.
pub fn handler<'info>(
    ctx: Context<'info, SubmitProofCache<'info>>,
    tx_raw: Vec<u8>,
    vout_index: u64,
    proof_block_height: u64,
    branch_le: Vec<[u8; 32]>,
    index: u64,
) -> Result<()> {
    let conversion = &mut ctx.accounts.conversion;

    require!(conversion.window_started, ConversionError::NoHeadersYet);
    require!(!conversion.proof_verified, ConversionError::AlreadyVerified);

    let window_end = conversion
        .window_start_height
        .checked_add(PROOF_BLOCKS_WINDOW)
        .unwrap()
        .checked_sub(1)
        .unwrap();
    require!(
        proof_block_height >= conversion.window_start_height && proof_block_height <= window_end,
        ConversionError::IncorrectWindow
    );
    require!(
        ctx.accounts.header.height == proof_block_height,
        ConversionError::InvalidHeader
    );

    // Native->bitcoin: the operator proves they paid the user's Bitcoin
    // address, releasing the user's own deposit to themselves. Bitcoin-
    // ->native: the user proves the operator paid *them*, releasing the
    // operator's reserved payout — same asymmetry the original design used,
    // preserved here (only who's *responsible* is now auction-won rather
    // than fixed).
    if conversion.is_native_to_bitcoin {
        require!(
            ctx.accounts.signer.key() == conversion.responsible_operator,
            ConversionError::Unauthorized
        );
    } else {
        require!(
            ctx.accounts.signer.key() == conversion.user,
            ConversionError::Unauthorized
        );
    }

    let (out_value_sats, out_program) = parse_output_at(&tx_raw, vout_index as usize)?;

    let hash1 = Sha256::digest(&tx_raw);
    let hash2 = Sha256::digest(&hash1);
    let mut txid_le = [0u8; 32];
    txid_le.copy_from_slice(&hash2);

    let mut current_hash = txid_le;
    let mut current_index = index;
    for sibling in branch_le.iter() {
        let mut hasher = Sha256::new();
        if current_index % 2 == 0 {
            hasher.update(current_hash);
            hasher.update(sibling);
        } else {
            hasher.update(sibling);
            hasher.update(current_hash);
        }
        let h1 = hasher.finalize();
        let h2 = Sha256::digest(&h1);
        current_hash.copy_from_slice(&h2);
        current_index /= 2;
    }
    require!(
        current_hash == ctx.accounts.header.merkle_root_le,
        ConversionError::InvalidHeader
    );

    let token_mint = conversion.token_mint;
    let escrow_bump = ctx.accounts.pool.escrow_bump;
    let escrow_seeds = &[b"escrow".as_ref(), token_mint.as_ref(), &[escrow_bump]];
    let escrow_signer_seeds = &[&escrow_seeds[..]];
    let fee_to_operator = conversion.commit_fee;
    let operator_info = ctx.accounts.operator.to_account_info();
    let signer_info = ctx.accounts.signer.to_account_info();
    let system_program_info = ctx.accounts.system_program.to_account_info();

    if conversion.is_native_to_bitcoin {
        require!(
            conversion.status == ConversionStatus::Deposited,
            ConversionError::BadState
        );
        require!(
            out_value_sats >= conversion.bitcoin_amount,
            ConversionError::IncorrectValue
        );
        require!(
            out_program == conversion.user_program,
            ConversionError::BadBitcoinProgram
        );

        ctx.accounts.pool.total_locked_deposits = ctx
            .accounts
            .pool
            .total_locked_deposits
            .checked_sub(conversion.native_amount)
            .unwrap();

        transfer_value_out(
            token_mint,
            conversion.native_amount,
            &operator_info,
            &ctx.accounts.operator_token_account.to_account_info(),
            &ctx.accounts.escrow_vault.to_account_info(),
            &ctx.accounts.escrow_ata.to_account_info(),
            &ctx.accounts.escrow_vault.to_account_info(),
            &signer_info,
            &system_program_info,
            &ctx.accounts.spl,
            escrow_signer_seeds,
        )?;

        // Bundle: each extra token pays out via `remaining_accounts`, same
        // shape and same re-derived-PDA validation as `deposit_conversion`'s
        // own loop — see its comment. `Pool.total_locked_deposits` for each
        // extra token's own pool is deliberately not touched here (that
        // bookkeeping is never load-bearing for correctness — see
        // docs/design/ipow-implementation.md's "No liquidity pool" section); only the real
        // transfers matter.
        const ACCOUNTS_PER_EXTRA: usize = 5;
        require!(
            ctx.remaining_accounts.len() == conversion.extra_tokens.len() * ACCOUNTS_PER_EXTRA,
            ConversionError::InvalidRemainingAccount
        );
        for (i, extra) in conversion.extra_tokens.iter().enumerate() {
            let base = i * ACCOUNTS_PER_EXTRA;
            let mint: &AccountInfo = &ctx.remaining_accounts[base];
            let token_program: &AccountInfo = &ctx.remaining_accounts[base + 1];
            let escrow_vault: &AccountInfo = &ctx.remaining_accounts[base + 2];
            let escrow_ata: &AccountInfo = &ctx.remaining_accounts[base + 3];
            let operator_token_account: &AccountInfo = &ctx.remaining_accounts[base + 4];

            require_keys_eq!(mint.key(), extra.mint, ConversionError::InvalidRemainingAccount);
            let (expected_escrow_vault, expected_bump) =
                Pubkey::find_program_address(&[b"escrow", extra.mint.as_ref()], ctx.program_id);
            require_keys_eq!(
                escrow_vault.key(),
                expected_escrow_vault,
                ConversionError::InvalidRemainingAccount
            );
            let extra_escrow_seeds: &[&[u8]] = &[b"escrow", extra.mint.as_ref(), &[expected_bump]];
            let extra_escrow_signer_seeds: &[&[&[u8]]] = &[extra_escrow_seeds];

            let extra_spl = SplAccounts {
                mint: UncheckedAccount::try_from(mint),
                token_program: UncheckedAccount::try_from(token_program),
                associated_token_program: ctx.accounts.spl.associated_token_program.clone(),
            };
            transfer_value_out(
                extra.mint,
                extra.amount,
                &operator_info,
                operator_token_account,
                escrow_vault,
                escrow_ata,
                escrow_vault,
                &signer_info,
                &system_program_info,
                &extra_spl,
                extra_escrow_signer_seeds,
            )?;
        }
    } else {
        let expected_prog = if conversion.ipow_receive_program.is_empty() {
            conversion.user_program.clone()
        } else {
            conversion.ipow_receive_program.clone()
        };
        require!(
            out_program == expected_prog,
            ConversionError::BadBitcoinProgram
        );
        require!(
            out_value_sats >= conversion.bitcoin_amount,
            ConversionError::IncorrectValue
        );

        let payout = conversion.native_amount;
        ctx.accounts.pool.total_reserved =
            ctx.accounts.pool.total_reserved.checked_sub(payout).unwrap();

        transfer_value_out(
            token_mint,
            payout,
            &ctx.accounts.user.to_account_info(),
            &ctx.accounts.user_token_account.to_account_info(),
            &ctx.accounts.escrow_vault.to_account_info(),
            &ctx.accounts.escrow_ata.to_account_info(),
            &ctx.accounts.escrow_vault.to_account_info(),
            &signer_info,
            &system_program_info,
            &ctx.accounts.spl,
            escrow_signer_seeds,
        )?;

        // Bundle: only ever populated by `open_bundle_tunnel` (no auctioned
        // bitcoin->token commit can carry one — see its own doc comment).
        // Same `remaining_accounts` shape and manual PDA re-validation as
        // the `is_native_to_bitcoin` branch above, paying out to the
        // user's own extra-token accounts instead of the operator's.
        const ACCOUNTS_PER_EXTRA: usize = 5;
        require!(
            ctx.remaining_accounts.len() == conversion.extra_tokens.len() * ACCOUNTS_PER_EXTRA,
            ConversionError::InvalidRemainingAccount
        );
        for (i, extra) in conversion.extra_tokens.iter().enumerate() {
            let base = i * ACCOUNTS_PER_EXTRA;
            let mint: &AccountInfo = &ctx.remaining_accounts[base];
            let token_program: &AccountInfo = &ctx.remaining_accounts[base + 1];
            let escrow_vault: &AccountInfo = &ctx.remaining_accounts[base + 2];
            let escrow_ata: &AccountInfo = &ctx.remaining_accounts[base + 3];
            let user_extra_token_account: &AccountInfo = &ctx.remaining_accounts[base + 4];

            require_keys_eq!(mint.key(), extra.mint, ConversionError::InvalidRemainingAccount);
            let (expected_escrow_vault, expected_bump) =
                Pubkey::find_program_address(&[b"escrow", extra.mint.as_ref()], ctx.program_id);
            require_keys_eq!(
                escrow_vault.key(),
                expected_escrow_vault,
                ConversionError::InvalidRemainingAccount
            );
            let extra_escrow_seeds: &[&[u8]] = &[b"escrow", extra.mint.as_ref(), &[expected_bump]];
            let extra_escrow_signer_seeds: &[&[&[u8]]] = &[extra_escrow_seeds];

            let extra_spl = SplAccounts {
                mint: UncheckedAccount::try_from(mint),
                token_program: UncheckedAccount::try_from(token_program),
                associated_token_program: ctx.accounts.spl.associated_token_program.clone(),
            };
            transfer_value_out(
                extra.mint,
                extra.amount,
                &ctx.accounts.user.to_account_info(),
                user_extra_token_account,
                escrow_vault,
                escrow_ata,
                escrow_vault,
                &signer_info,
                &system_program_info,
                &extra_spl,
                extra_escrow_signer_seeds,
            )?;
        }
    }

    if fee_to_operator > 0 {
        conversion.commit_fee = 0;
        let fee_bump = ctx.accounts.fee_pool.escrow_bump;
        let mint_key = Pubkey::default();
        let fee_seeds = &[b"escrow".as_ref(), mint_key.as_ref(), &[fee_bump]];
        let fee_signer_seeds = &[&fee_seeds[..]];
        ctx.accounts.fee_pool.total_held_commit_fees = ctx
            .accounts
            .fee_pool
            .total_held_commit_fees
            .checked_sub(fee_to_operator)
            .unwrap();
        transfer_native_from_escrow(
            fee_to_operator,
            &operator_info,
            &ctx.accounts.fee_escrow.to_account_info(),
            &system_program_info,
            fee_signer_seeds,
        )?;
    }

    // Auction stake releases back to the operator on success — the reward
    // for actually finishing the duty, paid from the separate stake
    // escrow (native SOL, independent of what the conversion itself
    // moves). Any *previous* claimant's forfeited stake was already paid
    // straight to `conversion.user` at reclaim time (see `reclaim_
    // expired_conversion`), not held here — this only ever returns the
    // current operator's own stake, never someone else's forfeiture.
    let stake_payout = conversion.staked_bond;
    conversion.staked_bond = 0;
    let stake_bump = ctx.bumps.stake_escrow;
    let stake_seeds = &[b"stake_escrow".as_ref(), &[stake_bump]];
    let stake_signer_seeds = &[&stake_seeds[..]];
    transfer_native_from_escrow(
        stake_payout,
        &operator_info,
        &ctx.accounts.stake_escrow.to_account_info(),
        &system_program_info,
        stake_signer_seeds,
    )?;

    conversion.proof_txid_le = txid_le;
    conversion.proof_block_height = proof_block_height;
    conversion.status = ConversionStatus::Completed;
    conversion.proof_verified = true;

    Ok(())
}

#[derive(Accounts)]
#[instruction(tx_raw: Vec<u8>, vout_index: u64, proof_block_height: u64)]
pub struct SubmitProofCache<'info> {
    #[account(mut)]
    pub conversion: Account<'info, Conversion>,

    #[account(mut, seeds = [b"pool", conversion.token_mint.as_ref()], bump)]
    pub pool: Account<'info, Pool>,

    /// CHECK: native-SOL vault when `conversion.token_mint == default`, the
    /// SPL escrow authority PDA otherwise — see `transfer_value_out`.
    #[account(mut, seeds = [b"escrow", conversion.token_mint.as_ref()], bump = pool.escrow_bump)]
    pub escrow_vault: UncheckedAccount<'info>,

    /// CHECK: only touched when `conversion.token_mint != default`.
    #[account(mut)]
    pub escrow_ata: UncheckedAccount<'info>,

    /// Always native SOL — commit fees never follow `token_mint`. `dup`:
    /// for a native conversion this is the exact same PDA as `pool` above
    /// (both seeded by `Pubkey::default()`) — a legitimate, intentional
    /// alias Anchor's default duplicate-mutable-account guard would
    /// otherwise reject.
    #[account(mut, seeds = [b"pool", Pubkey::default().as_ref()], bump, dup)]
    pub fee_pool: Account<'info, Pool>,

    #[account(mut, seeds = [b"escrow", Pubkey::default().as_ref()], bump = fee_pool.escrow_bump, dup)]
    pub fee_escrow: SystemAccount<'info>,

    #[account(mut, seeds = [b"stake_escrow"], bump)]
    pub stake_escrow: SystemAccount<'info>,

    pub header: Account<'info, GlobalHeader>,

    /// CHECK: address is constrained to `conversion.responsible_operator`
    /// below, and re-verified against the same field in the handler before
    /// any payout — without this, funds could be redirected to whatever
    /// account happens to be passed here.
    #[account(mut, address = conversion.responsible_operator @ ConversionError::Unauthorized)]
    pub operator: UncheckedAccount<'info>,
    /// CHECK: the operator's own token account for `conversion.token_mint`
    /// — only needs to be valid on the SPL, native->bitcoin payout path.
    #[account(mut)]
    pub operator_token_account: UncheckedAccount<'info>,

    /// CHECK: address is constrained to `conversion.user`.
    #[account(mut, address = conversion.user)]
    pub user: UncheckedAccount<'info>,
    /// CHECK: the user's own token account for `conversion.token_mint` —
    /// only needs to be valid on the SPL, bitcoin->native payout path.
    #[account(mut)]
    pub user_token_account: UncheckedAccount<'info>,

    pub spl: SplAccounts<'info>,

    #[account(mut)]
    pub signer: Signer<'info>,
    pub system_program: Program<'info, System>,
}
