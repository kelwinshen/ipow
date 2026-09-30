use anchor_lang::prelude::*;
use ipow_protocol::state::Job;

use crate::constants::CONFIG_SEED;
use crate::errors::ConversionError;
use crate::state::{Config, Swap};

/// Passes on to the swap's user this application's share of its slashed
/// job's escrow, 80% of x (D7, D36), once. What the protocol credited this
/// application is first withdrawn into the settings record.
pub fn pass_on<'info>(
    swap: &mut Account<'info, Swap>,
    config: &mut Account<'info, Config>,
    job: &Job,
    user: &AccountInfo<'info>,
    credit: &UncheckedAccount<'info>,
    protocol: &UncheckedAccount<'info>,
    vault: &UncheckedAccount<'info>,
) -> Result<()> {
    if swap.compensated {
        return Ok(());
    }
    swap.compensated = true;
    require_keys_eq!(user.key(), swap.user, ConversionError::WrongAccount);
    let config_info = config.to_account_info();
    let mut compensation = config.compensation;
    // The credit record is at its one address (checked by the accounts);
    // when it exists and holds something, it is withdrawn first.
    let has_credit = {
        let data = credit.try_borrow_data()?;
        credit.owner == &ipow_protocol::ID
            && ipow_protocol::state::Credit::try_deserialize(&mut &data[..]).is_ok_and(|c| c.amount > 0 && c.owner == config_info.key())
    };
    {
        if has_credit {
            let before = config_info.lamports();
            let seeds: &[&[u8]] = &[CONFIG_SEED, &[config.bump]];
            ipow_protocol::cpi::withdraw_credit(CpiContext::new_with_signer(
                ipow_protocol::ID,
                ipow_protocol::cpi::accounts::WithdrawCredit {
                    protocol: protocol.to_account_info(),
                    credit: credit.to_account_info(),
                    vault: vault.to_account_info(),
                    owner: config_info.clone(),
                },
                &[seeds],
            ))?;
            compensation = compensation
                .checked_add(config_info.lamports().checked_sub(before).ok_or(ConversionError::Overflow)?)
                .ok_or(ConversionError::Overflow)?;
        }
    }
    let share = (job.escrow as u128 * ipow_protocol::constants::APPLICATION_SHARE_BPS as u128 / ipow_protocol::constants::BPS as u128) as u64;
    let share = share.min(compensation);
    if share > 0 {
        let config_lamports = config_info.lamports().checked_sub(share).ok_or(ConversionError::Overflow)?;
        let user_lamports = user.lamports().checked_add(share).ok_or(ConversionError::Overflow)?;
        **config_info.try_borrow_mut_lamports()? = config_lamports;
        **user.try_borrow_mut_lamports()? = user_lamports;
    }
    config.compensation = compensation - share;
    Ok(())
}
