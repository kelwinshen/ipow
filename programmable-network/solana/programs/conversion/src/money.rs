//! Moving the swapped value: SOL is held in the swap's own account; a token
//! in the swap's associated token account. Both token programs are accepted.

use anchor_lang::prelude::*;
use anchor_spl::associated_token::{self, get_associated_token_address_with_program_id, AssociatedToken};
use anchor_spl::token_interface::{self, Mint, TokenAccount, TokenInterface, TransferChecked};

use crate::constants::SWAP_SEED;
use crate::errors::ConversionError;
use crate::state::Swap;

/// The accounts a token swap needs; all absent for SOL.
pub struct Token<'a, 'info> {
    pub mint: &'a Option<InterfaceAccount<'info, Mint>>,
    /// The swap's associated token account.
    pub escrow: &'a Option<UncheckedAccount<'info>>,
    /// The other side: where the token comes from or goes to.
    pub other: &'a Option<InterfaceAccount<'info, TokenAccount>>,
    pub token_program: &'a Option<Interface<'info, TokenInterface>>,
    pub associated_token_program: &'a Option<Program<'info, AssociatedToken>>,
}

struct Ready<'a, 'info> {
    mint: &'a InterfaceAccount<'info, Mint>,
    escrow: &'a UncheckedAccount<'info>,
    other: &'a InterfaceAccount<'info, TokenAccount>,
    token_program: &'a Interface<'info, TokenInterface>,
}

impl<'a, 'info> Token<'a, 'info> {
    /// The token accounts, checked against the swap's mint and address.
    fn ready(&self, swap: &AccountInfo<'info>, mint: &Pubkey) -> Result<Ready<'a, 'info>> {
        let (Some(m), Some(escrow), Some(other), Some(tp)) = (self.mint, self.escrow, self.other, self.token_program) else {
            return err!(ConversionError::WrongAccount);
        };
        require_keys_eq!(m.key(), *mint, ConversionError::WrongAccount);
        require_keys_eq!(*m.to_account_info().owner, tp.key(), ConversionError::WrongTokenProgram);
        require_keys_eq!(other.mint, *mint, ConversionError::WrongAccount);
        let expected = get_associated_token_address_with_program_id(swap.key, mint, &tp.key());
        require_keys_eq!(escrow.key(), expected, ConversionError::WrongAccount);
        Ok(Ready { mint: m, escrow, other, token_program: tp })
    }
}

fn token_amount(account: &AccountInfo) -> Result<u64> {
    let data = account.try_borrow_data()?;
    let a = anchor_spl::token_2022::spl_token_2022::extension::StateWithExtensions::<
        anchor_spl::token_2022::spl_token_2022::state::Account,
    >::unpack(&data)?;
    Ok(a.base.amount)
}

/// Takes `amount` from `from` (a signer) into the swap. Returns what
/// arrived: a token that takes a fee on transfer delivers less.
pub fn take<'info>(
    swap: &Account<'info, Swap>,
    mint: &Pubkey,
    amount: u64,
    from: &AccountInfo<'info>,
    system_program: &AccountInfo<'info>,
    token: &Token<'_, 'info>,
) -> Result<u64> {
    let swap_info = swap.to_account_info();
    if *mint == Pubkey::default() {
        anchor_lang::system_program::transfer(
            CpiContext::new(system_program.key(), anchor_lang::system_program::Transfer { from: from.clone(), to: swap_info }),
            amount,
        )?;
        return Ok(amount);
    }
    let t = token.ready(&swap_info, mint)?;
    let Some(atp) = token.associated_token_program else { return err!(ConversionError::WrongAccount) };
    associated_token::create_idempotent(CpiContext::new(
        atp.key(),
        associated_token::Create {
            payer: from.clone(),
            associated_token: t.escrow.to_account_info(),
            authority: swap_info,
            mint: t.mint.to_account_info(),
            system_program: system_program.clone(),
            token_program: t.token_program.to_account_info(),
        },
    ))?;
    require_keys_eq!(t.other.owner, from.key(), ConversionError::WrongAccount);
    let before = token_amount(&t.escrow.to_account_info())?;
    token_interface::transfer_checked(
        CpiContext::new(
            t.token_program.key(),
            TransferChecked {
                from: t.other.to_account_info(),
                mint: t.mint.to_account_info(),
                to: t.escrow.to_account_info(),
                authority: from.clone(),
            },
        ),
        amount,
        t.mint.decimals,
    )?;
    Ok(token_amount(&t.escrow.to_account_info())?.saturating_sub(before))
}

/// Pays the swap's value to `to`: its wallet for SOL, its token account for
/// a token.
pub fn pay<'info>(swap: &Account<'info, Swap>, to: &AccountInfo<'info>, token: &Token<'_, 'info>) -> Result<()> {
    let amount = swap.amount;
    if amount == 0 {
        return Ok(());
    }
    let swap_info = swap.to_account_info();
    if swap.mint == Pubkey::default() {
        **swap_info.try_borrow_mut_lamports()? = swap_info.lamports().checked_sub(amount).ok_or(ConversionError::Overflow)?;
        **to.try_borrow_mut_lamports()? = to.lamports().checked_add(amount).ok_or(ConversionError::Overflow)?;
        return Ok(());
    }
    let t = token.ready(&swap_info, &swap.mint)?;
    require_keys_eq!(t.other.owner, to.key(), ConversionError::WrongAccount);
    let id = swap.id.to_le_bytes();
    let seeds: &[&[u8]] = &[SWAP_SEED, &id, &[swap.bump]];
    token_interface::transfer_checked(
        CpiContext::new_with_signer(
            t.token_program.key(),
            TransferChecked {
                from: t.escrow.to_account_info(),
                mint: t.mint.to_account_info(),
                to: t.other.to_account_info(),
                authority: swap_info,
            },
            &[seeds],
        ),
        amount,
        t.mint.decimals,
    )
}
