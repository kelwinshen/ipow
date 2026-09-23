// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

import {ReentrancyGuard} from "@openzeppelin/contracts/utils/ReentrancyGuard.sol";
import {BitcoinPrimitives} from "../libraries/BitcoinPrimitives.sol";
import {iPoWV1Storage} from "../base/iPoWV1Storage.sol";

/**
 * @title iPoWV1ConversionSettlementFacet
 * @notice Hyperliquid-only split of `iPoWV1` (see `../iPoWV1Router.sol` and
 * docs/DESIGN_V2.md §8.16): the proof-submission/refund/settlement half of
 * the conversion lifecycle. Split out of what was originally one
 * `iPoWV1ConversionFacet` because that single facet's deployed bytecode
 * (~14.8KB) still didn't leave enough margin under HyperEVM testnet's
 * 3,000,000 block gas limit. This half and `iPoWV1ConversionEntryFacet`
 * need none of the same internal helpers, so the split needed zero
 * duplication. Never called directly — only ever reached via the router's
 * `delegatecall`, so it operates on the router's storage the whole time.
 * Bit-for-bit the same logic as the monolithic `iPoWV1.sol`'s equivalent
 * functions; only the file they live in changed, plus one addition —
 * `tryFinalizeProof`, a thin external wrapper around `_tryFinalizeProof`
 * purely so the Admin facet's header relay can reach it cross-facet
 * through the router (an `internal` function can't be called across
 * separately-deployed contracts; the same-facet caller below,
 * `submitBitcoinMerkleProofWithTx`, still calls `_tryFinalizeProof`
 * directly, same as the monolithic contract always did).
 */
contract iPoWV1ConversionSettlementFacet is iPoWV1Storage, ReentrancyGuard {
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

    /// @notice Cross-facet entry point only — the Admin facet's header relay reaches
    /// this via an external self-call through the router when a header arrives that
    /// unblocks a pending proof.
    function tryFinalizeProof(uint256 txId) external {
        _tryFinalizeProof(txId);
    }

    // ========= INTERNAL HELPERS =========

    /// @dev "Duty fulfilled" — kept identical to (and independently of) the Admin
    /// facet's own copy of this logic, since `internal` functions can't cross a
    /// `delegatecall` boundary. Both read the same shared storage, so they can never
    /// disagree.
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

    function _closeActive(uint256 txId) internal {
        HeaderWindow storage hw = windows[txId];
        if (!hw.started || hw.closed) return;

        hw.closed = true;
        activeOpenConversions -= 1;
    }

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
}
