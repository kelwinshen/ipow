use anchor_lang::prelude::*;
use ipow::state::GlobalHeader;

use crate::anchor_verify::verify_and_advance;
use crate::constants::*;
use crate::errors::FactoryError;
use crate::state::{AnchorStatus, Burn, FactoryConfig, Party, PartyKind, Pending, ProcessedAnchor};
use crate::utils::slash_party;
use crate::statement::Statement;
use crate::utils::{slash, transfer_from_pda};

/// Permissionless. Processes the next anchor on a party's statement chain
/// (DESIGN_V2 §6.2–6.6): verifies the Bitcoin tx, advances the pointer,
/// then judges/acts by kind — always in the same instruction so an anchor
/// can never be "seen but not judged":
///
/// - MINT: judged on its *Solana* predicate (pending lock exists, approved
///   for this Ethereum lock id, amounts/deadline match). True → queued for
///   `exercise_mint`. False → operator slashed, compensation to the named
///   user. The *Ethereum* predicate is judged on Ethereum.
/// - RELEASE: judged here (the burn is a Solana fact). True → burn marked
///   claimed. False → operator slashed to insurance.
/// - VETO (auditors only): dead-veto pauses the target operator; a veto on
///   a queued MINT holds it; a veto on a judged RELEASE is itself judged
///   against that verdict.
/// - CANCEL: advances only (acted on by Ethereum).
#[allow(clippy::too_many_arguments)]
pub fn handler(
    ctx: Context<ProcessAnchor>,
    txid_le: [u8; 32],
    statement: Vec<u8>,
    tx_raw: Vec<u8>,
    proof_block_height: u64,
    branch_le: Vec<[u8; 32]>,
    index: u64,
) -> Result<()> {
    let now = Clock::get()?.unix_timestamp;
    let party = &mut ctx.accounts.party;
    let parsed = verify_and_advance(
        party,
        txid_le,
        &tx_raw,
        &ctx.accounts.header,
        proof_block_height,
        &branch_le,
        index,
    )?;
    let payload = parsed.payload.ok_or(FactoryError::BadAnchorPayload)?;
    let stmt = Statement::decode(&statement)?;
    require!(payload.kind == stmt.kind(), FactoryError::KindMismatch);
    require!(
        payload.statement_hash == Statement::hash(&statement),
        FactoryError::StatementHashMismatch
    );

    let pa = &mut ctx.accounts.processed;
    pa.party_id = party.party_id;
    pa.txid_le = txid_le;
    pa.kind = stmt.kind();
    pa.statement_hash = payload.statement_hash;
    pa.block_height = proof_block_height;
    pa.processed_at = now;
    pa.challenge_until = now + ctx.accounts.config.params.t_challenge_secs;
    pa.held = false;
    pa.attested_by = [0u8; 32];
    pa.escrow = 0;
    pa.settled = false;

    let params = ctx.accounts.config.params;
    let escrow_bump = ctx.accounts.config.bond_escrow_bump;
    let escrow_seeds: &[&[u8]] = &[b"bond_escrow", &[escrow_bump]];
    let escrow_signer: &[&[&[u8]]] = &[escrow_seeds];

    match stmt {
        Statement::Mint {
            eth_lock_id,
            sol_user,
            nonce,
            units,
            deadline,
        } => {
            require!(party.kind == PartyKind::Operator, FactoryError::NotOperator);
            pa.eth_lock_id = eth_lock_id;
            pa.sol_user = sol_user;
            pa.nonce = nonce;
            pa.units = units;
            pa.deadline = deadline;

            let (expected_pending, _) = Pubkey::find_program_address(
                &[b"pending", sol_user.as_ref(), &nonce.to_le_bytes()],
                ctx.program_id,
            );
            // A slot already queued under another party may be taken over
            // only if that party has been retired — the submitter passes
            // the prior party account to prove it.
            let prior_dead = |queued_by: [u8; 32]| -> bool {
                match ctx.accounts.prior_party.as_ref() {
                    Some(pp) => pp.party_id == queued_by && pp.dead,
                    None => false,
                }
            };
            let predicate_true = match ctx.accounts.pending.as_ref() {
                Some(p) if p.key() == expected_pending => {
                    p.approved
                        && (!p.queued || (p.queued_by != party.party_id && prior_dead(p.queued_by)))
                        && p.eth_lock_id == eth_lock_id
                        && p.units == units
                        && p.deadline == deadline
                }
                _ => false,
            };
            if predicate_true {
                let p = ctx.accounts.pending.as_mut().unwrap();
                p.queued = true;
                p.queued_by = party.party_id;
                pa.status = AnchorStatus::Queued;
            } else {
                let victim = ctx
                    .accounts
                    .pending_user
                    .as_ref()
                    .ok_or(FactoryError::MissingAccount)?;
                require!(victim.key() == sol_user, FactoryError::AccountMismatch);
                let amount = units
                    .checked_mul(params.comp_lamports_per_unit)
                    .ok_or(FactoryError::Overflow)?;
                slash(
                    party,
                    amount,
                    params.bounty_bps,
                    &ctx.accounts.bond_escrow.to_account_info(),
                    &ctx.accounts.submitter.to_account_info(),
                    &victim.to_account_info(),
                    &ctx.accounts.system_program.to_account_info(),
                    escrow_signer,
                )?;
                pa.status = AnchorStatus::Slashed;
            }
        }
        Statement::Release {
            eth_lock_id: _,
            burn_id,
            to_eth,
            units,
        } => {
            require!(party.kind == PartyKind::Operator, FactoryError::NotOperator);
            let (expected_burn, _) =
                Pubkey::find_program_address(&[b"burn", &burn_id.to_le_bytes()], ctx.program_id);
            let predicate_true = match ctx.accounts.burn.as_ref() {
                Some(b) if b.key() == expected_burn => {
                    !b.claimed && b.units == units && b.to_eth == to_eth
                }
                _ => false,
            };
            if predicate_true {
                ctx.accounts.burn.as_mut().unwrap().claimed = true;
                pa.status = AnchorStatus::Exercised;
            } else {
                let amount = units
                    .checked_mul(params.comp_lamports_per_unit)
                    .ok_or(FactoryError::Overflow)?;
                slash(
                    party,
                    amount,
                    params.bounty_bps,
                    &ctx.accounts.bond_escrow.to_account_info(),
                    &ctx.accounts.submitter.to_account_info(),
                    &ctx.accounts.insurance.to_account_info(),
                    &ctx.accounts.system_program.to_account_info(),
                    escrow_signer,
                )?;
                pa.status = AnchorStatus::Slashed;
                // v3 fan-out: an ATTEST of this release that Solana already
                // recorded (processed before the release itself) is a lie too.
                // (Attests processed after this verdict are slashed on arrival.)
            }
        }
        Statement::Veto {
            target_party_id,
            target_txid_le,
        } => {
            require!(!party.dead, FactoryError::PartyDead);
            let target = ctx
                .accounts
                .target_party
                .as_mut()
                .ok_or(FactoryError::MissingAccount)?;
            require!(target.party_id == target_party_id, FactoryError::AccountMismatch);
            if target_txid_le == [0u8; 32] {
                // Dead-veto: pause the operator until an ALIVE clears it; judged on Ethereum.
                target.paused_until = PAUSED_FOREVER;
                pa.status = AnchorStatus::Exercised;
            } else {
                let ta = ctx
                    .accounts
                    .target_anchor
                    .as_mut()
                    .ok_or(FactoryError::TargetNotProcessed)?;
                require!(
                    ta.txid_le == target_txid_le && ta.party_id == target_party_id,
                    FactoryError::AccountMismatch
                );
                match ta.kind {
                    KIND_MINT => {
                        // Judged on Ethereum. Here: hold, with no expiry (§7.3).
                        require!(!ta.settled, FactoryError::AlreadySettled);
                        ta.held = true;
                        pa.status = AnchorStatus::Exercised;
                    }
                    KIND_RELEASE => match ta.status {
                        AnchorStatus::Exercised => {
                            // The release was true; the veto is a lie about Solana.
                            slash(
                                party,
                                params.veto_slash_lamports,
                                params.bounty_bps,
                                &ctx.accounts.bond_escrow.to_account_info(),
                                &ctx.accounts.submitter.to_account_info(),
                                &ctx.accounts.insurance.to_account_info(),
                                &ctx.accounts.system_program.to_account_info(),
                                escrow_signer,
                            )?;
                            pa.status = AnchorStatus::Slashed;
                        }
                        AnchorStatus::Slashed => {
                            let pool = ctx.accounts.reward_pool.to_account_info();
                            let reward = params.veto_reward_lamports.min(pool.lamports());
                            let pool_bump = ctx.bumps.reward_pool;
                            let pool_seeds: &[&[u8]] = &[b"rewards", &[pool_bump]];
                            transfer_from_pda(
                                reward,
                                &pool,
                                &ctx.accounts.party_owner.to_account_info(),
                                &ctx.accounts.system_program.to_account_info(),
                                &[pool_seeds],
                            )?;
                            pa.status = AnchorStatus::Exercised;
                        }
                        _ => return Err(error!(FactoryError::BadAnchorState)),
                    },
                    _ => return Err(error!(FactoryError::BadAnchorState)),
                }
            }
        }
        Statement::Attest { target_txid_le } => {
            require!(!party.dead, FactoryError::PartyDead);
            let ta = ctx
                .accounts
                .target_anchor
                .as_mut()
                .ok_or(FactoryError::TargetNotProcessed)?;
            require!(ta.txid_le == target_txid_le, FactoryError::AccountMismatch);
            match ta.kind {
                KIND_MINT => {
                    // Judged on Ethereum. Here: reserve escrow from the attester's
                    // bond and allow immediate exercise (§7.3 fast path). A
                    // redundant attest (already attested, already exercised,
                    // held, settled, or past the window) is a harmless no-op
                    // that only advances the attester's chain — an error here
                    // would wedge that chain behind the duplicate.
                    let redundant = ta.status != AnchorStatus::Queued
                        || ta.held
                        || ta.settled
                        || ta.attested_by != [0u8; 32]
                        || now >= ta.challenge_until;
                    if redundant {
                        pa.status = AnchorStatus::Exercised;
                        return Ok(());
                    }
                    let escrow = ta
                        .units
                        .checked_mul(params.comp_lamports_per_unit)
                        .ok_or(FactoryError::Overflow)?;
                    require!(party.bond >= escrow, FactoryError::InsufficientBond);
                    party.bond -= escrow;
                    ta.escrow = escrow;
                    ta.attested_by = party.party_id;
                    pa.status = AnchorStatus::Exercised;
                }
                KIND_RELEASE => match ta.status {
                    // Judged here: attesting a release Solana already found false is a lie.
                    AnchorStatus::Slashed => {
                        slash_party(
                            party,
                            ta.units.checked_mul(params.comp_lamports_per_unit).ok_or(FactoryError::Overflow)?,
                            params.bounty_bps,
                            &ctx.accounts.bond_escrow.to_account_info(),
                            &ctx.accounts.submitter.to_account_info(),
                            &ctx.accounts.insurance.to_account_info(),
                            &ctx.accounts.system_program.to_account_info(),
                            escrow_signer,
                        )?;
                        pa.status = AnchorStatus::Slashed;
                    }
                    AnchorStatus::Exercised => {
                        pa.status = AnchorStatus::Exercised;
                    }
                    _ => return Err(error!(FactoryError::BadAnchorState)),
                },
                _ => return Err(error!(FactoryError::BadAnchorState)),
            }
        }
        Statement::Clear { target_txid_le } => {
            require!(!party.dead, FactoryError::PartyDead);
            let ta = ctx
                .accounts
                .target_anchor
                .as_mut()
                .ok_or(FactoryError::TargetNotProcessed)?;
            require!(ta.txid_le == target_txid_le, FactoryError::AccountMismatch);
            match ta.kind {
                KIND_MINT => {
                    // Judged on Ethereum. Here: lift the hold.
                    require!(!ta.settled, FactoryError::AlreadySettled);
                    ta.held = false;
                    pa.status = AnchorStatus::Exercised;
                }
                KIND_RELEASE => match ta.status {
                    AnchorStatus::Slashed => {
                        slash_party(
                            party,
                            params.veto_slash_lamports,
                            params.bounty_bps,
                            &ctx.accounts.bond_escrow.to_account_info(),
                            &ctx.accounts.submitter.to_account_info(),
                            &ctx.accounts.insurance.to_account_info(),
                            &ctx.accounts.system_program.to_account_info(),
                            escrow_signer,
                        )?;
                        pa.status = AnchorStatus::Slashed;
                    }
                    AnchorStatus::Exercised => {
                        pa.status = AnchorStatus::Exercised;
                    }
                    _ => return Err(error!(FactoryError::BadAnchorState)),
                },
                _ => return Err(error!(FactoryError::BadAnchorState)),
            }
        }
        Statement::Alive { target_party_id } => {
            // Judged on Ethereum. Here: un-pause.
            let target = ctx
                .accounts
                .target_party
                .as_mut()
                .ok_or(FactoryError::MissingAccount)?;
            require!(target.party_id == target_party_id, FactoryError::AccountMismatch);
            target.paused_until = 0;
            pa.status = AnchorStatus::Exercised;
        }
        Statement::Cancel { eth_lock_id } => {
            require!(party.kind == PartyKind::Operator, FactoryError::NotOperator);
            pa.eth_lock_id = eth_lock_id;
            pa.status = AnchorStatus::Exercised;
        }
    }
    Ok(())
}

#[derive(Accounts)]
#[instruction(txid_le: [u8; 32])]
pub struct ProcessAnchor<'info> {
    #[account(mut, seeds = [b"config"], bump)]
    pub config: Box<Account<'info, FactoryConfig>>,
    #[account(mut)]
    pub party: Box<Account<'info, Party>>,
    /// CHECK: only used as the veto-reward destination, constrained to `party.owner`.
    #[account(mut, address = party.owner)]
    pub party_owner: UncheckedAccount<'info>,
    #[account(init, payer = submitter, space = 8 + ProcessedAnchor::INIT_SPACE, seeds = [b"anchor", txid_le.as_ref()], bump)]
    pub processed: Box<Account<'info, ProcessedAnchor>>,
    pub header: Box<Account<'info, GlobalHeader>>,
    #[account(mut, seeds = [b"bond_escrow"], bump = config.bond_escrow_bump)]
    pub bond_escrow: SystemAccount<'info>,
    #[account(mut, seeds = [b"insurance"], bump = config.insurance_bump)]
    pub insurance: SystemAccount<'info>,
    /// Governance-funded pool that pays veto rewards (`fund_rewards`).
    #[account(mut, seeds = [b"rewards"], bump)]
    pub reward_pool: SystemAccount<'info>,
    #[account(mut)]
    pub submitter: Signer<'info>,
    pub system_program: Program<'info, System>,

    // --- kind-specific, `None` = this program's ID in the slot ---
    /// MINT: the pending lock named by the statement (address re-derived in the handler).
    #[account(mut)]
    pub pending: Option<Box<Account<'info, Pending>>>,
    /// CHECK: MINT compensation destination on a false statement; must equal the statement's `sol_user`.
    #[account(mut)]
    pub pending_user: Option<UncheckedAccount<'info>>,
    /// RELEASE: the burn named by the statement (address re-derived in the handler).
    #[account(mut)]
    pub burn: Option<Box<Account<'info, Burn>>>,
    /// VETO: the operator being vetoed.
    #[account(mut)]
    pub target_party: Option<Box<Account<'info, Party>>>,
    /// VETO: the vetoed anchor (absent for a dead-veto).
    #[account(mut)]
    pub target_anchor: Option<Box<Account<'info, ProcessedAnchor>>>,
    /// MINT re-anchor: the retired party that previously queued this pending slot.
    pub prior_party: Option<Box<Account<'info, Party>>>,
}
