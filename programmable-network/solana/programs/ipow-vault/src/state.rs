use anchor_lang::prelude::*;

use crate::constants::{MAX_ACTING_BYTES, MAX_BATCH, MAX_RAW_TX};

/// The vault's one settings record. Its address is the key the vault
/// registered with the protocol, the authority of vETH, and the holder of
/// deposits and lamport credits.
#[account]
#[derive(InitSpace)]
pub struct Config {
    /// The vault on Ethereum, as a registration names it (D106).
    pub ethereum_vault: [u8; 20],
    /// V1: the flat deposit to object or answer, and the operator's deposit
    /// for a claim, in lamports.
    pub deposit: u64,
    /// D118: the least escrow of a job whose proof block counts as real, in
    /// lamports.
    pub min_certifying_escrow: u64,
    pub claim_count: u64,
    pub request_count: u64,
    pub checkpoint_count: u64,
    pub bump: u8,
}

/// An operator's pair chain (D106) and what it keeps in the vault.
#[account]
#[derive(InitSpace)]
pub struct Chain {
    pub operator: Pubkey,
    /// The same operator on Ethereum.
    pub peer_operator: [u8; 20],
    pub coin_txid: [u8; 32],
    pub coin_vout: u32,
    pub exited: bool,
    /// A false record was proven here: the bond is gone (D109).
    pub slashed: bool,
    /// A claim of the chain was refused here: no other claim acts (D111).
    pub refused: bool,
    pub messages: u64,
    /// vETH held for the chain, in gwei (D110).
    pub bond: u64,
    /// The part of the bond its BOND record stated for Solana.
    pub stated: u64,
    /// Lamports for the flat deposits of its claims.
    pub deposits: u64,
    /// The ETH bond on Ethereum, counted once its BOND claim is official.
    pub peer_bond: u64,
    pub peer_bond_carried: bool,
    /// The value of its open claims here, in gwei.
    pub open_value: u64,
    pub open_claims: u32,
    pub bump: u8,
}

/// The acting records of one message (D111).
#[account]
#[derive(InitSpace)]
pub struct Claim {
    pub id: u64,
    pub operator: Pubkey,
    pub last_at: i64,
    pub held: bool,
    pub decided: bool,
    pub accepted: bool,
    pub value: u64,
    pub peer_bond: u64,
    pub answers: u32,
    pub objections: u32,
    /// Set when decided: what each deposit of the winning side collects.
    pub payout: u64,
    /// The LOCK records it carries, one after the other.
    #[max_len(MAX_ACTING_BYTES)]
    pub records: Vec<u8>,
    pub bump: u8,
}

/// The deposits one address put down on each side of one claim.
#[account]
#[derive(InitSpace)]
pub struct Stake {
    pub answers: u32,
    pub objections: u32,
    pub bump: u8,
}

/// A block known to be on real Bitcoin (D108).
#[account]
#[derive(InitSpace)]
pub struct Real {
    pub bump: u8,
}

/// A burn of vETH for ETH on Ethereum (section 11.5).
#[account]
#[derive(InitSpace)]
pub struct Request {
    pub id: u64,
    pub owner: Pubkey,
    /// In gwei.
    pub amount: u64,
    pub to: [u8; 20],
    pub fee: u64,
    pub fee_paid: bool,
    pub bump: u8,
}

/// What became of a lock on Ethereum here: its receipt issued, or given up
/// by its recipient. It exists once either happened, so each happens once.
#[account]
#[derive(InitSpace)]
pub struct LockMark {
    pub lock_id: u64,
    pub issued: bool,
    pub given_up: bool,
    pub bump: u8,
}

/// Lamports and vETH an address can withdraw.
#[account]
#[derive(InitSpace)]
pub struct Credit {
    pub owner: Pubkey,
    pub lamports: u64,
    pub veth: u64,
    pub bump: u8,
}

/// A message's Bitcoin transaction and batch, uploaded in pieces when they do
/// not fit in one Solana transaction with its proof.
#[account]
#[derive(InitSpace)]
pub struct Buffer {
    pub owner: Pubkey,
    #[max_len(MAX_RAW_TX)]
    pub raw_tx: Vec<u8>,
    #[max_len(MAX_BATCH)]
    pub batch: Vec<u8>,
    pub bump: u8,
}

/// A Bitcoin transaction in a block below a real block (D108).
#[derive(AnchorSerialize, AnchorDeserialize, Clone)]
pub struct Btc {
    pub block: ipow_protocol::state::BlockRef,
    pub raw_tx: Vec<u8>,
    pub siblings: Vec<[u8; 32]>,
    pub tx_index: u64,
    /// A block already recorded as real, with `block` below it.
    pub real: ipow_protocol::state::BlockRef,
}
