use anchor_lang::prelude::*;
use anchor_lang::system_program::{transfer, Transfer};
use anchor_spl::token_interface::{self, Mint, TokenAccount, TokenInterface, TransferChecked};

use crate::constants::*;
use crate::errors::VaultError;
use crate::state::{FastPay, HomeAsset};
use crate::util::{now, sha256, token_owner, u32_at, u64_at};

/// Pays at once a burn on Ethereum of a receipt of an asset here, from the
/// sender's own money, to the address the burn names (section 11.7): SOL to
/// `to`, or tokens to `to`'s account. The sender states the burn's REQUEST
/// record. When a claim carrying the same record is accepted, the vault
/// pays it the amount and its share of the fast fee (D124). Stated wrongly,
/// the sender's money is lost: it checks the burn on Ethereum first. Anyone
/// may call; each record stated once, so nobody blocks the true one (D126).
pub fn handler<'info>(
    ctx: Context<'info, FastPayBurn<'info>>,
    _request_id: u64,
    _asset: u32,
    _record_hash: [u8; 32],
    record: Vec<u8>,
) -> Result<()> {
    require!(record.len() == REQUEST_LEN && record[0] == REQUEST && record[1] == ETHEREUM, VaultError::WrongRecord);
    require!(ctx.accounts.paid.data_is_empty(), VaultError::AlreadyPaid);
    let a = &ctx.accounts.home_asset;
    require!(u32_at(&record, 2) == a.number, VaultError::WrongRecord);
    let to_key = Pubkey::new_from_array(record[22..54].try_into().unwrap());
    let amount = u64_at(&record, 14);
    let native: u64 = (amount as u128 * a.unit as u128).try_into().map_err(|_| error!(VaultError::Overflow))?;
    let f = &mut ctx.accounts.fast_pay;
    f.attester = ctx.accounts.attester.key();
    f.paid_at = now()?;
    f.bump = ctx.bumps.fast_pay;
    let s = &ctx.accounts;
    if s.home_asset.number == 0 {
        require_keys_eq!(s.to.key(), to_key, VaultError::WrongAccount);
        transfer(CpiContext::new(s.system_program.key(), Transfer { from: s.attester.to_account_info(), to: s.to.to_account_info() }), native)
    } else {
        let (from, mint, program) = (
            s.from.as_ref().ok_or(VaultError::WrongAccount)?,
            s.mint.as_ref().ok_or(VaultError::WrongAccount)?,
            s.token_program.as_ref().ok_or(VaultError::WrongAccount)?,
        );
        require_keys_eq!(mint.key(), s.home_asset.mint, VaultError::WrongAccount);
        require_keys_eq!(token_owner(&s.to.to_account_info())?, to_key, VaultError::WrongAccount);
        token_interface::transfer_checked(
            CpiContext::new(
                program.key(),
                TransferChecked { from: from.to_account_info(), mint: mint.to_account_info(), to: s.to.to_account_info(), authority: s.attester.to_account_info() },
            ),
            native,
            s.home_asset.decimals,
        )
    }
}

/// The address of the record of a payment at once of burn `request_id`
/// under `record`.
pub fn fast_pay_address(request_id: u64, record: &[u8]) -> Pubkey {
    Pubkey::find_program_address(&[FAST_PAY_SEED, &request_id.to_le_bytes(), &sha256(&[record])], &crate::ID).0
}

#[derive(Accounts)]
#[instruction(request_id: u64, asset: u32, record_hash: [u8; 32], record: Vec<u8>)]
pub struct FastPayBurn<'info> {
    #[account(seeds = [ASSET_SEED, &asset.to_le_bytes()], bump = home_asset.bump)]
    pub home_asset: Box<Account<'info, HomeAsset>>,
    /// The record of this payment: once per burn and stated record. Its
    /// hash is checked against the record by the seeds.
    #[account(
        init,
        payer = attester,
        space = 8 + FastPay::INIT_SPACE,
        seeds = [FAST_PAY_SEED, &request_id.to_le_bytes(), &record_hash],
        bump,
        constraint = record_hash == sha256(&[&record]) @ VaultError::WrongRecord,
        constraint = request_id == u64_at(&record, 6) @ VaultError::WrongRecord
    )]
    pub fast_pay: Box<Account<'info, FastPay>>,
    /// CHECK: the burn's record of payment: still empty.
    #[account(seeds = [PAID_SEED, &request_id.to_le_bytes()], bump)]
    pub paid: UncheckedAccount<'info>,
    /// CHECK: the burn's address for SOL, or its token account; checked in
    /// the handler.
    #[account(mut)]
    pub to: UncheckedAccount<'info>,
    #[account(mut, token::authority = attester)]
    pub from: Option<Box<InterfaceAccount<'info, TokenAccount>>>,
    pub mint: Option<Box<InterfaceAccount<'info, Mint>>>,
    pub token_program: Option<Interface<'info, TokenInterface>>,
    #[account(mut)]
    pub attester: Signer<'info>,
    pub system_program: Program<'info, System>,
}
