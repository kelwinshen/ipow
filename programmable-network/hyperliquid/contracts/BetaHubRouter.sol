// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

import {BetaHubStorage} from "./base/BetaHubStorage.sol";
import {BetaHubGovernanceFacet} from "./facets/BetaHubGovernanceFacet.sol";
import {BetaHubAnchorFacet} from "./facets/BetaHubAnchorFacet.sol";
import {BetaHubMintFacet} from "./facets/BetaHubMintFacet.sol";
import {IIPoWV1HeadersView} from "./libraries/AnchorChainLib.sol";
import {HubToken} from "./HubToken.sol";

/**
 * @title BetaHubRouter
 * @notice Hyperliquid-only stand-in for plain `BetaHub` (DESIGN_V2.md
 * §8.19) — same reasoning, and the exact same pattern, as
 * `iPoWV1Router`/`BetaVaultRouter` (§8.16/§8.17): dispatches to
 * `BetaHubGovernanceFacet`, `BetaHubAnchorFacet`, and `BetaHubMintFacet`
 * via `delegatecall`. From any external caller's perspective this behaves
 * exactly like `BetaHub`. Every state variable lives in the router's own
 * storage; each facet only ever executes against it via `delegatecall`.
 * Read-only accessors (`getComposition`, `compositionExists`,
 * `pendingRemoteLockId`, `pendingRemoteAnchorTxid`) need no dispatch at
 * all — they're inherited directly from `BetaHubStorage` as the router's
 * own public getters, same as `iPoWV1Router`'s header-relay reads.
 *
 * Deliberately immutable, same as the other two routers: all three facet
 * addresses are set once in the constructor and never change.
 */
contract BetaHubRouter is BetaHubStorage {
    address public immutable governanceFacet;
    address public immutable anchorFacet;
    address public immutable mintFacet;

    constructor(
        address _governance,
        address _ipowHeadersAddr,
        uint256 _selfNetworkId,
        Params memory _params,
        string memory tokenName,
        string memory tokenSymbol,
        address _governanceFacet,
        address _anchorFacet,
        address _mintFacet
    ) {
        if (_governance == address(0) || _ipowHeadersAddr == address(0)) revert InvalidParams();
        if (_governanceFacet == address(0) || _anchorFacet == address(0) || _mintFacet == address(0)) revert InvalidConstructor();
        _validateParams(_params);
        governance = _governance;
        ipowHeaders = IIPoWV1HeadersView(_ipowHeadersAddr);
        SELF_NETWORK_ID = _selfNetworkId;
        params = _params;
        governanceFacet = _governanceFacet;
        anchorFacet = _anchorFacet;
        mintFacet = _mintFacet;
        token = new HubToken(tokenName, tokenSymbol, address(this));
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

    function _facetFor(bytes4 selector) internal view returns (address) {
        if (_isGovernanceSelector(selector)) return governanceFacet;
        if (_isAnchorSelector(selector)) return anchorFacet;
        if (_isMintSelector(selector)) return mintFacet;
        return address(0);
    }

    function _isGovernanceSelector(bytes4 selector) private pure returns (bool) {
        if (selector == BetaHubGovernanceFacet.setParams.selector) return true;
        if (selector == BetaHubGovernanceFacet.fundRewards.selector) return true;
        if (selector == BetaHubGovernanceFacet.approveOperator.selector) return true;
        if (selector == BetaHubGovernanceFacet.registerParty.selector) return true;
        if (selector == BetaHubGovernanceFacet.topUpBond.selector) return true;
        if (selector == BetaHubGovernanceFacet.requestUnbond.selector) return true;
        if (selector == BetaHubGovernanceFacet.withdrawBond.selector) return true;
        if (selector == BetaHubGovernanceFacet.registerComposition.selector) return true;
        return false;
    }

    function _isAnchorSelector(bytes4 selector) private pure returns (bool) {
        if (selector == BetaHubAnchorFacet.processAnchor.selector) return true;
        if (selector == BetaHubAnchorFacet.skipAnchor.selector) return true;
        return false;
    }

    function _isMintSelector(bytes4 selector) private pure returns (bool) {
        if (selector == BetaHubMintFacet.lockLocal.selector) return true;
        if (selector == BetaHubMintFacet.approvePending.selector) return true;
        if (selector == BetaHubMintFacet.exerciseMint.selector) return true;
        if (selector == BetaHubMintFacet.settleMint.selector) return true;
        if (selector == BetaHubMintFacet.expirePending.selector) return true;
        return false;
    }
}
