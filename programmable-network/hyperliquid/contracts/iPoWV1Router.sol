// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

import {iPoWV1Storage} from "./base/iPoWV1Storage.sol";
import {iPoWV1AdminFacet} from "./facets/iPoWV1AdminFacet.sol";
import {iPoWV1ConversionEntryFacet} from "./facets/iPoWV1ConversionEntryFacet.sol";
import {iPoWV1ConversionSettlementFacet} from "./facets/iPoWV1ConversionSettlementFacet.sol";

/**
 * @title iPoWV1Router
 * @notice Hyperliquid-only stand-in for the monolithic `iPoWV1` (DESIGN_V2.md
 * §8.16): HyperEVM testnet's block gas limit (3,000,000) is below what
 * `iPoWV1` needs to deploy as one contract (~4.79M), so on this network only,
 * its logic is split across three smaller facets (`iPoWV1AdminFacet`,
 * `iPoWV1ConversionEntryFacet`, `iPoWV1ConversionSettlementFacet`) that this
 * router dispatches to via `delegatecall` — every other network keeps the
 * plain, unmodified `iPoWV1.sol`.
 *
 * From any external caller's perspective (including `BetaVault`, which
 * reads `globalTipHeight`/`globalHeightToHashLE`/`globalHeaders` directly —
 * satisfied here by this router's own inherited public getters, no
 * delegatecall involved) this behaves exactly like `iPoWV1`. Every state
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
contract iPoWV1Router is iPoWV1Storage {
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
        if (selector == iPoWV1AdminFacet.setOperator.selector) return true;
        if (selector == iPoWV1AdminFacet.addNetwork.selector) return true;
        if (selector == iPoWV1AdminFacet.removeNetwork.selector) return true;
        if (selector == iPoWV1AdminFacet.setFees.selector) return true;
        if (selector == iPoWV1AdminFacet.addNativeLiquidity.selector) return true;
        if (selector == iPoWV1AdminFacet.removableNative.selector) return true;
        if (selector == iPoWV1AdminFacet.removeNativeLiquidity.selector) return true;
        if (selector == iPoWV1AdminFacet.commitGlobalBitcoinHeader80.selector) return true;
        if (selector == iPoWV1AdminFacet.proofInfo.selector) return true;
        if (selector == iPoWV1AdminFacet.anchorInfo.selector) return true;
        if (selector == iPoWV1AdminFacet.getTxIdsByFilter.selector) return true;
        if (selector == iPoWV1AdminFacet.getConversionWithPhase.selector) return true;
        if (selector == iPoWV1AdminFacet.expectedNext.selector) return true;
        if (selector == iPoWV1AdminFacet.windowsFor.selector) return true;
        if (selector == iPoWV1AdminFacet.debugDecodeHeader.selector) return true;
        return false;
    }

    function _isConversionEntrySelector(bytes4 selector) private pure returns (bool) {
        if (selector == iPoWV1ConversionEntryFacet.commitNativeToBitcoin.selector) return true;
        if (selector == iPoWV1ConversionEntryFacet.commitBitcoinToNative.selector) return true;
        if (selector == iPoWV1ConversionEntryFacet.approveAndStartWithAnchorAndFirst.selector) return true;
        if (selector == iPoWV1ConversionEntryFacet.depositApprovedConversion.selector) return true;
        return false;
    }

    function _isConversionSettlementSelector(bytes4 selector) private pure returns (bool) {
        if (selector == iPoWV1ConversionSettlementFacet.submitBitcoinMerkleProofWithTx.selector) return true;
        if (selector == iPoWV1ConversionSettlementFacet.timeoutNoDeposit_NativetoBitcoin.selector) return true;
        if (selector == iPoWV1ConversionSettlementFacet.refundAfterNoProof_NativeToBitcoin.selector) return true;
        if (selector == iPoWV1ConversionSettlementFacet.closeNoBitcoin_BitcoinToNative.selector) return true;
        if (selector == iPoWV1ConversionSettlementFacet.claimNative_AfterOperatorExpired.selector) return true;
        if (selector == iPoWV1ConversionSettlementFacet.refundIfNotApproved.selector) return true;
        if (selector == iPoWV1ConversionSettlementFacet.tryFinalizeProof.selector) return true;
        return false;
    }
}
