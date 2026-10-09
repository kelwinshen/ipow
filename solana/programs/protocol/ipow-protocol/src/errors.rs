use anchor_lang::prelude::*;

#[error_code]
pub enum ProtocolError {
    #[msg("The amount is zero")]
    ZeroAmount,
    #[msg("Not enough free bond")]
    BondNotFree,
    #[msg("The application is already registered")]
    AlreadyRegistered,
    #[msg("At most 32 kinds of claim (D78)")]
    TooManyClaimKinds,
    #[msg("A challenge period is between 36 hours and 7 days (D28)")]
    ChallengePeriodOutOfRange,
    #[msg("The application did not register this kind of claim")]
    UnknownClaimKind,
    #[msg("Confirmations are 6 or more, and the window at most 100 blocks (D41, D70)")]
    ConfirmationsOutOfRange,
    #[msg("The escrow fee is at most 100% of x (D77)")]
    EscrowFeeOutOfRange,
    #[msg("The escrow is below 5 times the commitment fee, or zero (D55, D56)")]
    EscrowTooLow,
    #[msg("Less was paid than the fees")]
    FeesNotPaid,
    #[msg("Bidding has closed")]
    AuctionClosed,
    #[msg("Bidding is still open")]
    AuctionOpen,
    #[msg("A bid is at least x, and at least 0.1% above the best bid (D32, D76)")]
    BidTooLow,
    #[msg("The job has a winner")]
    JobHasBid,
    #[msg("The fees were already returned")]
    FeesAlreadyReturned,
    #[msg("Nothing to withdraw")]
    NothingToWithdraw,
    #[msg("A number does not fit")]
    Overflow,
    #[msg("The transaction is not valid")]
    MalformedTx,
    #[msg("The transaction must be given without witness data")]
    WitnessSerialization,
    #[msg("The operator already has a chain head")]
    ChainHeadExists,
    #[msg("The operator has no chain head")]
    NoChainHead,
    #[msg("The transaction is not in the block")]
    NotInBlock,
    #[msg("The output is not a coin")]
    NoCoin,
    #[msg("The transaction does not carry the right tag")]
    WrongTag,
    #[msg("The transaction does not spend the operator's chain head (D30)")]
    WrongChainHead,
    #[msg("Only the operator of the job may do this")]
    NotOperator,
    #[msg("The job is not waiting for its proof")]
    NotAssigned,
    #[msg("The job has an anchor already")]
    AlreadyAnchored,
    #[msg("The job has no anchor")]
    NotAnchored,
    #[msg("The block is not stored in the light client")]
    UnknownBlock,
    #[msg("The anchor is more than 2 hours old (D39)")]
    AnchorTooOld,
    #[msg("The deadline has passed (D27)")]
    DeadlinePassed,
    #[msg("The deadline has not passed")]
    DeadlineNotPassed,
    #[msg("The proof is not in blocks 1 to 25 of the window (D15)")]
    OutsideProofRange,
    #[msg("The blocks are not linked: a finished walk from the light client is needed")]
    NotLinked,
    #[msg("Not enough blocks on top of the proof (D15, D41)")]
    NotEnoughConfirmations,
    #[msg("The job is not proven")]
    NotProven,
    #[msg("The lock has not ended")]
    LockNotEnded,
    #[msg("The guardian has no sealed note for this, sealed in an earlier slot (D47)")]
    NoNote,
    #[msg("A challenge of the job is open")]
    ChallengeOpen,
    #[msg("This account is not the one expected")]
    WrongAccount,
    #[msg("The transaction was used already (D80)")]
    TransactionUsed,
    #[msg("No challenge opens in the last 12 hours of the lock (D99)")]
    ChallengeWindowClosed,
    #[msg("The deposit is not the one this challenge needs (D81, D91)")]
    WrongDeposit,
    #[msg("At most 2,016 questions for a parent per job (D92)")]
    TooManyQuestions,
    #[msg("Nobody can show a parent of this block")]
    NoParent,
    #[msg("The 12 hours to answer are over")]
    ResponseTimeOver,
    #[msg("The 12 hours to answer are not over")]
    ResponseTimeNotOver,
    #[msg("This is not the kind of challenge this needs")]
    NoChallenge,
    #[msg("The two blocks do not name the same parent, or are the same block")]
    NotCompeting,
    #[msg("The guardian's branch does not have more work")]
    NotHeavier,
    #[msg("The branches must part at the anchor or after it (D81)")]
    NotOnOperatorBranch,
    #[msg("The block was not shown on that side before")]
    NotShown,
    #[msg("The lock has ended")]
    LockEnded,
    #[msg("The job has an attester already")]
    AlreadyAttested,
    #[msg("The message is not official yet (D11, D97)")]
    NotOfficial,
    #[msg("The address is empty")]
    ZeroAddress,
}
