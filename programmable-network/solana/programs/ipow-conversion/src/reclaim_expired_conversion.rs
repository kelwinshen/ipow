use anchor_lang::prelude::*;

use crate::errors::ConversionError;
use crate::state::{Conversion, ConversionStatus};
use crate::utils::transfer_native_from_escrow;

/// Permissionless. No equivalent existed in the old single-operator design —
/// that operator never staked anything, so there was nothing to reclaim.
/// Now that claiming is a real staked auction, a claimant who wins and then
/// goes dark needs their stake resolved. Mirrors `ipow-message-relay`'s
/// `reclaim_expired_message`, split by whether real value was already
/// genuinely committed by the time the claimant went dark:
///
/// - **native->bitcoin, user never deposited** (`status == Approved`):
///   nobody's fault — refund the claimant's stake in full, reopen to
///   `Committed`, a clean fresh round.
/// - **native->bitcoin, user already deposited** (`status == Deposited`), or
///   **bitcoin->native** (reservation is made at first `finalize_claim_
///   conversion` and always persists): the claimant had a real duty and
///   didn't do it — the stake pays out directly to `conversion.user` right
///   here (compensation for the wasted time, not held for whoever
///   eventually finishes — neither guaranteed-resolution path depends on a
///   replacement claimant ever showing up, so there's no need to reward
///   one), and `required_bond` escalates to match, a separate anti-
///   repeat-griefing measure. `status` is left exactly as it was
///   (`Deposited` / `Approved`): the user's deposit or the pool's
///   reservation is already real and doesn't need redoing, only a new
///   operator does.
///
/// Either way: `responsible_operator` resets to default and `operator_duty_
/// expires_at` resets to 0, which is exactly what `propose_claim_
/// conversion`'s guard checks to allow a fresh round on a non-`Committed`
/// status. `window_start_height`/`reserved_native` are never touched here —
/// see their own doc comments on why those deadlines and that reservation
/// must survive operator turnover unchanged.
pub fn handler(ctx: Context<ReclaimExpiredConversion>) -> Result<()> {
    let conversion = &mut ctx.accounts.conversion;

    require!(
        conversion.status == ConversionStatus::Approved
            || conversion.status == ConversionStatus::Deposited,
        ConversionError::BadState
    );

    let now = Clock::get()?.unix_timestamp;
    require!(
        conversion.operator_duty_expires_at > 0 && now > conversion.operator_duty_expires_at,
        ConversionError::DutyNotExpired
    );

    let real_commitment_happened =
        !conversion.is_native_to_bitcoin || conversion.status == ConversionStatus::Deposited;

    if real_commitment_happened {
        let forfeited = conversion.staked_bond;
        conversion.required_bond = forfeited;
        let escrow_bump = ctx.bumps.stake_escrow;
        let seeds = &[b"stake_escrow".as_ref(), &[escrow_bump]];
        let signer_seeds = &[&seeds[..]];
        transfer_native_from_escrow(
            forfeited,
            &ctx.accounts.user.to_account_info(),
            &ctx.accounts.stake_escrow.to_account_info(),
            &ctx.accounts.system_program.to_account_info(),
            signer_seeds,
        )?;
    } else {
        require!(
            ctx.accounts.previous_claimant.key() == conversion.responsible_operator,
            ConversionError::Unauthorized
        );
        let refund = conversion.staked_bond;
        let escrow_bump = ctx.bumps.stake_escrow;
        let seeds = &[b"stake_escrow".as_ref(), &[escrow_bump]];
        let signer_seeds = &[&seeds[..]];
        transfer_native_from_escrow(
            refund,
            &ctx.accounts.previous_claimant.to_account_info(),
            &ctx.accounts.stake_escrow.to_account_info(),
            &ctx.accounts.system_program.to_account_info(),
            signer_seeds,
        )?;
        conversion.status = ConversionStatus::Committed;
    }

    conversion.staked_bond = 0;
    conversion.responsible_operator = Pubkey::default();
    conversion.operator_duty_expires_at = 0;
    conversion.claim_started_at = now;
    conversion.last_claim_at = now;

    Ok(())
}

#[derive(Accounts)]
pub struct ReclaimExpiredConversion<'info> {
    #[account(mut, seeds = [b"stake_escrow"], bump)]
    pub stake_escrow: SystemAccount<'info>,

    #[account(mut)]
    pub conversion: Account<'info, Conversion>,

    /// CHECK: refund destination on the "never confirmed" path only,
    /// verified against `conversion.responsible_operator` in the handler;
    /// ignored on the forfeit path.
    #[account(mut)]
    pub previous_claimant: UncheckedAccount<'info>,

    /// CHECK: forfeited-stake destination on the forfeit path only,
    /// address-constrained to `conversion.user`; ignored on the "never
    /// confirmed" refund path.
    #[account(mut, address = conversion.user)]
    pub user: UncheckedAccount<'info>,

    pub system_program: Program<'info, System>,
}
