use anchor_lang::prelude::*;
use sha2::{Digest, Sha256};

use crate::constants::CLAIM_WINDOW_SEC;
use crate::errors::ConversionError;
use crate::state::{Conversion, ConversionStatus, Pool};
use crate::spl_accounts::{SplAccounts, SplAccountsBumps, __client_accounts_spl_accounts, __cpi_client_accounts_spl_accounts};
use crate::utils::{transfer_native_from_escrow, transfer_value_in, transfer_value_out};

/// Windowed staked auction for the claiming role, lifted directly from
/// `ipow-message-relay`'s `propose_claim` — see `docs/design/ipow-implementation.md`.
/// Replaces `approve_conversion`'s single `global_state.operator` gate.
///
/// Competes on **rate**, not stake: `stake_amount` is a fixed anti-
/// griefing bond (must meet `conversion.required_bond`, refunded in full
/// to an out-bid claimant, never itself the deciding factor) — but only
/// for `native->bitcoin`, where the claimant never self-escrows anything
/// on-chain (only a later-proven promise to pay real BTC), so the stake
/// is their *only* skin-in-the-game. `bitcoin->native` (single-token or
/// bundle) never posts a stake at all: the claimant self-escrows real
/// collateral right here, and that collateral's destination is identical
/// whether they complete the duty or abandon it — released via proof or
/// force-claim, always to the user either way — so a separate stake
/// would be redundant with the exposure the self-escrow already creates.
/// What actually wins a claim is `proposed_rate_amount` — the value this
/// claimant offers the user on top of what's already committed. For
/// `native->bitcoin`, that's how much real BTC they'll pay (higher is
/// better for the user, who receives it); for `bitcoin->native`, it's how
/// much native/SPL/ERC value they'll front for the fixed required Bitcoin
/// proof (again higher is better for the user, who receives it). The
/// committer's own commit-time value is what they asked for — a claimant
/// meeting it exactly can already win; beating it is a bonus, not a
/// requirement. Every later out-bidding claim must strictly beat the
/// current best. `submit_proof_cache` already enforces whatever rate wins
/// here (`out_value_sats >= conversion.bitcoin_amount`), so a claimant
/// can never renege on a rate they won with — no new enforcement needed
/// there.
///
/// Also carries what `approve_conversion` used to do inline: registering the
/// claimant's own Bitcoin receive-program for this conversion (the
/// destination for bitcoin->native's bitcoin leg, or the operator-supplied
/// program for a native->bitcoin conversion to a non-direct network) plus
/// its uniqueness check. This is genuinely the *claimant's own* data (it
/// says where *they* will send/expect Bitcoin), so it belongs with the claim
/// itself, re-suppliable on every out-staking claim exactly like `ipow-
/// message-relay`'s `operator_btc_address`.
///
/// For bitcoin->native, the claimant also self-escrows their own proposed
/// `native_amount` of `token_mint` right here — there is no separate
/// governance-funded liquidity pool (removed along with `add_liquidity`/
/// `remove_liquidity`); whoever wins the auction personally fronts the
/// value they're promising to deliver, symmetric with how native->bitcoin
/// already has the *user* front it via `deposit_conversion`. This only
/// happens pre-finalize (`!conversion.window_started`): out-staking during
/// that window refunds the previous claimant's own escrowed amount (which
/// may be smaller than the new claimant's, since rates only ever improve)
/// and escrows the new claimant's own (larger) proposal, exactly mirroring
/// the SOL stake refund below. Once a duty is actually locked in by
/// `finalize_claim_conversion`, the escrowed amount is real and must never
/// move again except via proof-based payout or force-claim — a later
/// reclaim-and-reopen round never re-touches it, only the SOL stake
/// changes hands (see `reclaim_expired_conversion`).
#[allow(clippy::too_many_arguments)]
pub fn handler<'info>(
    ctx: Context<'info, ProposeClaimConversion<'info>>,
    stake_amount: u64,
    proposed_rate_amount: u64,
    duty_window_seconds: i64,
    ipow_receive_program: Vec<u8>,
    program_hash: [u8; 32],
) -> Result<()> {
    if !ipow_receive_program.is_empty() {
        let hash = Sha256::digest(&ipow_receive_program);
        require!(
            hash.as_slice() == program_hash,
            ConversionError::InvalidProgramHash
        );
        require!(
            ctx.accounts.used_program_pda.data_is_empty(),
            ConversionError::ProgramAlreadyUsed
        );

        let bump = ctx.bumps.used_program_pda;
        let signer_seeds: &[&[&[u8]]] = &[&[b"used_prog", program_hash.as_ref(), &[bump]]];
        let rent = Rent::get()?;
        let space = 1;
        let lamports = rent.minimum_balance(space);
        anchor_lang::solana_program::program::invoke_signed(
            &anchor_lang::solana_program::system_instruction::create_account(
                &ctx.accounts.claimant.key(),
                &ctx.accounts.used_program_pda.key(),
                lamports,
                space as u64,
                ctx.program_id,
            ),
            &[
                ctx.accounts.claimant.to_account_info(),
                ctx.accounts.used_program_pda.to_account_info(),
                ctx.accounts.system_program.to_account_info(),
            ],
            signer_seeds,
        )?;
    }

    let conversion = &mut ctx.accounts.conversion;

    // Claimable in two situations: the normal fresh-commit case (`Committed`),
    // or a round reopened by `reclaim_expired_conversion` — which leaves
    // `status` at whatever the underlying flow had already reached
    // (`Deposited` for native->bitcoin if the user's real deposit is still
    // sitting in escrow, `Approved` for bitcoin->native since its reservation
    // persists across operator turnover) but always resets `operator_duty_
    // expires_at` to 0. A `Deposited`/`Approved` conversion with a nonzero
    // `operator_duty_expires_at` is a genuinely in-progress duty, not
    // reopened — must not be claimable. See `docs/design/ipow-implementation.md`'s reclaim
    // section.
    require!(
        conversion.status == ConversionStatus::Committed
            || ((conversion.status == ConversionStatus::Approved
                || conversion.status == ConversionStatus::Deposited)
                && conversion.operator_duty_expires_at == 0),
        ConversionError::BadState
    );
    require!(duty_window_seconds > 0, ConversionError::NeedDutyWindow);
    require!(
        stake_amount >= conversion.required_bond,
        ConversionError::BelowRequiredBond
    );

    let now = Clock::get()?.unix_timestamp;
    require!(
        now <= conversion.claim_started_at + CLAIM_WINDOW_SEC,
        ConversionError::ClaimWindowClosed
    );

    let is_first_claim = conversion.responsible_operator == Pubkey::default();

    // A `bitcoin->token` conversion carrying a bundle (`extra_tokens`
    // non-empty) can only ever have gotten there via `commit_bitcoin_to_
    // token` with `network_id == 0` — no auction can compare unequal
    // multi-token offers across claimants without a price oracle, so once
    // a bundle exists the *token* side becomes the fixed ask instead, and
    // `bitcoin_amount` becomes the competed field — with the direction
    // flipped (lower wins: claimants underbid how little BTC the user
    // needs to pay for the same fixed bundle).
    let is_bundle = !conversion.is_native_to_bitcoin && !conversion.extra_tokens.is_empty();

    // The competed-on side: `bitcoin_amount` for native->bitcoin (how much
    // real BTC the claimant will pay the user) and for a bitcoin->token
    // bundle (see above); `native_amount` for a single-token bitcoin->
    // token (how much value the claimant will front the user). The
    // committer's own commit-time value is the ask a first claim must meet
    // or beat.
    let current_rate_value = if conversion.is_native_to_bitcoin || is_bundle {
        conversion.bitcoin_amount
    } else {
        conversion.native_amount
    };
    if is_bundle {
        // Lower is better for the user here — see the comment above.
        if is_first_claim {
            require!(
                proposed_rate_amount <= current_rate_value,
                ConversionError::AboveAskedRate
            );
        } else {
            require!(
                proposed_rate_amount < current_rate_value,
                ConversionError::RateNotBetter
            );
        }
    } else {
        // Higher is better for the user (unchanged).
        if is_first_claim {
            require!(
                proposed_rate_amount >= current_rate_value,
                ConversionError::BelowAskedRate
            );
        } else {
            require!(
                proposed_rate_amount > current_rate_value,
                ConversionError::RateNotBetter
            );
        }
    }

    if !is_first_claim {
        require!(
            ctx.accounts.previous_claimant.key() == conversion.responsible_operator,
            ConversionError::Unauthorized
        );

        // No stake to refund for `bitcoin->token` — see the doc comment
        // above and `commit_bitcoin_to_token`'s own: the self-escrowed
        // collateral already carries real exposure, so this direction
        // never posts a stake in the first place (`conversion.staked_
        // bond` stays 0 throughout for it).
        if conversion.is_native_to_bitcoin {
            let previous_stake = conversion.staked_bond;
            let escrow_bump = ctx.bumps.stake_escrow;
            let seeds = &[b"stake_escrow".as_ref(), &[escrow_bump]];
            let signer_seeds = &[&seeds[..]];
            transfer_native_from_escrow(
                previous_stake,
                &ctx.accounts.previous_claimant.to_account_info(),
                &ctx.accounts.stake_escrow.to_account_info(),
                &ctx.accounts.system_program.to_account_info(),
                signer_seeds,
            )?;
        }
    }

    // `bitcoin->token` never posts a stake — see the handler doc comment.
    // `stake_amount` is simply ignored for that direction: nothing is
    // transferred, and `conversion.staked_bond` (set below) is left
    // untouched rather than set to a value with no matching transfer,
    // which would otherwise leave unaccounted-for expectations against
    // `stake_escrow` (a single pool shared across every conversion).
    if conversion.is_native_to_bitcoin {
        anchor_lang::solana_program::program::invoke(
            &anchor_lang::solana_program::system_instruction::transfer(
                &ctx.accounts.claimant.key(),
                &ctx.accounts.stake_escrow.key(),
                stake_amount,
            ),
            &[
                ctx.accounts.claimant.to_account_info(),
                ctx.accounts.stake_escrow.to_account_info(),
                ctx.accounts.system_program.to_account_info(),
            ],
        )?;
    }

    if !conversion.is_native_to_bitcoin && !conversion.window_started {
        let token_mint = conversion.token_mint;
        // What the *previous* claimant actually escrowed. For a
        // single-token claim, `conversion.native_amount` still holds their
        // winning proposal here, not yet overwritten by this claim's
        // (necessarily larger) one below. For a bundle, this is simply the
        // fixed ask — every claimant self-escrows the exact same amount,
        // since only `bitcoin_amount` is ever competed on.
        let previous_native_amount = conversion.native_amount;
        // What the *new* claimant is about to self-escrow: their own
        // (larger) proposal for a single-token claim, or the same fixed
        // ask as everyone else for a bundle (they're competing on
        // `bitcoin_amount`, not this).
        let self_escrow_amount = if is_bundle {
            conversion.native_amount
        } else {
            proposed_rate_amount
        };

        if !is_first_claim {
            let escrow_bump = ctx.accounts.pool.escrow_bump;
            let escrow_seeds = &[b"escrow".as_ref(), token_mint.as_ref(), &[escrow_bump]];
            let escrow_signer_seeds = &[&escrow_seeds[..]];
            transfer_value_out(
                token_mint,
                previous_native_amount,
                &ctx.accounts.previous_claimant.to_account_info(),
                &ctx.accounts.previous_claimant_token_account.to_account_info(),
                &ctx.accounts.escrow_vault.to_account_info(),
                &ctx.accounts.escrow_ata.to_account_info(),
                &ctx.accounts.escrow_vault.to_account_info(),
                &ctx.accounts.claimant.to_account_info(),
                &ctx.accounts.system_program.to_account_info(),
                &ctx.accounts.spl,
                escrow_signer_seeds,
            )?;
            ctx.accounts.pool.total_reserved = ctx
                .accounts
                .pool
                .total_reserved
                .checked_sub(previous_native_amount)
                .unwrap();
        }

        ctx.accounts.pool.token_mint = token_mint;
        ctx.accounts.pool.escrow_bump = ctx.bumps.escrow_vault;

        // The new claimant self-escrows `self_escrow_amount` (see above).
        let received = transfer_value_in(
            token_mint,
            self_escrow_amount,
            &ctx.accounts.claimant.to_account_info(),
            &ctx.accounts.claimant_token_account.to_account_info(),
            &ctx.accounts.escrow_vault.to_account_info(),
            &ctx.accounts.escrow_ata.to_account_info(),
            &ctx.accounts.escrow_vault.to_account_info(),
            &ctx.accounts.claimant.to_account_info(),
            &ctx.accounts.system_program.to_account_info(),
            &ctx.accounts.spl,
        )?;
        require!(
            received == self_escrow_amount,
            ConversionError::DepositShortfall
        );
        ctx.accounts.pool.total_reserved =
            ctx.accounts.pool.total_reserved.checked_add(received).unwrap();

        if is_bundle {
            // Bundle: every extra token's fixed amount also self-escrows
            // (and, on an out-bid, refunds to the previous claimant) here
            // — same `remaining_accounts` shape and manual PDA
            // re-validation every other bundle loop uses, but 6 accounts
            // per extra instead of 5: this is the first place that needs
            // *both* directions (refund-out to the outbid claimant, and
            // escrow-in from the new one) in the same call. On a first
            // claim, `previous_claimant_extra_token_account` is simply
            // unused — same as the typed `previous_claimant_token_account`
            // field above being present unconditionally but ignored then.
            const ACCOUNTS_PER_EXTRA: usize = 6;
            require!(
                ctx.remaining_accounts.len() == conversion.extra_tokens.len() * ACCOUNTS_PER_EXTRA,
                ConversionError::InvalidRemainingAccount
            );
            let claimant_info = ctx.accounts.claimant.to_account_info();
            let previous_claimant_info = ctx.accounts.previous_claimant.to_account_info();
            let system_program_info = ctx.accounts.system_program.to_account_info();
            for (i, extra) in conversion.extra_tokens.iter().enumerate() {
                let base = i * ACCOUNTS_PER_EXTRA;
                let mint: &AccountInfo = &ctx.remaining_accounts[base];
                let token_program: &AccountInfo = &ctx.remaining_accounts[base + 1];
                let extra_escrow_vault: &AccountInfo = &ctx.remaining_accounts[base + 2];
                let extra_escrow_ata: &AccountInfo = &ctx.remaining_accounts[base + 3];
                let previous_claimant_extra_token_account: &AccountInfo =
                    &ctx.remaining_accounts[base + 4];
                let claimant_extra_token_account: &AccountInfo = &ctx.remaining_accounts[base + 5];

                require_keys_eq!(mint.key(), extra.mint, ConversionError::InvalidRemainingAccount);
                let (expected_escrow_vault, expected_bump) =
                    Pubkey::find_program_address(&[b"escrow", extra.mint.as_ref()], ctx.program_id);
                require_keys_eq!(
                    extra_escrow_vault.key(),
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

                if !is_first_claim {
                    transfer_value_out(
                        extra.mint,
                        extra.amount,
                        &previous_claimant_info,
                        previous_claimant_extra_token_account,
                        extra_escrow_vault,
                        extra_escrow_ata,
                        extra_escrow_vault,
                        &claimant_info,
                        &system_program_info,
                        &extra_spl,
                        extra_escrow_signer_seeds,
                    )?;
                }

                let received_extra = transfer_value_in(
                    extra.mint,
                    extra.amount,
                    &claimant_info,
                    claimant_extra_token_account,
                    extra_escrow_vault,
                    extra_escrow_ata,
                    extra_escrow_vault,
                    &claimant_info,
                    &system_program_info,
                    &extra_spl,
                )?;
                require!(received_extra == extra.amount, ConversionError::DepositShortfall);
            }

            // Bundle: the fixed token side never changes — `bitcoin_amount`
            // is the competed field instead.
            conversion.bitcoin_amount = proposed_rate_amount;
        } else {
            // Lock in the winning rate here, inside the same
            // `!window_started` gate as the self-escrow move itself — this
            // field *is* the real, already-escrowed reservation amount for
            // bitcoin->native, so it must never change once real (same
            // reasoning `reserved_native`/`window_start_height` already
            // use). A round reopened after a claimant defaults (`reclaim_
            // expired_conversion`) always has `window_started == true` by
            // then, so this simply doesn't run — the new claimant inherits
            // the already-real reservation as-is, never re-escrows or
            // renegotiates it.
            conversion.native_amount = proposed_rate_amount;
        }
    }

    conversion.responsible_operator = ctx.accounts.claimant.key();
    // `bitcoin->token` never posts a stake — leave `staked_bond` at 0,
    // matching that no transfer happened above.
    if conversion.is_native_to_bitcoin {
        conversion.staked_bond = stake_amount;
    }
    conversion.last_claim_at = now;
    conversion.duty_window_seconds = duty_window_seconds;

    // native->bitcoin's `bitcoin_amount` is never backed by an on-chain
    // escrow at propose time (only a later-proven promise), so unlike
    // bitcoin->native's `native_amount` above, it's always safe to update
    // here regardless of `window_started` — nothing to keep in sync with.
    // `submit_proof_cache`'s existing check (`out_value_sats >= conversion.
    // bitcoin_amount`) and `claim_native_operator_expired`'s existing
    // `reserved_native` payout both read straight off these fields, so no
    // changes are needed there: whatever wins here is what gets enforced/
    // paid later, automatically.
    if conversion.is_native_to_bitcoin {
        conversion.bitcoin_amount = proposed_rate_amount;
    }

    if !conversion.is_native_to_bitcoin {
        require!(
            !ipow_receive_program.is_empty() && ipow_receive_program.len() <= 80,
            ConversionError::BadBitcoinProgram
        );
        conversion.ipow_receive_program = ipow_receive_program;
    } else if conversion.is_native_to_bitcoin && conversion.user_program.is_empty() {
        require!(
            !ipow_receive_program.is_empty() && ipow_receive_program.len() <= 80,
            ConversionError::BadBitcoinProgram
        );
        conversion.user_program = ipow_receive_program;
    }

    Ok(())
}

#[derive(Accounts)]
#[instruction(stake_amount: u64, proposed_rate_amount: u64, duty_window_seconds: i64, ipow_receive_program: Vec<u8>, program_hash: [u8; 32])]
pub struct ProposeClaimConversion<'info> {
    /// This program's own stake escrow — entirely separate from the per-mint
    /// value escrow in `pool.rs`; auction stakes are always native SOL
    /// regardless of what the conversion itself moves, same convention
    /// `ipow-message-relay` uses.
    #[account(mut, seeds = [b"stake_escrow"], bump)]
    pub stake_escrow: SystemAccount<'info>,

    #[account(mut)]
    pub conversion: Account<'info, Conversion>,

    /// CHECK: refund destination for an outbid claimant's own stake, verified
    /// against `conversion.responsible_operator` in the handler.
    #[account(mut)]
    pub previous_claimant: UncheckedAccount<'info>,

    /// CHECK: the outbid claimant's own token account for `conversion.
    /// token_mint` — only touched on a bitcoin->native out-stake, refunding
    /// the `native_amount` they self-escrowed on their own earlier claim.
    #[account(mut)]
    pub previous_claimant_token_account: UncheckedAccount<'info>,

    /// CHECK: pure existence marker, same pattern as `ipow-message-relay`'s
    /// `used_btc_address`.
    #[account(mut, seeds = [b"used_prog", program_hash.as_ref()], bump)]
    pub used_program_pda: UncheckedAccount<'info>,

    /// Only touched on the bitcoin->native, pre-finalize path — see the
    /// handler's doc comment. `init_if_needed` since this may be the very
    /// first conversion ever claimed for this mint (no more `add_liquidity`
    /// to have created it ahead of time).
    #[account(
        init_if_needed,
        payer = claimant,
        space = 8 + Pool::INIT_SPACE,
        seeds = [b"pool", conversion.token_mint.as_ref()],
        bump
    )]
    pub pool: Account<'info, Pool>,

    /// CHECK: native-SOL vault when `conversion.token_mint == default`, the
    /// SPL escrow authority PDA otherwise — see `transfer_value_in`/`_out`.
    #[account(mut, seeds = [b"escrow", conversion.token_mint.as_ref()], bump)]
    pub escrow_vault: UncheckedAccount<'info>,

    /// CHECK: only touched when `conversion.token_mint != default`, created
    /// idempotently.
    #[account(mut)]
    pub escrow_ata: UncheckedAccount<'info>,

    /// CHECK: the claimant's own token account for `conversion.token_mint`
    /// — only touched on the bitcoin->native, pre-finalize self-escrow path.
    #[account(mut)]
    pub claimant_token_account: UncheckedAccount<'info>,

    pub spl: SplAccounts<'info>,

    #[account(mut)]
    pub claimant: Signer<'info>,

    pub system_program: Program<'info, System>,
}
