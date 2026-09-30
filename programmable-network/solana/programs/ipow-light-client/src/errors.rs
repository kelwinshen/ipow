use anchor_lang::prelude::*;

#[error_code]
pub enum LightClientError {
    #[msg("Headers must be whole 80-byte headers, within the allowed count")]
    InvalidLength,
    #[msg("The stated block number is not allowed here")]
    InvalidHeight,
    #[msg("The stated block number is below the lowest allowed number (D93)")]
    BelowMinHeight,
    #[msg("The header's hash does not meet its own difficulty")]
    InsufficientWork,
    #[msg("The difficulty is below the minimum of 2^45 (D38)")]
    DifficultyTooLow,
    #[msg("The difficulty is not the one Bitcoin's rules give this block")]
    DifficultyMismatch,
    #[msg("The difficulty field is not a valid Bitcoin difficulty")]
    InvalidBits,
    #[msg("The block does not name the block before it")]
    NotLinked,
    #[msg("The block's time is more than 2 hours ahead")]
    TimeTooNew,
    #[msg("The anchor is more than 2 hours old (D39)")]
    AnchorTooOld,
    #[msg("The epoch start is too old to serve any anchor (D57)")]
    EpochStartTooOld,
    #[msg("The anchor is not within 4 weeks after its epoch start (D57)")]
    EpochStartOutOfRange,
    #[msg("The epoch start has less than half of the highest difficulty of the last 4 weeks (D45)")]
    EpochStartTooWeak,
    #[msg("The first block of an epoch gives the epoch its time")]
    EpochTimeMismatch,
    #[msg("This account is not the one the block must be stored in")]
    WrongAccount,
    #[msg("The block is not stored")]
    UnknownBlock,
    #[msg("The walk is longer than the longest window, 100 blocks (D70)")]
    WalkTooLong,
    #[msg("The walk is already finished")]
    WalkFinished,
    #[msg("A 64-byte transaction could be an inner node of the Merkle tree")]
    InvalidTransaction,
    #[msg("The position does not fit the proof")]
    InvalidIndex,
    #[msg("The transaction is not in the block")]
    NotInBlock,
    #[msg("Only the program's upgrade authority may initialize")]
    NotAuthority,
    #[msg("A block or epoch start can be closed only 8 weeks after it was stored (D101)")]
    TooEarlyToClose,
}
