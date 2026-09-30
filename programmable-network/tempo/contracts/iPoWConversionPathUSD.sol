// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

import {BitcoinPrimitives} from "./libraries/BitcoinPrimitives.sol";
import {IERC20} from "@openzeppelin/contracts/token/ERC20/IERC20.sol";
import {SafeERC20} from "@openzeppelin/contracts/token/ERC20/utils/SafeERC20.sol";

interface IIPoWHeaders {
    function globalTipHeight() external view returns (uint256);

    function globalHeightToHashLE(uint256 height) external view returns (bytes32);

    function globalHeaders(bytes32 hashLE)
        external
        view
        returns (
            bytes32 prevHashLE,
            bytes32 merkleRootLE,
            uint32 nBits,
            uint32 timestamp,
            bool set,
            uint64 arrivalTime
        );
}

/**
 * @title iPoWConversionPathUSD — Tempo-only `iPoWConversion` variant
 * @notice Tempo's custom `0x76` transaction type unconditionally rejects
 * any transaction carrying native value (confirmed live, see
 * `BetaVaultPathUSD.sol`'s own header comment — same chain-level rule,
 * not a quirk of this contract).
 * @dev Unlike `BetaVault`/`BetaHub`, this contract already has a fully
 * flexible `tokenAddr` choice for the *conversion value itself*
 * (`address(0)` = native, any other address = ERC20) — that's left
 * completely untouched here, exactly the same "provably unreachable on
 * Tempo, not a live footgun" choice `BetaVaultPathUSD.sol` already made
 * for its own native branch: nobody can ever fund a `tokenAddr ==
 * address(0)` conversion on Tempo (no nonzero `msg.value` is possible),
 * so that dead path stays for structural parity with the reference
 * contract rather than being special-cased away.
 *
 * What's actually hard-coded to native value *regardless* of `tokenAddr`
 * — and so genuinely needs fixing — is narrower: the **commit fee**
 * (`commitTokenToBitcoin`/`commitBitcoinToToken`), the **auction stake**
 * (`proposeClaimConversion`), and the **bounty** it can accumulate into
 * (`reclaimExpiredConversion`/`submitBitcoinMerkleProofWithTx`). All
 * three now move `BOND_TOKEN` (PathUSD) instead. `proposeClaimConversion`
 * gains an explicit `stakeAmount` parameter — it can no longer be
 * inferred from `msg.value`, since `msg.value` there now serves *only*
 * the (still-native, still dead-on-Tempo) bitcoin->native self-escrow
 * case for `tokenAddr == address(0)`. Every other function — the auction
 * rate-competition logic, statement/proof verification, the whole
 * `Conversion` struct and its state machine — is byte-identical to
 * `iPoWConversion.sol`.
 */
contract iPoWConversionPathUSD {
    using SafeERC20 for IERC20;
    // ========= ERRORS =========
    error Unauthorized();
    error ZeroValue();
    error NeedDutyWindow();
    error BadState();
    error WrongConversionType();
    error ClaimNotBetter();
    error BelowAskedRate();
    error RateNotBetter();
    error TooManyTokens();
    error DuplicateTokenMint();
    error InvalidTokenMint();
    error ClaimWindowClosed();
    error ClaimWindowStillOpen();
    error NoClaimsYet();
    error BelowRequiredBond();
    error DutyNotExpired();
    error IncorrectWindow();
    error NoHeadersYet();
    error AlreadyVerified();
    error IncorrectValue();
    error BadBitcoinProgram();
    error ProgramAlreadyUsed();
    error InvalidHeader();
    error IncorrectCommitFee();
    error UnexpectedValue();
    error IncorrectNetwork();
    error IncorrectNetworkAddress();
    error InvalidNetworkConfig();

    // ========= EVENTS =========
    event ConversionCommitted(uint256 indexed txId, address indexed user, bool isNativeToBitcoin);
    event ClaimProposed(uint256 indexed txId, address indexed claimant, uint256 stakeAmount, uint256 proposedRateAmount);
    event ClaimFinalized(uint256 indexed txId, address indexed responsibleOperator);
    event ClaimReclaimed(uint256 indexed txId, bool forfeited);
    event ConversionDeposited(uint256 indexed txId, uint256 nativeAmount);
    event ConversionCompleted(uint256 indexed txId);
    event ConversionRefunded(uint256 indexed txId, uint256 refundNative);

    // ========= CONSTANTS =========
    uint256 public constant BPS_DENOM = 10_000;
    uint256 public constant DEPOSIT_BLOCKS_WINDOW = 10;
    uint256 public constant PROOF_BLOCKS_WINDOW = 40;
    uint256 public constant DIFF_PERIOD = 2016;
    uint256 public constant CLAIM_WINDOW_SEC = 15 minutes;
    uint256 public constant CLAIM_QUIET_PERIOD_SEC = 1 minutes;

    enum Status {
        None,
        Committed,
        Approved,
        Deposited,
        Completed,
        Refunded
    }

    struct TokenAmount {
        address mint;
        uint256 amount;
    }

    struct Conversion {
        address user;
        bool isNativeToBitcoin;
        bytes userProgram;
        bytes ipowReceiveProgram;
        uint256 networkId;
        address tokenAddr;

        uint256 nativeAmount;
        uint256 bitcoinAmount;
        uint256 commitFee;
        uint256 reservedNative;

        TokenAmount[] extraTokens;

        uint256 createdAt;
        uint256 depositedAt;
        uint256 operatorDutyExpiresAt;

        Status status;

        bool windowStarted;
        uint256 windowStartHeight;

        bool proofVerified;
        uint256 proofBlockHeight;

        address responsibleOperator;
        uint256 stakedBond;
        uint256 requiredBond;
        uint256 claimStartedAt;
        uint256 lastClaimAt;
        uint256 dutyWindowSeconds;
        uint256 bounty;

        bytes networkAddress;
    }

    address public governance;
    IIPoWHeaders public immutable ipowHeaders;
    /// PathUSD — every commit fee, auction stake, and bounty moves this,
    /// never native `msg.value`. Same role as `BetaVaultPathUSD.BOND_TOKEN`
    /// / `BetaHubPathUSD.BOND_TOKEN`. The conversion *value* itself
    /// (`nativeAmount`, keyed by each `Conversion`'s own `tokenAddr`) is
    /// unrelated to this token and untouched by this variant.
    IERC20 public immutable BOND_TOKEN;
    uint256 public commitFeeBps;

    uint256 public nextTxId = 1;
    mapping(uint256 => Conversion) public conversions;

    mapping(address => uint256) public totalLockedDeposits;
    mapping(address => uint256) public totalReservedAmount;
    uint256 public totalHeldCommitFees;

    mapping(bytes => bool) public usedIPoWPrograms;

    struct NetworkConfig {
        bool enabled;
        uint16 minAddrLen;
        uint16 maxAddrLen;
    }

    mapping(uint256 => NetworkConfig) public networkConfigs;

    modifier validTx(uint256 txId) {
        if (txId == 0 || txId >= nextTxId) revert BadState();
        _;
    }

    modifier onlyGovernance() {
        if (msg.sender != governance) revert Unauthorized();
        _;
    }

    constructor(address _governance, address _ipowHeaders, address _bondToken, uint256 _commitFeeBps) {
        if (
            _governance == address(0) || _ipowHeaders == address(0) || _bondToken == address(0)
                || _commitFeeBps > BPS_DENOM
        ) {
            revert BadState();
        }
        governance = _governance;
        ipowHeaders = IIPoWHeaders(_ipowHeaders);
        BOND_TOKEN = IERC20(_bondToken);
        commitFeeBps = _commitFeeBps;
    }

    function addNetwork(uint256 networkId, uint16 minAddrLen, uint16 maxAddrLen)
        external
        onlyGovernance
    {
        if (
            networkId == 0 ||
            networkConfigs[networkId].enabled ||
            minAddrLen == 0 ||
            minAddrLen > maxAddrLen
        ) revert InvalidNetworkConfig();

        networkConfigs[networkId] = NetworkConfig({
            enabled: true,
            minAddrLen: minAddrLen,
            maxAddrLen: maxAddrLen
        });
    }

    function removeNetwork(uint256 networkId) external onlyGovernance {
        if (networkId == 0 || !networkConfigs[networkId].enabled) revert InvalidNetworkConfig();
        delete networkConfigs[networkId];
    }

    function _pullToken(address tokenAddr, address from, uint256 amount) internal returns (uint256) {
        uint256 before = IERC20(tokenAddr).balanceOf(address(this));
        IERC20(tokenAddr).safeTransferFrom(from, address(this), amount);
        return IERC20(tokenAddr).balanceOf(address(this)) - before;
    }

    /// @dev Native ETH or ERC20 payout for the conversion's own value,
    /// depending on `tokenAddr` — unchanged from `iPoWConversion.sol`.
    /// The native branch is unreachable on Tempo (see contract-level
    /// comment), left in place for structural parity rather than removed.
    function _payOut(address tokenAddr, address to, uint256 amount) internal {
        if (amount == 0) return;
        if (tokenAddr == address(0)) {
            (bool ok, ) = payable(to).call{value: amount}("");
            if (!ok) revert Unauthorized();
        } else {
            IERC20(tokenAddr).safeTransfer(to, amount);
        }
    }

    /// @dev Pulls PathUSD for a commit fee, an auction stake, or a
    /// self-escrowed stake top-up — the one place this variant actually
    /// diverges from the reference contract's `msg.value` handling.
    function _pullBond(address from, uint256 amount) internal returns (uint256) {
        uint256 before = BOND_TOKEN.balanceOf(address(this));
        BOND_TOKEN.safeTransferFrom(from, address(this), amount);
        return BOND_TOKEN.balanceOf(address(this)) - before;
    }

    /// @dev Pays out PathUSD for a commit-fee refund/release, a stake
    /// refund, or a stake+bounty payout — the PathUSD-side counterpart to
    /// `_payOut` above.
    function _payBond(address to, uint256 amount) internal {
        if (amount == 0) return;
        BOND_TOKEN.safeTransfer(to, amount);
    }

    /// @notice Same as `iPoWConversion.commitTokenToBitcoin` except the
    /// commit fee is pulled as PathUSD (`_pullBond`), not `msg.value` —
    /// `payable` removed; no other change to the bundle/validation logic.
    function commitTokenToBitcoin(
        uint256 nativeAmount,
        uint256 bitcoinAmount,
        bytes calldata userProgram,
        uint256 requiredBond,
        address tokenAddr,
        TokenAmount[] calldata extraTokens
    ) external {
        if (nativeAmount == 0 || bitcoinAmount == 0) revert ZeroValue();
        if (requiredBond == 0) revert ZeroValue();
        if (userProgram.length == 0 || userProgram.length > 80) revert BadBitcoinProgram();

        if (extraTokens.length > 3) revert TooManyTokens();
        for (uint256 i = 0; i < extraTokens.length; i++) {
            if (extraTokens[i].amount == 0) revert ZeroValue();
            if (extraTokens[i].mint == address(0)) revert InvalidTokenMint();
            if (extraTokens[i].mint == tokenAddr) revert DuplicateTokenMint();
            for (uint256 j = i + 1; j < extraTokens.length; j++) {
                if (extraTokens[i].mint == extraTokens[j].mint) revert DuplicateTokenMint();
            }
        }

        uint256 requiredFee = (nativeAmount * commitFeeBps) / BPS_DENOM;
        uint256 pulledFee = requiredFee == 0 ? 0 : _pullBond(msg.sender, requiredFee);

        uint256 txId = nextTxId++;
        Conversion storage c = conversions[txId];
        c.user = msg.sender;
        c.isNativeToBitcoin = true;
        c.userProgram = userProgram;
        c.tokenAddr = tokenAddr;
        c.nativeAmount = nativeAmount;
        c.bitcoinAmount = bitcoinAmount;
        c.commitFee = pulledFee;
        c.createdAt = block.timestamp;
        c.status = Status.Committed;
        c.requiredBond = requiredBond;
        c.claimStartedAt = block.timestamp;
        c.lastClaimAt = block.timestamp;
        for (uint256 i = 0; i < extraTokens.length; i++) {
            c.extraTokens.push(extraTokens[i]);
        }

        totalHeldCommitFees += pulledFee;
        emit ConversionCommitted(txId, msg.sender, true);
    }

    /// @notice Same as `iPoWConversion.commitBitcoinToToken` except the
    /// commit fee is pulled as PathUSD, not `msg.value`.
    function commitBitcoinToToken(
        uint256 bitcoinAmount,
        uint256 nativeAmount,
        bytes calldata userProgram,
        uint256 requiredBond,
        address tokenAddr
    ) external {
        if (nativeAmount == 0 || bitcoinAmount == 0) revert ZeroValue();
        if (requiredBond == 0) revert ZeroValue();
        if (userProgram.length == 0 || userProgram.length > 80) revert BadBitcoinProgram();

        uint256 requiredFee = (nativeAmount * commitFeeBps) / BPS_DENOM;
        uint256 pulledFee = requiredFee == 0 ? 0 : _pullBond(msg.sender, requiredFee);

        uint256 txId = nextTxId++;
        Conversion storage c = conversions[txId];
        c.user = msg.sender;
        c.isNativeToBitcoin = false;
        c.userProgram = userProgram;
        c.tokenAddr = tokenAddr;
        c.nativeAmount = nativeAmount;
        c.bitcoinAmount = bitcoinAmount;
        c.commitFee = pulledFee;
        c.createdAt = block.timestamp;
        c.status = Status.Committed;
        c.requiredBond = requiredBond;
        c.claimStartedAt = block.timestamp;
        c.lastClaimAt = block.timestamp;

        totalHeldCommitFees += pulledFee;
        emit ConversionCommitted(txId, msg.sender, false);
    }

    /// @notice Same as `iPoWConversion.openBundleTunnel` — unchanged.
    /// No commit fee here (bypasses the auction), and its only value
    /// movement is the conversion's own `nativeAmount`/`extraTokens`
    /// self-escrow, which already follows `tokenAddr`/each token's own
    /// address correctly. The `tokenAddr == address(0)` branch is
    /// unreachable on Tempo like every other native-value-choice branch
    /// in this contract — left in place for structural parity.
    function openBundleTunnel(
        uint256 nativeAmount,
        uint256 bitcoinAmount,
        address tokenAddr,
        TokenAmount[] calldata extraTokens,
        address destAddress,
        uint256 networkId,
        bytes calldata networkAddress,
        uint256 dutyWindowSeconds,
        bytes calldata ipowReceiveProgram
    ) external payable {
        if (nativeAmount == 0 || bitcoinAmount == 0) revert ZeroValue();
        if (dutyWindowSeconds == 0) revert NeedDutyWindow();

        if (extraTokens.length > 3) revert TooManyTokens();
        for (uint256 i = 0; i < extraTokens.length; i++) {
            if (extraTokens[i].amount == 0) revert ZeroValue();
            if (extraTokens[i].mint == address(0)) revert InvalidTokenMint();
            if (extraTokens[i].mint == tokenAddr) revert DuplicateTokenMint();
            for (uint256 j = i + 1; j < extraTokens.length; j++) {
                if (extraTokens[i].mint == extraTokens[j].mint) revert DuplicateTokenMint();
            }
        }

        if (ipowReceiveProgram.length == 0 || ipowReceiveProgram.length > 80) revert BadBitcoinProgram();
        if (usedIPoWPrograms[ipowReceiveProgram]) revert ProgramAlreadyUsed();
        usedIPoWPrograms[ipowReceiveProgram] = true;

        if (networkId == 0 || !networkConfigs[networkId].enabled) revert IncorrectNetwork();
        NetworkConfig memory netConfig = networkConfigs[networkId];
        if (networkAddress.length < netConfig.minAddrLen || networkAddress.length > netConfig.maxAddrLen) {
            revert IncorrectNetworkAddress();
        }

        if (tokenAddr == address(0)) {
            if (msg.value != nativeAmount) revert IncorrectValue();
        } else {
            if (msg.value != 0) revert UnexpectedValue();
            uint256 received = _pullToken(tokenAddr, msg.sender, nativeAmount);
            if (received != nativeAmount) revert IncorrectValue();
        }

        for (uint256 i = 0; i < extraTokens.length; i++) {
            uint256 receivedExtra = _pullToken(extraTokens[i].mint, msg.sender, extraTokens[i].amount);
            if (receivedExtra != extraTokens[i].amount) revert IncorrectValue();
        }

        uint256 txId = nextTxId++;
        Conversion storage c = conversions[txId];
        c.user = destAddress;
        c.isNativeToBitcoin = false;
        c.tokenAddr = tokenAddr;
        c.nativeAmount = nativeAmount;
        c.bitcoinAmount = bitcoinAmount;
        c.commitFee = 0;
        c.reservedNative = nativeAmount;
        for (uint256 i = 0; i < extraTokens.length; i++) {
            c.extraTokens.push(extraTokens[i]);
        }

        c.ipowReceiveProgram = ipowReceiveProgram;
        c.networkId = networkId;
        c.networkAddress = networkAddress;

        c.createdAt = block.timestamp;
        c.status = Status.Approved;
        c.requiredBond = 0;
        c.stakedBond = 0;
        c.claimStartedAt = block.timestamp;
        c.lastClaimAt = block.timestamp;
        c.dutyWindowSeconds = dutyWindowSeconds;
        c.operatorDutyExpiresAt = block.timestamp + dutyWindowSeconds;
        c.responsibleOperator = msg.sender;

        c.windowStarted = true;
        c.windowStartHeight = ipowHeaders.globalTipHeight();

        totalReservedAmount[tokenAddr] += nativeAmount;

        emit ConversionCommitted(txId, destAddress, false);
        emit ClaimFinalized(txId, msg.sender);
    }

    /// @notice Same auction as `iPoWConversion.proposeClaimConversion`
    /// except the stake is an explicit `stakeAmount` parameter pulled as
    /// PathUSD (`_pullBond`), not derived from `msg.value`. `msg.value`
    /// now serves *only* the still-native bitcoin->native self-escrow
    /// case (`tokenAddr == address(0)`), which stays exactly as in the
    /// reference contract — dead on Tempo, alive (and required to match
    /// `proposedRateAmount` exactly) elsewhere.
    function proposeClaimConversion(
        uint256 txId,
        uint256 proposedRateAmount,
        uint256 dutyWindowSeconds,
        bytes calldata ipowReceiveProgram,
        uint256 stakeAmount
    ) external payable validTx(txId) {
        Conversion storage c = conversions[txId];

        bool claimable = c.status == Status.Committed ||
            ((c.status == Status.Approved || c.status == Status.Deposited) &&
                c.operatorDutyExpiresAt == 0);
        if (!claimable) revert BadState();
        if (dutyWindowSeconds == 0) revert NeedDutyWindow();
        if (block.timestamp > c.claimStartedAt + CLAIM_WINDOW_SEC) revert ClaimWindowClosed();

        bool isFirstClaim = c.responsibleOperator == address(0);
        uint256 currentRateValue = c.isNativeToBitcoin ? c.bitcoinAmount : c.nativeAmount;
        if (isFirstClaim) {
            if (proposedRateAmount < currentRateValue) revert BelowAskedRate();
        } else {
            if (proposedRateAmount <= currentRateValue) revert RateNotBetter();
        }

        bool selfEscrow = !c.isNativeToBitcoin && !c.windowStarted;
        uint256 nativeEscrowPortion = (selfEscrow && c.tokenAddr == address(0)) ? proposedRateAmount : 0;
        if (msg.value != nativeEscrowPortion) revert IncorrectValue();
        if (stakeAmount < c.requiredBond) revert BelowRequiredBond();
        uint256 pulledStake = stakeAmount == 0 ? 0 : _pullBond(msg.sender, stakeAmount);

        if (!isFirstClaim) {
            address previous = c.responsibleOperator;
            uint256 previousStake = c.stakedBond;
            _payBond(previous, previousStake);
            if (selfEscrow) {
                uint256 previousNativeAmount = c.nativeAmount;
                totalReservedAmount[c.tokenAddr] -= previousNativeAmount;
                _payOut(c.tokenAddr, previous, previousNativeAmount);
            }
        }

        c.responsibleOperator = msg.sender;
        c.stakedBond = pulledStake;
        c.lastClaimAt = block.timestamp;
        c.dutyWindowSeconds = dutyWindowSeconds;

        if (selfEscrow) {
            if (c.tokenAddr != address(0)) {
                uint256 received = _pullToken(c.tokenAddr, msg.sender, proposedRateAmount);
                if (received != proposedRateAmount) revert IncorrectValue();
            }
            totalReservedAmount[c.tokenAddr] += proposedRateAmount;
            c.nativeAmount = proposedRateAmount;
        }

        if (c.isNativeToBitcoin) {
            c.bitcoinAmount = proposedRateAmount;
        }

        if (!c.isNativeToBitcoin) {
            if (ipowReceiveProgram.length == 0 || ipowReceiveProgram.length > 80) revert BadBitcoinProgram();
            if (usedIPoWPrograms[ipowReceiveProgram]) revert ProgramAlreadyUsed();
            usedIPoWPrograms[ipowReceiveProgram] = true;
            c.ipowReceiveProgram = ipowReceiveProgram;
        } else if (c.isNativeToBitcoin && c.userProgram.length == 0) {
            if (ipowReceiveProgram.length == 0 || ipowReceiveProgram.length > 80) revert BadBitcoinProgram();
            if (usedIPoWPrograms[ipowReceiveProgram]) revert ProgramAlreadyUsed();
            usedIPoWPrograms[ipowReceiveProgram] = true;
            c.userProgram = ipowReceiveProgram;
        }

        emit ClaimProposed(txId, msg.sender, pulledStake, proposedRateAmount);
    }

    /// @notice Same as `iPoWConversion.finalizeClaimConversion` —
    /// unchanged; no value movement here at all.
    function finalizeClaimConversion(uint256 txId) external validTx(txId) {
        Conversion storage c = conversions[txId];

        bool claimable = c.status == Status.Committed ||
            ((c.status == Status.Approved || c.status == Status.Deposited) &&
                c.operatorDutyExpiresAt == 0);
        if (!claimable) revert BadState();
        if (c.responsibleOperator == address(0)) revert NoClaimsYet();
        if (block.timestamp <= c.lastClaimAt + CLAIM_QUIET_PERIOD_SEC) revert ClaimWindowStillOpen();

        if (c.status == Status.Committed) {
            c.status = Status.Approved;
        }
        c.operatorDutyExpiresAt = block.timestamp + c.dutyWindowSeconds;

        if (!c.windowStarted) {
            c.windowStarted = true;
            c.windowStartHeight = ipowHeaders.globalTipHeight();

            if (!c.isNativeToBitcoin) {
                c.reservedNative = c.nativeAmount;
            }
        }

        emit ClaimFinalized(txId, c.responsibleOperator);
    }

    /// @notice Same as `iPoWConversion.reclaimExpiredConversion` except
    /// the stake refund/forfeit moves PathUSD (`_payBond`), not native
    /// value.
    function reclaimExpiredConversion(uint256 txId) external validTx(txId) {
        Conversion storage c = conversions[txId];

        if (c.status != Status.Approved && c.status != Status.Deposited) revert BadState();
        if (c.operatorDutyExpiresAt == 0 || block.timestamp <= c.operatorDutyExpiresAt) {
            revert DutyNotExpired();
        }

        bool realCommitmentHappened = !c.isNativeToBitcoin || c.status == Status.Deposited;

        if (realCommitmentHappened) {
            c.bounty += c.stakedBond;
            c.requiredBond = c.stakedBond;
        } else {
            address previous = c.responsibleOperator;
            uint256 refund = c.stakedBond;
            _payBond(previous, refund);
            c.status = Status.Committed;
        }

        c.stakedBond = 0;
        c.responsibleOperator = address(0);
        c.operatorDutyExpiresAt = 0;
        c.claimStartedAt = block.timestamp;
        c.lastClaimAt = block.timestamp;

        emit ClaimReclaimed(txId, realCommitmentHappened);
    }

    /// @notice Same as `iPoWConversion.depositApprovedConversion` —
    /// unchanged. Its native branch (`tokenAddr == address(0)`) is
    /// unreachable on Tempo like every other native-value-choice branch
    /// here; the ERC20 branch already works unmodified.
    function depositApprovedConversion(uint256 txId) external payable validTx(txId) {
        Conversion storage c = conversions[txId];

        if (!c.isNativeToBitcoin) revert WrongConversionType();
        if (msg.sender != c.user) revert Unauthorized();
        if (c.status != Status.Approved) revert BadState();
        if (!c.windowStarted) revert NoHeadersYet();
        if (ipowHeaders.globalTipHeight() > c.windowStartHeight + (DEPOSIT_BLOCKS_WINDOW - 1)) {
            revert IncorrectWindow();
        }

        if (c.tokenAddr == address(0)) {
            if (msg.value != c.nativeAmount) revert IncorrectValue();
        } else {
            if (msg.value != 0) revert UnexpectedValue();
            uint256 received = _pullToken(c.tokenAddr, msg.sender, c.nativeAmount);
            if (received != c.nativeAmount) revert IncorrectValue();
        }

        for (uint256 i = 0; i < c.extraTokens.length; i++) {
            TokenAmount storage extra = c.extraTokens[i];
            uint256 receivedExtra = _pullToken(extra.mint, msg.sender, extra.amount);
            if (receivedExtra != extra.amount) revert IncorrectValue();
        }

        c.status = Status.Deposited;
        c.depositedAt = block.timestamp;
        totalLockedDeposits[c.tokenAddr] += c.nativeAmount;

        emit ConversionDeposited(txId, c.nativeAmount);
    }

    /// @notice Same as `iPoWConversion.submitBitcoinMerkleProofWithTx`
    /// except the commit-fee release and the stake+bounty payout move
    /// PathUSD (`_payBond`), not native value. The conversion's own
    /// value payout (`_payOut`, following `tokenAddr`) is unchanged.
    function submitBitcoinMerkleProofWithTx(
        uint256 txId,
        bytes calldata txRaw,
        uint256 voutIndex,
        uint256 blockHeight,
        bytes32[] calldata branchLE,
        uint256 index
    ) external validTx(txId) {
        Conversion storage c = conversions[txId];

        if (!c.windowStarted) revert NoHeadersYet();
        if (
            blockHeight < c.windowStartHeight ||
            blockHeight > c.windowStartHeight + (PROOF_BLOCKS_WINDOW - 1)
        ) revert IncorrectWindow();
        if (c.proofVerified) revert AlreadyVerified();

        if (c.isNativeToBitcoin) {
            if (msg.sender != c.responsibleOperator) revert Unauthorized();
        } else {
            if (msg.sender != c.user) revert Unauthorized();
        }

        bytes32 headerHashLE = ipowHeaders.globalHeightToHashLE(blockHeight);
        (, bytes32 merkleRootLE, , , bool set, ) = ipowHeaders.globalHeaders(headerHashLE);
        if (!set) revert InvalidHeader();

        bytes32 txidLE = sha256(abi.encodePacked(sha256(txRaw)));
        (uint64 outValueSats, bytes memory outProgram) = BitcoinPrimitives._parseOutputAt(txRaw, voutIndex);

        bytes32 current = txidLE;
        uint256 idx = index;
        for (uint256 i = 0; i < branchLE.length; i++) {
            if (idx % 2 == 0) {
                current = sha256(abi.encodePacked(sha256(abi.encodePacked(current, branchLE[i]))));
            } else {
                current = sha256(abi.encodePacked(sha256(abi.encodePacked(branchLE[i], current))));
            }
            idx /= 2;
        }
        if (current != merkleRootLE) revert InvalidHeader();

        uint256 feeToOperator = c.commitFee;

        if (c.isNativeToBitcoin) {
            if (c.status != Status.Deposited) revert BadState();
            if (outValueSats < c.bitcoinAmount) revert IncorrectValue();
            if (keccak256(outProgram) != keccak256(c.userProgram)) revert BadBitcoinProgram();

            totalLockedDeposits[c.tokenAddr] -= c.nativeAmount;
            _payOut(c.tokenAddr, c.responsibleOperator, c.nativeAmount);

            for (uint256 i = 0; i < c.extraTokens.length; i++) {
                TokenAmount storage extra = c.extraTokens[i];
                _payOut(extra.mint, c.responsibleOperator, extra.amount);
            }
        } else {
            bytes memory expectedProg = c.ipowReceiveProgram.length == 0 ? c.userProgram : c.ipowReceiveProgram;
            if (keccak256(outProgram) != keccak256(expectedProg)) revert BadBitcoinProgram();
            if (outValueSats < c.bitcoinAmount) revert IncorrectValue();

            uint256 payout = c.nativeAmount;
            totalReservedAmount[c.tokenAddr] -= payout;
            _payOut(c.tokenAddr, c.user, payout);

            for (uint256 i = 0; i < c.extraTokens.length; i++) {
                TokenAmount storage extra = c.extraTokens[i];
                _payOut(extra.mint, c.user, extra.amount);
            }
        }

        if (feeToOperator > 0) {
            c.commitFee = 0;
            totalHeldCommitFees -= feeToOperator;
            _payBond(c.responsibleOperator, feeToOperator);
        }

        uint256 stakePayout = c.stakedBond + c.bounty;
        c.stakedBond = 0;
        c.bounty = 0;
        _payBond(c.responsibleOperator, stakePayout);

        c.proofBlockHeight = blockHeight;
        c.status = Status.Completed;
        c.proofVerified = true;

        emit ConversionCompleted(txId);
    }

    /// @notice Same as `iPoWConversion.refundNoProofNativeToBitcoin`
    /// except the commit-fee refund moves PathUSD (`_payBond`), not
    /// native value. The conversion's own value refund (`_payOut`,
    /// following `tokenAddr`) is unchanged.
    function refundNoProofNativeToBitcoin(uint256 txId) external validTx(txId) {
        Conversion storage c = conversions[txId];

        if (!c.isNativeToBitcoin) revert WrongConversionType();
        if (c.status != Status.Deposited) revert BadState();

        uint256 proofWindowEnd = c.windowStartHeight + (PROOF_BLOCKS_WINDOW - 1);
        if (ipowHeaders.globalTipHeight() <= proofWindowEnd) revert DutyNotExpired();

        c.status = Status.Refunded;

        uint256 refundedAmount = c.nativeAmount;
        uint256 refundedFee = c.commitFee;
        totalLockedDeposits[c.tokenAddr] -= refundedAmount;
        totalHeldCommitFees -= refundedFee;

        _payOut(c.tokenAddr, c.user, refundedAmount);
        _payBond(c.user, refundedFee);

        for (uint256 i = 0; i < c.extraTokens.length; i++) {
            TokenAmount storage extra = c.extraTokens[i];
            _payOut(extra.mint, c.user, extra.amount);
        }

        emit ConversionRefunded(txId, refundedAmount);
    }

    /// @notice Same as `iPoWConversion.claimNativeOperatorExpired` —
    /// unchanged; no fee/stake involved, only the conversion's own value
    /// (already follows `tokenAddr` correctly).
    function claimNativeOperatorExpired(uint256 txId) external validTx(txId) {
        Conversion storage c = conversions[txId];

        if (c.isNativeToBitcoin) revert WrongConversionType();
        if (c.status != Status.Approved) revert BadState();
        if (c.operatorDutyExpiresAt == 0 || block.timestamp <= c.operatorDutyExpiresAt) {
            revert DutyNotExpired();
        }

        c.status = Status.Completed;

        uint256 amountToClaim = c.reservedNative;
        totalReservedAmount[c.tokenAddr] -= amountToClaim;

        _payOut(c.tokenAddr, c.user, amountToClaim);

        for (uint256 i = 0; i < c.extraTokens.length; i++) {
            TokenAmount storage extra = c.extraTokens[i];
            _payOut(extra.mint, c.user, extra.amount);
        }

        emit ConversionCompleted(txId);
    }
}
