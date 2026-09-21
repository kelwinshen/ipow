use anchor_lang::prelude::*;

#[error_code]
pub enum FactoryError {
    #[msg("unauthorized")]
    Unauthorized,
    #[msg("factory is paused")]
    Paused,
    #[msg("invalid parameters")]
    InvalidParams,
    #[msg("bond below the minimum for this party kind")]
    BondTooSmall,
    #[msg("party is dead")]
    PartyDead,
    #[msg("party is not an operator")]
    NotOperator,
    #[msg("party is not an auditor")]
    NotAuditor,
    #[msg("unbond delay has not elapsed")]
    UnbondNotReady,
    #[msg("unbond was not requested")]
    UnbondNotRequested,
    #[msg("pending lock deadline has not passed")]
    PendingNotExpired,
    #[msg("pending lock is referenced by a queued mint")]
    PendingQueued,
    #[msg("pending lock already approved")]
    AlreadyApproved,
    #[msg("txid does not match the raw transaction")]
    TxidMismatch,
    #[msg("header height does not match the proof height")]
    InvalidHeader,
    #[msg("merkle branch does not reach the header's root")]
    InvalidMerkleBranch,
    #[msg("raw transaction is too short or malformed")]
    MalformedTx,
    #[msg("witness-serialized transactions are not accepted; strip the witness")]
    WitnessSerialization,
    #[msg("anchor input[0] does not spend the party's registered outpoint")]
    NotOnStatementChain,
    #[msg("anchor output[1] is not a valid OP_RETURN payload")]
    BadAnchorPayload,
    #[msg("anchor kind does not match the statement")]
    KindMismatch,
    #[msg("statement hash does not match the anchor")]
    StatementHashMismatch,
    #[msg("statement bytes are malformed")]
    MalformedStatement,
    #[msg("required account missing for this statement kind")]
    MissingAccount,
    #[msg("account does not match the statement")]
    AccountMismatch,
    #[msg("anchor already processed")]
    AlreadyProcessed,
    #[msg("anchor is not in a state that allows this action")]
    BadAnchorState,
    #[msg("anchor is held by a veto")]
    Held,
    #[msg("mint cap for this window is exhausted")]
    RateLimited,
    #[msg("skip delay has not elapsed")]
    SkipNotReady,
    #[msg("target anchor has not been processed yet")]
    TargetNotProcessed,
    #[msg("burn record already claimed by a release")]
    BurnAlreadyClaimed,
    #[msg("insufficient BETA balance")]
    InsufficientBeta,
    #[msg("arithmetic overflow")]
    Overflow,
    #[msg("challenge window still open")]
    ChallengeOpen,
    #[msg("anchor is neither attested nor past its challenge window")]
    NotAttested,
    #[msg("anchor already attested")]
    AlreadyAttested,
    #[msg("attester's bond cannot cover the escrow")]
    InsufficientBond,
    #[msg("anchor already settled")]
    AlreadySettled,
    #[msg("attester party account required")]
    AttesterRequired,
}
