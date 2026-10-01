use anchor_lang::prelude::*;
use ipow_light_client::bitcoin::sha256d;
use ipow_protocol::bitcoin_tx::read;

use crate::constants::*;
use crate::errors::VaultError;
use crate::state::{Btc, Buffer, Chain, Claim, ClaimAsset, Config, HomeAsset, HomeLock, LockMark, Message, Real, Request, Stake};
use crate::util::{b32_at, create_pda, lock_record, message_payload, record_len, request_record, require_real, save, u32_at, u64_at};

/// What a message asks once nothing false was found in it.
#[derive(Default)]
struct Plan {
    /// The records from Ethereum it acts on, one after the other.
    records: Vec<u8>,
    /// What it moves in each asset, and a BOND it carries.
    assets: Vec<ClaimAsset>,
    /// More assets than a claim holds, or more than an asset counts.
    overflow: bool,
    acting: bool,
    locks: usize,
    /// BOND records about Solana: the asset and the amount stated.
    stated: Vec<(u8, u32, u64)>,
    /// The accounts of true LOCK and REQUEST records here, which earn fees.
    lock_fees: Vec<usize>,
    request_fees: Vec<usize>,
    exit: bool,
}

impl Plan {
    fn add(&mut self, home: u8, asset: u32, value: u64, peer_bond: u64) {
        let i = match self.assets.iter().position(|a| a.home == home && a.asset == asset) {
            Some(i) => i,
            None => {
                if self.assets.len() == MAX_ASSETS {
                    self.overflow = true;
                    return;
                }
                self.assets.push(ClaimAsset { home, asset, value: 0, peer_bond: 0 });
                self.assets.len() - 1
            }
        };
        let a = &mut self.assets[i];
        match a.value.checked_add(value) {
            Some(v) => a.value = v,
            None => self.overflow = true,
        }
        if peer_bond != 0 && a.peer_bond == 0 {
            a.peer_bond = peer_bond;
        }
    }
}

/// Processes the next message of an operator's pair chain, with its batch
/// (D107, D108, D109, D114). Anyone may submit it. The remaining accounts
/// are, in the order of the batch: for each LOCK of Solana its lock, for
/// each REQUEST burned here its request, for each CANCEL given up here the
/// lock's mark, for each ASSET of Solana and each REQUEST burned on Ethereum
/// the asset, and for each CANCEL given up on Ethereum the lock here. With a
/// buffer, the transaction and the batch are read from it, and the
/// arguments' are empty.
pub fn handler<'info>(
    ctx: Context<'info, SubmitMessage<'info>>,
    operator: Pubkey,
    mut btc: Btc,
    input_index: u32,
    tag_index: u32,
    mut batch: Vec<u8>,
) -> Result<()> {
    require!(!ctx.accounts.chain.exited, VaultError::ChainEnded);
    if let Some(b) = &ctx.accounts.buffer {
        btc.raw_tx = b.raw_tx.clone();
        batch = b.batch.clone();
    }
    require_real(&btc, &ctx.accounts.walk, &ctx.accounts.node)?;
    let v = read(&btc.raw_tx, input_index as u64, input_index as u64, tag_index as u64)?;
    {
        let c = &ctx.accounts.chain;
        require!(v.spent_txid == c.coin_txid && v.spent_vout == c.coin_vout, VaultError::WrongCoin);
    }
    require!(v.has_coin, VaultError::NoCoin);
    require!(v.has_tag && v.tag == message_payload(&batch), VaultError::WrongTag);

    let (config_key, peer) = (ctx.accounts.config.key(), ctx.accounts.config.peer);
    let c = &mut ctx.accounts.chain;
    c.coin_txid = sha256d(&btc.raw_tx);
    c.coin_vout = input_index;
    let index = c.messages;
    c.messages += 1;

    // The batch is published in an account of its own (D120): nobody can
    // hide it from, or fake it for, whoever brings the message to the other
    // network.
    let (address, bump) = Pubkey::find_program_address(&[MESSAGE_SEED, config_key.as_ref(), operator.as_ref(), &index.to_le_bytes()], &crate::ID);
    require_keys_eq!(ctx.accounts.message.key(), address, VaultError::WrongAccount);
    create_pda(
        &ctx.accounts.message,
        &ctx.accounts.submitter.to_account_info(),
        &ctx.accounts.system_program.to_account_info(),
        Message::space(batch.len()),
        &[MESSAGE_SEED, config_key.as_ref(), operator.as_ref(), &index.to_le_bytes(), &[bump]],
    )?;
    save(&ctx.accounts.message, &Message { operator, payer: ctx.accounts.submitter.key(), index, processed_at: crate::util::now()?, batch: batch.clone(), bump })?;

    // D119: too large to be judged everywhere is false.
    let plan = if btc.raw_tx.len() > MAX_RAW_TX || batch.len() > MAX_BATCH {
        None
    } else {
        judge(&batch, c, &config_key, peer, ctx.remaining_accounts)?
    };
    match plan {
        Some(p) => apply(ctx, operator, p),
        None => slash(ctx, operator),
    }
}

/// The next of the remaining accounts.
fn take<'a, 'info>(remaining: &'a [AccountInfo<'info>], next: &mut usize) -> Result<&'a AccountInfo<'info>> {
    let a = remaining.get(*next).ok_or(VaultError::WrongAccount)?;
    *next += 1;
    Ok(a)
}

/// Reads the program account at `account`, expected at `seeds`: `None`
/// when it does not exist.
fn read_at<T: AccountDeserialize>(account: &AccountInfo, seeds: &[&[u8]]) -> Result<Option<T>> {
    let (address, _) = Pubkey::find_program_address(seeds, &crate::ID);
    require_keys_eq!(account.key(), address, VaultError::WrongAccount);
    if account.owner != &crate::ID || account.data_is_empty() {
        return Ok(None);
    }
    let data = account.try_borrow_data()?;
    Ok(Some(T::try_deserialize(&mut &data[..])?))
}

/// Judges the records about Solana and measures the others. `None` when the
/// message is false: a record that is not so here, or a batch that does not
/// parse, since its hash is what the operator wrote.
fn judge(batch: &[u8], c: &Chain, config: &Pubkey, peer: u8, remaining: &[AccountInfo]) -> Result<Option<Plan>> {
    let ck = config.as_ref();
    let mut plan = Plan::default();
    let mut o = 0;
    let mut next = 0;
    let mut counted = 0;
    while o < batch.len() {
        let kind = batch[o];
        let len = record_len(kind);
        if len == 0 || o + len > batch.len() {
            return Ok(None);
        }
        let r = &batch[o..o + len];
        o += len;
        if kind == EXIT {
            if o != batch.len() {
                return Ok(None);
            }
            plan.exit = true;
            continue;
        }
        let net = r[1];
        if net != peer && net != SOLANA {
            return Ok(None);
        }
        if kind != BOND {
            counted += 1;
            if counted > MAX_RECORDS {
                return Ok(None);
            }
        }
        if net == SOLANA {
            let ok = match kind {
                LOCK => {
                    let id = u64_at(r, 6);
                    let i = next;
                    let l: Option<HomeLock> = read_at(take(remaining, &mut next)?, &[HOME_LOCK_SEED, ck, &id.to_le_bytes()])?;
                    plan.lock_fees.push(i);
                    // Returned or not: Ethereum issues nothing for a lock it
                    // marked never usable (section 11.5).
                    l.is_some_and(|l| lock_record(SOLANA, l.asset, l.id, l.amount, &l.recipient, l.fee, l.fast_fee, l.locked_at) == r)
                }
                REQUEST => {
                    let id = u64_at(r, 6);
                    let i = next;
                    let q: Option<Request> = read_at(take(remaining, &mut next)?, &[REQUEST_SEED, ck, &id.to_le_bytes()])?;
                    plan.request_fees.push(i);
                    q.is_some_and(|q| request_record(SOLANA, q.asset, q.id, q.amount, &q.to, q.fee, q.fast_fee, q.requested_at) == r)
                }
                CANCEL => {
                    let id = u64_at(r, 2);
                    let m: Option<LockMark> = read_at(take(remaining, &mut next)?, &[LOCK_SEED, ck, &id.to_le_bytes()])?;
                    m.is_some_and(|m| m.given_up)
                }
                ASSET => {
                    let n = u32_at(r, 2);
                    let a: Option<HomeAsset> = read_at(take(remaining, &mut next)?, &[ASSET_SEED, ck, &n.to_le_bytes()])?;
                    a.is_some_and(|a| a.mint.to_bytes() == b32_at(r, 6) && a.record_decimals == r[38])
                }
                _ => {
                    // BOND, D110: once per chain and asset, from the bond.
                    // The same BOND again is true and changes nothing.
                    let (home, asset, amount) = (r[2], u32_at(r, 3), u64_at(r, 7));
                    let p = c.position(home, asset).map(|i| c.positions[i]).unwrap_or_default();
                    let bad = (home != peer && home != SOLANA)
                        || amount == 0
                        || plan.stated.iter().any(|s| s.0 == home && s.1 == asset)
                        || if p.stated != 0 { amount != p.stated } else { amount > p.bond };
                    plan.stated.push((home, asset, amount));
                    !bad
                }
            };
            if !ok {
                return Ok(None);
            }
            continue;
        }
        // A record from Ethereum: acted on here.
        match kind {
            LOCK => {
                plan.locks += 1;
                let value = u64_at(r, 14).checked_add(u64_at(r, 62));
                let Some(value) = value else { return Ok(None) };
                plan.add(peer, u32_at(r, 2), value, 0);
                plan.acting = true;
            }
            REQUEST => {
                // Paid here, in an asset registered here, to an address.
                let n = u32_at(r, 2);
                let a: Option<HomeAsset> = read_at(take(remaining, &mut next)?, &[ASSET_SEED, ck, &n.to_le_bytes()])?;
                if a.is_none() || b32_at(r, 22) == [0u8; 32] {
                    return Ok(None);
                }
                let Some(value) = u64_at(r, 14).checked_add(u64_at(r, 62)) else { return Ok(None) };
                plan.add(SOLANA, n, value, 0);
                plan.acting = true;
            }
            CANCEL => {
                // Ethereum gives up only a lock it learned from a true LOCK
                // record, so a lock that does not exist here is a lie.
                let id = u64_at(r, 2);
                let l: Option<HomeLock> = read_at(take(remaining, &mut next)?, &[HOME_LOCK_SEED, ck, &id.to_le_bytes()])?;
                let Some(l) = l else { return Ok(None) };
                let Some(value) = l.amount.checked_add(l.fast_fee) else { return Ok(None) };
                plan.add(SOLANA, l.asset, value, 0);
                plan.acting = true;
            }
            ASSET => {
                plan.acting = true;
            }
            _ => {
                // A fact of Ethereum, judged there (D109). Here the first one
                // carried in a claim counts, and a later one is not acted on.
                let (home, asset, amount) = (r[2], u32_at(r, 3), u64_at(r, 7));
                if home != peer && home != SOLANA {
                    return Ok(None);
                }
                let carried = c.position(home, asset).is_some_and(|i| c.positions[i].peer_bond_carried);
                if !carried && amount != 0 {
                    plan.add(home, asset, 0, amount);
                    plan.acting = true;
                }
            }
        }
        plan.records.extend_from_slice(r);
    }
    Ok(Some(plan))
}

fn apply<'info>(ctx: Context<'info, SubmitMessage<'info>>, operator: Pubkey, p: Plan) -> Result<()> {
    let a = ctx.accounts;
    let payer = a.submitter.to_account_info();
    let system = a.system_program.to_account_info();

    // Fees of true locks and requests here, once each, never to a slashed
    // operator: the fee then waits for another. The operator takes them.
    if !a.chain.slashed {
        for &i in &p.lock_fees {
            let account = &ctx.remaining_accounts[i];
            let mut l = {
                let data = account.try_borrow_data()?;
                HomeLock::try_deserialize(&mut &data[..])?
            };
            if !l.fee_paid {
                l.fee_paid = true;
                l.fee_to = operator;
                save(account, &l)?;
            }
        }
        for &i in &p.request_fees {
            let account = &ctx.remaining_accounts[i];
            let mut r = {
                let data = account.try_borrow_data()?;
                Request::try_deserialize(&mut &data[..])?
            };
            if !r.fee_paid {
                r.fee_paid = true;
                r.fee_to = operator;
                save(account, &r)?;
            }
        }
    }

    let c = &mut a.chain;
    for &(home, asset, amount) in &p.stated {
        let i = c.slot(home, asset)?;
        c.positions[i].stated = amount;
    }

    // The acting records become one claim (D111), unless the chain was
    // refused or proven false here, they pass 80% of its bond in an asset on
    // Ethereum, they move more than a claim holds, or its deposit lamports
    // cannot pay the deposit. Then nothing acts, and they can be carried
    // again.
    if p.acting {
        let deposit = a.config.deposit;
        let mut open = !c.refused && !c.slashed && !p.overflow && p.locks <= MAX_LOCKS && c.deposits >= deposit;
        // Room for its assets, and each within 80% of its bond, before any
        // position is made.
        if open {
            let new = p.assets.iter().filter(|ca| c.position(ca.home, ca.asset).is_none()).count();
            open = c.positions.len() + new <= MAX_ASSETS;
            for ca in &p.assets {
                let pos = c.position(ca.home, ca.asset).map(|i| c.positions[i]).unwrap_or_default();
                if ca.value != 0 && (pos.open_value as u128 + ca.value as u128) * BPS as u128 > pos.peer_bond as u128 * COVER_BPS as u128 {
                    open = false;
                }
            }
        }
        if open {
            let mut slots = vec![];
            for ca in &p.assets {
                slots.push(c.slot(ca.home, ca.asset)?);
            }
            let id = a.config.claim_count + 1;
            let (address, bump) = Pubkey::find_program_address(&[CLAIM_SEED, a.config.key().as_ref(), &id.to_le_bytes()], &crate::ID);
            require_keys_eq!(a.claim.key(), address, VaultError::WrongAccount);
            create_pda(&a.claim, &payer, &system, Claim::space(p.assets.len(), p.records.len()), &[CLAIM_SEED, a.config.key().as_ref(), &id.to_le_bytes(), &[bump]])?;
            let t = crate::util::now()?;
            save(
                &a.claim,
                &Claim {
                    id,
                    operator,
                    last_at: t,
                    opened_at: t,
                    held: false,
                    decided: false,
                    accepted: false,
                    answers: 1,
                    objections: 0,
                    payout: 0,
                    assets: p.assets.clone(),
                    records: p.records.clone(),
                    bump,
                },
            )?;
            let (stake, stake_bump) = Pubkey::find_program_address(&[STAKE_SEED, a.config.key().as_ref(), &id.to_le_bytes(), operator.as_ref()], &crate::ID);
            require_keys_eq!(a.stake.key(), stake, VaultError::WrongAccount);
            create_pda(&a.stake, &payer, &system, 8 + Stake::INIT_SPACE, &[STAKE_SEED, a.config.key().as_ref(), &id.to_le_bytes(), operator.as_ref(), &[stake_bump]])?;
            save(&a.stake, &Stake { answers: 1, objections: 0, bump: stake_bump })?;
            a.config.claim_count = id;
            c.deposits -= deposit;
            c.open_claims += 1;
            for (ca, &i) in p.assets.iter().zip(&slots) {
                let pos = &mut c.positions[i];
                pos.open_value += ca.value;
                // Only a claim that opened carries it: otherwise it can be
                // carried again.
                if ca.peer_bond != 0 {
                    pos.peer_bond_carried = true;
                }
            }
        }
    }
    if p.exit {
        c.exited = true;
    }
    Ok(())
}

/// D109, D129: every bond of the chain here. Its 80% backs each asset's
/// receipts, its 20% goes to the submitter, all of it to the backing when
/// the operator submitted its own false message. Settled per asset by
/// `settle_slash`, so that a message names no account per asset.
fn slash<'info>(ctx: Context<'info, SubmitMessage<'info>>, operator: Pubkey) -> Result<()> {
    let a = ctx.accounts;
    let submitter = a.submitter.key();
    let c = &mut a.chain;
    // The first slash takes the bonds; a later false message finds none,
    // and must not take the first submitter's share.
    if c.slashed {
        return Ok(());
    }
    c.slashed = true;
    c.slasher = submitter;
    for p in c.positions.iter_mut() {
        let amount = std::mem::take(&mut p.bond);
        p.stated = 0;
        let share = if submitter == operator { 0 } else { (amount as u128 * (BPS - BACKING_SHARE_BPS) as u128 / BPS as u128) as u64 };
        p.slash_share += share;
        p.slash_backing += amount - share;
    }
    Ok(())
}

#[derive(Accounts)]
#[instruction(operator: Pubkey, btc: Btc)]
pub struct SubmitMessage<'info> {
    #[account(mut, seeds = [CONFIG_SEED, &[config.peer]], bump = config.bump)]
    pub config: Box<Account<'info, Config>>,
    #[account(mut, seeds = [CHAIN_SEED, config.key().as_ref(), operator.as_ref()], bump = chain.bump)]
    pub chain: Box<Account<'info, Chain>>,
    #[account(seeds = [REAL_SEED, config.key().as_ref(), btc.real.hash.as_ref(), &btc.real.height.to_le_bytes(), &btc.real.epoch_time.to_le_bytes()], bump = real.bump)]
    pub real: Box<Account<'info, Real>>,
    /// CHECK: a finished walk from the real block down; read and checked.
    pub walk: UncheckedAccount<'info>,
    /// CHECK: the message's block in the light client; read and checked.
    pub node: UncheckedAccount<'info>,
    /// CHECK: the account publishing this message's batch; checked and
    /// created in the handler.
    #[account(mut)]
    pub message: UncheckedAccount<'info>,
    /// CHECK: the claim this message would open, number `claim_count + 1`;
    /// checked and created only if it opens.
    #[account(mut)]
    pub claim: UncheckedAccount<'info>,
    /// CHECK: the operator's deposits in that claim; created with it.
    #[account(mut)]
    pub stake: UncheckedAccount<'info>,
    #[account(mut)]
    pub submitter: Signer<'info>,
    /// The transaction and batch, when uploaded to a buffer of the submitter.
    #[account(seeds = [BUFFER_SEED, submitter.key().as_ref()], bump = buffer.bump)]
    pub buffer: Option<Box<Account<'info, Buffer>>>,
    pub system_program: Program<'info, System>,
}
