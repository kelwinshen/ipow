// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

import {BitcoinPrimitives} from "../libraries/BitcoinPrimitives.sol";

/**
 * @title BitcoinPrimitivesHarness
 * @notice Test-only contract exposing BitcoinPrimitives' internal functions as external
 * calls, so the library's pure Bitcoin-consensus math can be unit tested in isolation
 * from iPoW's state machine. Not part of the deployed protocol.
 */
contract BitcoinPrimitivesHarness {
    bytes32[] private _branchScratch;

    function readCompact(bytes calldata header80) external pure returns (uint32) {
        return BitcoinPrimitives._readCompact(header80);
    }

    function setBranch(bytes32[] calldata branch) external {
        delete _branchScratch;
        for (uint256 i = 0; i < branch.length; i++) {
            _branchScratch.push(branch[i]);
        }
    }

    function packBranch() external view returns (bytes memory) {
        return BitcoinPrimitives._packBranchStorage(_branchScratch);
    }

    function hash256Pair(bytes32 a, bytes32 b) external pure returns (bytes32) {
        return BitcoinPrimitives._hash256Pair(a, b);
    }

    function proveMerkleLE(
        bytes32 leafLE,
        bytes32 rootLE,
        bytes calldata siblingsLE,
        uint256 index
    ) external pure returns (bool) {
        return BitcoinPrimitives._proveMerkleLE(leafLE, rootLE, siblingsLE, index);
    }

    function uintFromLE(bytes32 le) external pure returns (uint256) {
        return BitcoinPrimitives._uintFromLE(le);
    }

    function validateWorkLE(bytes32 hashLE, uint256 target) external pure returns (bool) {
        return BitcoinPrimitives._validateWorkLE(hashLE, target);
    }

    function extractTimestamp(bytes calldata header80) external pure returns (uint32) {
        return BitcoinPrimitives._extractTimestamp(header80);
    }

    function targetFromBits(uint32 bits) external pure returns (uint256) {
        return BitcoinPrimitives._targetFromBits(bits);
    }

    function powLimit() external pure returns (uint256) {
        return BitcoinPrimitives._powLimit();
    }

    function parseOutputAt(bytes calldata txRaw, uint256 voutIndex)
        external
        pure
        returns (uint64 valueSats, bytes memory program)
    {
        return BitcoinPrimitives._parseOutputAt(txRaw, voutIndex);
    }

    function readVarInt(bytes calldata b, uint256 o) external pure returns (uint256 v, uint256 next) {
        return BitcoinPrimitives._readVarInt(b, o);
    }

    function readLE8(bytes calldata b, uint256 o) external pure returns (uint64) {
        return BitcoinPrimitives._readLE8(b, o);
    }

    function hashHeaderLE(bytes calldata header80) external pure returns (bytes32) {
        return BitcoinPrimitives._hashHeaderLE(header80);
    }

    function bitsFromTarget(uint256 target) external pure returns (uint32) {
        return BitcoinPrimitives._bitsFromTarget(target);
    }

    function extractPrevLE(bytes calldata header80) external pure returns (bytes32) {
        return BitcoinPrimitives._extractPrevLE(header80);
    }

    function extractMerkleLE(bytes calldata header80) external pure returns (bytes32) {
        return BitcoinPrimitives._extractMerkleLE(header80);
    }

    function extractTarget(bytes calldata header80) external pure returns (uint256) {
        return BitcoinPrimitives._extractTarget(header80);
    }
}
