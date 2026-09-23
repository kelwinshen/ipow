// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

/**
 * @title iPoWV1Types
 * @notice Shared errors, events, structs, enums, and constants for iPoWV1.
 * @dev Purely declarative. `iPoWV1` inherits this, so everything here compiles into
 * the same deployed contract and shows up on `iPoWV1`'s ABI as if declared inline.
 */
abstract contract iPoWV1Types {

    // --- Role check ---
    error Unauthorized();

    // --- Configuration ---
    error InvalidConstructor();
    error InvalidFeeConfig();
    error InvalidNetworkConfig();
    error NetworkChangeLocked();

    // --- Input validation ---
    error IncorrectCommitFee();
    error UnexpectedValue();
    error ZeroValue();
    error IncorrectValue();
    error NeedDutyWindow();
    error NeedBitcoinAmount();
    error BadBitcoinProgram();
    error UserBitcoinProgramNotAllowed();
    error NeedDestAddress();

    // --- Network routing ---
    error IncorrectNetwork();
    error IncorrectNetworkAddress();
    error NetworkNotAllowed();
    error NetworkAddressNotAllowed();

    // --- Lifecycle / state ---
    error BadTxId();
    error BadState();
    error WrongConversionType();
    error ApproveWindowOver();
    error DutyExpired();
    error DutyNotExpired();
    error NoHeadersYet();
    error HeaderStarted();
    error IncorrectWindow();
    error AlreadyVerified();
    error InvalidFirstOrAnchor();
    error InvalidAnchorHeight();
    error InvalidTypeFilter();

    // --- Liquidity / economics ---
    error LowLiquidity();
    /// @notice Insufficient reserve for a Bitcoin-to-Native payout specifically (distinct
    /// from LowLiquidity, which covers general withdrawal availability).
    error LowReserve();
    error ExceedsRemovable();
    error BadSlippage();
    error SlippageNotAllowed();

    // --- Bitcoin headers / consensus ---
    error ProgramAlreadyUsed();
    error InvalidHeader();
    error LowWork();
    error HeightRewrite();
    error NoJumpWhenActive();
    error PrevAndTipUnmatch();
    error EpochFirstMissing();
    error InvalidRetarget();
    error EpochAnchorsMissing();
    error EpochMetaMissing();
    error GlobalFirstHeaderMissing();
    error MetaFirstHeaderMissing();
    error GlobalAnchorMissing();
    error MetaAnchorHeaderMissing();
    error AnchorMustBeTip();

    // --- Transaction parsing ---
    error TransactionTooShort();
    error TransactionOverflow();
    error VoutOutOfBounds();
    error ValueOutOfBounds();
    error ProgramOutOfBounds();
    error VarIntOutOfBounds();
    error Var16OutOfBounds();
    error Var32OutOfBounds();
    error Var64OutOfBounds();
    error LE8OutOfBounds();

    // --- Transfers ---
    error TransferFailed();

    // ========= EVENTS =========
    event OperatorChanged(address newOperator);
    event FeesUpdated(uint256 newCommitFeeBps);
    event LiquidityUpdated(uint256 nativeLiquidity);
    event ConversionCommitted(uint256 indexed txId, address indexed user, bool isNativetoBitcoin);

    event ConversionApproved(
        uint256 indexed txId,
        uint256 dutyWindowSeconds,
        uint256 firstHeight,
        bytes32 firstHeaderHashLE
    );

    event ConversionDeposited(uint256 indexed txId, uint256 nativeAmount);
    event ConversionCompleted(uint256 indexed txId);

    /// @param commitFeeRefunded Whether the initial commit fee was included in this refund.
    event ConversionRefunded(uint256 indexed txId, uint256 refundNative, bool commitFeeRefunded);

    event GlobalHeaderAppended(
        uint256 height,
        bytes32 hashLE,
        bytes32 prevHashLE,
        bytes32 merkleRootLE,
        uint32 nBits,
        uint32 timestamp
    );

    /// @notice Full state of a cross-chain conversion, from commitment through
    /// finalization or refund.
    struct Conversion {
        address user;
        /// @dev true = Native-to-Bitcoin, false = Bitcoin-to-Native.
        bool isNativeToBitcoin;
        /// @dev User's Bitcoin payout script (Native-to-Bitcoin only).
        bytes userProgram;
        /// @dev Protocol's Bitcoin receiving script to monitor (Bitcoin-to-Native only).
        bytes ipowReceiveProgram;
        bytes networkAddress;
        uint256 networkId;

        uint256 nativeAmount;
        /// @dev In Satoshis.
        uint256 bitcoinAmount;
        uint256 commitFee;
        /// @dev Native liquidity reserved for a Bitcoin-to-Native payout at approval time.
        uint256 reservedNative;

        uint256 createdAt;
        uint256 approvedAt;
        uint256 depositedAt;
        uint256 operatorDutyExpiresAt;

        bool approved;
        bool deposited;
        bool completed;
        bool refunded;
    }

    /// @dev Hashes are Little-Endian, matching native Bitcoin encoding.
    struct GlobalHeaderMeta {
        bytes32 prevHashLE;
        bytes32 merkleRootLE;
        uint32 nBits;
        uint32 timestamp;
        bool set;
        /// @dev Local timestamp when this header was recorded — used to check whether
        /// the operator streamed it before their duty deadline.
        uint64 arrivalTime;
    }

    /// @notice Cached Simplified Payment Verification (SPV) data awaiting confirmation
    /// by the header relay.
    struct ProofCache {
        bool set;
        bool verified;
        /// @dev Sticky: once true, this proof can't be retried.
        bool invalid;
        uint8 attempts;

        /// @dev Double-SHA256 txid, Little-Endian.
        bytes32 txidLE;
        bytes32 blockHashLE;
        uint256 blockHeight;

        bytes32[] branchLE;
        uint256 index;

        uint64 outValueSats;
        bytes outProgram;
        bool outSet;
    }

    /// @dev Anchors a conversion to a specific Bitcoin difficulty epoch (2016 blocks).
    struct HeaderWindow {
        bool started;
        bool closed;
        uint256 epochStartHeight;
        uint256 windowStartHeight;
        ProofCache proof;
    }

    struct NetworkConfig {
        bool enabled;
        uint16 minAddrLen;
        uint16 maxAddrLen;
    }

    enum Phase {
        NONE,
        WAITING_OPERATOR_APPROVAL,
        OPERATOR_APPROVAL_EXPIRED,
        WAITING_USER_ACTION,
        USER_ACTION_EXPIRED,
        ACTIVE_WAITING_PROOF,
        OPERATOR_DUTY_EXPIRED,
        COMPLETED,
        REFUNDED
    }

    enum TypeFilter {
        ANY,
        BITCOIN_TO_NATIVE,
        NATIVE_TO_BITCOIN,
        NATIVE_TO_NATIVE_IN,
        NATIVE_TO_NATIVE_OUT
    }

    // ========= CONSTANTS =========
    /// @dev 10,000 = 100%.
    uint256 public constant BPS_DENOM = 10_000;

    uint256 public constant APPROVAL_WINDOW_SEC = 15 minutes;
    uint256 public constant DEPOSIT_BLOCKS_WINDOW = 10;
    uint256 public constant PROOF_BLOCKS_WINDOW = 40;
    /// @dev 10,000 = 100% — currently no partial reserving.
    uint256 public constant RESERVE_MARGIN_BPS = 10_000;
    uint256 public constant CONFIRMATIONS_REQUIRED = 1;
    uint256 public constant DIFF_PERIOD = 2016;
    uint256 public constant RETARGET_PERIOD_SEC = 14 days;
    uint256 public constant MIN_TIMESPAN_SEC = RETARGET_PERIOD_SEC / 4;
    uint256 public constant MAX_TIMESPAN_SEC = RETARGET_PERIOD_SEC * 4;
    uint256 public constant BTC_DECIMALS = 8;
}
