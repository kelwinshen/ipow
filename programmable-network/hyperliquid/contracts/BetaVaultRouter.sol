// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

import {BetaVaultStorage, IIPoWHeadersView} from "./base/BetaVaultStorage.sol";
import {BetaVaultCoreFacet} from "./facets/BetaVaultCoreFacet.sol";
import {BetaVaultAnchorFacet} from "./facets/BetaVaultAnchorFacet.sol";

/**
 * @title BetaVaultRouter
 * @notice Hyperliquid-only stand-in for plain `BetaVault` (design/ipow-implementation.md
 * §8.17) — same reasoning, and the exact same pattern, as `iPoWRouter`
 * (§8.16): dispatches to `BetaVaultCoreFacet` and `BetaVaultAnchorFacet` via
 * `delegatecall`. From any external caller's perspective this behaves
 * exactly like `BetaVault`. Every state variable lives in the router's own
 * storage; each facet only ever executes against it via `delegatecall`.
 *
 * Deliberately immutable, same as `iPoWRouter`: both facet addresses are
 * set once in the constructor and never change. No admin function exists to
 * swap a facet.
 */
contract BetaVaultRouter is BetaVaultStorage {
    address public immutable coreFacet;
    address public immutable anchorFacet;

    constructor(address _governance, address _ipowHeaders, Params memory _params, address _coreFacet, address _anchorFacet) {
        if (_governance == address(0) || _ipowHeaders == address(0)) revert InvalidParams();
        if (_coreFacet == address(0) || _anchorFacet == address(0)) revert InvalidConstructor();
        _validate(_params);
        governance = _governance;
        ipowHeaders = IIPoWHeadersView(_ipowHeaders);
        params = _params;
        coreFacet = _coreFacet;
        anchorFacet = _anchorFacet;
    }

    /// @dev Plain transfers top up insurance — real logic, not a delegatecall
    /// dispatch: `receive` carries no calldata to dispatch on a selector, so
    /// this is implemented directly against the router's own (inherited)
    /// storage, same as plain `BetaVault`'s `receive`.
    receive() external payable {
        insurance += msg.value;
    }

    fallback() external payable {
        address facet = _facetFor(msg.sig);
        if (facet == address(0)) revert Unauthorized();

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

    /// @dev Explicit selector -> facet mapping, same style as
    /// `iPoWRouter._facetFor` — cheap (immutable comparisons, no SLOAD),
    /// every entry individually visible here for review rather than hidden
    /// behind a constructor-populated mapping.
    function _facetFor(bytes4 selector) internal view returns (address) {
        if (_isCoreSelector(selector)) return coreFacet;
        if (_isAnchorSelector(selector)) return anchorFacet;
        return address(0);
    }

    function _isCoreSelector(bytes4 selector) private pure returns (bool) {
        if (selector == BetaVaultCoreFacet.setParams.selector) return true;
        if (selector == BetaVaultCoreFacet.approveOperator.selector) return true;
        if (selector == BetaVaultCoreFacet.setTokenParams.selector) return true;
        if (selector == BetaVaultCoreFacet.registerParty.selector) return true;
        if (selector == BetaVaultCoreFacet.topUpBond.selector) return true;
        if (selector == BetaVaultCoreFacet.requestUnbond.selector) return true;
        if (selector == BetaVaultCoreFacet.withdrawBond.selector) return true;
        if (selector == BetaVaultCoreFacet.deposit.selector) return true;
        if (selector == BetaVaultCoreFacet.refund.selector) return true;
        if (selector == BetaVaultCoreFacet.executeRelease.selector) return true;
        if (selector == BetaVaultCoreFacet.settleRelease.selector) return true;
        if (selector == BetaVaultCoreFacet.fundRewards.selector) return true;
        return false;
    }

    function _isAnchorSelector(bytes4 selector) private pure returns (bool) {
        if (selector == BetaVaultAnchorFacet.processAnchor.selector) return true;
        if (selector == BetaVaultAnchorFacet.skipAnchor.selector) return true;
        return false;
    }
}
