// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

/// @title BitcoinTxLib
/// @notice Reads what the iPoW protocol needs from a Bitcoin transaction: the
/// coin one input spends, whether one output is a coin that can be spent, and
/// the 32 bytes of one `OP_RETURN` output.
/// @dev The transaction is in the serialization without witness data, the one
/// whose double SHA-256 is the txid.
library BitcoinTxLib {
    error MalformedTx();
    error WitnessSerialization();

    struct View {
        /// The coin that input `inputIndex` spends.
        bytes32 spentTxid;
        uint32 spentVout;
        /// Whether output `coinIndex` exists and is not an `OP_RETURN`.
        bool hasCoin;
        /// Whether output `tagIndex` is `OP_RETURN` followed by 32 bytes.
        bool hasTag;
        bytes32 tag;
    }

    function txid(bytes calldata raw) internal pure returns (bytes32) {
        return sha256(abi.encodePacked(sha256(raw)));
    }

    /// @notice Reads one input and two outputs. The whole transaction must
    /// parse, to the last byte.
    function read(
        bytes calldata raw,
        uint256 inputIndex,
        uint256 coinIndex,
        uint256 tagIndex
    ) internal pure returns (View memory v) {
        if (raw.length < 10) revert MalformedTx();
        uint256 o = 4;
        if (raw[o] == 0x00 && raw[o + 1] == 0x01) revert WitnessSerialization();

        uint256 inputs;
        (inputs, o) = _varInt(raw, o);
        if (inputs == 0 || inputIndex >= inputs) revert MalformedTx();
        for (uint256 i = 0; i < inputs; i++) {
            if (o + 36 > raw.length) revert MalformedTx();
            if (i == inputIndex) {
                v.spentTxid = bytes32(raw[o:o + 32]);
                v.spentVout = _le4(raw, o + 32);
            }
            o += 36;
            uint256 scriptLength;
            (scriptLength, o) = _varInt(raw, o);
            o += scriptLength + 4;
            if (o > raw.length) revert MalformedTx();
        }

        uint256 outputs;
        (outputs, o) = _varInt(raw, o);
        if (outputs == 0) revert MalformedTx();
        for (uint256 j = 0; j < outputs; j++) {
            if (o + 8 > raw.length) revert MalformedTx();
            o += 8;
            uint256 scriptLength;
            (scriptLength, o) = _varInt(raw, o);
            if (o + scriptLength > raw.length) revert MalformedTx();

            if (j == coinIndex) {
                v.hasCoin = scriptLength > 0 && raw[o] != 0x6a;
            }
            if (j == tagIndex && scriptLength == 34 && raw[o] == 0x6a && raw[o + 1] == 0x20) {
                v.hasTag = true;
                v.tag = bytes32(raw[o + 2:o + 34]);
            }
            o += scriptLength;
        }

        // The lock time, and nothing after it.
        if (o + 4 != raw.length) revert MalformedTx();
    }

    function _le4(bytes calldata b, uint256 o) private pure returns (uint32) {
        return
            uint32(uint8(b[o])) |
            (uint32(uint8(b[o + 1])) << 8) |
            (uint32(uint8(b[o + 2])) << 16) |
            (uint32(uint8(b[o + 3])) << 24);
    }

    /// @dev Bitcoin's variable length integer.
    function _varInt(bytes calldata b, uint256 o) private pure returns (uint256 v, uint256 next) {
        if (o >= b.length) revert MalformedTx();
        uint8 first = uint8(b[o]);
        if (first < 0xFD) return (first, o + 1);

        uint256 size = first == 0xFD ? 2 : first == 0xFE ? 4 : 8;
        if (o + 1 + size > b.length) revert MalformedTx();
        for (uint256 i = 0; i < size; i++) {
            v |= uint256(uint8(b[o + 1 + i])) << (8 * i);
        }
        // Bitcoin accepts only the shortest way to write a number.
        uint256 least = size == 2 ? 0xFD : size == 4 ? 0x10000 : 0x100000000;
        if (v < least) revert MalformedTx();
        return (v, o + 1 + size);
    }
}
