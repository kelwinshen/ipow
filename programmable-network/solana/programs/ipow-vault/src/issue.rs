use anchor_lang::prelude::*;
use anchor_spl::token::{Mint, Token, TokenAccount};

use crate::constants::*;
use crate::errors::VaultError;
use crate::state::{Claim, Config, LockMark};
use crate::util::{create_pda, find_lock, now, save, Veth};

/// Issues the vETH of a lock carried by an accepted claim, to its recipient,
/// with the fast fee nobody earned, once per lock, and never for a lock its
/// recipient gave up (sections 11.5, 11.7). For a lock that was attested,
/// only once every attest was settled, none stated the true record, and the
/// lock takes no more attests (D126). Anyone may call.
pub fn handler(ctx: Context<Issue>, _claim_id: u64, lock_id: u64) -> Result<()> {
    let claim = &ctx.accounts.claim;
    require!(claim.accepted, VaultError::NotAccepted);
    let record = find_lock(&claim.records, lock_id).ok_or(VaultError::NotInClaim)?;
    require_keys_eq!(ctx.accounts.to.owner, record.recipient, VaultError::WrongAccount);
    let (address, bump) = Pubkey::find_program_address(&[LOCK_SEED, &lock_id.to_le_bytes()], &crate::ID);
    let mark_info = ctx.accounts.mark.to_account_info();
    require_keys_eq!(mark_info.key(), address, VaultError::WrongAccount);
    let mut mark = if mark_info.owner == &crate::ID && !mark_info.data_is_empty() {
        let m = {
            let data = mark_info.try_borrow_data()?;
            LockMark::try_deserialize(&mut &data[..])?
        };
        require!(!m.issued && !m.given_up, VaultError::AlreadyDone);
        require!(m.settled == m.attests && now()? >= m.first_at + FAST_OPEN_WINDOW, VaultError::NotInOrder);
        m
    } else {
        create_pda(
            &mark_info,
            &ctx.accounts.payer.to_account_info(),
            &ctx.accounts.system_program.to_account_info(),
            8 + LockMark::INIT_SPACE,
            &[LOCK_SEED, &lock_id.to_le_bytes(), &[bump]],
        )?;
        LockMark { lock_id, issued: false, given_up: false, attests: 0, settled: 0, first_at: 0, last_attest: 0, bump }
    };
    mark.issued = true;
    save(&mark_info, &mark)?;
    let a = &ctx.accounts;
    let veth = Veth {
        config: &a.config.to_account_info(),
        config_bump: a.config.bump,
        mint: &a.mint.to_account_info(),
        holding: &a.holding.to_account_info(),
        token_program: &a.token_program,
    };
    veth.mint_to(&a.to.to_account_info(), record.value()?)
}

#[derive(Accounts)]
#[instruction(claim_id: u64, lock_id: u64)]
pub struct Issue<'info> {
    #[account(seeds = [CLAIM_SEED, &claim_id.to_le_bytes()], bump = claim.bump)]
    pub claim: Account<'info, Claim>,
    /// CHECK: the lock's mark at ["lock", lock_id]; checked, and created if
    /// missing, in the handler.
    #[account(mut)]
    pub mark: UncheckedAccount<'info>,
    #[account(seeds = [CONFIG_SEED], bump = config.bump)]
    pub config: Account<'info, Config>,
    #[account(mut, seeds = [MINT_SEED], bump)]
    pub mint: Account<'info, Mint>,
    /// CHECK: the vault's vETH account; only its address is used here.
    #[account(seeds = [HOLDING_SEED], bump)]
    pub holding: UncheckedAccount<'info>,
    #[account(mut, token::mint = mint)]
    pub to: Account<'info, TokenAccount>,
    #[account(mut)]
    pub payer: Signer<'info>,
    pub token_program: Program<'info, Token>,
    pub system_program: Program<'info, System>,
}
