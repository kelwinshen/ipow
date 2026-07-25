use anchor_lang::prelude::*;
use sha2::{Digest, Sha256};

use crate::errors::IPoWError;
use crate::state::{
    Conversion, ConversionStatus, GlobalHeader, GlobalState, HeightTracker, ProofCache,
};
use crate::utils::transfer_from_escrow;

pub fn handler(ctx: Context<FinalizeProofCached>) -> Result<()> {
    let header = match ctx.accounts.header.as_ref() {
        Some(h) => h,
        None => return Ok(()),
    };

    let pc = &mut ctx.accounts.proof_cache;
    if !pc.is_set || pc.is_verified || pc.is_invalid {
        return Ok(());
    }

    let conversion = &mut ctx.accounts.conversion;
    if conversion.status == ConversionStatus::Completed
        || conversion.status == ConversionStatus::Refunded
    {
        return Ok(());
    }

    let mut current_hash = pc.txid_le;
    let mut current_index = pc.merkle_index;

    for sibling in pc.branch_le.iter() {
        let mut hasher = Sha256::new();
        if current_index % 2 == 0 {
            hasher.update(current_hash);
            hasher.update(sibling);
        } else {
            hasher.update(sibling);
            hasher.update(current_hash);
        }
        let hash1 = hasher.finalize();
        let hash2 = Sha256::digest(&hash1);
        current_hash.copy_from_slice(&hash2);
        current_index /= 2;
    }

    if current_hash != header.merkle_root_le {
        pc.is_invalid = true;
        ctx.accounts.height_tracker.count =
            ctx.accounts.height_tracker.count.checked_sub(1).unwrap();
        return Ok(());
    }

    let mut is_valid_payout = true;

    if conversion.is_native_to_bitcoin {
        if conversion.status != ConversionStatus::Deposited
            || pc.out_value_sats < conversion.bitcoin_amount
            || pc.out_program != conversion.user_program
        {
            is_valid_payout = false;
        }
    } else {
        let expected_prog = if conversion.ipow_receive_program.is_empty() {
            conversion.user_program.clone()
        } else {
            conversion.ipow_receive_program.clone()
        };

        if pc.out_program != expected_prog || pc.out_value_sats < conversion.bitcoin_amount {
            is_valid_payout = false;
        }
    }

    if !is_valid_payout {
        pc.is_invalid = true;
        ctx.accounts.height_tracker.count =
            ctx.accounts.height_tracker.count.checked_sub(1).unwrap();
        return Ok(());
    }

    let bump = ctx.accounts.global_state.escrow_bump;
    let signer_seeds: &[&[&[u8]]] = &[&[b"escrow", &[bump]]];
    let fee_to_operator = conversion.commit_fee;

    if conversion.is_native_to_bitcoin {
        ctx.accounts.global_state.total_locked_deposits = ctx
            .accounts
            .global_state
            .total_locked_deposits
            .checked_sub(conversion.native_amount)
            .unwrap();

        transfer_from_escrow(
            conversion.native_amount,
            &ctx.accounts.operator.to_account_info(),
            &ctx.accounts.escrow_vault.to_account_info(),
            &ctx.accounts.system_program.to_account_info(),
            signer_seeds,
        )?;
    } else {
        let payout = conversion.native_amount;
        ctx.accounts.global_state.total_reserved_native = ctx
            .accounts
            .global_state
            .total_reserved_native
            .checked_sub(payout)
            .unwrap();

        transfer_from_escrow(
            payout,
            &ctx.accounts.user.to_account_info(),
            &ctx.accounts.escrow_vault.to_account_info(),
            &ctx.accounts.system_program.to_account_info(),
            signer_seeds,
        )?;
    }

    if fee_to_operator > 0 {
        conversion.commit_fee = 0;
        ctx.accounts.global_state.total_held_commit_fees = ctx
            .accounts
            .global_state
            .total_held_commit_fees
            .checked_sub(fee_to_operator)
            .unwrap();

        transfer_from_escrow(
            fee_to_operator,
            &ctx.accounts.operator.to_account_info(),
            &ctx.accounts.escrow_vault.to_account_info(),
            &ctx.accounts.system_program.to_account_info(),
            signer_seeds,
        )?;
    }

    conversion.proof_txid_le = pc.txid_le;
    conversion.proof_block_height = pc.proof_block_height;
    conversion.status = ConversionStatus::Completed;
    conversion.proof_verified = true;
    pc.is_verified = true;
    ctx.accounts.global_state.active_open_conversions = ctx
        .accounts
        .global_state
        .active_open_conversions
        .checked_sub(1)
        .unwrap();

    ctx.accounts.height_tracker.count = ctx.accounts.height_tracker.count.checked_sub(1).unwrap();

    Ok(())
}

#[derive(Accounts)]
pub struct FinalizeProofCached<'info> {
    #[account(mut, seeds = [b"global_state"], bump)]
    pub global_state: Account<'info, GlobalState>,

    /// CHECK: address is fully constrained by the `seeds`/`bump` PDA derivation
    /// above; only used as the source of a System Program lamport transfer via
    /// `invoke_signed` with the escrow's own PDA seeds, which the runtime itself
    /// validates.
    #[account(mut, seeds = [b"escrow"], bump = global_state.escrow_bump)]
    pub escrow_vault: UncheckedAccount<'info>,

    #[account(mut)]
    pub conversion: Account<'info, Conversion>,

    #[account(mut, seeds = [b"proof", conversion.tx_id.to_le_bytes().as_ref()], bump)]
    pub proof_cache: Account<'info, ProofCache>,

    #[account(
        mut,
        seeds = [b"tracker", proof_cache.proof_block_height.to_le_bytes().as_ref()],
        bump
    )]
    pub height_tracker: Account<'info, HeightTracker>,

    pub header: Option<Account<'info, GlobalHeader>>,

    /// CHECK: address is constrained to `global_state.operator` above. Fixed
    /// after being flagged as a real fund-redirection risk: without this, the
    /// handler could pay out `conversion.native_amount` and any outstanding
    /// `commit_fee` to whatever account was supplied here.
    #[account(mut, address = global_state.operator @ IPoWError::Unauthorized)]
    pub operator: UncheckedAccount<'info>,

    /// CHECK: address is constrained to `conversion.user` above.
    #[account(mut, address = conversion.user)]
    pub user: UncheckedAccount<'info>,

    #[account(mut)]
    pub payer: Signer<'info>,
    pub system_program: Program<'info, System>,
}
