use anchor_lang::prelude::*;
use ipow_light_client::bitcoin::sha256d;
use ipow_protocol::bitcoin_tx::read;

use crate::constants::*;
use crate::errors::VaultError;
use crate::state::{Btc, Chain, Config, Real};
use crate::util::{pair_commitment, require_real};

/// Registers the caller's pair chain: the coin made by a Bitcoin transaction
/// that names the caller here and `peer_operator` on Ethereum (D106).
pub fn handler(ctx: Context<RegisterChain>, peer_operator: [u8; 20], btc: Btc, coin_index: u32, tag_index: u32) -> Result<()> {
    require!(peer_operator != [0u8; 20], VaultError::ZeroAddress);
    require_real(&btc, &ctx.accounts.walk, &ctx.accounts.node)?;
    let v = read(&btc.raw_tx, 0, coin_index as u64, tag_index as u64)?;
    require!(v.has_coin, VaultError::NoCoin);
    let operator = ctx.accounts.operator.key();
    require!(v.has_tag && v.tag == pair_commitment(ctx.accounts.config.peer, &ctx.accounts.config.peer_vault, &peer_operator, &ctx.accounts.config.key(), &operator), VaultError::WrongTag);

    let c = &mut ctx.accounts.chain;
    c.operator = operator;
    c.peer_operator = peer_operator;
    c.coin_txid = sha256d(&btc.raw_tx);
    c.coin_vout = coin_index;
    c.bump = ctx.bumps.chain;
    Ok(())
}

#[derive(Accounts)]
#[instruction(peer_operator: [u8; 20], btc: Btc)]
pub struct RegisterChain<'info> {
    #[account(seeds = [CONFIG_SEED, &[config.peer]], bump = config.bump)]
    pub config: Account<'info, Config>,
    #[account(init, payer = operator, space = 8 + Chain::INIT_SPACE, seeds = [CHAIN_SEED, config.key().as_ref(), operator.key().as_ref()], bump)]
    pub chain: Account<'info, Chain>,
    #[account(seeds = [REAL_SEED, config.key().as_ref(), btc.real.hash.as_ref(), &btc.real.height.to_le_bytes(), &btc.real.epoch_time.to_le_bytes()], bump = real.bump)]
    pub real: Account<'info, Real>,
    /// CHECK: a finished walk from the real block down; read and checked.
    pub walk: UncheckedAccount<'info>,
    /// CHECK: the transaction's block in the light client; read and checked.
    pub node: UncheckedAccount<'info>,
    #[account(mut)]
    pub operator: Signer<'info>,
    pub system_program: Program<'info, System>,
}
