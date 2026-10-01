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
    /// Attests of locks so far: attest `n` is at ["fast", n] (section 11.7).
    pub attest_count: u64,
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
    /// When it opened: an attest waits only for a claim opened in time
    /// (section 11.7).
    pub opened_at: i64,
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
    /// For the attester who pays it at once on Ethereum, or the user
    /// (D122). Burned with the amount.
    pub fast_fee: u64,
    /// When it was made, in its REQUEST record (D124).
    pub requested_at: i64,
    pub fee_paid: bool,
    pub bump: u8,
}

/// What became of a lock on Ethereum here: its receipt issued, or given up
/// by its recipient. It exists once either happened, so each happens once.
#[account]
#[derive(InitSpace)]
pub struct LockMark {
    pub lock_id: u64,
    /// Its receipt counted: issued after its claim, or delivered by an
    /// attest settled with the true record.
    pub issued: bool,
    pub given_up: bool,
    /// Attests of the lock so far (D126), and those settled. With any, its
    /// receipt comes by settling them, or after `first_at` and 7 days when
    /// none stated the true record.
    pub attests: u32,
    pub settled: u32,
    /// When its first attest was made: it takes attests for 7 days from it.
    pub first_at: i64,
    /// Its latest attest: each names the one before, and they are settled
    /// in the order made.
    pub last_attest: u64,
    pub bump: u8,
}

/// A lock on Ethereum whose receipt an attester issued at once, with the
/// LOCK record it stated and the vETH it locked (section 11.7, D123).
#[account]
#[derive(InitSpace)]
pub struct FastLock {
    /// Its number, counted in `Config::attest_count`: guardians find every
    /// attest by it, also one naming a lock that does not exist.
    pub id: u64,
    pub lock_id: u64,
    pub attester: Pubkey,
    pub amount: u64,
    pub recipient: Pubkey,
    pub fee: u64,
    pub fast_fee: u64,
    pub locked_at: i64,
    /// vETH locked by the attester, 1.25 times the amount.
    pub collateral: u64,
    pub attested_at: i64,
    /// A claim carrying the same record, opened in time; zero if none.
    pub claim: u64,
    /// The attest of the same lock made before it, zero if none: settled
    /// first (D126).
    pub prev: u64,
    /// The attester's vETH was burned: when the true record arrives, the
    /// receipt goes to the attester.
    pub burned: bool,
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

/// The batch of one processed message, published so that anyone can bring
/// the message to the other network (section 11.3). Anyone may close it
/// `MESSAGE_KEPT` after it was processed, its rent back to whoever paid it.
#[account]
pub struct Message {
    pub operator: Pubkey,
    pub payer: Pubkey,
    pub index: u64,
    pub processed_at: i64,
    pub batch: Vec<u8>,
    pub bump: u8,
}

impl Message {
    pub fn space(batch_len: usize) -> usize {
        8 + 32 + 32 + 8 + 8 + 4 + batch_len + 1
    }
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
