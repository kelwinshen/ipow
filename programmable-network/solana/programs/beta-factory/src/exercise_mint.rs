use anchor_lang::prelude::*;
use anchor_spl::token::{self, Mint, MintTo, Token, TokenAccount};

use crate::constants::*;
use crate::errors::FactoryError;
use crate::state::{AnchorStatus, FactoryConfig, Party, Pending, ProcessedAnchor};
use crate::utils::transfer_from_pda;

/// Permissionless. Mints once *every* remote component of the
/// composition is ready (DESIGN_V2 §8.5, generalizing §7.3's single ETH
/// leg): each needs its own MINT anchor `Queued`, un-held, and either
/// ATTESTed or past its own challenge window — a slow component blocks
/// the whole mint, but never blocks the others from being individually
/// attested while they wait on it. Every local (Solana) leg needs
/// nothing further; its funds are already the vault's, from `lock_sol`
/// (§8.10: Solana may hold more than one local token). A composition
/// with no remote legs at all needs no operator and no `party` — it
/// mints as soon as it's locked and approved, since nothing remains to
/// relay. The pending mint cannot expire while any remote component is
/// queued (`queued_by != 0`), so a user is never stranded with FINAL
/// remote locks and no BETA.
pub fn handler(ctx: Context<ExerciseMint>) -> Result<()> {
    let now = Clock::get()?.unix_timestamp;
    let p = &ctx.accounts.pending;
    require!(p.approved, FactoryError::BadAnchorState);
    require!(!ctx.accounts.config.paused, FactoryError::Paused);

    // A composition with no remote legs at all (§8.10: Solana may now
    // lock more than one local token by itself) needs no operator —
    // every leg was already verified directly by `lock_sol`'s own
    // transfers, so it mints as soon as it's locked and approved. Only
    // when there's at least one remote leg to relay does a carrier
    // (`party`) and its anchored, attested-or-windowed remotes matter.
    let remote_count = p.remote_count();
    if remote_count > 0 {
        require!(p.queued_by != [0u8; 32], FactoryError::BadAnchorState);
        // Liveness is checked against the passed-in party directly,
        // exactly like §7.3's single leg: a dead or paused carrier blocks
        // exercise even before we look at whether it still owns this
        // pending slot (so a retired operator's stale anchor reports
        // `PartyDead`, not a less informative account mismatch).
        let party = ctx.accounts.party.as_deref().ok_or(FactoryError::MissingAccount)?;
        require!(!party.dead, FactoryError::PartyDead);
        require!(party.paused_until == 0 || now >= party.paused_until, FactoryError::Paused);

        {
            let remotes = [
                ctx.accounts.remote_0.as_deref(),
                ctx.accounts.remote_1.as_deref(),
                ctx.accounts.remote_2.as_deref(),
                ctx.accounts.remote_3.as_deref(),
                ctx.accounts.remote_4.as_deref(),
                ctx.accounts.remote_5.as_deref(),
                ctx.accounts.remote_6.as_deref(),
            ];
            for i in 0..remote_count {
                let ra = remotes[i].ok_or(FactoryError::MissingAccount)?;
                require!(ra.txid_le == p.remote_anchor_txid[i], FactoryError::AccountMismatch);
                require!(ra.party_id == party.party_id, FactoryError::AccountMismatch);
                require!(
                    ra.kind == KIND_MINT
                        && ra.status == AnchorStatus::Queued
                        && ra.composition_id == p.composition_id
                        && ra.component_index as usize == i
                        && ra.sol_user == p.user
                        && ra.nonce == p.nonce,
                    FactoryError::AccountMismatch
                );
                require!(!ra.held, FactoryError::Held);
                require!(ra.attested_by != [0u8; 32] || now >= ra.challenge_until, FactoryError::NotAttested);
            }
        }
        // This party is still the pending's current carrier — implied by
        // every remote's cached txid still matching (a takeover resets
        // them all, DESIGN_V2 §8.3), checked again explicitly since it's
        // cheap.
        require!(party.party_id == p.queued_by, FactoryError::BadAnchorState);
        if remote_count > 0 {
            ctx.accounts.remote_0.as_deref_mut().unwrap().status = AnchorStatus::Exercised;
        }
        if remote_count > 1 {
            ctx.accounts.remote_1.as_deref_mut().unwrap().status = AnchorStatus::Exercised;
        }
        if remote_count > 2 {
            ctx.accounts.remote_2.as_deref_mut().unwrap().status = AnchorStatus::Exercised;
        }
        if remote_count > 3 {
            ctx.accounts.remote_3.as_deref_mut().unwrap().status = AnchorStatus::Exercised;
        }
        if remote_count > 4 {
            ctx.accounts.remote_4.as_deref_mut().unwrap().status = AnchorStatus::Exercised;
        }
        if remote_count > 5 {
            ctx.accounts.remote_5.as_deref_mut().unwrap().status = AnchorStatus::Exercised;
        }
        if remote_count > 6 {
            ctx.accounts.remote_6.as_deref_mut().unwrap().status = AnchorStatus::Exercised;
        }
    }
    let c = &mut ctx.accounts.config;

    // Fee refund: paid out immediately to the first attester of any
    // component (process_anchor's Attest/KIND_MINT branch already zeroed
    // it there); still nonzero here only if nobody ever attested anything.
    if p.attest_fee > 0 {
        let fees_bump = ctx.bumps.fees;
        let fees_seeds: &[&[u8]] = &[b"fees", &[fees_bump]];
        transfer_from_pda(
            p.attest_fee,
            &ctx.accounts.fees.to_account_info(),
            &ctx.accounts.user.to_account_info(),
            &ctx.accounts.system_program.to_account_info(),
            &[fees_seeds],
        )?;
    }

    let composition = &ctx.accounts.composition;
    require!(composition.id == p.composition_id, FactoryError::AccountMismatch);
    require!(composition.components.iter().any(|c| c.network_id == 0), FactoryError::InvalidParams);
    let units = p.units;
    // Only the native-SOL local leg (if the composition has one — it may
    // instead, or additionally, lock SPL tokens, §8.10) moves through
    // `vault`'s lamport balance, so only it feeds these global tallies;
    // an SPL local leg's "reserve" is just its own vault ATA balance,
    // already moved at `lock_sol`, needing no separate counter here.
    let native = composition.components.iter().find(|c| c.network_id == 0 && c.token_id == [0u8; 32]);
    let lamports = match native {
        Some(n) => units.checked_mul(n.amount_per_unit).ok_or(FactoryError::Overflow)?,
        None => 0,
    };
    c.pending_lamports = c.pending_lamports.checked_sub(lamports).ok_or(FactoryError::Overflow)?;
    c.reserve_lamports = c.reserve_lamports.checked_add(lamports).ok_or(FactoryError::Overflow)?;
    c.eth_claims_units = c.eth_claims_units.checked_add(units).ok_or(FactoryError::Overflow)?;

    let bump = c.mint_authority_bump;
    let seeds: &[&[u8]] = &[b"mint_authority", &[bump]];
    token::mint_to(
        CpiContext::new_with_signer(
            ctx.accounts.token_program.key(),
            MintTo {
                mint: ctx.accounts.beta_mint.to_account_info(),
                to: ctx.accounts.user_beta.to_account_info(),
                authority: ctx.accounts.mint_authority.to_account_info(),
            },
            &[seeds],
        ),
        units.checked_mul(UNIT).ok_or(FactoryError::Overflow)?,
    )?;
    Ok(())
}

#[derive(Accounts)]
pub struct ExerciseMint<'info> {
    #[account(mut, seeds = [b"config"], bump)]
    pub config: Box<Account<'info, FactoryConfig>>,
    #[account(mut, close = user, seeds = [b"pending", pending.user.as_ref(), pending.nonce.to_le_bytes().as_ref()], bump)]
    pub pending: Box<Account<'info, Pending>>,
    pub composition: Box<Account<'info, crate::state::Composition>>,
    /// Required only when the composition has at least one remote leg.
    pub party: Option<Box<Account<'info, Party>>>,
    /// CHECK: rent destination, constrained to the pending mint's user.
    #[account(mut, address = pending.user)]
    pub user: UncheckedAccount<'info>,
    #[account(mut, associated_token::mint = beta_mint, associated_token::authority = user)]
    pub user_beta: Box<Account<'info, TokenAccount>>,
    #[account(mut, address = config.beta_mint)]
    pub beta_mint: Box<Account<'info, Mint>>,
    /// CHECK: PDA signer.
    #[account(seeds = [b"mint_authority"], bump = config.mint_authority_bump)]
    pub mint_authority: UncheckedAccount<'info>,
    /// Holds posted acceleration fees; refunded from here if nobody attested.
    #[account(mut, seeds = [b"fees"], bump)]
    pub fees: SystemAccount<'info>,
    pub token_program: Program<'info, Token>,
    pub system_program: Program<'info, System>,

    // --- one slot per possible remote component (MAX_REMOTE_COMPONENTS —
    // §8.12: MAX_COMPONENTS(8) - the one mandatory local leg) ---
    #[account(mut)]
    pub remote_0: Option<Box<Account<'info, ProcessedAnchor>>>,
    #[account(mut)]
    pub remote_1: Option<Box<Account<'info, ProcessedAnchor>>>,
    #[account(mut)]
    pub remote_2: Option<Box<Account<'info, ProcessedAnchor>>>,
    #[account(mut)]
    pub remote_3: Option<Box<Account<'info, ProcessedAnchor>>>,
    #[account(mut)]
    pub remote_4: Option<Box<Account<'info, ProcessedAnchor>>>,
    #[account(mut)]
    pub remote_5: Option<Box<Account<'info, ProcessedAnchor>>>,
    #[account(mut)]
    pub remote_6: Option<Box<Account<'info, ProcessedAnchor>>>,
}
