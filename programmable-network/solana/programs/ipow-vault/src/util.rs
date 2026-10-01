//! Shared helpers: accounts at program addresses, credits, the real-Bitcoin
//! rule (D108) and moving vETH.

use anchor_lang::prelude::*;
use anchor_lang::system_program::{allocate, assign, create_account, transfer, Allocate, Assign, CreateAccount, Transfer};
use anchor_spl::token::{self, Burn, MintTo, Token, Transfer as TokenTransfer};
use ipow_light_client::utils::tx_in_block;
use ipow_protocol::duty::{read_node, read_walk};
use sha2::{Digest, Sha256};

use crate::constants::*;
use crate::errors::VaultError;
use crate::state::{Btc, Credit};

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

/// What a registration carries in its `OP_RETURN` (D106): the same bytes as
/// `pairCommitment` on Ethereum.
pub fn pair_commitment(ethereum_vault: &[u8; 20], peer_operator: &[u8; 20], operator: &Pubkey) -> [u8; 32] {
    sha256(&[b"iPoW pair", ethereum_vault, peer_operator, crate::ID.as_ref(), operator.as_ref()])
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

/// Credits `owner` in its credit account, created if needed. Works when the
/// same account is passed more than once.
pub fn credit_to<'info>(
    account: &AccountInfo<'info>,
    owner: &Pubkey,
    lamports: u64,
    veth: u64,
    payer: &AccountInfo<'info>,
    system_program: &AccountInfo<'info>,
) -> Result<()> {
    let (address, bump) = Pubkey::find_program_address(&[CREDIT_SEED, owner.as_ref()], &crate::ID);
    require_keys_eq!(account.key(), address, VaultError::WrongAccount);
    let mut record = if account.owner == &crate::ID && !account.data_is_empty() {
        let data = account.try_borrow_data()?;
        Credit::try_deserialize(&mut &data[..])?
    } else {
        create_pda(account, payer, system_program, 8 + Credit::INIT_SPACE, &[CREDIT_SEED, owner.as_ref(), &[bump]])?;
        Credit { owner: *owner, lamports: 0, veth: 0, bump }
    };
    record.lamports = record.lamports.checked_add(lamports).ok_or(VaultError::Overflow)?;
    record.veth = record.veth.checked_add(veth).ok_or(VaultError::Overflow)?;
    let mut data = account.try_borrow_mut_data()?;
    record.try_serialize(&mut &mut data[..])?;
    Ok(())
}

/// Writes a program account's record.
pub fn save<T: AccountSerialize>(account: &AccountInfo, record: &T) -> Result<()> {
    let mut data = account.try_borrow_mut_data()?;
    record.try_serialize(&mut &mut data[..])
}

/// Moves lamports out of the settings record, which holds deposits and
/// lamport credits.
pub fn pay_lamports(config: &AccountInfo, to: &AccountInfo, amount: u64) -> Result<()> {
    config.sub_lamports(amount)?;
    to.add_lamports(amount)?;
    Ok(())
}

pub struct Veth<'a, 'info> {
    pub config: &'a AccountInfo<'info>,
    pub config_bump: u8,
    pub mint: &'a AccountInfo<'info>,
    pub holding: &'a AccountInfo<'info>,
    pub token_program: &'a Program<'info, Token>,
}

impl<'a, 'info> Veth<'a, 'info> {
    fn seeds(&self) -> [&[u8]; 2] {
        [CONFIG_SEED, std::slice::from_ref(&self.config_bump)]
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

pub fn u64_at(b: &[u8], o: usize) -> u64 {
    u64::from_be_bytes(b[o..o + 8].try_into().unwrap())
}

/// The LOCK record for `lock_id` among a claim's records.
pub struct LockRecord {
    pub amount: u64,
    pub recipient: Pubkey,
    pub fee: u64,
    pub fast_fee: u64,
    pub locked_at: i64,
}

impl LockRecord {
    /// What the record's lock backs: the receipt and the fast fee.
    pub fn value(&self) -> Result<u64> {
        self.amount.checked_add(self.fast_fee).ok_or_else(|| error!(VaultError::Overflow))
    }

    /// Whether an attest stated this record.
    pub fn stated_by(&self, f: &crate::state::FastLock) -> bool {
        self.amount == f.amount && self.recipient == f.recipient && self.fee == f.fee && self.fast_fee == f.fast_fee && self.locked_at == f.locked_at
    }
}

/// D124: an attester's share of `fast_fee`, attesting at `at` a lock or
/// burn made at `made_at`: the fee times the time left to 8 days after it,
/// over 8 days.
pub fn fast_share(fast_fee: u64, made_at: i64, at: i64) -> u64 {
    let left = (made_at.saturating_add(FAST_FEE_DEADLINE)).saturating_sub(at).clamp(0, FAST_FEE_DEADLINE);
    (fast_fee as u128 * left as u128 / FAST_FEE_DEADLINE as u128) as u64
}

pub fn find_lock(records: &[u8], lock_id: u64) -> Option<LockRecord> {
    let mut o = 0;
    while o + LOCK_LEN <= records.len() {
        if u64_at(records, o + 1) == lock_id {
            let recipient = Pubkey::new_from_array(records[o + 17..o + 49].try_into().unwrap());
            return Some(LockRecord {
                amount: u64_at(records, o + 9),
                recipient,
                fee: u64_at(records, o + 49),
                fast_fee: u64_at(records, o + 57),
                locked_at: u64_at(records, o + 65) as i64,
            });
        }
        o += LOCK_LEN;
    }
    None
}
