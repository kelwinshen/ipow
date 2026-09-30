use anchor_lang::prelude::*;

use crate::constants::*;
use crate::errors::VaultError;
use crate::state::Config;
use crate::util::sha256;

/// The tag of checkpoint number `n`.
pub fn checkpoint_tag(n: u64) -> [u8; 32] {
    sha256(&[b"iPoW checkpoint", crate::ID.as_ref(), &n.to_le_bytes()])
}

/// Opens a job whose only use is to make a real block (D116), with the
/// escrow D118 asks, or the protocol's lowest if that is more. The funder
/// pays `paid` (the fees, and what is above them goes to the operator, D79)
/// and receives the fees back if nobody takes it.
pub fn handler(ctx: Context<OpenCheckpoint>, confirmations: u16, paid: u64) -> Result<()> {
    let fee = ipow_protocol::fees::commitment_fee_for(confirmations)?;
    let lowest = fee.checked_mul(ipow_protocol::constants::MIN_ESCROW_MULTIPLE).ok_or(VaultError::Overflow)?;
    let escrow = lowest.max(ctx.accounts.config.min_certifying_escrow);
    let n = ctx.accounts.config.checkpoint_count + 1;
    ctx.accounts.config.checkpoint_count = n;
    let a = &ctx.accounts;
    let seeds: &[&[u8]] = &[CONFIG_SEED, &[a.config.bump]];
    ipow_protocol::cpi::open_job(
        CpiContext::new_with_signer(
            ipow_protocol::ID,
            ipow_protocol::cpi::accounts::OpenJob {
                protocol: a.protocol.to_account_info(),
                application: a.application.to_account_info(),
                job: a.job.to_account_info(),
                tag_record: a.tag_record.to_account_info(),
                vault: a.protocol_vault.to_account_info(),
                key: a.config.to_account_info(),
                funder: a.funder.to_account_info(),
                system_program: a.system_program.to_account_info(),
            },
            &[seeds],
        ),
        checkpoint_tag(n),
        escrow,
        ipow_protocol::constants::DEFAULT_ESCROW_FEE_BPS,
        confirmations,
        0,
        a.funder.key(),
        paid,
    )
}

#[derive(Accounts)]
pub struct OpenCheckpoint<'info> {
    #[account(mut, seeds = [CONFIG_SEED], bump = config.bump)]
    pub config: Account<'info, Config>,
    #[account(mut, seeds = [b"protocol"], bump = protocol.bump, seeds::program = ipow_protocol::ID)]
    pub protocol: Account<'info, ipow_protocol::state::Protocol>,
    /// CHECK: the vault's application record in the protocol; checked there.
    pub application: UncheckedAccount<'info>,
    /// CHECK: the new job, created by the protocol.
    #[account(mut)]
    pub job: UncheckedAccount<'info>,
    /// CHECK: the tag's record, created by the protocol.
    #[account(mut)]
    pub tag_record: UncheckedAccount<'info>,
    /// CHECK: the protocol's vault; checked there.
    #[account(mut)]
    pub protocol_vault: UncheckedAccount<'info>,
    #[account(mut)]
    pub funder: Signer<'info>,
    /// CHECK: the protocol program.
    #[account(address = ipow_protocol::ID)]
    pub protocol_program: UncheckedAccount<'info>,
    pub system_program: Program<'info, System>,
}
