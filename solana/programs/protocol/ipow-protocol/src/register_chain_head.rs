use anchor_lang::prelude::*;

use crate::bitcoin_tx::read;
use crate::constants::{OPERATOR_SEED, USED_TX_SEED};
use crate::duty::{chain_head_commitment, read_node};
use crate::errors::ProtocolError;
use crate::state::{BlockRef, Operator, UsedTx};
use ipow_light_client::bitcoin::sha256d;
use ipow_light_client::utils::tx_in_block;

/// Registers the caller's chain head on this network (D34): a coin made by a
/// Bitcoin transaction that names the caller (N22). The transaction can
/// never settle a job.
pub fn register(
    ctx: Context<RegisterChainHead>,
    txid: [u8; 32],
    block: BlockRef,
    raw_tx: Vec<u8>,
    siblings: Vec<[u8; 32]>,
    tx_index: u64,
    coin_index: u32,
    tag_index: u32,
) -> Result<()> {
    let operator = &mut ctx.accounts.operator;
    require!(!operator.chain_head_set, ProtocolError::ChainHeadExists);
    let node = read_node(&ctx.accounts.node, &block)?;
    require!(tx_in_block(&node, &raw_tx, &siblings, tx_index)?, ProtocolError::NotInBlock);

    let v = read(&raw_tx, 0, coin_index as u64, tag_index as u64)?;
    require!(v.has_coin, ProtocolError::NoCoin);
    require!(v.has_tag && v.tag == chain_head_commitment(&operator.owner), ProtocolError::WrongTag);

    // The record's address comes from `txid`, so it must be this transaction's.
    require!(sha256d(&raw_tx) == txid, ProtocolError::WrongAccount);
    ctx.accounts.used_tx.bump = ctx.bumps.used_tx;
    operator.chain_head_txid = txid;
    operator.chain_head_vout = coin_index;
    operator.chain_head_set = true;
    emit!(crate::ChainHeadSet { operator: operator.owner, txid, vout: coin_index });
    Ok(())
}

/// Moves the caller's chain head past a transaction that spent it and did
/// not settle a job, for example one that came after its deadline (N25).
/// Only the operator itself may do this.
pub fn advance(
    ctx: Context<RegisterChainHead>,
    txid: [u8; 32],
    block: BlockRef,
    raw_tx: Vec<u8>,
    siblings: Vec<[u8; 32]>,
    tx_index: u64,
    head_index: u32,
) -> Result<()> {
    let operator = &mut ctx.accounts.operator;
    require!(operator.chain_head_set, ProtocolError::NoChainHead);
    let node = read_node(&ctx.accounts.node, &block)?;
    require!(tx_in_block(&node, &raw_tx, &siblings, tx_index)?, ProtocolError::NotInBlock);

    let v = read(&raw_tx, head_index as u64, head_index as u64, u64::MAX)?;
    require!(
        v.spent_txid == operator.chain_head_txid && v.spent_vout == operator.chain_head_vout,
        ProtocolError::WrongChainHead
    );
    require!(v.has_coin, ProtocolError::NoCoin);

    require!(sha256d(&raw_tx) == txid, ProtocolError::WrongAccount);
    ctx.accounts.used_tx.bump = ctx.bumps.used_tx;
    operator.chain_head_txid = txid;
    operator.chain_head_vout = head_index;
    emit!(crate::ChainHeadSet { operator: operator.owner, txid, vout: head_index });
    Ok(())
}

#[derive(Accounts)]
#[instruction(txid: [u8; 32])]
pub struct RegisterChainHead<'info> {
    #[account(mut, seeds = [OPERATOR_SEED, owner.key().as_ref()], bump = operator.bump)]
    pub operator: Account<'info, Operator>,
    /// CHECK: the light client's block that holds the transaction; checked
    /// in `read_node`.
    pub node: UncheckedAccount<'info>,
    /// The transaction's record: it can only be created once (D80).
    #[account(
        init,
        payer = owner,
        space = 8 + UsedTx::INIT_SPACE,
        seeds = [USED_TX_SEED, &txid],
        bump
    )]
    pub used_tx: Account<'info, UsedTx>,
    #[account(mut)]
    pub owner: Signer<'info>,
    pub system_program: Program<'info, System>,
}
