//! BETA: a token backed by a fixed basket of up to 8 parts, one per
//! network, all held on this network: for example 1 SOL (as wrapped SOL),
//! 1 vETH, the vault's receipt for ETH on Ethereum (spec section 11), and a
//! tokenised stock. Design: docs/specs/ipow-beta-app.md.
//!
//! Anyone creates a basket, with no approval, and sets a creator fee on
//! mint and on burn, each at most 1%, fixed at creation and paid as a share
//! of each part. Fees are set aside in the basket and collected by their
//! receiver, so nobody but a holder can stop a holder's mint, burn or
//! collection. The creator may hand the fee to another address, never
//! change the basket or the fee.
//!
//! A part may be a token whose issuer controls it (E3): one that can be
//! frozen, paused, moved by the issuer, or held only by allowed accounts.
//! Its powers are recorded when the basket is created. A burn pays each
//! part on its own, and a share of what the basket actually holds, so one
//! issuer's action falls only on its own part, and equally on every holder.
//!
//! A part may be another network's asset, held here as the vault's receipt
//! of it, named before the vault has made that mint (E5): the part is then
//! pending (no token program recorded), and is read, its powers with it,
//! when first minted. Nothing can be minted while a part's mint does not
//! exist: the asset must be bridged here first.
//! No key can change the program once its upgrade authority is removed
//! (D59).

use anchor_lang::prelude::*;
use anchor_lang::solana_program::instruction::{AccountMeta, Instruction};
use anchor_lang::solana_program::program::invoke_signed;
use anchor_spl::associated_token::{self, get_associated_token_address_with_program_id, AssociatedToken};
use anchor_spl::token::{self, Burn, Mint, MintTo, Token, TokenAccount};
use anchor_spl::token_2022::spl_token_2022::extension::{BaseStateWithExtensions, ExtensionType, StateWithExtensions};
use anchor_spl::token_2022::spl_token_2022::extension::default_account_state::DefaultAccountState;
use anchor_spl::token_2022::spl_token_2022::state::{Account as SplAccount, AccountState, Mint as SplMint};
use anchor_spl::token_interface::{self, TransferChecked};

declare_id!("3DuG4iNPptEyCAA7FM1YQja43G6HkTAwrswT3Ds2d94R");

/// At most 8 parts, one per network.
pub const MAX_PARTS: usize = 8;
/// Each fee is at most 1% (100 hundredths of a percent).
pub const MAX_FEE_BPS: u16 = 100;
pub const BPS: u128 = 10_000;
/// BETA has 9 decimals; a part's amount is what one whole BETA holds when
/// the basket is new.
pub const BETA_DECIMALS: u8 = 9;
pub const ONE: u128 = 1_000_000_000;

/// The basket token's name, symbol and metadata URI (E4): Token Metadata's
/// limits, in bytes; a name and a symbol of at least one.
pub const MAX_NAME: usize = 32;
pub const MAX_SYMBOL: usize = 10;
pub const MAX_URI: usize = 200;
/// Metaplex Token Metadata: the account wallets read a token's name,
/// symbol and image from. Its `CreateMetadataAccountV3` is sent as the
/// instruction's bytes, so the program depends on no crate of it.
pub const METADATA_PROGRAM: Pubkey = Pubkey::from_str_const("metaqbxxUerdq28cj1RbAWkYQm3ybzjb6a8bt518x1s");
pub const METADATA_SEED: &[u8] = b"metadata";

pub const BASKET_SEED: &[u8] = b"basket";
pub const BETA_SEED: &[u8] = b"beta";
pub const OWED_SEED: &[u8] = b"owed";
pub const FEES_SEED: &[u8] = b"fees";

/// A part's issuer powers, recorded when the basket is created.
pub const CAN_FREEZE: u8 = 1;
pub const CAN_PAUSE: u8 = 2;
/// A permanent delegate: the issuer can move or burn tokens out of any
/// account, the basket's included.
pub const CAN_MOVE: u8 = 4;
/// Accounts are frozen when made: only accounts the issuer allows can hold
/// the token.
pub const ALLOWED_ONLY: u8 = 8;
/// The issuer can close the mint once its supply is zero, and so make a
/// token of other rules at the same address: a mint refuses a part whose
/// token no longer matches what was recorded.
pub const CAN_CLOSE: u8 = 16;
/// The issuer can change what one unit is worth (interest, or a scaled
/// amount, as a stock split does). The basket counts units, so its shares
/// stay exact; their worth moves.
pub const CAN_RESCALE: u8 = 32;

/// Accounts per part in a mint or a burn: its mint, the basket's account of
/// it, the user's account of it, and its token program.
pub const PER_PART: usize = 4;

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, PartialEq, Eq, Debug, InitSpace)]
pub struct Part {
    pub mint: Pubkey,
    /// What one whole BETA holds of it when the basket is new, in its
    /// smallest unit.
    pub amount: u64,
    /// The classic token program, or Token-2022; the default while the
    /// part's mint does not exist yet (E5), read when it is first minted.
    pub token_program: Pubkey,
    pub decimals: u8,
    /// Its issuer's powers: `CAN_FREEZE`, `CAN_PAUSE`, `CAN_MOVE`,
    /// `ALLOWED_ONLY`.
    pub powers: u8,
    /// Set aside, not yet collected: what burners who deferred this part
    /// are owed, and the fees of its receivers.
    pub owed: u64,
}

#[account]
#[derive(InitSpace)]
pub struct Basket {
    pub creator: Pubkey,
    pub id: u64,
    /// Receives the fees; the only thing that can change, by itself.
    pub fee_to: Pubkey,
    pub mint_fee_bps: u16,
    pub burn_fee_bps: u16,
    #[max_len(MAX_PARTS)]
    pub parts: Vec<Part>,
    pub beta: Pubkey,
    pub bump: u8,
    pub beta_bump: u8,
}

/// The fees a receiver has earned in a basket and not collected, by part.
/// Fees are set aside in the basket, not sent: an issuer that freezes the
/// receiver's account then stops nothing but that receiver's collection.
#[account]
#[derive(InitSpace)]
pub struct Fees {
    pub amounts: [u64; MAX_PARTS],
    pub bump: u8,
}

/// A part owed to a holder who deferred it in a burn.
#[account]
#[derive(InitSpace)]
pub struct Owed {
    pub amount: u64,
    pub bump: u8,
}

#[error_code]
pub enum BasketError {
    #[msg("a part is a receipt the vault has not made yet: bridge the asset here first")]
    PartNotMadeYet,
    #[msg("the token's name is 1 to 32 bytes, its symbol 1 to 10, its URI at most 200")]
    BadMetadata,
    #[msg("A basket has 1 to 8 parts, each a different token with an amount")]
    BadParts,
    #[msg("A fee is at most 1%")]
    FeeTooHigh,
    #[msg("The accounts of the parts are not the ones expected")]
    WrongAccount,
    #[msg("A part must be a token of the classic program or of Token-2022, without transfer fees, transfer hooks, confidential transfers, or a ban on transfers")]
    BadMint,
    ZeroAmount,
    Overflow,
    NothingOwed,
    #[msg("A basket's first mint is a whole number of BETA, so it holds exactly the parts it was created with")]
    FirstMintNotWhole,
    #[msg("A part of the basket holds nothing: its issuer took it all. Nothing can be minted")]
    PartEmpty,
    #[msg("A part's token no longer has the rules recorded for it")]
    PartChanged,
    #[msg("The fee receiver's record of fees is needed")]
    NoFeesRecord,
}

fn mul_div(a: u64, b: u64, c: u128, up: bool) -> Result<u64> {
    let v = a as u128 * b as u128;
    let q = if up { v.div_ceil(c) } else { v / c };
    u64::try_from(q).map_err(|_| error!(BasketError::Overflow))
}

/// The creator's fee on `value`, rounded up: splitting a mint or a burn
/// into small ones never avoids it. At most `value`, since bps <= 100.
fn fee_of(value: u64, bps: u16) -> u64 {
    (value as u128 * bps as u128).div_ceil(BPS) as u64
}

/// A token account's amount, owner and state, of either token program.
fn read_account(a: &AccountInfo, program: &Pubkey) -> Result<SplAccount> {
    require_keys_eq!(*a.owner, *program, BasketError::WrongAccount);
    let data = a.try_borrow_data()?;
    Ok(StateWithExtensions::<SplAccount>::unpack(&data).map_err(|_| error!(BasketError::WrongAccount))?.base)
}

/// What the basket holds of a part and can pay out: its account's balance,
/// less what is owed to those who deferred it.
fn held(part: &Part, account: &AccountInfo) -> Result<u64> {
    if account.data_is_empty() {
        return Ok(0);
    }
    Ok(read_account(account, &part.token_program)?.amount.saturating_sub(part.owed))
}

/// The issuer powers of a mint, or an error for a mint this program cannot
/// hold safely.
fn powers_of(mint: &AccountInfo) -> Result<(u8, u8)> {
    let program = *mint.owner;
    require!(program == token::ID || program == anchor_spl::token_2022::ID, BasketError::BadMint);
    let data = mint.try_borrow_data()?;
    let m = StateWithExtensions::<SplMint>::unpack(&data).map_err(|_| error!(BasketError::BadMint))?;
    let mut powers = 0;
    if m.base.freeze_authority.is_some() {
        powers |= CAN_FREEZE;
    }
    for ext in m.get_extension_types().map_err(|_| error!(BasketError::BadMint))? {
        match ext {
            ExtensionType::PermanentDelegate => powers |= CAN_MOVE,
            ExtensionType::Pausable => powers |= CAN_PAUSE,
            ExtensionType::MintCloseAuthority => powers |= CAN_CLOSE,
            ExtensionType::InterestBearingConfig | ExtensionType::ScaledUiAmount => powers |= CAN_RESCALE,
            ExtensionType::DefaultAccountState => {
                let d = m.get_extension::<DefaultAccountState>().map_err(|_| error!(BasketError::BadMint))?;
                if d.state == AccountState::Frozen as u8 {
                    powers |= ALLOWED_ONLY;
                }
            }
            // What changes how a token is described, not how it moves.
            ExtensionType::MetadataPointer
            | ExtensionType::TokenMetadata
            | ExtensionType::GroupPointer
            | ExtensionType::TokenGroup
            | ExtensionType::GroupMemberPointer
            | ExtensionType::TokenGroupMember => {}
            // Transfer fees, transfer hooks, confidential transfers, a ban
            // on transfers, and anything not known yet.
            _ => return err!(BasketError::BadMint),
        }
    }
    Ok((m.base.decimals, powers))
}

/// The four accounts of each part, in the order of the basket's parts.
struct PartAccounts<'a, 'info> {
    mint: &'a AccountInfo<'info>,
    held: &'a AccountInfo<'info>,
    user: &'a AccountInfo<'info>,
    program: &'a AccountInfo<'info>,
}

/// Checks each part's accounts. The user's account is checked unless the
/// part is `skip_user`.
fn part_accounts<'a, 'info>(
    basket: &Basket,
    basket_key: &Pubkey,
    remaining: &'a [AccountInfo<'info>],
    user: &Pubkey,
    skip_user: u8,
) -> Result<Vec<PartAccounts<'a, 'info>>> {
    require!(remaining.len() >= PER_PART * basket.parts.len(), BasketError::WrongAccount);
    let mut out = vec![];
    for (i, part) in basket.parts.iter().enumerate() {
        let a = &remaining[PER_PART * i..PER_PART * i + PER_PART];
        require_keys_eq!(a[0].key(), part.mint, BasketError::WrongAccount);
        require_keys_eq!(a[3].key(), part.token_program, BasketError::WrongAccount);
        require_keys_eq!(
            a[1].key(),
            get_associated_token_address_with_program_id(basket_key, &part.mint, &part.token_program),
            BasketError::WrongAccount
        );
        if skip_user & (1 << i) == 0 {
            let t = read_account(&a[2], &part.token_program)?;
            require!(t.mint == part.mint && t.owner == *user, BasketError::WrongAccount);
        }
        out.push(PartAccounts { mint: &a[0], held: &a[1], user: &a[2], program: &a[3] });
    }
    Ok(out)
}

/// Creates a program account at a program address, even when someone sent
/// lamports to the address first.
fn create_pda<'info>(account: &AccountInfo<'info>, payer: &AccountInfo<'info>, system_program: &AccountInfo<'info>, space: usize, seeds: &[&[u8]]) -> Result<()> {
    use anchor_lang::system_program::{allocate, assign, create_account, transfer, Allocate, Assign, CreateAccount, Transfer};
    let rent = Rent::get()?.minimum_balance(space);
    let current = account.lamports();
    let system = system_program.key();
    if current == 0 {
        return create_account(CpiContext::new_with_signer(system, CreateAccount { from: payer.clone(), to: account.clone() }, &[seeds]), rent, space as u64, &crate::ID);
    }
    if current < rent {
        transfer(CpiContext::new(system, Transfer { from: payer.clone(), to: account.clone() }), rent - current)?;
    }
    allocate(CpiContext::new_with_signer(system, Allocate { account_to_allocate: account.clone() }, &[seeds]), space as u64)?;
    assign(CpiContext::new_with_signer(system, Assign { account_to_assign: account.clone() }, &[seeds]), &crate::ID)?;
    Ok(())
}

fn move_part<'info>(part: &Part, acc: &PartAccounts<'_, 'info>, from: &AccountInfo<'info>, to: &AccountInfo<'info>, authority: &AccountInfo<'info>, seeds: Option<&[&[u8]]>, value: u64) -> Result<()> {
    if value == 0 {
        return Ok(());
    }
    let accounts = TransferChecked { from: from.clone(), mint: acc.mint.clone(), to: to.clone(), authority: authority.clone() };
    let signers = seeds.map(|s| [s]);
    let ctx = match &signers {
        Some(s) => CpiContext::new_with_signer(acc.program.key(), accounts, s),
        None => CpiContext::new(acc.program.key(), accounts),
    };
    token_interface::transfer_checked(ctx, value, part.decimals)
}

/// The `PER_PART` accounts of one part, checked but for the third, which
/// the caller checks.
fn part_one<'a, 'info>(part: &Part, basket_key: &Pubkey, r: &'a [AccountInfo<'info>]) -> Result<PartAccounts<'a, 'info>> {
    require!(r.len() == PER_PART, BasketError::WrongAccount);
    require_keys_eq!(r[0].key(), part.mint, BasketError::WrongAccount);
    require_keys_eq!(r[3].key(), part.token_program, BasketError::WrongAccount);
    require_keys_eq!(r[1].key(), get_associated_token_address_with_program_id(basket_key, &part.mint, &part.token_program), BasketError::WrongAccount);
    Ok(PartAccounts { mint: &r[0], held: &r[1], user: &r[2], program: &r[3] })
}

/// Sets a fee aside in the basket for its receiver, in the receiver's record.
/// Handed to the basket itself, it is not set aside: it backs BETA.
fn set_fee_aside(basket: &mut Basket, basket_key: &Pubkey, fees: &mut Option<Account<Fees>>, i: usize, fee: u64) -> Result<()> {
    if fee == 0 || basket.fee_to == *basket_key {
        return Ok(());
    }
    let record = fees.as_mut().ok_or(error!(BasketError::NoFeesRecord))?;
    record.amounts[i] = record.amounts[i].checked_add(fee).ok_or(error!(BasketError::Overflow))?;
    basket.parts[i].owed = basket.parts[i].owed.checked_add(fee).ok_or(error!(BasketError::Overflow))?;
    Ok(())
}

#[program]
pub mod beta_basket {
    use super::*;

    /// Creates basket `id` of the caller, and its BETA token. The remaining
    /// accounts are the parts' mints, in order; each part's issuer powers
    /// are read from its mint and recorded.
    pub fn create_basket<'info>(
        ctx: Context<'info, CreateBasket<'info>>,
        id: u64,
        name: String,
        symbol: String,
        uri: String,
        parts: Vec<Part>,
        mint_fee_bps: u16,
        burn_fee_bps: u16,
    ) -> Result<()> {
        require!(!parts.is_empty() && parts.len() <= MAX_PARTS, BasketError::BadParts);
        require!(ctx.remaining_accounts.len() == parts.len(), BasketError::WrongAccount);
        require!(mint_fee_bps <= MAX_FEE_BPS && burn_fee_bps <= MAX_FEE_BPS, BasketError::FeeTooHigh);
        require!(
            !name.is_empty() && name.len() <= MAX_NAME && !symbol.is_empty() && symbol.len() <= MAX_SYMBOL && uri.len() <= MAX_URI,
            BasketError::BadMetadata
        );
        let mut recorded = vec![];
        for (i, p) in parts.iter().enumerate() {
            require!(p.amount > 0 && parts[..i].iter().all(|q| q.mint != p.mint), BasketError::BadParts);
            let m = &ctx.remaining_accounts[i];
            require_keys_eq!(m.key(), p.mint, BasketError::WrongAccount);
            if m.data_is_empty() && *m.owner == anchor_lang::system_program::ID {
                // A receipt the vault has not made yet (E5): pending.
                recorded.push(Part { mint: p.mint, amount: p.amount, token_program: Pubkey::default(), decimals: 0, powers: 0, owed: 0 });
                continue;
            }
            let (decimals, powers) = powers_of(m)?;
            recorded.push(Part { mint: p.mint, amount: p.amount, token_program: *m.owner, decimals, powers, owed: 0 });
        }
        let b = &mut ctx.accounts.basket;
        b.creator = ctx.accounts.creator.key();
        b.id = id;
        b.fee_to = ctx.accounts.creator.key();
        b.mint_fee_bps = mint_fee_bps;
        b.burn_fee_bps = burn_fee_bps;
        b.parts = recorded;
        b.beta = ctx.accounts.beta.key();
        b.bump = ctx.bumps.basket;
        b.beta_bump = ctx.bumps.beta;
        ctx.accounts.fees.bump = ctx.bumps.fees;
        // The token's name, symbol and URI (E4), in its Token Metadata
        // account, fixed for good: the basket is its update authority and
        // the account is made immutable.
        let a = &ctx.accounts;
        let mut data = Vec::with_capacity(4 * 3 + name.len() + symbol.len() + uri.len() + 8);
        data.push(33); // CreateMetadataAccountV3
        for v in [&name, &symbol, &uri] {
            data.extend_from_slice(&(v.len() as u32).to_le_bytes());
            data.extend_from_slice(v.as_bytes());
        }
        data.extend_from_slice(&[0, 0]); // seller_fee_basis_points
        data.extend_from_slice(&[0, 0, 0]); // creators, collection, uses: none
        data.push(0); // is_mutable: false
        data.push(0); // collection_details: none
        let ix = Instruction {
            program_id: METADATA_PROGRAM,
            accounts: vec![
                AccountMeta::new(a.metadata.key(), false),
                AccountMeta::new_readonly(a.beta.key(), false),
                AccountMeta::new_readonly(a.basket.key(), true),
                AccountMeta::new(a.creator.key(), true),
                AccountMeta::new_readonly(a.basket.key(), true),
                AccountMeta::new_readonly(a.system_program.key(), false),
            ],
            data,
        };
        let creator_key = a.creator.key();
        let id_bytes = id.to_le_bytes();
        let seeds: &[&[u8]] = &[BASKET_SEED, creator_key.as_ref(), &id_bytes, &[ctx.bumps.basket]];
        invoke_signed(
            &ix,
            &[
                a.metadata.to_account_info(),
                a.beta.to_account_info(),
                a.basket.to_account_info(),
                a.creator.to_account_info(),
                a.system_program.to_account_info(),
                a.metadata_program.to_account_info(),
            ],
            &[seeds],
        )?;
        Ok(())
    }

    /// Mints `amount` BETA (smallest units) to the caller. The first mint
    /// deposits each part's amount per BETA; after that, each part in
    /// proportion to what the basket holds per BETA, rounded up, so new
    /// holders never pay for a past loss. The creator's fee, a share of
    /// each part, is paid on top and set aside in the basket for its
    /// receiver. The remaining accounts are `PER_PART` per part.
    pub fn mint<'info>(ctx: Context<'info, MintBeta<'info>>, amount: u64) -> Result<()> {
        require!(amount > 0, BasketError::ZeroAmount);
        let mut fees_set = vec![];
        let a = &ctx.accounts;
        let user = a.user.key();
        let basket_key = a.basket.key();
        let supply = a.beta.supply;
        // A whole number of BETA first: a dust first mint would set the
        // basket's mix for every later minter.
        require!(supply > 0 || amount as u128 % ONE == 0, BasketError::FirstMintNotWhole);
        // A pending part (E5) is read now that its mint exists, its powers
        // recorded as a part's are at creation; or the mint waits.
        let pending: Vec<usize> = a.basket.parts.iter().enumerate().filter(|(_, p)| p.token_program == Pubkey::default()).map(|(i, _)| i).collect();
        if !pending.is_empty() {
            require!(ctx.remaining_accounts.len() >= PER_PART * a.basket.parts.len(), BasketError::WrongAccount);
            let basket = &mut ctx.accounts.basket;
            for i in pending {
                let m = &ctx.remaining_accounts[PER_PART * i];
                require_keys_eq!(m.key(), basket.parts[i].mint, BasketError::WrongAccount);
                require!(!(m.data_is_empty() && *m.owner == anchor_lang::system_program::ID), BasketError::PartNotMadeYet);
                let (decimals, powers) = powers_of(m)?;
                basket.parts[i].token_program = *m.owner;
                basket.parts[i].decimals = decimals;
                basket.parts[i].powers = powers;
            }
        }
        let a = &ctx.accounts;
        let accounts = part_accounts(&a.basket, &basket_key, ctx.remaining_accounts, &user, 0)?;
        for (part, acc) in a.basket.parts.iter().zip(accounts) {
            associated_token::create_idempotent(CpiContext::new(
                a.associated_token_program.key(),
                associated_token::Create {
                    payer: a.user.to_account_info(),
                    associated_token: acc.held.clone(),
                    authority: a.basket.to_account_info(),
                    mint: acc.mint.clone(),
                    system_program: a.system_program.to_account_info(),
                    token_program: acc.program.clone(),
                },
            ))?;
            // The token must still have the rules recorded for it.
            let (decimals, powers) = powers_of(acc.mint)?;
            require!(decimals == part.decimals && powers == part.powers && *acc.mint.owner == part.token_program, BasketError::PartChanged);
            let need = if supply == 0 {
                mul_div(amount, part.amount, ONE, true)?
            } else {
                let h = held(part, acc.held)?;
                // A part its issuer took entirely would be minted for free.
                require!(h > 0, BasketError::PartEmpty);
                mul_div(amount, h, supply as u128, true)?
            };
            let fee = fee_of(need, a.basket.mint_fee_bps);
            let who = a.user.to_account_info();
            let total = need.checked_add(fee).ok_or(error!(BasketError::Overflow))?;
            move_part(part, &acc, acc.user, acc.held, &who, None, total)?;
            fees_set.push(fee);
        }
        let a = &mut *ctx.accounts;
        for (i, fee) in fees_set.into_iter().enumerate() {
            set_fee_aside(&mut a.basket, &basket_key, &mut a.fees, i, fee)?;
        }
        let b = &a.basket;
        let id = b.id.to_le_bytes();
        let seeds: &[&[u8]] = &[BASKET_SEED, b.creator.as_ref(), &id, &[b.bump]];
        token::mint_to(
            CpiContext::new_with_signer(
                a.token_program.key(),
                MintTo { mint: a.beta.to_account_info(), to: a.user_beta.to_account_info(), authority: a.basket.to_account_info() },
                &[seeds],
            ),
            amount,
        )
    }

    /// Burns `amount` BETA of the caller, who receives of each part its
    /// share of what the basket holds, rounded down, less the creator's fee.
    /// A part whose bit is set in `defer` is not moved now: it is owed to the
    /// caller, and collected later, the fee taken then. After the `PER_PART`
    /// accounts of every part come the caller's owed records of the deferred
    /// parts, in order.
    pub fn burn<'info>(ctx: Context<'info, BurnBeta<'info>>, amount: u64, defer: u8) -> Result<()> {
        require!(amount > 0, BasketError::ZeroAmount);
        let user = ctx.accounts.user.key();
        let basket_key = ctx.accounts.basket.key();
        let supply = ctx.accounts.beta.supply;
        let n = ctx.accounts.basket.parts.len();
        // Only parts the basket has may be deferred (8 parts use every bit).
        require!(n >= 8 || defer >> n == 0, BasketError::WrongAccount);
        let fee_bps = ctx.accounts.basket.burn_fee_bps;
        let remaining = ctx.remaining_accounts;
        let accounts = part_accounts(&ctx.accounts.basket, &basket_key, remaining, &user, defer)?;
        let mut outs = vec![];
        for (part, acc) in ctx.accounts.basket.parts.iter().zip(&accounts) {
            outs.push(mul_div(amount, held(part, acc.held)?, supply as u128, false)?);
        }
        token::burn(
            CpiContext::new(
                ctx.accounts.token_program.key(),
                Burn { mint: ctx.accounts.beta.to_account_info(), from: ctx.accounts.user_beta.to_account_info(), authority: ctx.accounts.user.to_account_info() },
            ),
            amount,
        )?;
        let (creator, id, bump) = (ctx.accounts.basket.creator, ctx.accounts.basket.id.to_le_bytes(), ctx.accounts.basket.bump);
        let seeds: &[&[u8]] = &[BASKET_SEED, creator.as_ref(), &id, &[bump]];
        let basket_info = ctx.accounts.basket.to_account_info();
        let mut owed_at = PER_PART * n;
        for i in 0..n {
            let out = outs[i];
            let part = ctx.accounts.basket.parts[i];
            let acc = &accounts[i];
            if defer & (1 << i) != 0 {
                let record = remaining.get(owed_at).ok_or(error!(BasketError::WrongAccount))?;
                owed_at += 1;
                let (address, owed_bump) = Pubkey::find_program_address(&[OWED_SEED, basket_key.as_ref(), user.as_ref(), &[i as u8]], &crate::ID);
                require_keys_eq!(record.key(), address, BasketError::WrongAccount);
                let mut owed = if record.owner == &crate::ID && !record.data_is_empty() {
                    Owed::try_deserialize(&mut &record.try_borrow_data()?[..])?
                } else {
                    create_pda(
                        record,
                        &ctx.accounts.user.to_account_info(),
                        &ctx.accounts.system_program.to_account_info(),
                        8 + Owed::INIT_SPACE,
                        &[OWED_SEED, basket_key.as_ref(), user.as_ref(), &[i as u8], &[owed_bump]],
                    )?;
                    Owed { amount: 0, bump: owed_bump }
                };
                owed.amount = owed.amount.checked_add(out).ok_or(error!(BasketError::Overflow))?;
                owed.try_serialize(&mut &mut record.try_borrow_mut_data()?[..])?;
                ctx.accounts.basket.parts[i].owed = part.owed.checked_add(out).ok_or(error!(BasketError::Overflow))?;
            } else {
                let fee = fee_of(out, fee_bps);
                move_part(&part, acc, acc.held, acc.user, &basket_info, Some(seeds), out - fee)?;
                let a = &mut *ctx.accounts;
                set_fee_aside(&mut a.basket, &basket_key, &mut a.fees, i, fee)?;
            }
        }
        Ok(())
    }

    /// Pays the caller what it is owed of part `index`, less the creator's
    /// burn fee, taken here rather than when the part was deferred and set
    /// aside for its receiver, and closes the record, its rent back to the
    /// caller. The remaining accounts are that part's `PER_PART` accounts.
    pub fn collect_owed<'info>(ctx: Context<'info, CollectOwed<'info>>, index: u8) -> Result<()> {
        let i = index as usize;
        let user = ctx.accounts.user.key();
        let basket_key = ctx.accounts.basket.key();
        let part = *ctx.accounts.basket.parts.get(i).ok_or(error!(BasketError::WrongAccount))?;
        let owed = ctx.accounts.owed.amount;
        require!(owed > 0, BasketError::NothingOwed);
        let acc = part_one(&part, &basket_key, ctx.remaining_accounts)?;
        let t = read_account(acc.user, &part.token_program)?;
        require!(t.mint == part.mint && t.owner == user, BasketError::WrongAccount);
        let fee = fee_of(owed, ctx.accounts.basket.burn_fee_bps);
        ctx.accounts.owed.amount = 0;
        ctx.accounts.basket.parts[i].owed = part.owed.checked_sub(owed).ok_or(error!(BasketError::Overflow))?;
        let a = &mut *ctx.accounts;
        set_fee_aside(&mut a.basket, &basket_key, &mut a.fees, i, fee)?;
        let (creator, id, bump) = (a.basket.creator, a.basket.id.to_le_bytes(), a.basket.bump);
        let seeds: &[&[u8]] = &[BASKET_SEED, creator.as_ref(), &id, &[bump]];
        let basket_info = a.basket.to_account_info();
        move_part(&part, &acc, acc.held, acc.user, &basket_info, Some(seeds), owed - fee)
    }

    /// Pays the caller, a receiver of the basket's fees now or before, its
    /// fees of part `index`, to any account of the part (the third of its
    /// `PER_PART` accounts, the remaining accounts).
    pub fn collect_fees<'info>(ctx: Context<'info, CollectFees<'info>>, index: u8) -> Result<()> {
        let i = index as usize;
        let basket_key = ctx.accounts.basket.key();
        let part = *ctx.accounts.basket.parts.get(i).ok_or(error!(BasketError::WrongAccount))?;
        let amount = ctx.accounts.fees.amounts[i];
        require!(amount > 0, BasketError::NothingOwed);
        let acc = part_one(&part, &basket_key, ctx.remaining_accounts)?;
        let t = read_account(acc.user, &part.token_program)?;
        require!(t.mint == part.mint, BasketError::WrongAccount);
        ctx.accounts.fees.amounts[i] = 0;
        ctx.accounts.basket.parts[i].owed = part.owed.checked_sub(amount).ok_or(error!(BasketError::Overflow))?;
        let (creator, id, bump) = (ctx.accounts.basket.creator, ctx.accounts.basket.id.to_le_bytes(), ctx.accounts.basket.bump);
        let seeds: &[&[u8]] = &[BASKET_SEED, creator.as_ref(), &id, &[bump]];
        let basket_info = ctx.accounts.basket.to_account_info();
        move_part(&part, &acc, acc.held, acc.user, &basket_info, Some(seeds), amount)
    }

    /// Hands the fee to another address. Only the one receiving it may; it
    /// opens the new receiver's record of fees, paying its rent, and keeps
    /// what it earned in its own. Records are never closed. Handing it to the basket itself sends every later fee into
    /// the backing, for good: the basket can never sign to hand it on.
    pub fn set_fee_to(ctx: Context<SetFeeTo>, fee_to: Pubkey) -> Result<()> {
        let basket_key = ctx.accounts.basket.key();
        // Fees for these could never be collected: nobody signs for them.
        require!(fee_to != Pubkey::default() && fee_to != crate::ID, BasketError::WrongAccount);
        if fee_to != basket_key {
            let record = ctx.accounts.new_fees.as_ref().ok_or(error!(BasketError::NoFeesRecord))?;
            let (address, bump) = Pubkey::find_program_address(&[FEES_SEED, basket_key.as_ref(), fee_to.as_ref()], &crate::ID);
            require_keys_eq!(record.key(), address, BasketError::WrongAccount);
            if record.owner != &crate::ID || record.data_is_empty() {
                create_pda(
                    record,
                    &ctx.accounts.fee_to.to_account_info(),
                    &ctx.accounts.system_program.to_account_info(),
                    8 + Fees::INIT_SPACE,
                    &[FEES_SEED, basket_key.as_ref(), fee_to.as_ref(), &[bump]],
                )?;
                Fees { amounts: [0; MAX_PARTS], bump }.try_serialize(&mut &mut record.try_borrow_mut_data()?[..])?;
            }
        }
        ctx.accounts.basket.fee_to = fee_to;
        Ok(())
    }
}

#[derive(Accounts)]
#[instruction(id: u64)]
pub struct CreateBasket<'info> {
    #[account(init, payer = creator, space = 8 + Basket::INIT_SPACE, seeds = [BASKET_SEED, creator.key().as_ref(), &id.to_le_bytes()], bump)]
    pub basket: Account<'info, Basket>,
    #[account(init, payer = creator, seeds = [BETA_SEED, basket.key().as_ref()], bump, mint::decimals = BETA_DECIMALS, mint::authority = basket)]
    pub beta: Account<'info, Mint>,
    /// The creator's record of fees: it receives them first.
    #[account(init, payer = creator, space = 8 + Fees::INIT_SPACE, seeds = [FEES_SEED, basket.key().as_ref(), creator.key().as_ref()], bump)]
    pub fees: Account<'info, Fees>,
    /// CHECK: the token's metadata account, made by Token Metadata at its
    /// own address for the mint; checked to be that address.
    #[account(mut, seeds = [METADATA_SEED, METADATA_PROGRAM.as_ref(), beta.key().as_ref()], bump, seeds::program = METADATA_PROGRAM)]
    pub metadata: UncheckedAccount<'info>,
    #[account(mut)]
    pub creator: Signer<'info>,
    pub token_program: Program<'info, Token>,
    pub system_program: Program<'info, System>,
    /// CHECK: Token Metadata itself, by its id.
    #[account(address = METADATA_PROGRAM)]
    pub metadata_program: UncheckedAccount<'info>,
}

#[derive(Accounts)]
pub struct MintBeta<'info> {
    #[account(mut, seeds = [BASKET_SEED, basket.creator.as_ref(), &basket.id.to_le_bytes()], bump = basket.bump)]
    pub basket: Account<'info, Basket>,
    /// The fee receiver's record; needed when a fee is set aside.
    #[account(mut, seeds = [FEES_SEED, basket.key().as_ref(), basket.fee_to.as_ref()], bump = fees.bump)]
    pub fees: Option<Account<'info, Fees>>,
    #[account(mut, seeds = [BETA_SEED, basket.key().as_ref()], bump = basket.beta_bump)]
    pub beta: Account<'info, Mint>,
    #[account(mut, token::mint = beta, token::authority = user)]
    pub user_beta: Account<'info, TokenAccount>,
    #[account(mut)]
    pub user: Signer<'info>,
    pub token_program: Program<'info, Token>,
    pub associated_token_program: Program<'info, AssociatedToken>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct BurnBeta<'info> {
    #[account(mut, seeds = [BASKET_SEED, basket.creator.as_ref(), &basket.id.to_le_bytes()], bump = basket.bump)]
    pub basket: Account<'info, Basket>,
    /// The fee receiver's record; needed when a fee is set aside.
    #[account(mut, seeds = [FEES_SEED, basket.key().as_ref(), basket.fee_to.as_ref()], bump = fees.bump)]
    pub fees: Option<Account<'info, Fees>>,
    #[account(mut, seeds = [BETA_SEED, basket.key().as_ref()], bump = basket.beta_bump)]
    pub beta: Account<'info, Mint>,
    #[account(mut, token::mint = beta, token::authority = user)]
    pub user_beta: Account<'info, TokenAccount>,
    #[account(mut)]
    pub user: Signer<'info>,
    pub token_program: Program<'info, Token>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
#[instruction(index: u8)]
pub struct CollectOwed<'info> {
    #[account(mut, seeds = [BASKET_SEED, basket.creator.as_ref(), &basket.id.to_le_bytes()], bump = basket.bump)]
    pub basket: Account<'info, Basket>,
    #[account(mut, close = user, seeds = [OWED_SEED, basket.key().as_ref(), user.key().as_ref(), &[index]], bump = owed.bump)]
    pub owed: Account<'info, Owed>,
    /// The fee receiver's record; needed when a fee is set aside.
    #[account(mut, seeds = [FEES_SEED, basket.key().as_ref(), basket.fee_to.as_ref()], bump = fees.bump)]
    pub fees: Option<Account<'info, Fees>>,
    #[account(mut)]
    pub user: Signer<'info>,
}

#[derive(Accounts)]
pub struct CollectFees<'info> {
    #[account(mut, seeds = [BASKET_SEED, basket.creator.as_ref(), &basket.id.to_le_bytes()], bump = basket.bump)]
    pub basket: Account<'info, Basket>,
    #[account(mut, seeds = [FEES_SEED, basket.key().as_ref(), receiver.key().as_ref()], bump = fees.bump)]
    pub fees: Account<'info, Fees>,
    pub receiver: Signer<'info>,
}

#[derive(Accounts)]
pub struct SetFeeTo<'info> {
    #[account(mut, seeds = [BASKET_SEED, basket.creator.as_ref(), &basket.id.to_le_bytes()], bump = basket.bump, has_one = fee_to)]
    pub basket: Account<'info, Basket>,
    /// The new receiver's record of fees, opened if needed; none when the fee
    /// is handed to the basket itself.
    /// CHECK: checked and created in the handler.
    #[account(mut)]
    pub new_fees: Option<UncheckedAccount<'info>>,
    #[account(mut)]
    pub fee_to: Signer<'info>,
    pub system_program: Program<'info, System>,
}
