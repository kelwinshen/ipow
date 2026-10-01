use anchor_lang::prelude::*;
use anchor_spl::token::{Mint, Token, TokenAccount};
use ipow_light_client::bitcoin::sha256d;
use ipow_protocol::bitcoin_tx::read;

use crate::constants::*;
use crate::errors::VaultError;
use crate::state::{Btc, Buffer, Chain, Claim, Config, LockMark, Message, Real, Request, Stake};
use crate::util::{create_pda, credit_to, message_payload, require_real, save, u64_at, Veth};

/// What a message asks once nothing false was found in it.
struct Plan {
    /// LOCK records, the acting part of the message here.
    locks: Vec<u8>,
    /// Their value, in gwei.
    value: u64,
    /// A BOND record for Ethereum carried here for the first time.
    peer_bond: u64,
    /// A BOND record for Solana.
    stated: Option<u64>,
    /// The request accounts of true REQUEST records, in order.
    requests: Vec<usize>,
    exit: bool,
}

/// Processes the next message of an operator's pair chain, with its batch
/// (D107, D108, D109, D114). Anyone may submit it. The remaining accounts
/// are, in the order of the batch, the request of each REQUEST record and
/// the lock mark of each CANCEL record. With a buffer, the transaction and
/// the batch are read from it, and the arguments' are empty.
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

    let c = &mut ctx.accounts.chain;
    c.coin_txid = sha256d(&btc.raw_tx);
    c.coin_vout = input_index;
    let index = c.messages;
    c.messages += 1;

    // The batch is published in an account of its own: nobody can hide it
    // from, or fake it for, whoever brings the message to the other network.
    let (address, bump) = Pubkey::find_program_address(&[MESSAGE_SEED, operator.as_ref(), &index.to_le_bytes()], &crate::ID);
    require_keys_eq!(ctx.accounts.message.key(), address, VaultError::WrongAccount);
    create_pda(
        &ctx.accounts.message,
        &ctx.accounts.submitter.to_account_info(),
        &ctx.accounts.system_program.to_account_info(),
        Message::space(batch.len()),
        &[MESSAGE_SEED, operator.as_ref(), &index.to_le_bytes(), &[bump]],
    )?;
    save(&ctx.accounts.message, &Message { operator, payer: ctx.accounts.submitter.key(), index, processed_at: crate::util::now()?, batch: batch.clone(), bump })?;

    // Section 11.3: too large to be judged everywhere is false.
    let plan = if btc.raw_tx.len() > MAX_RAW_TX || batch.len() > MAX_BATCH {
        None
    } else {
        judge(&batch, c, ctx.remaining_accounts)?
    };
    match plan {
        Some(p) => apply(ctx, operator, p),
        None => slash(ctx, operator),
    }
}

/// Judges the records about Solana and measures the rest. `None` when the
/// message is false: a record that is not so here, or a batch that does not
/// parse, since its hash is what the operator wrote.
fn judge(batch: &[u8], c: &Chain, remaining: &[AccountInfo]) -> Result<Option<Plan>> {
    let mut plan = Plan { locks: vec![], value: 0, peer_bond: 0, stated: None, requests: vec![], exit: false };
    let mut o = 0;
    let mut next = 0;
    let mut home = 0;
    while o < batch.len() {
        if batch[o] == REQUEST || batch[o] == CANCEL {
            home += 1;
            if home > MAX_HOME_RECORDS {
                return Ok(None);
            }
        }
        match batch[o] {
            LOCK => {
                if o + LOCK_LEN > batch.len() {
                    return Ok(None);
                }
                plan.locks.extend_from_slice(&batch[o..o + LOCK_LEN]);
                // More than any lock can hold: false, as Ethereum finds it.
                // The receipt and the fast fee are both minted here.
                let Some(v) = plan.value.checked_add(u64_at(batch, o + 9)).and_then(|v| v.checked_add(u64_at(batch, o + 57))) else {
                    return Ok(None);
                };
                plan.value = v;
                o += LOCK_LEN;
            }
            REQUEST => {
                if o + REQUEST_LEN > batch.len() {
                    return Ok(None);
                }
                let id = u64_at(batch, o + 1);
                let account = remaining.get(next).ok_or(VaultError::WrongAccount)?;
                let (address, _) = Pubkey::find_program_address(&[REQUEST_SEED, &id.to_le_bytes()], &crate::ID);
                require_keys_eq!(account.key(), address, VaultError::WrongAccount);
                if account.owner != &crate::ID || account.data_is_empty() {
                    return Ok(None);
                }
                let r = {
                    let data = account.try_borrow_data()?;
                    Request::try_deserialize(&mut &data[..])?
                };
                let to: [u8; 20] = batch[o + 17..o + 37].try_into().unwrap();
                if r.amount != u64_at(batch, o + 9) || r.to != to || r.fee != u64_at(batch, o + 37) || r.fast_fee != u64_at(batch, o + 45)
                    || r.requested_at != u64_at(batch, o + 53) as i64
                {
                    return Ok(None);
                }
                plan.requests.push(next);
                next += 1;
                o += REQUEST_LEN;
            }
            CANCEL => {
                if o + CANCEL_LEN > batch.len() {
                    return Ok(None);
                }
                let id = u64_at(batch, o + 1);
                let account = remaining.get(next).ok_or(VaultError::WrongAccount)?;
                let (address, _) = Pubkey::find_program_address(&[LOCK_SEED, &id.to_le_bytes()], &crate::ID);
                require_keys_eq!(account.key(), address, VaultError::WrongAccount);
                if account.owner != &crate::ID || account.data_is_empty() {
                    return Ok(None);
                }
                let m = {
                    let data = account.try_borrow_data()?;
                    LockMark::try_deserialize(&mut &data[..])?
                };
                if !m.given_up {
                    return Ok(None);
                }
                next += 1;
                o += CANCEL_LEN;
            }
            BOND => {
                if o + BOND_LEN > batch.len() {
                    return Ok(None);
                }
                let amount = u64_at(batch, o + 2);
                match batch[o + 1] {
                    // A fact of Ethereum, judged there (D109). The first one
                    // carried in a claim counts here.
                    ETHEREUM => {
                        if !c.peer_bond_carried && plan.peer_bond == 0 && amount != 0 {
                            plan.peer_bond = amount;
                        }
                    }
                    // Judged here: once per chain, from the bond (D110). The
                    // same BOND again is true and changes nothing: it is
                    // carried again when its claim could not open on Ethereum.
                    SOLANA => {
                        if plan.stated.is_some() || amount == 0 {
                            return Ok(None);
                        }
                        let bad = if c.stated != 0 { amount != c.stated } else { amount > c.bond };
                        if bad {
                            return Ok(None);
                        }
                        plan.stated = Some(amount);
                    }
                    _ => return Ok(None),
                }
                o += BOND_LEN;
            }
            EXIT => {
                if o + 1 != batch.len() {
                    return Ok(None);
                }
                plan.exit = true;
                o += 1;
            }
            _ => return Ok(None),
        }
    }
    Ok(Some(plan))
}

fn apply<'info>(ctx: Context<'info, SubmitMessage<'info>>, operator: Pubkey, p: Plan) -> Result<()> {
    let a = ctx.accounts;
    let payer = a.submitter.to_account_info();
    let system = a.system_program.to_account_info();

    // Fees of true requests, once each, never to a slashed operator (the fee
    // then waits for another).
    let mut fees = 0u64;
    if !a.chain.slashed {
        for &i in &p.requests {
            let account = &ctx.remaining_accounts[i];
            let mut r = {
                let data = account.try_borrow_data()?;
                Request::try_deserialize(&mut &data[..])?
            };
            if !r.fee_paid {
                r.fee_paid = true;
                fees = fees.checked_add(r.fee).ok_or(VaultError::Overflow)?;
                save(account, &r)?;
            }
        }
    }
    if fees > 0 {
        credit_to(&a.operator_credit, &operator, 0, fees, &payer, &system)?;
    }

    // The acting records become one claim (D111), unless the chain was
    // refused or proven false here, they pass 80% of its bond on Ethereum,
    // they are too many, or its deposit lamports cannot pay the deposit.
    // Then nothing acts, and they can be carried again.
    let acting = !p.locks.is_empty() || p.peer_bond != 0;
    let mut opened = false;
    if acting {
        let c = &a.chain;
        let covered = p.value == 0
            || (c.open_value as u128 + p.value as u128) * BPS as u128 <= c.peer_bond as u128 * COVER_BPS as u128;
        let deposit = a.config.deposit;
        if !c.refused && !c.slashed && covered && p.locks.len() <= MAX_ACTING_BYTES && c.deposits >= deposit {
            let id = a.config.claim_count + 1;
            let (address, bump) = Pubkey::find_program_address(&[CLAIM_SEED, &id.to_le_bytes()], &crate::ID);
            require_keys_eq!(a.claim.key(), address, VaultError::WrongAccount);
            create_pda(&a.claim, &payer, &system, 8 + Claim::INIT_SPACE, &[CLAIM_SEED, &id.to_le_bytes(), &[bump]])?;
            let t = crate::util::now()?;
            save(
                &a.claim,
                &Claim {
                    id,
                    operator,
                    last_at: t,
                    held: false,
                    decided: false,
                    accepted: false,
                    value: p.value,
                    peer_bond: p.peer_bond,
                    answers: 1,
                    objections: 0,
                    payout: 0,
                    opened_at: t,
                    records: p.locks,
                    bump,
                },
            )?;
            let (stake, stake_bump) = Pubkey::find_program_address(&[STAKE_SEED, &id.to_le_bytes(), operator.as_ref()], &crate::ID);
            require_keys_eq!(a.stake.key(), stake, VaultError::WrongAccount);
            create_pda(&a.stake, &payer, &system, 8 + Stake::INIT_SPACE, &[STAKE_SEED, &id.to_le_bytes(), operator.as_ref(), &[stake_bump]])?;
            save(&a.stake, &Stake { answers: 1, objections: 0, bump: stake_bump })?;
            a.config.claim_count = id;
            let c = &mut a.chain;
            c.deposits -= deposit;
            c.open_value += p.value;
            c.open_claims += 1;
            opened = true;
        }
    }

    let c = &mut a.chain;
    if let Some(s) = p.stated {
        c.stated = s;
    }
    // Only a claim that opened carries it: otherwise it can be carried again.
    if opened && p.peer_bond != 0 {
        c.peer_bond_carried = true;
    }
    if p.exit {
        c.exited = true;
    }
    Ok(())
}

/// D109: the whole bond. 80% is burned, which backs vETH as the ETH it
/// stands for, and 20% goes to the submitter; all of it is burned when the
/// operator submitted its own false message.
fn slash<'info>(ctx: Context<'info, SubmitMessage<'info>>, operator: Pubkey) -> Result<()> {
    let a = ctx.accounts;
    let c = &mut a.chain;
    let amount = c.bond;
    c.bond = 0;
    c.stated = 0;
    c.slashed = true;
    let submitter = a.submitter.key();
    let to_submitter = if submitter == operator { 0 } else { (amount as u128 * (BPS - BACKING_SHARE_BPS) as u128 / BPS as u128) as u64 };
    if to_submitter > 0 {
        credit_to(&a.submitter_credit, &submitter, 0, to_submitter, &a.submitter.to_account_info(), &a.system_program.to_account_info())?;
    }
    let veth = Veth {
        config: &a.config.to_account_info(),
        config_bump: a.config.bump,
        mint: &a.mint.to_account_info(),
        holding: &a.holding.to_account_info(),
        token_program: &a.token_program,
    };
    veth.burn_held(amount - to_submitter)
}

#[derive(Accounts)]
#[instruction(operator: Pubkey, btc: Btc)]
pub struct SubmitMessage<'info> {
    #[account(mut, seeds = [CONFIG_SEED], bump = config.bump)]
    pub config: Account<'info, Config>,
    #[account(mut, seeds = [CHAIN_SEED, operator.as_ref()], bump = chain.bump)]
    pub chain: Account<'info, Chain>,
    #[account(seeds = [REAL_SEED, btc.real.hash.as_ref(), &btc.real.height.to_le_bytes(), &btc.real.epoch_time.to_le_bytes()], bump = real.bump)]
    pub real: Account<'info, Real>,
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
    /// CHECK: the operator's credit; checked when used.
    #[account(mut)]
    pub operator_credit: UncheckedAccount<'info>,
    /// CHECK: the submitter's credit; checked when used.
    #[account(mut)]
    pub submitter_credit: UncheckedAccount<'info>,
    #[account(mut, seeds = [MINT_SEED], bump)]
    pub mint: Account<'info, Mint>,
    #[account(mut, seeds = [HOLDING_SEED], bump)]
    pub holding: Account<'info, TokenAccount>,
    #[account(mut)]
    pub submitter: Signer<'info>,
    /// The transaction and batch, when uploaded to a buffer of the submitter.
    #[account(seeds = [BUFFER_SEED, submitter.key().as_ref()], bump = buffer.bump)]
    pub buffer: Option<Account<'info, Buffer>>,
    pub token_program: Program<'info, Token>,
    pub system_program: Program<'info, System>,
}
