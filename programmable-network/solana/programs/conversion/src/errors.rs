use anchor_lang::prelude::*;

#[error_code]
pub enum ConversionError {
    InvalidAmount,
    InvalidScript,
    TooLarge,
    WrongState,
    NotOperator,
    FundingTimeOver,
    FundingTimeNotOver,
    NotProven,
    LockNotEnded,
    WrongTransaction,
    NotPaid,
    NotRefundable,
    NotReclaimable,
    OutsidePaymentBlocks,
    TooFewConfirmations,
    NotLinked,
    NotInBlock,
    WrongAccount,
    MalformedTx,
    WrongTokenProgram,
    Overflow,
    NotAnchored,
    AnchorTooOld,
    NotSlashed,
}
