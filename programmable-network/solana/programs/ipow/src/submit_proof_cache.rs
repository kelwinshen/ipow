use anchor_lang::prelude::*;
use sha2::{Digest, Sha256};

use crate::bitcoin::parse_output_at;
use crate::constants::PROOF_BLOCKS_WINDOW;
use crate::errors::IPoWError;
use crate::state::{
    Conversion, ConversionStatus, GlobalHeader, GlobalState, HeightTracker, ProofCache,
};
use crate::utils::transfer_from_escrow;

pub fn handler(
    ctx: Context<SubmitProofCache>,
    tx_raw: Vec<u8>,
    vout_index: u64,
    proof_block_height: u64,
    branch_le: Vec<[u8; 32]>,
    index: u64,
) -> Result<()> {
    let conversion = &mut ctx.accounts.conversion;
    let proof_cache = &mut ctx.accounts.proof_cache;

    require!(conversion.window_started, IPoWError::NoHeadersYet);
    require!(!conversion.proof_verified, IPoWError::AlreadyVerified);

    let window_end = conversion
        .window_start_height
        .checked_add(PROOF_BLOCKS_WINDOW)
        .unwrap()
        .checked_sub(1)
        .unwrap();

    require!(
        proof_block_height >= conversion.window_start_height && proof_block_height <= window_end,
        IPoWError::IncorrectWindow
    );

    if conversion.is_native_to_bitcoin {
        require!(
            ctx.accounts.signer.key() == ctx.accounts.global_state.operator,
            IPoWError::Unauthorized
        );
    } else {
        require!(
            ctx.accounts.signer.key() == conversion.user,
            IPoWError::Unauthorized
        );
    }

    let (parsed_value_sats, parsed_program) = parse_output_at(&tx_raw, vout_index as usize)?;

    let hash1 = Sha256::digest(&tx_raw);
    let hash2 = Sha256::digest(&hash1);
    let mut txid_le = [0u8; 32];
    txid_le.copy_from_slice(&hash2);

    ctx.accounts.height_tracker.count = ctx.accounts.height_tracker.count.checked_add(1).unwrap();

    if proof_cache.is_set {
        proof_cache.attempts = proof_cache.attempts.saturating_add(1);
    } else {
        proof_cache.attempts = 1;
    }

    proof_cache.tx_id = conversion.tx_id;
    proof_cache.is_set = true;
    proof_cache.is_verified = false;
    proof_cache.is_invalid = false;
    proof_cache.txid_le = txid_le;
    proof_cache.proof_block_height = proof_block_height;
    proof_cache.branch_le = branch_le.clone();
    proof_cache.merkle_index = index;
    proof_cache.out_value_sats = parsed_value_sats;
    proof_cache.out_program = parsed_program;
    proof_cache.out_set = true;

    if let Some(ref header) = ctx.accounts.header {
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
            let hash1 = hasher.finalize();
            let hash2 = Sha256::digest(&hash1);
            current_hash.copy_from_slice(&hash2);
            current_index /= 2;
        }
        require!(
            current_hash == header.merkle_root_le,
            IPoWError::InvalidHeader
        );

        let bump = ctx.accounts.global_state.escrow_bump;
        let signer_seeds: &[&[&[u8]]] = &[&[b"escrow", &[bump]]];
        let fee_to_operator = conversion.commit_fee;

        if conversion.is_native_to_bitcoin {
            require!(
                conversion.status == ConversionStatus::Deposited,
                IPoWError::BadState
            );
            require!(
                proof_cache.out_value_sats >= conversion.bitcoin_amount,
                IPoWError::IncorrectValue
            );
            require!(
                proof_cache.out_program == conversion.user_program,
                IPoWError::BadBitcoinProgram
            );

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
            let expected_prog = if conversion.ipow_receive_program.is_empty() {
                conversion.user_program.clone()
            } else {
                conversion.ipow_receive_program.clone()
            };

            require!(
                proof_cache.out_program == expected_prog,
                IPoWError::BadBitcoinProgram
            );
            require!(
                proof_cache.out_value_sats >= conversion.bitcoin_amount,
                IPoWError::IncorrectValue
            );

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

        conversion.proof_txid_le = proof_cache.txid_le;
        conversion.proof_block_height = proof_cache.proof_block_height;
        conversion.status = ConversionStatus::Completed;
        conversion.proof_verified = true;
        proof_cache.is_verified = true;
        ctx.accounts.global_state.active_open_conversions = ctx
            .accounts
            .global_state
            .active_open_conversions
            .checked_sub(1)
            .unwrap();

        ctx.accounts.height_tracker.count =
            ctx.accounts.height_tracker.count.checked_sub(1).unwrap();
    }

    Ok(())
}

#[derive(Accounts)]
#[instruction(tx_raw: Vec<u8>, vout_index: u64, proof_block_height: u64)]
pub struct SubmitProofCache<'info> {
    #[account(mut, seeds = [b"global_state"], bump)]
    pub global_state: Account<'info, GlobalState>,

    #[account(mut, seeds = [b"escrow"], bump = global_state.escrow_bump)]
    pub escrow_vault: SystemAccount<'info>,

    #[account(mut)]
    pub conversion: Account<'info, Conversion>,

    #[account(
        init_if_needed,
        payer = signer,
        space = 8 + ProofCache::MAX_SPACE,
        seeds = [b"proof", conversion.tx_id.to_le_bytes().as_ref()],
        bump
    )]
    pub proof_cache: Account<'info, ProofCache>,

    #[account(
        init_if_needed,
        payer = signer,
        space = 8 + HeightTracker::INIT_SPACE,
        seeds = [b"tracker", proof_block_height.to_le_bytes().as_ref()],
        bump
    )]
    pub height_tracker: Account<'info, HeightTracker>,

    pub header: Option<Account<'info, GlobalHeader>>,

    /// CHECK: address is constrained to `global_state.operator` above. Fixed
    /// after being flagged as a real fund-redirection risk: without this, the
    /// handler would pay `conversion.native_amount` (native-to-bitcoin
    /// completion path) and any outstanding `commit_fee` to whatever account
    /// was supplied here, with no check tying it to the protocol's actual
    /// operator.
    #[account(mut, address = global_state.operator @ IPoWError::Unauthorized)]
    pub operator: UncheckedAccount<'info>,

    /// CHECK: address is constrained to `conversion.user` above.
    #[account(mut, address = conversion.user)]
    pub user: UncheckedAccount<'info>,

    #[account(mut)]
    pub signer: Signer<'info>,
    pub system_program: Program<'info, System>,
}
