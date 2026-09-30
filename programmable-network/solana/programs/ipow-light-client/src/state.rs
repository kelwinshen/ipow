use anchor_lang::prelude::*;

use crate::constants::DAY_SLOTS;

/// Set once at initialize. The lowest allowed block number (D93) and the
/// difficulty limits. A deployment always has the limits of the rules; only
/// a test build can lower them.
#[account]
#[derive(InitSpace)]
pub struct Config {
    pub min_height: u32,
    pub max_target: [u8; 32],
    pub pow_limit: [u8; 32],
    pub bump: u8,
}

/// A stored Bitcoin block (D48). Its address comes from its hash, its
/// stated number and the time of the first block of its epoch, so a block
/// stored under a false number cannot disturb the entry honest operators
/// use (D64).
#[account]
#[derive(InitSpace)]
pub struct Node {
    pub hash: [u8; 32],
    pub prev_hash: [u8; 32],
    pub merkle_root: [u8; 32],
    pub bits: u32,
    pub time: u32,
    pub height: u32,
    pub epoch_time: u32,
    pub stored_at: i64,
    /// When the block first passed the jump guards, by anyone. Zero if never.
    pub anchored_at: i64,
    /// Who paid the rent; it goes back to them when the block is closed (D101).
    pub payer: Pubkey,
    pub bump: u8,
}

/// An epoch start: 6 blocks in a row with one difficulty (D44).
#[account]
#[derive(InitSpace)]
pub struct EpochStart {
    pub bits: u32,
    pub first_time: u32,
    pub height: u32,
    pub recorded_at: i64,
    /// Who paid the rent; it goes back to them when the record is closed (D101).
    pub payer: Pubkey,
    pub bump: u8,
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, Default, InitSpace)]
pub struct DaySlot {
    pub day: u32,
    /// The lowest target (highest difficulty) of the epoch starts whose
    /// first block is from that day. All zero means none.
    pub lowest_target: [u8; 32],
}

/// D45 reads the epoch starts of the last 4 weeks by day. The slot of a day
/// is `day % 32`, so a day older than 32 days is simply overwritten.
#[account]
#[derive(InitSpace)]
pub struct DayTable {
    pub slots: [DaySlot; DAY_SLOTS],
    pub bump: u8,
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, Default, PartialEq, Eq, Debug, InitSpace)]
pub struct BlockRef {
    pub hash: [u8; 32],
    pub height: u32,
    pub epoch_time: u32,
}

/// A walk down from a block through the blocks before it, done over several
/// transactions. The same checks as `walk` in `iPoWLightClient.sol`.
#[account]
#[derive(InitSpace)]
pub struct Walk {
    pub owner: Pubkey,
    pub descendant: BlockRef,
    /// The epoch time of the older epoch, if the walk crosses into one.
    pub prev_epoch_time: u32,
    /// The block the walk must reach. It is read last. A walk finishes only
    /// when it reaches exactly this block: `finished` means "linked".
    pub ancestor: BlockRef,
    /// The next block to read.
    pub next: BlockRef,
    /// The difficulty of the block read before `next`, and whether that
    /// block was the first of its epoch.
    pub child_bits: u32,
    pub child_first_of_epoch: bool,
    pub read_any: bool,
    /// Set when the ancestor has been reached, read and checked.
    pub finished: bool,
    /// The block read last. When finished, the ancestor.
    pub last: BlockRef,
    /// The mining work of every block read, except the last one.
    pub work: [u8; 32],
    pub bump: u8,
}
