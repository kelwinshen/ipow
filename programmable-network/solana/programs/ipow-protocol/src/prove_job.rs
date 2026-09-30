use anchor_lang::prelude::*;

use crate::bitcoin_tx::read;
use crate::constants::{APPLICATION_SEED, JOB_SEED, OPERATOR_SEED, PROOF_RANGE, USED_TX_SEED};
use crate::duty::{deadline, lock_time_of, read_node, read_walk, status, tag_payload, Status};
use crate::errors::ProtocolError;
use crate::state::{Application, BlockRef, Job, Operator, UsedTx};
use ipow_light_client::bitcoin::sha256d;
use ipow_light_client::utils::tx_in_block;

#[derive(AnchorSerialize, AnchorDeserialize, Clone)]
pub struct Proof {
    /// The block that holds the transaction.
    pub proof_block: BlockRef,
    /// A block on top of it, far enough for the confirmations.
    pub tip: BlockRef,
    /// The transaction, without witness data, and its id.
    pub raw_tx: Vec<u8>,
    pub txid: [u8; 32],
    pub siblings: Vec<[u8; 32]>,
    pub tx_index: u64,
    /// The input that spends the chain head; the output with the same number
    /// is the next chain head (N23).
    pub head_index: u32,
    /// The output that carries the tag.
    pub tag_index: u32,
}

/// Proves the operator's tagged transaction (D6, D9, D15, D27, D30, D54).
/// Only the operator of the job submits it (D89). The links from the anchor
/// to the proof's block, and from that block to the tip, come as two
/// finished walks of the light client.
pub fn handler(ctx: Context<ProveJob>, proof: Proof) -> Result<()> {
    let job = &mut ctx.accounts.job;
    let now = Clock::get()?.unix_timestamp;
    require!(status(job, now) == Status::Assigned, ProtocolError::NotAssigned);
    require_keys_eq!(ctx.accounts.owner.key(), job.operator, ProtocolError::NotOperator);
    require!(job.anchored_at != 0, ProtocolError::NotAnchored);
    // D27.
    require!(now <= deadline(job)?, ProtocolError::DeadlinePassed);

    // D15, D41, D68: blocks 1 to 25 after the anchor, enough blocks on top.
    let at = proof.proof_block;
    require!(
        at.height > job.anchor.height && at.height - job.anchor.height <= PROOF_RANGE as u32,
        ProtocolError::OutsideProofRange
    );
    read_walk(&ctx.accounts.walk_to_proof, &at, &job.anchor)?;
    let tip = proof.tip;
    require!(
        tip.height >= at.height && (tip.height - at.height) as u64 + 1 >= job.confirmations as u64,
        ProtocolError::NotEnoughConfirmations
    );
    read_walk(&ctx.accounts.walk_to_tip, &tip, &at)?;

    let node = read_node(&ctx.accounts.proof_node, &at)?;
    require!(tx_in_block(&node, &proof.raw_tx, &proof.siblings, proof.tx_index)?, ProtocolError::NotInBlock);
    require!(sha256d(&proof.raw_tx) == proof.txid, ProtocolError::WrongAccount);

    let v = read(&proof.raw_tx, proof.head_index as u64, proof.head_index as u64, proof.tag_index as u64)?;
    // D30: it comes from the operator.
    let operator = &mut ctx.accounts.operator;
    require!(operator.chain_head_set, ProtocolError::NoChainHead);
    require!(
        v.spent_txid == operator.chain_head_txid && v.spent_vout == operator.chain_head_vout,
        ProtocolError::WrongChainHead
    );
    require!(v.has_coin, ProtocolError::NoCoin);
    // D6.
    require!(v.has_tag && v.tag == tag_payload(&job.tag), ProtocolError::WrongTag);

    ctx.accounts.used_tx.bump = ctx.bumps.used_tx;
    operator.chain_head_txid = proof.txid;
    operator.chain_head_vout = proof.head_index;

    // D54: the duty ends here.
    job.proof_block = at;
    job.tip = tip;
    job.deepest = job.anchor;
    job.txid = proof.txid;
    job.proven_at = now;
    job.lock_end = now + lock_time_of(job, &ctx.accounts.application)?;
    emit!(crate::JobProven { job_id: job.id, txid: proof.txid, lock_end: job.lock_end });
    Ok(())
}

#[derive(Accounts)]
#[instruction(proof: Proof)]
pub struct ProveJob<'info> {
    #[account(mut, seeds = [JOB_SEED, &job.id.to_le_bytes()], bump = job.bump)]
    pub job: Account<'info, Job>,
    #[account(seeds = [APPLICATION_SEED, job.application.as_ref()], bump = application.bump)]
    pub application: Account<'info, Application>,
    #[account(mut, seeds = [OPERATOR_SEED, owner.key().as_ref()], bump = operator.bump)]
    pub operator: Account<'info, Operator>,
    /// CHECK: the light client's block that holds the transaction.
    pub proof_node: UncheckedAccount<'info>,
    /// CHECK: a finished light client walk from the proof's block to the anchor.
    pub walk_to_proof: UncheckedAccount<'info>,
    /// CHECK: a finished light client walk from the tip to the proof's block.
    pub walk_to_tip: UncheckedAccount<'info>,
    /// The transaction's record: it can only be created once (D80).
    #[account(
        init,
        payer = owner,
        space = 8 + UsedTx::INIT_SPACE,
        seeds = [USED_TX_SEED, &proof.txid],
        bump
    )]
    pub used_tx: Account<'info, UsedTx>,
    #[account(mut)]
    pub owner: Signer<'info>,
    pub system_program: Program<'info, System>,
}
