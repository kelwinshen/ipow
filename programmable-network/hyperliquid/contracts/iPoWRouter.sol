// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

import {iPoWStorage} from "./base/iPoWStorage.sol";
import {iPoWAdminFacet} from "./facets/iPoWAdminFacet.sol";
import {iPoWConversionEntryFacet} from "./facets/iPoWConversionEntryFacet.sol";
import {iPoWConversionSettlementFacet} from "./facets/iPoWConversionSettlementFacet.sol";

/**
 * @title iPoWRouter
 * @notice Hyperliquid-only stand-in for the monolithic `iPoW` (design/ipow-implementation.md
 * §8.16): HyperEVM testnet's block gas limit (3,000,000) is below what
 * `iPoW` needs to deploy as one contract (~4.79M), so on this network only,
 * its logic is split across three smaller facets (`iPoWAdminFacet`,
 * `iPoWConversionEntryFacet`, `iPoWConversionSettlementFacet`) that this
 * router dispatches to via `delegatecall` — every other network keeps the
 * plain, unmodified `iPoW.sol`.
 *
 * From any external caller's perspective (including `BetaVault`, which
 * reads `globalTipHeight`/`globalHeightToHashLE`/`globalHeaders` directly —
 * satisfied here by this router's own inherited public getters, no
 * delegatecall involved) this behaves exactly like `iPoW`. Every state
 * variable lives in the router's own storage; each facet only ever executes
 * against it via `delegatecall` (never called directly with any effect —
 * see each facet's own contract-level comment).
 *
 * Deliberately immutable: all three facet addresses are set once in the
 * constructor and never change. There is no admin function to swap a
 * facet, unlike a general-purpose (EIP-2535) diamond — the single new risk
 * that pattern is usually criticized for (a compromised or malicious
 * upgrade swapping in bad logic later) doesn't exist here at all.
 */
contract iPoWRouter is iPoWStorage {
    address public immutable adminFacet;
    address public immutable conversionEntryFacet;
    address public immutable conversionSettlementFacet;

    constructor(
        uint256 _nativeDecimals,
        uint256 _selfNetworkId,
        address _operator,
        uint256 _commitFeeBps,
        address _adminFacet,
        address _conversionEntryFacet,
        address _conversionSettlementFacet
    ) {
        if (_selfNetworkId == 0 || _operator == address(0) || _commitFeeBps > BPS_DENOM) revert InvalidConstructor();
        if (_adminFacet == address(0) || _conversionEntryFacet == address(0) || _conversionSettlementFacet == address(0)) {
            revert InvalidConstructor();
        }

        NATIVE_DECIMALS = _nativeDecimals;
        SELF_NETWORK_ID = _selfNetworkId;
        operator = _operator;
        commitFeeBps = _commitFeeBps;
        adminFacet = _adminFacet;
        conversionEntryFacet = _conversionEntryFacet;
        conversionSettlementFacet = _conversionSettlementFacet;
    }

    receive() external payable {}

    fallback() external payable {
        address facet = _facetFor(msg.sig);
        if (facet == address(0)) revert Unauthorized();

        // Standard EIP-2535-style dispatch: delegatecall into the resolved facet
        // with the original calldata, then relay its return data or revert
        // reason back to the original caller unchanged.
        (bool ok, bytes memory ret) = facet.delegatecall(msg.data);
        if (!ok) {
            assembly ("memory-safe") {
                revert(add(ret, 0x20), mload(ret))
            }
        }
        assembly ("memory-safe") {
            return(add(ret, 0x20), mload(ret))
        }
    }

    /// @dev Explicit selector -> facet mapping, checked one at a time rather than via
    /// a storage-backed table — cheaper (immutable comparisons, no SLOAD) and every
    /// entry is individually visible here for review rather than hidden behind a
    /// constructor-populated mapping. Split into one helper per facet purely to keep
    /// each function's IR shallow enough for the optimizer — a single function with
    /// every selector comparison inline hit a "stack too deep" compile error even
    /// with `viaIR`.
    function _facetFor(bytes4 selector) internal view returns (address) {
        if (_isAdminSelector(selector)) return adminFacet;
        if (_isConversionEntrySelector(selector)) return conversionEntryFacet;
        if (_isConversionSettlementSelector(selector)) return conversionSettlementFacet;
        return address(0);
    }

    function _isAdminSelector(bytes4 selector) private pure returns (bool) {
        if (selector == iPoWAdminFacet.setOperator.selector) return true;
        if (selector == iPoWAdminFacet.addNetwork.selector) return true;
        if (selector == iPoWAdminFacet.removeNetwork.selector) return true;
        if (selector == iPoWAdminFacet.setFees.selector) return true;
        if (selector == iPoWAdminFacet.addNativeLiquidity.selector) return true;
        if (selector == iPoWAdminFacet.removableNative.selector) return true;
        if (selector == iPoWAdminFacet.removeNativeLiquidity.selector) return true;
        if (selector == iPoWAdminFacet.commitGlobalBitcoinHeader80.selector) return true;
        if (selector == iPoWAdminFacet.proofInfo.selector) return true;
        if (selector == iPoWAdminFacet.anchorInfo.selector) return true;
        if (selector == iPoWAdminFacet.getTxIdsByFilter.selector) return true;
        if (selector == iPoWAdminFacet.getConversionWithPhase.selector) return true;
        if (selector == iPoWAdminFacet.expectedNext.selector) return true;
        if (selector == iPoWAdminFacet.windowsFor.selector) return true;
        if (selector == iPoWAdminFacet.debugDecodeHeader.selector) return true;
        return false;
    }

    function _isConversionEntrySelector(bytes4 selector) private pure returns (bool) {
        if (selector == iPoWConversionEntryFacet.commitNativeToBitcoin.selector) return true;
        if (selector == iPoWConversionEntryFacet.commitBitcoinToNative.selector) return true;
        if (selector == iPoWConversionEntryFacet.approveAndStartWithAnchorAndFirst.selector) return true;
        if (selector == iPoWConversionEntryFacet.depositApprovedConversion.selector) return true;
        return false;
    }

    function _isConversionSettlementSelector(bytes4 selector) private pure returns (bool) {
        if (selector == iPoWConversionSettlementFacet.submitBitcoinMerkleProofWithTx.selector) return true;
        if (selector == iPoWConversionSettlementFacet.timeoutNoDeposit_NativetoBitcoin.selector) return true;
        if (selector == iPoWConversionSettlementFacet.refundAfterNoProof_NativeToBitcoin.selector) return true;
        if (selector == iPoWConversionSettlementFacet.closeNoBitcoin_BitcoinToNative.selector) return true;
        if (selector == iPoWConversionSettlementFacet.claimNative_AfterOperatorExpired.selector) return true;
        if (selector == iPoWConversionSettlementFacet.refundIfNotApproved.selector) return true;
        if (selector == iPoWConversionSettlementFacet.tryFinalizeProof.selector) return true;
        return false;
    }
}
