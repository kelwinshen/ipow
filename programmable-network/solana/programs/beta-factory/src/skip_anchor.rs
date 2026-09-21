use anchor_lang::prelude::*;
use ipow::state::GlobalHeader;

use crate::anchor_verify::verify_and_advance;
use crate::errors::FactoryError;
use crate::state::{AnchorStatus, FactoryConfig, Party, ProcessedAnchor};

/// Permissionless. Advances past an anchor whose preimage nobody has
/// revealed, once the header carrying it is `t_skip_secs` old (DESIGN_V2
/// §6.2). Records whatever payload it carried so `audit_skipped_release`
/// can still judge it if the preimage surfaces later.
#[allow(clippy::too_many_arguments)]
pub fn handler(
    ctx: Context<SkipAnchor>,
    txid_le: [u8; 32],
    tx_raw: Vec<u8>,
    proof_block_height: u64,
    branch_le: Vec<[u8; 32]>,
    index: u64,
) -> Result<()> {
    let now = Clock::get()?.unix_timestamp;
    require!(
        now >= ctx.accounts.header.timestamp as i64 + ctx.accounts.config.params.t_skip_secs,
        FactoryError::SkipNotReady
    );
    let party = &mut ctx.accounts.party;
    let parsed = verify_and_advance(
        party,
        txid_le,
        &tx_raw,
        &ctx.accounts.header,
        proof_block_height,
        &branch_le,
        index,
    )?;
    let pa = &mut ctx.accounts.processed;
    pa.party_id = party.party_id;
    pa.txid_le = txid_le;
    pa.status = AnchorStatus::Skipped;
    pa.block_height = proof_block_height;
    pa.processed_at = now;
    if let Some(p) = parsed.payload {
        pa.kind = p.kind;
        pa.statement_hash = p.statement_hash;
    }
    Ok(())
}

#[derive(Accounts)]
#[instruction(txid_le: [u8; 32])]
pub struct SkipAnchor<'info> {
    #[account(seeds = [b"config"], bump)]
    pub config: Box<Account<'info, FactoryConfig>>,
    #[account(mut)]
    pub party: Box<Account<'info, Party>>,
    #[account(init, payer = submitter, space = 8 + ProcessedAnchor::INIT_SPACE, seeds = [b"anchor", txid_le.as_ref()], bump)]
    pub processed: Box<Account<'info, ProcessedAnchor>>,
    pub header: Box<Account<'info, GlobalHeader>>,
    #[account(mut)]
    pub submitter: Signer<'info>,
    pub system_program: Program<'info, System>,
}
