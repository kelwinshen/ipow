// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

import {ReentrancyGuard} from "@openzeppelin/contracts/utils/ReentrancyGuard.sol";
import {BitcoinPrimitives} from "../libraries/BitcoinPrimitives.sol";
import {iPoWV1Storage} from "../base/iPoWV1Storage.sol";

/**
 * @title iPoWV1AdminFacet
 * @notice Hyperliquid-only split of `iPoWV1` (see `../iPoWV1Router.sol` and
 * docs/DESIGN_V2.md §8.16): admin, the Bitcoin header relay, and every view
 * function. Never called directly — only ever reached via the router's
 * `delegatecall`, so it operates on the router's storage the whole time.
 * Bit-for-bit the same logic as the monolithic `iPoWV1.sol`'s equivalent
 * functions; only the file they live in changed.
 */
contract iPoWV1AdminFacet is iPoWV1Storage, ReentrancyGuard {
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

        // Retry any proofs that were waiting on a header at this height. This facet
        // doesn't hold `_tryFinalizeProof` itself (that's the Conversion facet's) —
        // reach it via an external self-call through the router, exactly like any
        // other cross-facet call in this split.
        uint256[] memory autoIds = pendingProofsAtHeight[height];
        if (autoIds.length > 0) {
            for (uint256 i = 0; i < autoIds.length; i++) {
                (bool ok, ) = address(this).call(
                    abi.encodeWithSignature("tryFinalizeProof(uint256)", autoIds[i])
                );
                // `_tryFinalizeProof` never reverts by design (a bad proof is left to be
                // resubmitted, not treated as an error) — but the call itself could
                // fail to reach the Conversion facet if it's ever misconfigured, and
                // that must not brick header relay. Swallowed deliberately.
                ok;
            }
        }
    }

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

    function _filterTypeOf(Conversion storage c) internal view returns (TypeFilter) {
        if (!c.isNativeToBitcoin && c.networkId == 0) return TypeFilter.BITCOIN_TO_NATIVE;
        if (c.isNativeToBitcoin && c.networkId == 0) return TypeFilter.NATIVE_TO_BITCOIN;
        if (c.isNativeToBitcoin && c.networkId != 0) return TypeFilter.NATIVE_TO_NATIVE_OUT;
        if (!c.isNativeToBitcoin && c.networkId != 0) return TypeFilter.NATIVE_TO_NATIVE_IN;
        revert InvalidTypeFilter();
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
        if (c.refunded) return Phase.REFUNDED;

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
        if (header80.length != 80) revert InvalidHeader();

        hashLE = BitcoinPrimitives._hashHeaderLE(header80);
        prevLE = BitcoinPrimitives._extractPrevLE(header80);
        merkleLE = BitcoinPrimitives._extractMerkleLE(header80);
        nBits = BitcoinPrimitives._readCompact(header80);
        timestamp = BitcoinPrimitives._extractTimestamp(header80);
    }
}
