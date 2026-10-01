//! Shared helpers: accounts at program addresses, credits, the real-Bitcoin
//! rule (D108), receipts and the vault's payouts, and the records of
//! section 11.9.

use anchor_lang::prelude::*;
use anchor_lang::system_program::{allocate, assign, create_account, transfer, Allocate, Assign, CreateAccount, Transfer};
use anchor_spl::token::{self, Burn, MintTo, Token, Transfer as TokenTransfer};
use anchor_spl::token_interface;
use ipow_light_client::utils::tx_in_block;
use ipow_protocol::duty::{read_node, read_walk};
use sha2::{Digest, Sha256};

use crate::constants::*;
use crate::errors::VaultError;
use crate::state::{Btc, Credit, HomeAsset};

pub fn now() -> Result<i64> {
    Ok(Clock::get()?.unix_timestamp)
}

pub fn sha256(parts: &[&[u8]]) -> [u8; 32] {
    let mut h = Sha256::new();
    for p in parts {
        h.update(p);
    }
    h.finalize().into()
}

/// What a registration carries in its `OP_RETURN` (D106, D133): the same
/// bytes as `pairCommitment` on the peer. Each side is its network's number,
/// its vault and its operator in 32 bytes, the lower number first; this
/// side's vault is the pair's configuration.
pub fn pair_commitment(peer: u8, peer_vault: &[u8; 20], peer_operator: &[u8; 20], config: &Pubkey, operator: &Pubkey) -> [u8; 32] {
    let pad = |a: &[u8; 20]| {
        let mut out = [0u8; 32];
        out[12..].copy_from_slice(a);
        out
    };
    let (theirs_vault, theirs_op) = (pad(peer_vault), pad(peer_operator));
    let mine: [&[u8]; 3] = [&[SOLANA], config.as_ref(), operator.as_ref()];
    let theirs: [&[u8]; 3] = [&[peer], &theirs_vault, &theirs_op];
    let (a, b) = if SOLANA < peer { (mine, theirs) } else { (theirs, mine) };
    sha256(&[b"iPoW pair", a[0], a[1], a[2], b[0], b[1], b[2]])
}

/// What a message carries in its `OP_RETURN` (D107).
pub fn message_payload(batch: &[u8]) -> [u8; 32] {
    sha256(&[b"iPoW vault", batch])
}

/// D108: the transaction is in `btc.block`, and that block is `btc.real` or
/// below it. The real block's record is checked by the caller's accounts.
pub fn require_real(btc: &Btc, walk: &AccountInfo, node: &AccountInfo) -> Result<()> {
    if btc.block != btc.real {
        read_walk(walk, &btc.real, &btc.block).map_err(|_| error!(VaultError::NotReal))?;
    }
    let n = read_node(node, &btc.block).map_err(|_| error!(VaultError::NotInBlock))?;
    require!(tx_in_block(&n, &btc.raw_tx, &btc.siblings, btc.tx_index)?, VaultError::NotInBlock);
    Ok(())
}

/// Creates a program account at a program address, even when someone sent
/// lamports to the address first.
pub fn create_pda<'info>(
    account: &AccountInfo<'info>,
    payer: &AccountInfo<'info>,
    system_program: &AccountInfo<'info>,
    space: usize,
    seeds: &[&[u8]],
) -> Result<()> {
    let rent = Rent::get()?.minimum_balance(space);
    let current = account.lamports();
    let system = system_program.key();
    if current == 0 {
        return create_account(
            CpiContext::new_with_signer(system, CreateAccount { from: payer.clone(), to: account.clone() }, &[seeds]),
            rent,
            space as u64,
            &crate::ID,
        );
    }
    if current < rent {
        transfer(CpiContext::new(system, Transfer { from: payer.clone(), to: account.clone() }), rent - current)?;
    }
    allocate(CpiContext::new_with_signer(system, Allocate { account_to_allocate: account.clone() }, &[seeds]), space as u64)?;
    assign(CpiContext::new_with_signer(system, Assign { account_to_assign: account.clone() }, &[seeds]), &crate::ID)?;
    Ok(())
}

/// Credits `owner` `amount` in asset (`home`, `asset`), in its credit
/// account, created if needed.
pub fn credit_to<'info>(
    account: &AccountInfo<'info>,
    config: &Pubkey,
    owner: &Pubkey,
    home: u8,
    asset: u32,
    amount: u64,
    payer: &AccountInfo<'info>,
    system_program: &AccountInfo<'info>,
) -> Result<()> {
    if amount == 0 {
        return Ok(());
    }
    let a = asset.to_le_bytes();
    let (address, bump) = Pubkey::find_program_address(&[CREDIT_SEED, config.as_ref(), owner.as_ref(), &[home], &a], &crate::ID);
    require_keys_eq!(account.key(), address, VaultError::WrongAccount);
    let mut record = if account.owner == &crate::ID && !account.data_is_empty() {
        let data = account.try_borrow_data()?;
        Credit::try_deserialize(&mut &data[..])?
    } else {
        create_pda(account, payer, system_program, 8 + Credit::INIT_SPACE, &[CREDIT_SEED, config.as_ref(), owner.as_ref(), &[home], &a, &[bump]])?;
        Credit { owner: *owner, home, asset, amount: 0, bump }
    };
    record.amount = record.amount.checked_add(amount).ok_or(VaultError::Overflow)?;
    save(account, &record)
}

/// Writes a program account's record.
pub fn save<T: AccountSerialize>(account: &AccountInfo, record: &T) -> Result<()> {
    let mut data = account.try_borrow_mut_data()?;
    record.try_serialize(&mut &mut data[..])
}

/// Moves lamports out of the settings record, which holds SOL.
pub fn pay_lamports(config: &AccountInfo, to: &AccountInfo, amount: u64) -> Result<()> {
    if amount == 0 {
        return Ok(());
    }
    config.sub_lamports(amount)?;
    to.add_lamports(amount)?;
    Ok(())
}

/// The receipt here of an asset of Ethereum, and the vault's account of it.
pub struct Receipt<'a, 'info> {
    pub config: &'a AccountInfo<'info>,
    pub config_peer: u8,
    pub config_bump: u8,
    pub mint: &'a AccountInfo<'info>,
    pub holding: &'a AccountInfo<'info>,
    pub token_program: &'a Program<'info, Token>,
}

impl<'a, 'info> Receipt<'a, 'info> {
    fn seeds(&self) -> [&[u8]; 3] {
        [CONFIG_SEED, std::slice::from_ref(&self.config_peer), std::slice::from_ref(&self.config_bump)]
    }

    pub fn burn_held(&self, amount: u64) -> Result<()> {
        if amount == 0 {
            return Ok(());
        }
        let seeds = self.seeds();
        token::burn(
            CpiContext::new_with_signer(
                self.token_program.key(),
                Burn { mint: self.mint.clone(), from: self.holding.clone(), authority: self.config.clone() },
                &[&seeds],
            ),
            amount,
        )
    }

    pub fn mint_to(&self, to: &AccountInfo<'info>, amount: u64) -> Result<()> {
        if amount == 0 {
            return Ok(());
        }
        let seeds = self.seeds();
        token::mint_to(
            CpiContext::new_with_signer(
                self.token_program.key(),
                MintTo { mint: self.mint.clone(), to: to.clone(), authority: self.config.clone() },
                &[&seeds],
            ),
            amount,
        )
    }

    pub fn pay_held(&self, to: &AccountInfo<'info>, amount: u64) -> Result<()> {
        if amount == 0 {
            return Ok(());
        }
        let seeds = self.seeds();
        token::transfer(
            CpiContext::new_with_signer(
                self.token_program.key(),
                TokenTransfer { from: self.holding.clone(), to: to.clone(), authority: self.config.clone() },
                &[&seeds],
            ),
            amount,
        )
    }
}

/// Pays `amount` record units of asset `a`, whose home is Solana, out of the
/// vault: lamports from the settings record to `to`, or tokens from the
/// vault's account `tokens` to the token account `to`.
#[allow(clippy::too_many_arguments)]
pub fn pay_home<'info>(
    a: &HomeAsset,
    amount: u64,
    config: &AccountInfo<'info>,
    config_peer: u8,
    config_bump: u8,
    to: &AccountInfo<'info>,
    tokens: Option<&AccountInfo<'info>>,
    mint: Option<&AccountInfo<'info>>,
    token_program: Option<&AccountInfo<'info>>,
) -> Result<()> {
    let native = (amount as u128 * a.unit as u128).try_into().map_err(|_| error!(VaultError::Overflow))?;
    if a.number == 0 {
        return pay_lamports(config, to, native);
    }
    if native == 0 {
        return Ok(());
    }
    let (tokens, mint, program) = (
        tokens.ok_or(VaultError::WrongAccount)?,
        mint.ok_or(VaultError::WrongAccount)?,
        token_program.ok_or(VaultError::WrongAccount)?,
    );
    require_keys_eq!(mint.key(), a.mint, VaultError::WrongAccount);
    require_keys_eq!(program.key(), a.token_program, VaultError::WrongAccount);
    let seeds: &[&[u8]] = &[CONFIG_SEED, &[config_peer], &[config_bump]];
    token_interface::transfer_checked(
        CpiContext::new_with_signer(
            program.key(),
            token_interface::TransferChecked { from: tokens.clone(), mint: mint.clone(), to: to.clone(), authority: config.clone() },
            &[seeds],
        ),
        native,
        a.decimals,
    )
}

pub fn u64_at(b: &[u8], o: usize) -> u64 {
    u64::from_be_bytes(b[o..o + 8].try_into().unwrap())
}

pub fn u32_at(b: &[u8], o: usize) -> u32 {
    u32::from_be_bytes(b[o..o + 4].try_into().unwrap())
}

pub fn b32_at(b: &[u8], o: usize) -> [u8; 32] {
    b[o..o + 32].try_into().unwrap()
}

/// The length of a record of `kind`, zero when it is not one.
pub fn record_len(kind: u8) -> usize {
    match kind {
        LOCK => LOCK_LEN,
        REQUEST => REQUEST_LEN,
        CANCEL => CANCEL_LEN,
        BOND => BOND_LEN,
        ASSET => ASSET_LEN,
        EXIT => 1,
        _ => 0,
    }
}

/// Whether 32 bytes name an Ethereum address: the last 20, the first 12
/// zero, not all zero.
pub fn is_eth_address(b: &[u8; 32]) -> bool {
    b[..12] == [0u8; 12] && b[12..] != [0u8; 20]
}

/// D124: an attester's share of `fast_fee`, attesting at `at` a lock or
/// burn made at `made_at`: the fee times the time left to 8 days after it,
/// over 8 days.
pub fn fast_share(fast_fee: u64, made_at: i64, at: i64) -> u64 {
    let left = (made_at.saturating_add(FAST_FEE_DEADLINE)).saturating_sub(at).clamp(0, FAST_FEE_DEADLINE);
    (fast_fee as u128 * left as u128 / FAST_FEE_DEADLINE as u128) as u64
}

/// The LOCK record of a lock.
#[allow(clippy::too_many_arguments)]
pub fn lock_record(home: u8, asset: u32, id: u64, amount: u64, recipient: &[u8; 32], fee: u64, fast_fee: u64, at: i64) -> Vec<u8> {
    [
        &[LOCK, home][..],
        &asset.to_be_bytes(),
        &id.to_be_bytes(),
        &amount.to_be_bytes(),
        recipient,
        &fee.to_be_bytes(),
        &fast_fee.to_be_bytes(),
        &(at as u64).to_be_bytes(),
    ]
    .concat()
}

/// The REQUEST record of a burn.
#[allow(clippy::too_many_arguments)]
pub fn request_record(network: u8, asset: u32, id: u64, amount: u64, to: &[u8; 32], fee: u64, fast_fee: u64, at: i64) -> Vec<u8> {
    [
        &[REQUEST, network][..],
        &asset.to_be_bytes(),
        &id.to_be_bytes(),
        &amount.to_be_bytes(),
        to,
        &fee.to_be_bytes(),
        &fast_fee.to_be_bytes(),
        &(at as u64).to_be_bytes(),
    ]
    .concat()
}

/// A LOCK record of a lock on Ethereum, read: its number, asset, amount,
/// recipient here, fee, fast fee and time.
/// A lock of an asset of the pair's peer, as a LOCK record states it.
pub struct EthLock {
    pub id: u64,
    pub asset: u32,
    pub amount: u64,
    pub recipient: Pubkey,
    pub fee: u64,
    pub fast_fee: u64,
    pub locked_at: i64,
}

impl EthLock {
    /// A LOCK record of the pair's peer `peer`.
    pub fn read(record: &[u8], peer: u8) -> Result<EthLock> {
        require!(record.len() == LOCK_LEN && record[0] == LOCK && record[1] == peer, VaultError::WrongRecord);
        Ok(EthLock {
            asset: u32_at(record, 2),
            id: u64_at(record, 6),
            amount: u64_at(record, 14),
            recipient: Pubkey::new_from_array(b32_at(record, 22)),
            fee: u64_at(record, 54),
            fast_fee: u64_at(record, 62),
            locked_at: u64_at(record, 70) as i64,
        })
    }

    /// What it backs: the receipt and the fast fee.
    pub fn value(&self) -> Result<u64> {
        self.amount.checked_add(self.fast_fee).ok_or_else(|| error!(VaultError::Overflow))
    }

    /// Whether an attest stated this record.
    pub fn stated_by(&self, f: &crate::state::FastLock) -> bool {
        self.id == f.lock_id
            && self.asset == f.asset
            && self.amount == f.amount
            && self.recipient == f.recipient
            && self.fee == f.fee
            && self.fast_fee == f.fast_fee
            && self.locked_at == f.locked_at
    }
}

/// The LOCK record of lock `lock_id` on the peer `peer` among a claim's
/// records, and its bytes.
pub fn find_eth_lock(records: &[u8], lock_id: u64, peer: u8) -> Option<(EthLock, Vec<u8>)> {
    let mut o = 0;
    while o < records.len() {
        let len = record_len(records[o]);
        if len == 0 || o + len > records.len() {
            return None;
        }
        let r = &records[o..o + len];
        if r[0] == LOCK && r[1] == peer && u64_at(r, 6) == lock_id {
            return EthLock::read(r, peer).ok().map(|l| (l, r.to_vec()));
        }
        o += len;
    }
    None
}

/// The LOCK record an attest stated, of a lock on the peer `peer`.
pub fn stated_record(f: &crate::state::FastLock, peer: u8) -> Vec<u8> {
    lock_record(peer, f.asset, f.lock_id, f.amount, &f.recipient.to_bytes(), f.fee, f.fast_fee, f.locked_at)
}

/// The owner of a token account of either token program.
pub fn token_owner(account: &AccountInfo) -> Result<Pubkey> {
    require!(
        account.owner == &anchor_spl::token::ID || account.owner == &anchor_spl::token_2022::ID,
        VaultError::WrongAccount
    );
    let data = account.try_borrow_data()?;
    require!(data.len() >= 64, VaultError::WrongAccount);
    Ok(Pubkey::new_from_array(data[32..64].try_into().unwrap()))
}
