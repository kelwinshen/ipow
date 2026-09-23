// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

import {ReentrancyGuard} from "@openzeppelin/contracts/utils/ReentrancyGuard.sol";
import {iPoWV1Storage} from "../base/iPoWV1Storage.sol";

/**
 * @title iPoWV1ConversionEntryFacet
 * @notice Hyperliquid-only split of `iPoWV1` (see `../iPoWV1Router.sol` and
 * docs/DESIGN_V2.md §8.16): the commit/approve/deposit half of the
 * conversion lifecycle. Split out of what was originally one
 * `iPoWV1ConversionFacet` because that single facet's deployed bytecode
 * (~14.8KB) still didn't leave enough margin under HyperEVM testnet's
 * 3,000,000 block gas limit. This half and `iPoWV1ConversionSettlementFacet`
 * need none of the same internal helpers, so the split needed zero
 * duplication. Never called directly — only ever reached via the router's
 * `delegatecall`, so it operates on the router's storage the whole time.
 * Bit-for-bit the same logic as the monolithic `iPoWV1.sol`'s equivalent
 * functions; only the file they live in changed.
 */
contract iPoWV1ConversionEntryFacet is iPoWV1Storage, ReentrancyGuard {
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

    // ========= INTERNAL HELPERS =========

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

    function _availableForReserve() internal view returns (uint256) {
        // Same tiny computation as the Admin facet's `removableNative` — duplicated
        // rather than cross-facet-called, since `internal` can't cross a
        // `delegatecall` boundary and this one is small enough that a cross-facet
        // external call would cost more gas than it saves.
        uint256 bal = address(this).balance;
        uint256 unavailable = totalLockedDeposits + totalReservedNative + totalHeldCommitFees;
        if (bal <= unavailable) return 0;
        uint256 byBalance = bal - unavailable;
        return nativeLiquidity < byBalance ? nativeLiquidity : byBalance;
    }
}
