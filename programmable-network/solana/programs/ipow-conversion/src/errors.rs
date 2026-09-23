use anchor_lang::prelude::*;

#[error_code]
pub enum ConversionError {
    #[msg("Required input value is zero")]
    ZeroValue,
    #[msg("Need Duty Window")]
    NeedDutyWindow,
    #[msg("Contract is in an invalid state for this action")]
    BadState,
    #[msg("Wrong conversion type for this instruction")]
    WrongConversionType,
    #[msg("Claim does not beat the current stake")]
    ClaimNotBetter,
    #[msg("Proposed rate does not meet the committer's requested value")]
    BelowAskedRate,
    #[msg("Proposed rate exceeds the committer's fixed Bitcoin ask for a bundle")]
    AboveAskedRate,
    #[msg("Proposed rate does not beat the current best offer")]
    RateNotBetter,
    #[msg("Claim window has closed")]
    ClaimWindowClosed,
    #[msg("Claim window is still open")]
    ClaimWindowStillOpen,
    #[msg("No claims have been placed yet")]
    NoClaimsYet,
    #[msg("Staked amount is below the required bond")]
    BelowRequiredBond,
    #[msg("Duty window has not yet expired")]
    DutyNotExpired,
    #[msg("Duty window has already expired")]
    DutyExpired,
    #[msg("Not in the correct block-height window for this action")]
    IncorrectWindow,
    #[msg("No headers relayed yet for this conversion's window")]
    NoHeadersYet,
    #[msg("Proof has already been verified")]
    AlreadyVerified,
    #[msg("Incorrect value provided")]
    IncorrectValue,
    #[msg("Bitcoin program/receive-script is invalid or missing")]
    BadBitcoinProgram,
    #[msg("This Bitcoin receive-program hash has already been used")]
    ProgramAlreadyUsed,
    #[msg("Supplied program bytes do not match the committed hash")]
    InvalidProgramHash,
    #[msg("Network address not allowed for direct-Bitcoin conversions")]
    NetworkAddressNotAllowed,
    #[msg("User-supplied Bitcoin program not allowed for this network")]
    UserBitcoinProgramNotAllowed,
    #[msg("Unsupported or inactive network")]
    IncorrectNetwork,
    #[msg("Destination address length is invalid for this network")]
    InvalidAddressLength,
    #[msg("Invalid Bitcoin Header or Merkle Proof")]
    InvalidHeader,
    #[msg("Action performed by unauthorized party")]
    Unauthorized,
    #[msg("Conversion's token_mint does not match the supplied vault/token accounts")]
    MintMismatch,
    #[msg("Supplied token_program/associated_token_program does not match the real SPL programs")]
    WrongTokenProgram,
    #[msg("Supplied token_program does not match the mint's actual owning program")]
    MintTokenProgramMismatch,
    #[msg("The amount actually received (measured via balance delta) fell short of the required deposit — likely a fee-on-transfer token")]
    DepositShortfall,
    #[msg("A token bundle may carry at most 3 extra tokens beyond the primary slot")]
    TooManyTokens,
    #[msg("Duplicate token mint within a bundle")]
    DuplicateTokenMint,
    #[msg("A remaining-account did not match the expected PDA for this token")]
    InvalidRemainingAccount,
    #[msg("An extra token in a bundle cannot be the native-SOL default mint")]
    InvalidTokenMint,
    #[msg("Invalid network configuration: network_id zero, min_addr_len zero, or min_addr_len exceeds max_addr_len")]
    InvalidNetworkConfig,
}
