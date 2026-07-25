use anchor_lang::prelude::*;

#[error_code]
pub enum IPoWError {
    #[msg("Action performed by unauthorized party")]
    Unauthorized,
    #[msg("Invalid fee configuration")]
    InvalidFeeConfig,
    #[msg("Required input value is zero")]
    ZeroValue,
    #[msg("Contract is in an invalid state for this action")]
    BadState,
    #[msg("Approve window has expired")]
    ApproveWindowOver,
    #[msg("Action falls outside the allowed block window")]
    IncorrectWindow,
    #[msg("No headers available yet")]
    NoHeadersYet,
    #[msg("Incorrect value provided")]
    IncorrectValue,
    #[msg("Bitcoin program is malformed")]
    BadBitcoinProgram,
    #[msg("Wrong conversion type for this action")]
    WrongConversionType,
    #[msg("Invalid Bitcoin Header or Merkle Proof")]
    InvalidHeader,
    #[msg("Proof has already been verified")]
    AlreadyVerified,
    #[msg("Need Duty Window")]
    NeedDutyWindow,
    #[msg("Low Proof of Work")]
    LowWork,
    #[msg("Invalid Difficulty Retarget")]
    InvalidRetarget,
    #[msg("Transaction too short")]
    TransactionTooShort,
    #[msg("Transaction overflow")]
    TransactionOverflow,
    #[msg("Vout out of bounds")]
    VoutOutOfBounds,
    #[msg("Value out of bounds")]
    ValueOutOfBounds,
    #[msg("Program out of bounds")]
    ProgramOutOfBounds,
    #[msg("VarInt out of bounds")]
    VarIntOutOfBounds,
    #[msg("Var16 out of bounds")]
    Var16OutOfBounds,
    #[msg("Var32 out of bounds")]
    Var32OutOfBounds,
    #[msg("Var64 out of bounds")]
    Var64OutOfBounds,
    #[msg("Duty Not Expired")]
    DutyNotExpired,
    #[msg("Invalid Network Address Length")]
    InvalidAddressLength,
    #[msg("Blocks must be submitted contiguously while conversions are active")]
    MustBeContiguous,
    #[msg("Jump height must be strictly greater than current tip")]
    InvalidJump,
    #[msg("Network ID not allowed for this action")]
    NetworkNotAllowed,
    #[msg("Network Address must be empty for Network ID 0")]
    NetworkAddressNotAllowed,
    #[msg("User Bitcoin Program is not allowed when routing to an external network")]
    UserBitcoinProgramNotAllowed,
    #[msg("Incorrect Network Configuration")]
    IncorrectNetwork,
    #[msg("Invalid Tracker Account Provided")]
    InvalidTrackerAccount,
    #[msg("Pending proofs exist for the previous block")]
    PendingProofsExist,
    #[msg("Invalid Anchor Height provided")]
    InvalidAnchorHeight,
    #[msg("Insufficient unreserved liquidity in escrow vault")]
    InsufficientLiquidity,
    #[msg("Program already used for another conversion")]
    ProgramAlreadyUsed,
    #[msg("Provided program hash does not match the program bytes")]
    InvalidProgramHash,
}
