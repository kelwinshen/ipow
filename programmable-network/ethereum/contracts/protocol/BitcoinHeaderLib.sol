// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

import {Math} from "@openzeppelin/contracts/utils/math/Math.sol";

/// @title BitcoinHeaderLib
/// @notice Stateless Bitcoin math for the iPoW light client: header fields, the
/// compact difficulty encoding, the difficulty change at an epoch boundary, and
/// Merkle proofs.
/// @dev Hashes are kept in the byte order Bitcoin uses inside a header (the raw
/// double SHA-256 output). `uintFromLE` turns one into a number.
library BitcoinHeaderLib {
    error InvalidHeaderLength();
    error InvalidBits();

    uint256 internal constant HEADER_LENGTH = 80;
    /// @dev Bitcoin changes its difficulty every 2,016 blocks.
    uint256 internal constant EPOCH_BLOCKS = 2016;
    /// @dev The time Bitcoin aims for per epoch.
    uint256 internal constant TARGET_TIMESPAN = 14 days;

    function hashLE(bytes calldata header80) internal pure returns (bytes32) {
        if (header80.length != HEADER_LENGTH) revert InvalidHeaderLength();
        return sha256(abi.encodePacked(sha256(header80)));
    }

    function prevLE(bytes calldata header80) internal pure returns (bytes32 raw) {
        assembly ("memory-safe") {
            raw := calldataload(add(header80.offset, 4))
        }
    }

    function merkleRootLE(bytes calldata header80) internal pure returns (bytes32 raw) {
        assembly ("memory-safe") {
            raw := calldataload(add(header80.offset, 36))
        }
    }

    function time(bytes calldata header80) internal pure returns (uint32) {
        return _readLE4(header80, 68);
    }

    function bits(bytes calldata header80) internal pure returns (uint32) {
        return _readLE4(header80, 72);
    }

    function _readLE4(bytes calldata b, uint256 o) private pure returns (uint32) {
        return
            uint32(uint8(b[o])) |
            (uint32(uint8(b[o + 1])) << 8) |
            (uint32(uint8(b[o + 2])) << 16) |
            (uint32(uint8(b[o + 3])) << 24);
    }

    /// @dev Byte-swaps a hash so it can be compared with a target as a number.
    function uintFromLE(bytes32 le) internal pure returns (uint256 v) {
        v = uint256(le);
        v = ((v >> 8)  & 0x00FF00FF00FF00FF00FF00FF00FF00FF00FF00FF00FF00FF00FF00FF00FF00FF) |
            ((v & 0x00FF00FF00FF00FF00FF00FF00FF00FF00FF00FF00FF00FF00FF00FF00FF00FF) << 8);
        v = ((v >> 16) & 0x0000FFFF0000FFFF0000FFFF0000FFFF0000FFFF0000FFFF0000FFFF0000FFFF) |
            ((v & 0x0000FFFF0000FFFF0000FFFF0000FFFF0000FFFF0000FFFF0000FFFF0000FFFF) << 16);
        v = ((v >> 32) & 0x00000000FFFFFFFF00000000FFFFFFFF00000000FFFFFFFF00000000FFFFFFFF) |
            ((v & 0x00000000FFFFFFFF00000000FFFFFFFF00000000FFFFFFFF00000000FFFFFFFF) << 32);
        v = ((v >> 64) & 0x0000000000000000FFFFFFFFFFFFFFFF0000000000000000FFFFFFFFFFFFFFFF) |
            ((v & 0x0000000000000000FFFFFFFFFFFFFFFF0000000000000000FFFFFFFFFFFFFFFF) << 64);
        v = (v >> 128) | (v << 128);
    }

    /// @dev Compact "nBits" -> 256-bit target. Rejects what Bitcoin rejects: a
    /// negative value, zero, and a value that does not fit 256 bits.
    function targetFromBits(uint32 nBits) internal pure returns (uint256 target) {
        uint256 exp = nBits >> 24;
        uint256 mant = nBits & 0x007FFFFF;

        if (nBits & 0x00800000 != 0) revert InvalidBits();
        if (mant == 0) revert InvalidBits();
        if (exp > 34 || (mant > 0xFF && exp > 33) || (mant > 0xFFFF && exp > 32)) revert InvalidBits();

        if (exp <= 3) {
            target = mant >> (8 * (3 - exp));
        } else {
            target = mant << (8 * (exp - 3));
        }
        if (target == 0) revert InvalidBits();
    }

    /// @dev 256-bit target -> compact "nBits", the way Bitcoin's GetCompact does it.
    function bitsFromTarget(uint256 target) internal pure returns (uint32) {
        uint256 size = 0;
        uint256 t = target;
        while (t > 0) {
            t >>= 8;
            size++;
        }

        uint256 compact;
        if (size <= 3) {
            compact = target << (8 * (3 - size));
        } else {
            compact = target >> (8 * (size - 3));
        }
        // The top bit of the mantissa is a sign bit, so a mantissa that uses it
        // moves one byte down.
        if (compact & 0x00800000 != 0) {
            compact >>= 8;
            size++;
        }
        return uint32(compact | (size << 24));
    }

    /// @notice The difficulty of the first block of a new epoch, by Bitcoin's rule.
    /// @param lastBits The difficulty of the epoch that ends.
    /// @param firstTime The time of the first block of the epoch that ends.
    /// @param lastTime The time of the last block of the epoch that ends.
    /// @param powLimit The easiest target the network allows.
    function retarget(
        uint32 lastBits,
        uint256 firstTime,
        uint256 lastTime,
        uint256 powLimit
    ) internal pure returns (uint32) {
        uint256 timespan = lastTime > firstTime ? lastTime - firstTime : 0;
        if (timespan < TARGET_TIMESPAN / 4) timespan = TARGET_TIMESPAN / 4;
        if (timespan > TARGET_TIMESPAN * 4) timespan = TARGET_TIMESPAN * 4;

        uint256 next = Math.mulDiv(targetFromBits(lastBits), timespan, TARGET_TIMESPAN);
        if (next > powLimit) next = powLimit;
        return bitsFromTarget(next);
    }

    /// @dev The mining work one block at this target stands for: 2^256 / (target + 1).
    function work(uint256 target) internal pure returns (uint256) {
        return (~target / (target + 1)) + 1;
    }

    function hash256Pair(bytes32 a, bytes32 b) internal pure returns (bytes32) {
        return sha256(abi.encodePacked(sha256(abi.encodePacked(a, b))));
    }

    /// @dev Rebuilds the Merkle root from a leaf and its siblings.
    function merkleRootFrom(
        bytes32 leafLE,
        bytes32[] calldata siblingsLE,
        uint256 index
    ) internal pure returns (bytes32 h) {
        h = leafLE;
        for (uint256 i = 0; i < siblingsLE.length; i++) {
            if (index % 2 == 0) {
                h = hash256Pair(h, siblingsLE[i]);
            } else {
                h = hash256Pair(siblingsLE[i], h);
            }
            index /= 2;
        }
    }
}
