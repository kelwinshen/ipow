// SPDX-License-Identifier: MIT
pragma solidity ^0.8.28;

/// A test stand-in for a vault's core that has accepted every claim and finds
/// every record carried: lets a test make a receipts part's receipt directly,
/// to read its name.
contract AcceptingCore {
    function claimAccepted(uint256) external pure returns (bool) {
        return true;
    }

    function carries(uint256, bytes32) external pure returns (bool) {
        return true;
    }
}
