//! Shared rules of the duty: status, deadline, lock, reading the light
//! client, notes, credits and the slash.

use anchor_lang::prelude::*;
use sha2::{Digest, Sha256};
use anchor_lang::system_program::{allocate, assign, create_account, transfer, Allocate, Assign, CreateAccount, Transfer};

use crate::auction::auction_end;
use crate::constants::*;
use crate::errors::ProtocolError;
use crate::fees::duty_time_for;
use crate::state::{Application, BlockRef, Credit, Job, Operator};
use ipow_light_client::state::{Node as LcNode, Walk as LcWalk};

#[derive(PartialEq, Eq, Debug)]
pub enum Status {
    Auction,
    Expired,
    Assigned,
    Proven,
    Slashed,
    Settled,
}

pub fn status(job: &Job, now: i64) -> Status {
    if now < auction_end(job) {
        return Status::Auction;
    }
    if !job.has_operator {
        return Status::Expired;
    }
    if job.slashed {
        return Status::Slashed;
    }
    if job.settled {
        return Status::Settled;
    }
    if job.proven_at != 0 {
        return Status::Proven;
    }
    Status::Assigned
}

/// D60, D69: the moment a proof is no longer accepted (D27).
pub fn deadline(job: &Job) -> Result<i64> {
    Ok(auction_end(job) + duty_time_for(job.confirmations)?)
}

/// D71: the lock and the challenge period are at least 1.5 times the deadline.
pub fn floor_of(job: &Job) -> Result<i64> {
    Ok(duty_time_for(job.confirmations)? * LOCK_NUMERATOR / LOCK_DENOMINATOR)
}

/// How long the escrow stays locked after the proof (D50, D66, D71, D74).
pub fn lock_time_of(job: &Job, app: &Application) -> Result<i64> {
    let floor = floor_of(job)?;
    let mut lock = floor.max(MIN_LOCK);
    if job.claim_kind != 0 {
        let period = app.challenge_periods[job.claim_kind as usize - 1] as i64;
        lock = lock.max(period.max(floor));
    }
    Ok(lock)
}

/// The challenge period of a job, from its proof (D74). Zero for a settlement.
pub fn challenge_period_of(job: &Job, app: &Application) -> Result<i64> {
    if job.claim_kind == 0 {
        return Ok(0);
    }
    let period = app.challenge_periods[job.claim_kind as usize - 1] as i64;
    Ok(period.max(floor_of(job)?))
}

/// What a job's transaction carries: the application's tag, marked as the
/// tag of a job. The same on every network (D80).
pub fn tag_payload(tag: &[u8; 32]) -> [u8; 32] {
    sha256(&[b"iPoW job", tag])
}

/// SHA-256 of the parts written one after the other.
pub fn sha256(parts: &[&[u8]]) -> [u8; 32] {
    let mut h = Sha256::new();
    for p in parts {
        h.update(p);
    }
    h.finalize().into()
}

/// What the first transaction of an operator's chain carries, so that nobody
/// can register a coin of someone else. It can never equal a job's payload:
/// the two start from different words.
pub fn chain_head_commitment(operator: &Pubkey) -> [u8; 32] {
    sha256(&[b"iPoW chain head", crate::ID.as_ref(), operator.as_ref()])
}

/// D47: a guardian's sealed note.
pub fn note_for(guardian: &Pubkey, job_id: u64, evidence: &[u8; 32], salt: &[u8; 32]) -> [u8; 32] {
    sha256(&[guardian.as_ref(), &job_id.to_le_bytes(), evidence, salt])
}

/// A stored block of the light client, at the address its reference gives.
pub fn read_node(account: &AccountInfo, at: &BlockRef) -> Result<LcNode> {
    require_keys_eq!(*account.owner, ipow_light_client::ID, ProtocolError::UnknownBlock);
    let node = {
        let data = account.try_borrow_data()?;
        LcNode::try_deserialize(&mut &data[..]).map_err(|_| error!(ProtocolError::UnknownBlock))?
    };
    let address = Pubkey::create_program_address(
        &[b"node", &at.hash, &at.height.to_le_bytes(), &at.epoch_time.to_le_bytes(), &[node.bump]],
        &ipow_light_client::ID,
    )
    .map_err(|_| error!(ProtocolError::UnknownBlock))?;
    require_keys_eq!(account.key(), address, ProtocolError::UnknownBlock);
    Ok(node)
}

/// A finished walk of the light client from `descendant` down to exactly
/// `ancestor`: the two are linked through stored blocks with the right
/// difficulties.
pub fn read_walk(account: &AccountInfo, descendant: &BlockRef, ancestor: &BlockRef) -> Result<LcWalk> {
    require_keys_eq!(*account.owner, ipow_light_client::ID, ProtocolError::NotLinked);
    let walk = {
        let data = account.try_borrow_data()?;
        LcWalk::try_deserialize(&mut &data[..]).map_err(|_| error!(ProtocolError::NotLinked))?
    };
    let same = |a: &ipow_light_client::state::BlockRef, b: &BlockRef| {
        a.hash == b.hash && a.height == b.height && a.epoch_time == b.epoch_time
    };
    require!(
        walk.finished && same(&walk.descendant, descendant) && same(&walk.ancestor, ancestor),
        ProtocolError::NotLinked
    );
    Ok(walk)
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

/// Credits `amount` to `owner`, in its credit account, created if needed.
/// Works when the same account is passed more than once.
pub fn credit_to<'info>(
    account: &AccountInfo<'info>,
    owner: &Pubkey,
    amount: u64,
    payer: &AccountInfo<'info>,
    system_program: &AccountInfo<'info>,
) -> Result<()> {
    let (address, bump) = Pubkey::find_program_address(&[CREDIT_SEED, owner.as_ref()], &crate::ID);
    require_keys_eq!(account.key(), address, ProtocolError::WrongAccount);
    let mut record = if account.owner == &crate::ID && !account.data_is_empty() {
        let data = account.try_borrow_data()?;
        Credit::try_deserialize(&mut &data[..])?
    } else {
        create_pda(account, payer, system_program, 8 + Credit::INIT_SPACE, &[CREDIT_SEED, owner.as_ref(), &[bump]])?;
        Credit { owner: *owner, amount: 0, bump }
    };
    record.amount = record.amount.checked_add(amount).ok_or(ProtocolError::Overflow)?;
    let mut data = account.try_borrow_mut_data()?;
    record.try_serialize(&mut &mut data[..])?;
    emit!(crate::Credited { to: *owner, amount });
    Ok(())
}

/// Accounts a slash needs, in one place.
pub struct SlashAccounts<'a, 'info> {
    pub operator: &'a mut Operator,
    pub attester: Option<&'a mut Operator>,
    pub application_credit: &'a AccountInfo<'info>,
    pub guardian_credit: &'a AccountInfo<'info>,
    pub payer_credit: &'a AccountInfo<'info>,
    pub funder: &'a AccountInfo<'info>,
    pub system_program: &'a AccountInfo<'info>,
}

/// D26: the escrow x of the job, not the whole bond, and not what the
/// operator locked above x. D36: 80% to the application, 20% to the
/// guardian. D62: both fees back to the user. D42: an attester pays in the
/// operator's place.
pub fn slash(job: &mut Job, guardian: &Pubkey, acc: SlashAccounts) -> Result<()> {
    job.slashed = true;
    job.fees_returned = true;
    if job.has_attester {
        // The operator may be its own attester (D95); its record is then
        // passed once, as the operator.
        let att = if job.attester == job.operator {
            require_keys_eq!(acc.operator.owner, job.operator, ProtocolError::WrongAccount);
            acc.operator
        } else {
            acc.attester.ok_or(ProtocolError::WrongAccount)?
        };
        require_keys_eq!(att.owner, job.attester, ProtocolError::WrongAccount);
        att.locked -= job.escrow;
        att.bond -= job.escrow;
    } else {
        require_keys_eq!(acc.operator.owner, job.operator, ProtocolError::WrongAccount);
        acc.operator.locked -= job.bid;
        acc.operator.bond -= job.escrow;
    }
    let to_application = ((job.escrow as u128 * APPLICATION_SHARE_BPS as u128) / BPS as u128) as u64;
    let to_guardian = job.escrow - to_application;
    emit!(crate::JobSlashed {
        job_id: job.id,
        operator: job.operator,
        guardian: *guardian,
        to_application,
        to_guardian,
    });
    credit_to(acc.application_credit, &job.application, to_application, acc.funder, acc.system_program)?;
    credit_to(acc.guardian_credit, guardian, to_guardian, acc.funder, acc.system_program)?;
    credit_to(acc.payer_credit, &job.payer, job.commitment_fee + job.escrow_fee, acc.funder, acc.system_program)?;
    Ok(())
}
