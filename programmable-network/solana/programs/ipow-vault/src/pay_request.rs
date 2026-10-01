use anchor_lang::prelude::*;
use anchor_spl::token_interface::{Mint, TokenAccount, TokenInterface};

use crate::constants::*;
use crate::errors::VaultError;
use crate::state::{Claim, Config, FastPay, HomeAsset, Paid};
use crate::util::{fast_share, pay_home, sha256, token_owner, u32_at, u64_at};

/// Pays a burn on Ethereum carried by an accepted claim, once, with its fast
/// fee: to the attester that paid it at once with the same record, its
/// amount and share of the fast fee, the rest to the burn's address; or
/// else all to the address. SOL goes to `to` and `attester` themselves,
/// tokens to their accounts. Anyone may call.
pub fn handler<'info>(
    ctx: Context<'info, PayRequest<'info>>,
    _claim_id: u64,
    request_id: u64,
    _asset: u32,
    record: Vec<u8>,
) -> Result<()> {
    require!(record.len() == REQUEST_LEN && record[0] == REQUEST && record[1] == ETHEREUM, VaultError::WrongRecord);
    require!(u64_at(&record, 6) == request_id, VaultError::WrongRecord);
    let claim = &ctx.accounts.claim;
    require!(claim.accepted && claim.carries(&record), VaultError::NotAccepted);
    require!(u32_at(&record, 2) == ctx.accounts.home_asset.number, VaultError::WrongRecord);
    let to_key = Pubkey::new_from_array(record[22..54].try_into().unwrap());
    let (amount, fast_fee, at) = (u64_at(&record, 14), u64_at(&record, 62), u64_at(&record, 70) as i64);
    let value = amount.checked_add(fast_fee).ok_or(VaultError::Overflow)?;
    let a = &mut ctx.accounts.home_asset;
    require!(value <= a.reserve, VaultError::Underfunded);
    a.reserve -= value;
    ctx.accounts.paid.bump = ctx.bumps.paid;

    // A payment at once under this record, if any: its account is always
    // named, so nobody can pay the burn's address twice by leaving it out.
    let f = &ctx.accounts.fast_pay;
    let (address, _) = Pubkey::find_program_address(&[FAST_PAY_SEED, &u64_at(&record, 6).to_le_bytes(), &sha256(&[&record])], &crate::ID);
    require_keys_eq!(f.key(), address, VaultError::WrongAccount);
    let paid_at_once = !f.data_is_empty() && f.owner == &crate::ID;
    let (to_attester, attester) = match paid_at_once {
        true => {
            let f: FastPay = {
                let data = f.try_borrow_data()?;
                FastPay::try_deserialize(&mut &data[..])?
            };
            (amount + fast_share(fast_fee, at, f.paid_at), Some(f.attester))
        }
        false => (0, None),
    };
    let s = &ctx.accounts;
    let pay = |to: &AccountInfo<'info>, owner: &Pubkey, amount: u64| -> Result<()> {
        if amount == 0 {
            return Ok(());
        }
        if s.home_asset.number == 0 {
            require_keys_eq!(to.key(), *owner, VaultError::WrongAccount);
        } else {
            require_keys_eq!(token_owner(to)?, *owner, VaultError::WrongAccount);
        }
        pay_home(
            &s.home_asset,
            amount,
            &s.config.to_account_info(),
            s.config.bump,
            to,
            s.tokens.as_ref().map(|t| t.to_account_info()).as_ref(),
            s.mint.as_ref().map(|m| m.to_account_info()).as_ref(),
            s.token_program.as_ref().map(|p| p.to_account_info()).as_ref(),
        )
    };
    pay(&s.to.to_account_info(), &to_key, value - to_attester)?;
    if let Some(attester) = attester {
        let account = s.attester.as_ref().ok_or(VaultError::WrongAccount)?;
        pay(&account.to_account_info(), &attester, to_attester)?;
    }
    Ok(())
}

#[derive(Accounts)]
#[instruction(claim_id: u64, request_id: u64, asset: u32)]
pub struct PayRequest<'info> {
    #[account(mut, seeds = [CONFIG_SEED], bump = config.bump)]
    pub config: Box<Account<'info, Config>>,
    #[account(seeds = [CLAIM_SEED, &claim_id.to_le_bytes()], bump = claim.bump)]
    pub claim: Box<Account<'info, Claim>>,
    #[account(mut, seeds = [ASSET_SEED, &asset.to_le_bytes()], bump = home_asset.bump)]
    pub home_asset: Box<Account<'info, HomeAsset>>,
    /// Once per burn number.
    #[account(init, payer = payer, space = 8 + Paid::INIT_SPACE, seeds = [PAID_SEED, &request_id.to_le_bytes()], bump)]
    pub paid: Box<Account<'info, Paid>>,
    /// CHECK: the record of a payment at once under this record; checked in
    /// the handler, and empty when nobody paid.
    pub fast_pay: UncheckedAccount<'info>,
    /// CHECK: the burn's address, or its token account; checked in the
    /// handler.
    #[account(mut)]
    pub to: UncheckedAccount<'info>,
    /// CHECK: the attester, or its token account; checked in the handler.
    #[account(mut)]
    pub attester: Option<UncheckedAccount<'info>>,
    #[account(mut, seeds = [HOME_TOKENS_SEED, home_asset.mint.as_ref()], bump)]
    pub tokens: Option<Box<InterfaceAccount<'info, TokenAccount>>>,
    pub mint: Option<Box<InterfaceAccount<'info, Mint>>>,
    pub token_program: Option<Interface<'info, TokenInterface>>,
    #[account(mut)]
    pub payer: Signer<'info>,
    pub system_program: Program<'info, System>,
}
