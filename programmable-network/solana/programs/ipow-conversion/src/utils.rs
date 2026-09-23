use anchor_lang::prelude::*;
use anchor_spl::associated_token::{self, AssociatedToken};
use anchor_spl::token::ID as TOKEN_PROGRAM_ID;
use anchor_spl::token_2022::spl_token_2022::extension::StateWithExtensions;
use anchor_spl::token_2022::{self as token_2022, TransferChecked, ID as TOKEN_2022_PROGRAM_ID};

use crate::errors::ConversionError;
use crate::spl_accounts::SplAccounts;

/// Native-SOL escrow payout — identical to `ipow`'s own `utils::
/// transfer_from_escrow`, duplicated rather than imported since this
/// program's escrow PDAs are entirely separate (different program,
/// different seeds authority, per-mint rather than singular).
pub fn transfer_native_from_escrow<'info>(
    amount: u64,
    to_acc: &AccountInfo<'info>,
    escrow_acc: &AccountInfo<'info>,
    sys_prog: &AccountInfo<'info>,
    signer_seeds: &[&[&[u8]]],
) -> Result<()> {
    if amount > 0 {
        anchor_lang::solana_program::program::invoke_signed(
            &anchor_lang::solana_program::system_instruction::transfer(
                &escrow_acc.key(),
                &to_acc.key(),
                amount,
            ),
            &[escrow_acc.clone(), to_acc.clone(), sys_prog.clone()],
            signer_seeds,
        )?;
    }
    Ok(())
}

/// Accepts either the classic SPL Token program or Token-2022 — `token_
/// interface::transfer_checked` (used below) works against either one
/// identically, since Token-2022's instruction format is a strict superset
/// of the classic one. Also checks the mint account is really owned by
/// whichever program was claimed, so a caller can't point `token_program`
/// at one program while `mint` actually belongs to the other.
fn require_spl_programs(accounts: &SplAccounts) -> Result<()> {
    let token_program_key = accounts.token_program.key();
    require!(
        token_program_key == TOKEN_PROGRAM_ID || token_program_key == TOKEN_2022_PROGRAM_ID,
        ConversionError::WrongTokenProgram
    );
    require_keys_eq!(
        accounts.associated_token_program.key(),
        AssociatedToken::id(),
        ConversionError::WrongTokenProgram
    );
    require_keys_eq!(
        *accounts.mint.owner,
        token_program_key,
        ConversionError::MintTokenProgramMismatch
    );
    Ok(())
}

/// Reads a mint's `decimals`, working for both classic SPL Token and
/// Token-2022 (with or without extensions) — `transfer_checked` requires
/// this to guard against a mismatched/fake mint being substituted.
fn mint_decimals(mint: &AccountInfo) -> Result<u8> {
    let data = mint.data.borrow();
    let unpacked = StateWithExtensions::<anchor_spl::token_2022::spl_token_2022::state::Mint>::unpack(&data)?;
    Ok(unpacked.base.decimals)
}

/// Reads a token account's own `amount` field — works for classic SPL
/// Token and Token-2022 alike (extensions, if any, are appended after the
/// base account layout `StateWithExtensions` already knows how to skip).
/// Used to measure the *real* amount a transfer moved, rather than trusting
/// the requested amount — some Token-2022 mints (the transfer-fee
/// extension) or unusual ERC20-equivalents credit the recipient less than
/// what was sent, and this program's accounting needs to know the true
/// figure, not the nominal one.
fn token_account_amount(token_account: &AccountInfo) -> Result<u64> {
    let data = token_account.data.borrow();
    let unpacked = StateWithExtensions::<anchor_spl::token_2022::spl_token_2022::state::Account>::unpack(&data)?;
    Ok(unpacked.base.amount)
}

/// Moves `amount` of `token_mint` from `source` into escrow (`escrow_native`
/// when native, `escrow_ata` when SPL) — the deposit/lock side of a
/// conversion. `source_owner` signs; `payer` funds the escrow ATA's
/// creation if this is the first deposit of this mint (idempotent, so safe
/// to call unconditionally). Returns the amount *actually* credited to
/// escrow — for native this always equals `amount`; for SPL it's measured
/// via the escrow ATA's own balance before/after the transfer, so a
/// fee-on-transfer mint (Token-2022's transfer-fee extension, or an
/// equivalent) is reflected honestly rather than assumed away. Both current
/// callers (`deposit_conversion`, `propose_claim_conversion`'s bitcoin->
/// native self-escrow) require an exact match and revert on a shortfall —
/// each moves a value that's already load-bearing for a promise made
/// elsewhere, so silently accepting less would under-collateralize it.
#[allow(clippy::too_many_arguments)]
pub fn transfer_value_in<'info>(
    token_mint: Pubkey,
    amount: u64,
    source_owner: &AccountInfo<'info>,
    source_token_account: &AccountInfo<'info>,
    escrow_native: &AccountInfo<'info>,
    escrow_ata: &AccountInfo<'info>,
    escrow_authority: &AccountInfo<'info>,
    payer: &AccountInfo<'info>,
    system_program: &AccountInfo<'info>,
    spl: &SplAccounts<'info>,
) -> Result<u64> {
    if amount == 0 {
        return Ok(0);
    }
    if token_mint == Pubkey::default() {
        anchor_lang::solana_program::program::invoke(
            &anchor_lang::solana_program::system_instruction::transfer(
                source_owner.key,
                escrow_native.key,
                amount,
            ),
            &[source_owner.clone(), escrow_native.clone(), system_program.clone()],
        )?;
        return Ok(amount);
    }

    require_spl_programs(spl)?;
    let decimals = mint_decimals(&spl.mint.to_account_info())?;

    associated_token::create_idempotent(CpiContext::new(
        spl.associated_token_program.key(),
        associated_token::Create {
            payer: payer.clone(),
            associated_token: escrow_ata.clone(),
            authority: escrow_authority.clone(),
            mint: spl.mint.to_account_info(),
            system_program: system_program.clone(),
            token_program: spl.token_program.to_account_info(),
        },
    ))?;

    let before = token_account_amount(escrow_ata)?;

    token_2022::transfer_checked(
        CpiContext::new(
            spl.token_program.key(),
            TransferChecked {
                from: source_token_account.clone(),
                mint: spl.mint.to_account_info(),
                to: escrow_ata.clone(),
                authority: source_owner.clone(),
            },
        ),
        amount,
        decimals,
    )?;

    let after = token_account_amount(escrow_ata)?;
    Ok(after.saturating_sub(before))
}

/// Moves `amount` of `token_mint` out of escrow to `dest` — the payout/
/// release side of a conversion. `escrow_authority_seeds` signs on behalf
/// of the escrow (native system-account transfer, or SPL authority over
/// `escrow_ata`, depending on `token_mint`). `payer` funds the recipient's
/// ATA creation on first payout of this mint to them (idempotent).
///
/// Unlike `transfer_value_in`, this doesn't need to measure/verify the
/// actual amount moved: the escrow's own bookkeeping already decremented
/// by the exact real amount it originally took custody of (via `transfer_
/// value_in`'s honest accounting on the way in), so a fee taken on the way
/// *out* only ever reduces what the recipient receives — it can't create a
/// solvency gap in this program's own books the way an under-measured
/// deposit could.
#[allow(clippy::too_many_arguments)]
pub fn transfer_value_out<'info>(
    token_mint: Pubkey,
    amount: u64,
    dest_owner: &AccountInfo<'info>,
    dest_token_account: &AccountInfo<'info>,
    escrow_native: &AccountInfo<'info>,
    escrow_ata: &AccountInfo<'info>,
    escrow_authority: &AccountInfo<'info>,
    payer: &AccountInfo<'info>,
    system_program: &AccountInfo<'info>,
    spl: &SplAccounts<'info>,
    escrow_authority_seeds: &[&[&[u8]]],
) -> Result<()> {
    if amount == 0 {
        return Ok(());
    }
    if token_mint == Pubkey::default() {
        return transfer_native_from_escrow(
            amount,
            dest_owner,
            escrow_native,
            system_program,
            escrow_authority_seeds,
        );
    }

    require_spl_programs(spl)?;
    let decimals = mint_decimals(&spl.mint.to_account_info())?;

    associated_token::create_idempotent(CpiContext::new(
        spl.associated_token_program.key(),
        associated_token::Create {
            payer: payer.clone(),
            associated_token: dest_token_account.clone(),
            authority: dest_owner.clone(),
            mint: spl.mint.to_account_info(),
            system_program: system_program.clone(),
            token_program: spl.token_program.to_account_info(),
        },
    ))?;

    token_2022::transfer_checked(
        CpiContext::new_with_signer(
            spl.token_program.key(),
            TransferChecked {
                from: escrow_ata.clone(),
                mint: spl.mint.to_account_info(),
                to: dest_token_account.clone(),
                authority: escrow_authority.clone(),
            },
            escrow_authority_seeds,
        ),
        amount,
        decimals,
    )
}
