// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

import {iPoWTypes} from "../base/iPoWTypes.sol";

/// @title BitcoinPrimitives
/// @notice Stateless Bitcoin-consensus math and parsing used by iPoW: header field
/// extraction, PoW/difficulty math, Merkle proof verification, and raw tx parsing.
/// @dev All functions are `internal`, so this gets inlined into the caller's bytecode
/// rather than deployed separately — `iPoW` stays a single deployed contract.
library BitcoinPrimitives {
    function _readCompact(bytes calldata header80) internal pure returns (uint32 nBits) {
        nBits = uint32(uint8(header80[72])) |
                (uint32(uint8(header80[73])) << 8) |
                (uint32(uint8(header80[74])) << 16) |
                (uint32(uint8(header80[75])) << 24);
    }

    function _packBranchStorage(bytes32[] storage branch) internal view returns (bytes memory out) {
        out = new bytes(branch.length * 32);
        for (uint256 i = 0; i < branch.length; i++) {
            bytes32 v = branch[i];
            assembly ("memory-safe") {
                mstore(add(add(out, 32), mul(i, 32)), v)
            }
        }
    }

    function _hash256Pair(bytes32 a, bytes32 b) internal pure returns (bytes32) {
        return sha256(abi.encodePacked(sha256(abi.encodePacked(a, b))));
    }

    /// @dev Rebuilds the Merkle root from a leaf and its sibling path and compares it
    /// to the claimed root.
    function _proveMerkleLE(
        bytes32 leafLE,
        bytes32 rootLE,
        bytes memory siblingsLE,
        uint256 index
    ) internal pure returns (bool) {
        if (siblingsLE.length % 32 != 0) return false;

        bytes32 h = leafLE;
        unchecked {
            for (uint256 off = 0; off < siblingsLE.length; off += 32) {
                bytes32 sib;
                assembly ("memory-safe") {
                    sib := mload(add(add(siblingsLE, 32), off))
                }

                if (index % 2 == 0) {
                    h = _hash256Pair(h, sib);
                } else {
                    h = _hash256Pair(sib, h);
                }
                index /= 2;
            }
        }
        return h == rootLE;
    }

    /// @dev Byte-swaps Little-Endian to Big-Endian, needed to compare a Bitcoin hash
    /// against a difficulty target numerically.
    function _uintFromLE(bytes32 le) internal pure returns (uint256 v) {
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

    function _validateWorkLE(bytes32 hashLE, uint256 target) internal pure returns (bool) {
        if (hashLE == bytes32(0)) return false;
        return _uintFromLE(hashLE) <= target;
    }

    function _extractTimestamp(bytes calldata header80) internal pure returns (uint32 ts) {
        ts = uint32(uint8(header80[68])) |
             (uint32(uint8(header80[69])) << 8) |
             (uint32(uint8(header80[70])) << 16) |
             (uint32(uint8(header80[71])) << 24);
    }

    /// @dev Bitcoin's compact "nBits" difficulty encoding -> full 256-bit target.
    function _targetFromBits(uint32 bits) internal pure returns (uint256 target) {
        uint256 exp = bits >> 24;
        uint256 mant = bits & 0x007FFFFF;

        if (exp <= 3) {
            target = mant >> (8 * (3 - exp));
        } else {
            target = mant << (8 * (exp - 3));
        }

        uint256 limit = _powLimit();
        if (target > limit) target = limit;
    }

    function _powLimit() internal pure returns (uint256) {
        return uint256(0xFFFF) << (8 * (0x1D - 3));
    }

    /// @dev Parses a raw Bitcoin tx just far enough to pull one output's value and
    /// script. SegWit's marker/flag bytes (if present) are skipped so the input count
    /// that follows isn't misread — that's the only segwit-specific handling needed,
    /// since witness data lives after the outputs section this function reads.
    function _parseOutputAt(bytes memory txRaw, uint256 voutIndex)
        internal
        pure
        returns (uint64 valueSats, bytes memory program)
    {
        uint256 o = 0;
        if(txRaw.length < 4) revert iPoWTypes.TransactionTooShort();

        o += 4;

        bool isSegwit = false;
        if (txRaw.length >= o + 2 && txRaw[o] == 0x00 && txRaw[o + 1] == 0x01) {
            isSegwit = true;
            o += 2;
        }

        (uint256 inCount, uint256 s1) = _readVarInt(txRaw, o);
        o = s1;

        for (uint256 i = 0; i < inCount; i++) {
            // Outpoint (36 bytes) + script length (varint) + script + sequence (4 bytes).
            o += 36;
            (uint256 slen, uint256 s2) = _readVarInt(txRaw, o);
            o = s2 + slen + 4;
            if (o > txRaw.length) revert iPoWTypes.TransactionOverflow();
        }

        (uint256 outCount, uint256 s3) = _readVarInt(txRaw, o);
        o = s3;

        if (voutIndex >= outCount) revert iPoWTypes.VoutOutOfBounds();

        for (uint256 j = 0; j < outCount; j++) {
            if (o + 8 > txRaw.length) revert iPoWTypes.ValueOutOfBounds();

            if (j == voutIndex) {
                valueSats = _readLE8(txRaw, o);
            }
            o += 8;

            (uint256 progLen, uint256 s4) = _readVarInt(txRaw, o);
            o = s4;

            if (o + progLen > txRaw.length) revert iPoWTypes.ProgramOutOfBounds();

            if (j == voutIndex) {
                program = new bytes(progLen);
                for (uint256 k = 0; k < progLen; k++) {
                    program[k] = txRaw[o + k];
                }
                return (valueSats, program);
            }
            o += progLen;
        }

        isSegwit;
    }

    /// @dev Bitcoin's VarInt/CompactSize encoding.
    function _readVarInt(bytes memory b, uint256 o)
        internal
        pure
        returns (uint256 v, uint256 next)
    {
        if (o >= b.length) revert iPoWTypes.VarIntOutOfBounds();

        uint8 p = uint8(b[o]);
        if (p < 0xFD) {
            v = p;
            next = o + 1;
        } else if (p == 0xFD) {
            if (o + 3 > b.length) revert iPoWTypes.Var16OutOfBounds();
            v = uint16(uint8(b[o + 1])) | (uint16(uint8(b[o + 2])) << 8);
            next = o + 3;
        } else if (p == 0xFE) {
            if (o + 5 > b.length) revert iPoWTypes.Var32OutOfBounds();
            v = uint32(uint8(b[o + 1])) |
                (uint32(uint8(b[o + 2])) << 8) |
                (uint32(uint8(b[o + 3])) << 16) |
                (uint32(uint8(b[o + 4])) << 24);
            next = o + 5;
        } else {
            if (o + 9 > b.length) revert iPoWTypes.Var64OutOfBounds();
            v = _readLE8(b, o + 1);
            next = o + 9;
        }
    }

    function _readLE8(bytes memory b, uint256 o) internal pure returns (uint64 v) {
        if (o + 8 > b.length) revert iPoWTypes.LE8OutOfBounds();
        v = uint64(uint8(b[o])) |
            (uint64(uint8(b[o + 1])) << 8) |
            (uint64(uint8(b[o + 2])) << 16) |
            (uint64(uint8(b[o + 3])) << 24) |
            (uint64(uint8(b[o + 4])) << 32) |
            (uint64(uint8(b[o + 5])) << 40) |
            (uint64(uint8(b[o + 6])) << 48) |
            (uint64(uint8(b[o + 7])) << 56);
    }

    function _hashHeaderLE(bytes calldata header80) internal pure returns (bytes32 outer) {
        if(header80.length != 80) revert iPoWTypes.InvalidHeader();
        bytes memory tmp = new bytes(80);
        assembly ("memory-safe") {
            calldatacopy(add(tmp, 32), header80.offset, 80)
        }
        bytes32 inner = sha256(tmp);
        outer = sha256(abi.encodePacked(inner));
    }

    /// @dev The inverse of _targetFromBits: full 256-bit target -> compact "nBits".
    function _bitsFromTarget(uint256 target) internal pure returns (uint32 bits) {
        if (target == 0) return 0;

        uint256 exp = 0;
        uint256 t = target;
        while (t > 0) {
            t >>= 8;
            exp++;
        }

        uint256 mant;
        if (exp <= 3) {
            mant = target << (8 * (3 - exp));
            exp = 3;
        } else {
            uint256 shift = 8 * (exp - 3);
            mant = target >> shift;
        }

        mant &= 0x00FFFFFF;
        bits = uint32((exp << 24) | mant);
    }

    function _extractPrevLE(bytes calldata header80) internal pure returns (bytes32 raw) {
        assembly ("memory-safe") {
            raw := calldataload(add(header80.offset, 4))
        }
        return raw;
    }

    function _extractMerkleLE(bytes calldata header80) internal pure returns (bytes32 raw) {
        assembly ("memory-safe") {
            raw := calldataload(add(header80.offset, 36))
        }
        return raw;
    }

    function _extractTarget(bytes calldata header80) internal pure returns (uint256) {
        uint32 bits = _readCompact(header80);
        return _targetFromBits(bits);
    }
}
