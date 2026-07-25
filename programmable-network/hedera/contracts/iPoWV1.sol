// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

import "@openzeppelin/contracts/utils/ReentrancyGuard.sol";
import {BitcoinPrimitives} from "./libraries/BitcoinPrimitives.sol";
import {iPoWV1Types} from "./base/iPoWV1Types.sol";

/**
 * @title iPoWV1
 * @author kelwinshen - Protocol Engineer, iPoW
 * @notice iPoW (Interoperable Proof of Work) lets otherwise-incompatible programmable
 * networks communicate trustlessly, using Bitcoin's Proof of Work as the shared root
 * of trust rather than relying on any one chain's own security model.
 * @dev Built around a shared, canonical relay of Bitcoin block headers and windowed
 * SPV proofs anchored to specific block heights. The relay streams contiguously
 * (height = tip + 1) while any conversion is active, and can jump ahead otherwise.
 */
contract iPoWV1 is iPoWV1Types, ReentrancyGuard {

    address public operator;

    modifier onlyOperator() {
        if (msg.sender != operator) revert Unauthorized();
        _;
    }

    uint256 public immutable NATIVE_DECIMALS;
    uint256 public immutable SELF_NETWORK_ID;

    /// @dev Basis points, e.g. 50 = 0.5%.
    uint256 public commitFeeBps;

    uint256 public nativeLiquidity;

    // --- Safety buckets: liabilities carved out of the contract's raw balance ---
    uint256 public totalLockedDeposits;
    uint256 public totalReservedNative;
    uint256 public totalHeldCommitFees;

    uint256 public minAnchorHeight;

    uint256 public nextTxId = 1;
    mapping(uint256 => Conversion) public conversions;

    // --- Global Bitcoin header relay ---
    mapping(uint256 => bytes32) public globalHeightToHashLE;
    mapping(bytes32 => GlobalHeaderMeta) public globalHeaders;
    uint256 public globalTipHeight;

    /// @dev Keyed by keccak256(txidLE || headerHashLE) — stops a single proof from
    /// settling more than one conversion.
    mapping(bytes32 => bool) public usedProofs;

    mapping(uint256 => NetworkConfig) public networkConfigs;

    /// @dev Bitcoin height -> txIds whose proofs are waiting on a header at that height.
    mapping(uint256 => uint256[]) internal pendingProofsAtHeight;

    /// @dev Guards against reusing the same protocol-owned Bitcoin script across
    /// conversions.
    mapping(bytes => bool) public usedIPoWPrograms;

    mapping(uint256 => HeaderWindow) private windows;

    /// @dev Gates whether the header relay may jump ahead or must stream contiguously.
    uint256 public activeOpenConversions;

    modifier validTx(uint256 txId) {
        if (txId == 0 || txId >= nextTxId) revert BadTxId();
        _;
    }

    constructor(
        uint256 _nativeDecimals,
        uint256 _selfNetworkId,
        address _operator,
        uint256 _commitFeeBps
    ) {
        if (_selfNetworkId == 0 || _operator == address(0) || _commitFeeBps > BPS_DENOM) revert InvalidConstructor();

        NATIVE_DECIMALS = _nativeDecimals;
        SELF_NETWORK_ID = _selfNetworkId;
        operator = _operator;
        commitFeeBps = _commitFeeBps;
    }

    function setOperator(address newOperator) external onlyOperator {
        operator = newOperator;
        emit OperatorChanged(newOperator);
    }

    /// @dev Locked while any conversion is active, since changing routing mid-flight
    /// could strand an in-progress conversion.
    function addNetwork(
        uint256 networkId,
        uint16 minAddrLen,
        uint16 maxAddrLen
    ) external onlyOperator {
        if (activeOpenConversions > 0) revert NetworkChangeLocked();
        if (
            networkId == 0 ||
            networkId == SELF_NETWORK_ID ||
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

    function removeNetwork(uint256 networkId) external onlyOperator {
        if (activeOpenConversions > 0) revert NetworkChangeLocked();
        if (networkId == 0 || networkConfigs[networkId].enabled == false) revert InvalidNetworkConfig();

        delete networkConfigs[networkId];
    }

    function setFees(uint256 newCommitFeeBps) external onlyOperator {
        if (newCommitFeeBps > BPS_DENOM) revert InvalidFeeConfig();
        commitFeeBps = newCommitFeeBps;
        emit FeesUpdated(newCommitFeeBps);
    }

    function addNativeLiquidity() external payable onlyOperator {
        if (msg.value == 0) revert ZeroValue();
        nativeLiquidity += msg.value;
        emit LiquidityUpdated(nativeLiquidity);
    }

    /// @notice Native assets available for withdrawal: contract balance minus every
    /// liability bucket, capped at whatever's actually tracked as liquidity.
    function removableNative() public view returns (uint256) {
        uint256 bal = address(this).balance;

        uint256 unavailable = totalLockedDeposits + totalReservedNative + totalHeldCommitFees;
        if (bal <= unavailable) return 0;

        uint256 byBalance = bal - unavailable;

        if (nativeLiquidity < byBalance) return nativeLiquidity;
        return byBalance;
    }


    function removeNativeLiquidity(uint256 amount) external onlyOperator nonReentrant {
        uint256 removable = removableNative();
        if (amount > removable) revert ExceedsRemovable();
        
        nativeLiquidity -= amount;
        
        (bool ok, ) = payable(msg.sender).call{value: amount}("");
        if (!ok) revert TransferFailed();
        
        emit LiquidityUpdated(nativeLiquidity);
    }

    /// @notice Starts a Native-to-Bitcoin conversion: direct settlement if networkId is 0,
    /// otherwise routed to a remote network. Escrows the commit fee and records the
    /// conversion; the operator settles it later.
    function commitNativeToBitcoin(
        uint256 nativeAmount,
        uint256 bitcoinAmount,
        uint256 networkId,
        bytes calldata networkAddress,
        bytes calldata userProgram
    ) external payable {
        uint256 requiredFee = (nativeAmount * commitFeeBps) / BPS_DENOM;
        if (msg.value != requiredFee) revert IncorrectCommitFee();
        if (nativeAmount == 0 || bitcoinAmount == 0) revert ZeroValue();

        _validateNetwork(networkId);

        if (networkId == 0) {
            if (userProgram.length == 0 || userProgram.length > 80) revert BadBitcoinProgram();
            if (networkAddress.length > 0) revert NetworkAddressNotAllowed();
        } else {
            if (userProgram.length > 0) revert UserBitcoinProgramNotAllowed();
            _validateNetworkAddress(networkId, networkAddress);
        }

        uint256 txId = nextTxId++;
        conversions[txId] = Conversion({
            user: msg.sender,
            isNativeToBitcoin: true,
            userProgram: networkId == 0 ? userProgram : bytes(""),
            ipowReceiveProgram: "",
            nativeAmount: nativeAmount,
            bitcoinAmount: bitcoinAmount,
            createdAt: block.timestamp,
            approvedAt: 0,
            depositedAt: 0,
            commitFee: msg.value,
            approved: false,
            deposited: false,
            completed: false,
            refunded: false,
            reservedNative: 0,
            operatorDutyExpiresAt: 0,
            networkId: networkId,
            networkAddress: networkId == 0 ? bytes("") : networkAddress
        });

        totalHeldCommitFees += msg.value;
        emit ConversionCommitted(txId, msg.sender, true);
    }

    /// @notice Starts a Bitcoin-to-Native conversion. Two very different paths share
    /// this one function: a regular user commit (awaits operator approval later), or
    /// the operator directly opening an already-approved "tunnel". See the branch
    /// below for why they're split this way.
    function commitBitcoinToNative(
        uint256 bitcoinAmount,
        uint256 nativeAmount,
        uint256 networkId,
        bytes calldata userProgram,
        address destAddress,
        bytes calldata networkAddress,
        uint256 dutyWindowSeconds,
        bytes calldata ipowReceiveProgram,
        uint256 lockedAnchorHeight
    ) external payable {
        if (bitcoinAmount == 0 || nativeAmount == 0) revert ZeroValue();

        bool isOperator = (msg.sender == operator);
        uint256 txId = nextTxId++;

        // --- User path ---
        if (!isOperator) {
            uint256 requiredFee = (nativeAmount * commitFeeBps) / BPS_DENOM;
            if (msg.value != requiredFee) revert IncorrectCommitFee();

            if (networkId > 0) revert NetworkNotAllowed();
            if (networkAddress.length > 0) revert NetworkAddressNotAllowed();
            if (userProgram.length == 0 || userProgram.length > 80) revert BadBitcoinProgram();
        }
        // --- Operator tunnel path: builds and saves its own Conversion, then
        // returns early below, skipping the "user path" save at the bottom. ---
        else {
            if (lockedAnchorHeight < minAnchorHeight || lockedAnchorHeight > globalTipHeight) revert InvalidAnchorHeight();
            if (destAddress == address(0)) revert NeedDestAddress();
            if (msg.value > 0) revert UnexpectedValue();
            if (networkId == 0) revert IncorrectNetwork();
            if (dutyWindowSeconds == 0) revert NeedDutyWindow();
            if (ipowReceiveProgram.length == 0 || ipowReceiveProgram.length > 80) revert BadBitcoinProgram();
            if (userProgram.length > 0) revert UserBitcoinProgramNotAllowed();

            if (usedIPoWPrograms[ipowReceiveProgram]) revert ProgramAlreadyUsed();
            usedIPoWPrograms[ipowReceiveProgram] = true;

            _validateNetwork(networkId);
            _validateNetworkAddress(networkId, networkAddress);

            uint256 reserve = (nativeAmount * RESERVE_MARGIN_BPS) / BPS_DENOM;
            if (_availableForReserve() < reserve) revert LowReserve();

            conversions[txId] = Conversion({
                user: destAddress,
                isNativeToBitcoin: false,
                userProgram: "",
                ipowReceiveProgram: ipowReceiveProgram,
                nativeAmount: nativeAmount,
                bitcoinAmount: bitcoinAmount,
                createdAt: block.timestamp,
                approvedAt: block.timestamp,
                depositedAt: 0,
                commitFee: 0,
                approved: true,
                deposited: false,
                completed: false,
                refunded: false,
                reservedNative: reserve,
                operatorDutyExpiresAt: block.timestamp + dutyWindowSeconds,
                networkId: networkId,
                networkAddress: networkAddress
            });

            totalReservedNative += reserve;

            uint256 firstHeight = lockedAnchorHeight - (lockedAnchorHeight % DIFF_PERIOD);
            _startHeaderWindow(txId, lockedAnchorHeight, firstHeight);

            emit ConversionCommitted(txId, msg.sender, false);
            emit ConversionApproved(txId, dutyWindowSeconds, firstHeight, globalHeightToHashLE[firstHeight]);

            return;
        }

        conversions[txId] = Conversion({
            user: msg.sender,
            isNativeToBitcoin: false,
            userProgram: userProgram,
            ipowReceiveProgram: "",
            nativeAmount: nativeAmount,
            bitcoinAmount: bitcoinAmount,
            createdAt: block.timestamp,
            approvedAt: 0,
            depositedAt: 0,
            commitFee: msg.value,
            approved: false,
            deposited: false,
            completed: false,
            refunded: false,
            reservedNative: 0,
            operatorDutyExpiresAt: 0,
            networkId: networkId,
            networkAddress: networkId == 0 ? bytes("") : networkAddress
        });
        
        totalHeldCommitFees += msg.value;
        emit ConversionCommitted(txId, msg.sender, false);
    }

    /// @notice Operator approval for a pending conversion: reserves liquidity, binds the
    /// Bitcoin script to watch, and anchors the header window it'll be proven against.
    function approveAndStartWithAnchorAndFirst(
        uint256 txId,
        uint256 dutyWindowSeconds,
        bytes calldata ipowReceiveProgram
    ) external onlyOperator validTx(txId) {
        Conversion storage c = conversions[txId];

        if (c.networkId > 0) {
            _validateNetwork(c.networkId);
            _validateNetworkAddress(c.networkId, c.networkAddress);
        }
        if (c.approved || c.refunded || c.completed) revert BadState();
        if (dutyWindowSeconds == 0) revert NeedDutyWindow();
        if (block.timestamp > c.createdAt + APPROVAL_WINDOW_SEC) revert ApproveWindowOver();

        c.approved = true;
        c.approvedAt = block.timestamp;
        c.operatorDutyExpiresAt = block.timestamp + dutyWindowSeconds;

        if (!c.isNativeToBitcoin) {
            uint256 reserve = (c.nativeAmount * RESERVE_MARGIN_BPS) / BPS_DENOM;
            if (_availableForReserve() < reserve) revert LowReserve();

            c.reservedNative = reserve;
            totalReservedNative += reserve;

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

        uint256 firstHeight = globalTipHeight - (globalTipHeight % DIFF_PERIOD);
        _startHeaderWindow(txId, globalTipHeight, firstHeight);

        emit ConversionApproved(
            txId,
            dutyWindowSeconds,
            firstHeight,
            globalHeightToHashLE[firstHeight]
        );
    }

    /// @notice Appends one Bitcoin block header to the global relay, after checking its
    /// Proof-of-Work and (at epoch boundaries) its difficulty retarget.
    function commitGlobalBitcoinHeader80(
        bytes calldata header80,
        uint256 height
    ) external onlyOperator {
        if (header80.length != 80) revert InvalidHeader();

        bytes32 hHashLE = BitcoinPrimitives._hashHeaderLE(header80);
        bytes32 prevLE = BitcoinPrimitives._extractPrevLE(header80);
        bytes32 mRootLE = BitcoinPrimitives._extractMerkleLE(header80);
        uint256 target = BitcoinPrimitives._extractTarget(header80);
        uint32 bits = BitcoinPrimitives._readCompact(header80);
        uint32 ts = BitcoinPrimitives._extractTimestamp(header80);

        if (!BitcoinPrimitives._validateWorkLE(hHashLE, target)) revert LowWork();

        bytes32 existing = globalHeightToHashLE[height];
        if (existing != bytes32(0)) {
            // Already-recorded heights are immutable — same header only.
            if (existing != hHashLE) revert HeightRewrite();
        } else {
            if (globalTipHeight != 0) {
                if (activeOpenConversions > 0) {
                    // Active conversions rely on contiguous proofs — no jumping ahead.
                    if (height != globalTipHeight + 1) revert NoJumpWhenActive();
                    if (prevLE != globalHeightToHashLE[globalTipHeight]) revert PrevAndTipUnmatch();
                } else {
                    // No active conversions, so the relay is free to jump ahead.
                    if (height == globalTipHeight + 1) {
                        if (prevLE != globalHeightToHashLE[globalTipHeight]) revert PrevAndTipUnmatch();
                    } else if (height > globalTipHeight + 1) {
                        // Need the epoch's first header on record to validate future retargets.
                        uint256 epochStart = height - (height % DIFF_PERIOD);
                        if (epochStart > 0 && epochStart < height) {
                            if (globalHeightToHashLE[epochStart] == bytes32(0)) revert EpochFirstMissing();
                        }
                        if (height > minAnchorHeight) minAnchorHeight = height;
                    }
                }
            }

            if (height % DIFF_PERIOD == 0 && globalTipHeight != 0 && height == globalTipHeight + 1) {
                uint32 expectedBits = _expectedRetargetBits(height);
                if (bits != expectedBits) revert InvalidRetarget();
            }

            globalHeightToHashLE[height] = hHashLE;
            globalHeaders[hHashLE] = GlobalHeaderMeta({
                prevHashLE: prevLE,
                merkleRootLE: mRootLE,
                nBits: bits,
                timestamp: ts,
                set: true,
                arrivalTime: uint64(block.timestamp)
            });

            globalTipHeight = height;
            emit GlobalHeaderAppended(height, hHashLE, prevLE, mRootLE, bits, ts);
        }

        // Retry any proofs that were waiting on a header at this height.
        uint256[] memory autoIds = pendingProofsAtHeight[height];
        if (autoIds.length > 0) {
            for (uint256 i = 0; i < autoIds.length; i++) {
                _tryFinalizeProof(autoIds[i]);
            }
        }
    }


    function depositApprovedConversion(uint256 txId) external payable validTx(txId) nonReentrant {
        Conversion storage c = conversions[txId];

        if (!c.isNativeToBitcoin) revert WrongConversionType();
        if (msg.sender != c.user) revert Unauthorized();
        if (!c.approved || c.deposited || c.completed || c.refunded) revert BadState();

        HeaderWindow storage hw = windows[txId];
        if (!hw.started) revert NoHeadersYet();

        if (globalTipHeight > hw.windowStartHeight + (DEPOSIT_BLOCKS_WINDOW - 1)) revert IncorrectWindow();

        if (msg.value != c.nativeAmount) revert IncorrectValue();

        c.deposited = true;
        c.depositedAt = block.timestamp;
        totalLockedDeposits += msg.value;

        emit ConversionDeposited(txId, msg.value);
    }

    /// @notice Submits a Bitcoin Merkle proof for a conversion, replacing any previous
    /// pending proof. Native-to-Bitcoin: the operator proves the payout was sent.
    /// Bitcoin-to-Native: the user proves their deposit was made.
    function submitBitcoinMerkleProofWithTx(
        uint256 txId,
        bytes calldata txRaw,
        uint256 voutIndex,
        bytes32 blockHashLE,
        uint256 blockHeight,
        bytes32[] calldata branchLE,
        uint256 index
    ) external validTx(txId) {
        HeaderWindow storage hw = windows[txId];

        if (!hw.started) revert NoHeadersYet();
        if (blockHeight < hw.windowStartHeight || blockHeight > hw.windowStartHeight + (PROOF_BLOCKS_WINDOW - 1)) {
            revert IncorrectWindow();
        }

        Conversion storage c = conversions[txId];

        if (c.isNativeToBitcoin) {
            if (msg.sender != operator) revert Unauthorized();
        } else {
            if (msg.sender != c.user) revert Unauthorized();
        }

        ProofCache storage p = hw.proof;
        if (p.verified) revert AlreadyVerified();

        bytes32 txidLE = sha256(abi.encodePacked(sha256(txRaw)));
        p.txidLE = txidLE;

        (uint64 valueSats, bytes memory program) = BitcoinPrimitives._parseOutputAt(txRaw, voutIndex);

        if (p.set && !p.verified) {
            delete p.branchLE;
            p.attempts += 1;
        } else {
            p.attempts = 1;
        }

        p.set = true;
        p.verified = false;
        p.invalid = false;
        p.blockHashLE = blockHashLE;
        p.blockHeight = blockHeight;
        p.index = index;

        // Persist the Merkle path
        for (uint256 i = 0; i < branchLE.length; i++) {
            p.branchLE.push(branchLE[i]);
        }

        p.outValueSats = valueSats;
        p.outProgram = program;
        p.outSet = true;

        // Wait for the header to arrive (if it hasn't already) before this can finalize.
        pendingProofsAtHeight[blockHeight].push(txId);

        _tryFinalizeProof(txId);
    }

    /// @notice Cancels a Native-to-Bitcoin conversion if the user never deposited within
    /// the window. Only usable once the operator's own duty is still on track — an
    /// operator who's already fallen behind can't use this to bail out.
    function timeoutNoDeposit_NativetoBitcoin(uint256 txId) external validTx(txId) onlyOperator nonReentrant {
        Conversion storage c = conversions[txId];

        if (!c.isNativeToBitcoin) revert WrongConversionType();
        if (!c.approved || c.deposited || c.completed || c.refunded) revert BadState();
        if (!_isOperatorDutyFulfilled(txId)) revert DutyExpired();

        HeaderWindow storage hw = windows[txId];
        if (!hw.started) revert NoHeadersYet();

        if (globalTipHeight <= hw.windowStartHeight + (DEPOSIT_BLOCKS_WINDOW - 1)) revert IncorrectWindow();

        _payOperatorCommitFee(c);
        c.refunded = true;
        _closeActive(txId);

        emit ConversionRefunded(txId, 0, false);
    }

    /// @notice Refunds a Native-to-Bitcoin conversion once it's clear the operator won't
    /// finalize it: full refund (deposit + fee) if the user had deposited, fee-only
    /// refund otherwise. The specific deadline that applies depends on whether the
    /// operator ever started streaming headers for this conversion.
    function refundAfterNoProof_NativeToBitcoin(uint256 txId) external validTx(txId) nonReentrant {
        Conversion storage c = conversions[txId];
        if (!c.isNativeToBitcoin) revert WrongConversionType();

        if (msg.sender != c.user && msg.sender != operator) revert Unauthorized();
        if (!c.approved || c.completed || c.refunded) revert BadState();

        HeaderWindow storage hw = windows[txId];

        bool dutyFulfilled = _isOperatorDutyFulfilled(txId);
        if (dutyFulfilled) {
            // Operator already completed the swap — nothing to refund here.
            revert AlreadyVerified();
        }

        if (!hw.started) {
            // Operator never submitted headers for this conversion at all.
            if (c.operatorDutyExpiresAt == 0 || block.timestamp <= c.operatorDutyExpiresAt) {
                revert DutyNotExpired();
            }
        } else {
            // Operator streamed headers, but the proof still isn't verified.
            uint256 endHeight = hw.windowStartHeight + (PROOF_BLOCKS_WINDOW - 1) + (CONFIRMATIONS_REQUIRED - 1);
            if (globalTipHeight <= endHeight) revert IncorrectWindow();
        }

        c.refunded = true;

        if (c.deposited) {
            totalLockedDeposits -= c.nativeAmount;
            totalHeldCommitFees -= c.commitFee;

            _closeActive(txId);

            (bool ok, ) = payable(c.user).call{value: c.nativeAmount + c.commitFee}("");
            if (!ok) revert TransferFailed();
            emit ConversionRefunded(txId, c.nativeAmount + c.commitFee, true);
        } else {
            uint256 fee = c.commitFee;
            totalHeldCommitFees -= fee;
            c.commitFee = 0;
            
            _closeActive(txId);

            (bool ok, ) = payable(c.user).call{value: fee}("");
            if (!ok) revert TransferFailed();
            emit ConversionRefunded(txId, fee, true);
        }
    }

    /// @notice Operator closes a Bitcoin-to-Native conversion the user never proved.
    /// Gated on the operator's own duty being fulfilled, so an operator who stalled the
    /// relay can't use this to walk away clean.
    function closeNoBitcoin_BitcoinToNative(uint256 txId) external validTx(txId) onlyOperator nonReentrant {
        Conversion storage c = conversions[txId];

        if (c.isNativeToBitcoin) revert WrongConversionType();
        if (!c.approved || c.completed || c.refunded) revert BadState();

        HeaderWindow storage hw = windows[txId];
        if (!hw.started) revert NoHeadersYet();

        uint256 endHeight = hw.windowStartHeight + (PROOF_BLOCKS_WINDOW - 1) + (CONFIRMATIONS_REQUIRED - 1);
        if (globalTipHeight <= endHeight) revert IncorrectWindow();

        if (!_isOperatorDutyFulfilled(txId)) revert DutyExpired();

        c.refunded = true;
        totalReservedNative -= c.reservedNative;
        c.reservedNative = 0;

        _payOperatorCommitFee(c);
        _closeActive(txId);

        emit ConversionRefunded(txId, 0, false);
    }

    /// @notice Lets the user claim their reserved payout plus commit fee directly if the
    /// operator misses their duty window — the operator's penalty for failing to deliver.
    function claimNative_AfterOperatorExpired(uint256 txId) external validTx(txId) nonReentrant {
        Conversion storage c = conversions[txId];

        if (c.isNativeToBitcoin) revert WrongConversionType();
        if (msg.sender != c.user && msg.sender != operator) revert Unauthorized();
        if (!c.approved || c.completed || c.refunded) revert BadState();

        if (c.operatorDutyExpiresAt == 0 || block.timestamp <= c.operatorDutyExpiresAt || _isOperatorDutyFulfilled(txId)) {
            revert DutyNotExpired();
        }

        uint256 nativeOut = c.reservedNative;

        totalReservedNative -= c.reservedNative;
        nativeLiquidity -= nativeOut;
        c.reservedNative = 0;
        
        totalHeldCommitFees -= c.commitFee;

        c.completed = true;
        _closeActive(txId);

        (bool ok, ) = payable(c.user).call{value: nativeOut + c.commitFee}("");
        if (!ok) revert TransferFailed();

        emit ConversionRefunded(txId, nativeOut + c.commitFee, true);
        emit ConversionCompleted(txId);
    }

    function refundIfNotApproved(uint256 txId) external validTx(txId) nonReentrant {
        Conversion storage c = conversions[txId];
        if (c.user != msg.sender) revert Unauthorized();
        if (c.approved || c.completed || c.refunded) revert BadState();

        c.refunded = true;
        totalHeldCommitFees -= c.commitFee;

        (bool ok, ) = payable(c.user).call{value: c.commitFee}("");
        if (!ok) revert TransferFailed();

        emit ConversionRefunded(txId, 0, true);
    }

    // ========= INTERNAL HELPERS =========

    function _filterTypeOf(Conversion storage c) internal view returns (TypeFilter) {
        if (!c.isNativeToBitcoin && c.networkId == 0) return TypeFilter.BITCOIN_TO_NATIVE;
        if (c.isNativeToBitcoin && c.networkId == 0) return TypeFilter.NATIVE_TO_BITCOIN;
        if (c.isNativeToBitcoin && c.networkId != 0) return TypeFilter.NATIVE_TO_NATIVE_OUT;
        if (!c.isNativeToBitcoin && c.networkId != 0) return TypeFilter.NATIVE_TO_NATIVE_IN;
        revert InvalidTypeFilter();
    }

    function _validateNetwork(uint256 networkId) internal view {
        if (networkId == 0) {
            // Direct Bitcoin settlement is always eligible.
            return;
        }

        if (networkId == SELF_NETWORK_ID || !networkConfigs[networkId].enabled) {
            revert IncorrectNetwork();
        }
    }

    function _validateNetworkAddress(
        uint256 networkId,
        bytes memory networkAddress
    ) internal view {
        // Direct Bitcoin settlement has no network-address length to check.
        if (networkId == 0) return;

        NetworkConfig memory cfg = networkConfigs[networkId];
        uint256 len = networkAddress.length;

        if (len < cfg.minAddrLen || len > cfg.maxAddrLen) {
            revert IncorrectNetworkAddress();
        }
    }

    // ========= INTERNAL WINDOW & DUTY LOGIC =========

    function _startHeaderWindow(
        uint256 txId,
        uint256 anchorHeight,
        uint256 firstHeight
    ) internal {
        if (anchorHeight < minAnchorHeight || anchorHeight > globalTipHeight) revert InvalidAnchorHeight();

        HeaderWindow storage hw = windows[txId];
        if (hw.started) revert HeaderStarted();

        bytes32 fHashLE = globalHeightToHashLE[firstHeight];
        if (fHashLE == bytes32(0)) revert GlobalFirstHeaderMissing();

        GlobalHeaderMeta storage fMeta = globalHeaders[fHashLE];
        if (!fMeta.set) revert MetaFirstHeaderMissing();

        bytes32 aHashLE = globalHeightToHashLE[anchorHeight];
        if (aHashLE == bytes32(0)) revert GlobalAnchorMissing();

        GlobalHeaderMeta storage aMeta = globalHeaders[aHashLE];
        if (!aMeta.set) revert MetaAnchorHeaderMissing();

        hw.started = true;
        hw.epochStartHeight = firstHeight;
        hw.windowStartHeight = anchorHeight;

        activeOpenConversions += 1;
    }

    /// @dev "Duty fulfilled" means: the header this conversion needs (10 blocks deep for
    /// a pending deposit, 40 for a pending proof) has arrived, and arrived before the
    /// operator's deadline. For a deposited Native-to-Bitcoin conversion, the proof must
    /// also actually be verified — arriving on time isn't enough by itself.
    function _isOperatorDutyFulfilled(uint256 txId) internal view returns (bool) {
        HeaderWindow storage hw = windows[txId];
        Conversion storage c = conversions[txId];

        if (!hw.started) return false;

        uint256 requiredWindow = PROOF_BLOCKS_WINDOW;
        if (c.isNativeToBitcoin && !c.deposited) {
            requiredWindow = DEPOSIT_BLOCKS_WINDOW;
        }

        uint256 targetHeight = hw.windowStartHeight + (requiredWindow - 1);
        bytes32 targetHash = globalHeightToHashLE[targetHeight];

        if (targetHash == bytes32(0)) return false;

        GlobalHeaderMeta storage meta = globalHeaders[targetHash];
        bool headersOnTime = (meta.arrivalTime <= c.operatorDutyExpiresAt);

        if (!headersOnTime) return false;

        if (c.isNativeToBitcoin && c.deposited) {
            return hw.proof.verified;
        }

        return true;
    }

    function _computePhase(
        Conversion storage c,
        HeaderWindow storage hw,
        uint256 txId
    ) internal view returns (Phase) {

        if (c.user == address(0)) return Phase.NONE;
        if (c.completed) return Phase.COMPLETED;
        if (c.refunded)  return Phase.REFUNDED;

        if (!c.approved) {
            if (block.timestamp <= c.createdAt + APPROVAL_WINDOW_SEC) {
                return Phase.WAITING_OPERATOR_APPROVAL;
            } else {
                return Phase.OPERATOR_APPROVAL_EXPIRED;
            }
        }

        bool isTimeExpired = (c.operatorDutyExpiresAt != 0 && block.timestamp > c.operatorDutyExpiresAt);
        bool isDutyFulfilled = _isOperatorDutyFulfilled(txId);

        if (c.isNativeToBitcoin) {
            if (!c.deposited) {
                uint256 depositEnd = hw.windowStartHeight + (DEPOSIT_BLOCKS_WINDOW - 1);

                if (globalTipHeight > depositEnd || isTimeExpired) {
                    return Phase.USER_ACTION_EXPIRED;
                }
                return Phase.WAITING_USER_ACTION;
            }

            uint256 targetHeight = hw.windowStartHeight + (PROOF_BLOCKS_WINDOW - 1);
            bool streamFinished = (globalHeightToHashLE[targetHeight] != bytes32(0));

            if ((isTimeExpired || streamFinished) && !isDutyFulfilled) {
                return Phase.OPERATOR_DUTY_EXPIRED;
            }

            return Phase.ACTIVE_WAITING_PROOF;

        } else {
            ProofCache storage p = hw.proof;

            if (isDutyFulfilled) {
                return Phase.USER_ACTION_EXPIRED;
            }

            // isDutyFulfilled is known false here (checked above), so an expired
            // duty window is unconditionally the operator's fault.
            if (isTimeExpired) {
                return Phase.OPERATOR_DUTY_EXPIRED;
            }

            if (!p.set) return Phase.WAITING_USER_ACTION;
            return Phase.ACTIVE_WAITING_PROOF;
        }
    }

    // ========= INTERNAL CONSENSUS & STATE =========

    /// @dev Standard Bitcoin difficulty retarget: clamp the epoch's actual timespan to
    /// [1/4, 4x] of the target, then scale the previous target by that ratio.
    function _expectedRetargetBits(uint256 newEpochHeight) internal view returns (uint32) {
        uint256 prevEpochStart = newEpochHeight - DIFF_PERIOD;
        uint256 prevEpochEnd = newEpochHeight - 1;

        bytes32 startHash = globalHeightToHashLE[prevEpochStart];
        bytes32 endHash = globalHeightToHashLE[prevEpochEnd];

        if (startHash == bytes32(0) || endHash == bytes32(0)) revert EpochAnchorsMissing();

        GlobalHeaderMeta storage startMeta = globalHeaders[startHash];
        GlobalHeaderMeta storage endMeta = globalHeaders[endHash];

        if (!startMeta.set || !endMeta.set) revert EpochMetaMissing();

        uint256 actual = endMeta.timestamp - startMeta.timestamp;

        if (actual < MIN_TIMESPAN_SEC) actual = MIN_TIMESPAN_SEC;
        if (actual > MAX_TIMESPAN_SEC) actual = MAX_TIMESPAN_SEC;

        uint256 prevTarget = BitcoinPrimitives._targetFromBits(startMeta.nBits);
        uint256 newTarget = (prevTarget * actual) / RETARGET_PERIOD_SEC;

        uint256 limit = BitcoinPrimitives._powLimit();
        if (newTarget > limit) newTarget = limit;

        return BitcoinPrimitives._bitsFromTarget(newTarget);
    }

    function _availableForReserve() internal view returns (uint256) {
        return removableNative();
    }

    function _closeActive(uint256 txId) internal {
        HeaderWindow storage hw = windows[txId];
        if (!hw.started || hw.closed) return;

        hw.closed = true;
        activeOpenConversions -= 1;
    }

    // ========= INTERNAL SETTLEMENT & VERIFICATION =========

    function _payOperatorCommitFee(Conversion storage c) internal {
        if (c.commitFee > 0) {
            uint256 fee = c.commitFee;
            c.commitFee = 0; // zeroed before the transfer, so it can't be claimed twice
            totalHeldCommitFees -= fee;

            (bool ok, ) = payable(operator).call{value: fee}("");
            if (!ok) revert TransferFailed();
        }
    }

    /// @dev "Soft" verifier: checks the proof cryptographically but never reverts on a
    /// logical failure, so a bad proof can just be resubmitted rather than bricking the
    /// conversion.
    function _tryFinalizeProof(uint256 txId) internal {
        Conversion storage c = conversions[txId];
        HeaderWindow storage hw = windows[txId];
        ProofCache storage p = hw.proof;

        if (!p.set || p.verified || c.completed) return;

        bytes32 bh = globalHeightToHashLE[p.blockHeight];
        if (bh == bytes32(0)) return; // header hasn't arrived yet

        if (globalTipHeight < p.blockHeight + (CONFIRMATIONS_REQUIRED - 1)) return;

        GlobalHeaderMeta storage hm = globalHeaders[bh];
        if (!hm.set || !p.outSet) {
            p.invalid = true;
            return;
        }

        bytes32 reuseKey = keccak256(abi.encodePacked(p.txidLE, bh));
        if (usedProofs[reuseKey]) {
            p.invalid = true;
            return;
        }

        bool ok = BitcoinPrimitives._proveMerkleLE(
            p.txidLE,
            hm.merkleRootLE,
            BitcoinPrimitives._packBranchStorage(p.branchLE),
            p.index
        );

        if (!ok) {
            p.invalid = true;
            return;
        }

        bool finalized = _finalizeAfterProof(txId, p.txidLE, bh);
        if (!finalized) return;

        usedProofs[reuseKey] = true;
        p.verified = true;
    }

    /// @notice Settles a conversion once its SPV proof has verified: checks the proved
    /// Bitcoin output against what was agreed, then releases/pays out accordingly.
    function _finalizeAfterProof(
        uint256 txId,
        bytes32 /* txidLE */,
        bytes32 /* headerHashLE */
    ) internal returns (bool) {
        Conversion storage c = conversions[txId];
        HeaderWindow storage hw = windows[txId];
        ProofCache storage p = hw.proof;

        if (!p.outSet) return false;

   if (c.isNativeToBitcoin) {
            if (!c.deposited) return false;

            if (uint256(p.outValueSats) < c.bitcoinAmount) return false;
            if (keccak256(p.outProgram) != keccak256(c.userProgram)) return false;

            totalLockedDeposits -= c.nativeAmount;
        } else {
            uint256 provedBitcoin = uint256(p.outValueSats);

            // Must match the script the operator committed to watch at approval time.
            if (keccak256(p.outProgram) != keccak256(c.ipowReceiveProgram)) {
                return false;
            }

            if (provedBitcoin < c.bitcoinAmount) return false;

            // Fixed-price settlement: exactly the agreed amount, no oracle or slippage.
            uint256 nativeOutNow = c.nativeAmount;

            uint256 reserve = c.reservedNative;
            totalReservedNative -= reserve;
            c.reservedNative = 0;
            nativeLiquidity -= nativeOutNow;

            (bool ok, ) = payable(c.user).call{value: nativeOutNow}("");
            if (!ok) return false;
        }

        // 2. Global State Cleanup & Rewards (Consolidated here to save gas)
        _payOperatorCommitFee(c);
        c.completed = true;
        emit ConversionCompleted(txId);
        
        _closeActive(txId);
        return true;
    }

    // ========= PUBLIC VIEW HELPERS =========

    function proofInfo(uint256 txId)
        external
        view
        validTx(txId)
        returns (
            bool set,
            bool verified,
            bool invalid,
            uint8 attempts,
            bytes32 txidLE,
            bytes32 blockHashLE,
            uint256 blockHeight,
            uint64 outValueSats,
            bytes memory outProgram
        )
    {
        HeaderWindow storage hw = windows[txId];
        ProofCache storage p = hw.proof;

        set = p.set;
        verified = p.verified;
        invalid = p.invalid;
        attempts = p.attempts;
        txidLE = p.txidLE;
        blockHashLE = p.blockHashLE;
        blockHeight = p.blockHeight;
        outValueSats = p.outValueSats;
        outProgram = p.outProgram;
    }

    function anchorInfo(uint256 txId)
        external
        view
        validTx(txId)
        returns (
            uint256 anchorHeight,
            uint256 epochFirstHeight
        )
    {
        HeaderWindow storage hw = windows[txId];
        if (hw.started == false) revert NoHeadersYet();

        anchorHeight = hw.windowStartHeight;
        epochFirstHeight = hw.epochStartHeight;
    }

    /// @notice Scans conversions in [fromTxId, toTxId] for ones matching the given
    /// filters, returning up to maxResults txIds. Meant for off-chain eth_call use.
    function getTxIdsByFilter(
        TypeFilter typeFilter,
        Phase phaseFilter,
        address userFilter,
        bytes calldata bitcoinProgramFilter,
        bool searchUserProgram,
        uint256 networkIdFilter,
        bool useNetworkIdFilter,
        uint256 fromTxId,
        uint256 toTxId,
        uint256 maxResults
    ) external view returns (uint256[] memory txIds) {
        if (fromTxId < 1) fromTxId = 1;
        if (toTxId >= nextTxId) toTxId = nextTxId - 1;

        if (fromTxId > toTxId || maxResults == 0) {
            return new uint256[](0);
        }

        uint256[] memory tmp = new uint256[](maxResults);
        uint256 count = 0;

        bytes32 filterHash;
        bool hasProgramFilter = bitcoinProgramFilter.length > 0;
        if (hasProgramFilter) {
            filterHash = keccak256(bitcoinProgramFilter);
        }

        for (uint256 txId = fromTxId; txId <= toTxId; txId++) {
            Conversion storage c = conversions[txId];
            if (c.user == address(0)) continue;

            if (typeFilter != TypeFilter.ANY && _filterTypeOf(c) != typeFilter) continue;
            if (userFilter != address(0) && c.user != userFilter) continue;
            if (useNetworkIdFilter && c.networkId != networkIdFilter) continue;

            if (hasProgramFilter) {
                bytes memory prog = searchUserProgram ? c.userProgram : c.ipowReceiveProgram;
                if (keccak256(prog) != filterHash) continue;
            }

            Phase currentPhase = _computePhase(c, windows[txId], txId);
            if (phaseFilter != Phase.NONE && currentPhase != phaseFilter) continue;

            tmp[count++] = txId;
            if (count == maxResults) break;
        }

        txIds = new uint256[](count);
        for (uint256 i = 0; i < count; i++) {
            txIds[i] = tmp[i];
        }
    }

    function getConversionWithPhase(uint256 txId)
        external
        view
        validTx(txId)
        returns (Conversion memory c, Phase phase)
    {
        Conversion storage cs = conversions[txId];
        HeaderWindow storage hw = windows[txId];

        // Computed fresh each call, so it reflects timeouts/expirations as of now
        // rather than whatever was last written to storage.
        phase = _computePhase(cs, hw, txId);
        c = cs;
    }

    function expectedNext(uint256 txId)
        external
        view
        validTx(txId)
        returns (
            bool headersStarted,
            uint256 nextHeight,
            bytes32 expectedPrevHashLE
        )
    {
        HeaderWindow storage hw = windows[txId];
        headersStarted = hw.started;

        if (!headersStarted) {
            return (false, 0, bytes32(0));
        }

        nextHeight = globalTipHeight + 1;
        expectedPrevHashLE = bytes32(0);

        if (globalTipHeight > 0) {
            expectedPrevHashLE = globalHeightToHashLE[globalTipHeight];
        }
    }

    function windowsFor(uint256 txId)
        external
        view
        validTx(txId)
        returns (
            bool headersStarted,
            uint256 startHeight,
            uint256 lastHeight,
            uint256 depositWindowEndHeight,
            uint256 proofWindowEndHeight,
            uint256 operatorDutyExpiresAt
        )
    {
        HeaderWindow storage hw = windows[txId];

        headersStarted = hw.started;
        startHeight = hw.windowStartHeight;
        lastHeight = globalTipHeight;

        depositWindowEndHeight = hw.started ? (hw.windowStartHeight + (DEPOSIT_BLOCKS_WINDOW - 1)) : 0;
        proofWindowEndHeight = hw.started ? (hw.windowStartHeight + (PROOF_BLOCKS_WINDOW - 1)) : 0;

        operatorDutyExpiresAt = conversions[txId].operatorDutyExpiresAt;
    }

    function debugDecodeHeader(bytes calldata header80)
        external
        pure
        returns (
            bytes32 hashLE,
            bytes32 prevLE,
            bytes32 merkleLE,
            uint32 nBits,
            uint32 timestamp
        )
    {
        if(header80.length != 80) revert InvalidHeader();

        hashLE = BitcoinPrimitives._hashHeaderLE(header80);
        prevLE = BitcoinPrimitives._extractPrevLE(header80);
        merkleLE = BitcoinPrimitives._extractMerkleLE(header80);
        nBits = BitcoinPrimitives._readCompact(header80);
        timestamp = BitcoinPrimitives._extractTimestamp(header80);
    }

    receive() external payable {}
}