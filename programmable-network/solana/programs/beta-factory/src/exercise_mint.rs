use anchor_lang::prelude::*;
use anchor_spl::token::{self, Mint, MintTo, Token, TokenAccount};

use crate::constants::*;
use crate::errors::FactoryError;
use crate::state::{AnchorStatus, FactoryConfig, Party, Pending, ProcessedAnchor};

/// Permissionless. Executes a queued MINT (DESIGN_V2 §7.3): immediately
/// once a bonded party has ATTESTed it (escrow reserved), otherwise after
/// the challenge window — in both cases only while no VETO stands, the
/// operator is live and un-paused, and the factory isn't paused. The
/// pending lock cannot expire while queued, so a user is never stranded
/// with a FINAL ETH lock and no BETA.
pub fn handler(ctx: Context<ExerciseMint>, _txid_le: [u8; 32]) -> Result<()> {
    let now = Clock::get()?.unix_timestamp;
    let pa = &mut ctx.accounts.processed;
    require!(pa.kind == KIND_MINT && pa.status == AnchorStatus::Queued, FactoryError::BadAnchorState);
    let party = &ctx.accounts.party;
    require!(party.party_id == pa.party_id, FactoryError::AccountMismatch);
    require!(!party.dead, FactoryError::PartyDead);
    require!(party.paused_until == 0 || now >= party.paused_until, FactoryError::Paused);
    require!(!pa.held, FactoryError::Held);
    // v3 (§7.3): early only when attested (escrow reserved), else after the window.
    require!(pa.attested_by != [0u8; 32] || now >= pa.challenge_until, FactoryError::NotAttested);
    let c = &mut ctx.accounts.config;
    require!(!c.paused, FactoryError::Paused);

    let p = &ctx.accounts.pending;
    require!(p.user == pa.sol_user && p.nonce == pa.nonce && p.queued, FactoryError::AccountMismatch);
    require!(p.queued_by == pa.party_id, FactoryError::BadAnchorState);
    let lamports = pa.units.checked_mul(c.params.sol_per_unit).ok_or(FactoryError::Overflow)?;
    c.pending_lamports = c.pending_lamports.checked_sub(lamports).ok_or(FactoryError::Overflow)?;
    c.reserve_lamports = c.reserve_lamports.checked_add(lamports).ok_or(FactoryError::Overflow)?;
    c.eth_claims_units = c.eth_claims_units.checked_add(pa.units).ok_or(FactoryError::Overflow)?;

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
        pa.units.checked_mul(UNIT).ok_or(FactoryError::Overflow)?,
    )?;
    pa.status = AnchorStatus::Exercised;
    Ok(())
}

#[derive(Accounts)]
#[instruction(txid_le: [u8; 32])]
pub struct ExerciseMint<'info> {
    #[account(mut, seeds = [b"config"], bump)]
    pub config: Box<Account<'info, FactoryConfig>>,
    #[account(mut, seeds = [b"anchor", txid_le.as_ref()], bump)]
    pub processed: Box<Account<'info, ProcessedAnchor>>,
    pub party: Box<Account<'info, Party>>,
    #[account(mut, close = user, seeds = [b"pending", processed.sol_user.as_ref(), processed.nonce.to_le_bytes().as_ref()], bump)]
    pub pending: Box<Account<'info, Pending>>,
    /// CHECK: rent destination, constrained to the statement's user.
    #[account(mut, address = processed.sol_user)]
    pub user: UncheckedAccount<'info>,
    #[account(mut, associated_token::mint = beta_mint, associated_token::authority = user)]
    pub user_beta: Box<Account<'info, TokenAccount>>,
    #[account(mut, address = config.beta_mint)]
    pub beta_mint: Box<Account<'info, Mint>>,
    /// CHECK: PDA signer.
    #[account(seeds = [b"mint_authority"], bump = config.mint_authority_bump)]
    pub mint_authority: UncheckedAccount<'info>,
    pub token_program: Program<'info, Token>,
}
