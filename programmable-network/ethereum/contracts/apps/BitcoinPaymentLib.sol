// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

/// @title BitcoinPaymentLib
/// @notice Reads the payments of a Bitcoin transaction for an application:
/// whether an output pays a script at least an amount, and whether an input
/// spends a coin.
/// @dev The transaction is in the serialization without witness data, the one
/// whose double SHA-256 is the txid. The whole transaction must parse, to the
/// last byte.
library BitcoinPaymentLib {
    error MalformedTx();
    error WitnessSerialization();

    /// @notice Whether any output pays exactly `script` at least `sats`.
    function paysAtLeast(bytes calldata raw, bytes memory script, uint64 sats) internal pure returns (bool found) {
        uint256 o = _outputsStart(raw);
        uint256 outputs;
        (outputs, o) = _varInt(raw, o);
        for (uint256 j = 0; j < outputs; j++) {
            bool pays;
            (pays, o) = _output(raw, o, script, sats);
            found = found || pays;
        }
        _end(raw, o);
    }

    /// @notice Whether output `vout` pays exactly `script` at least `sats`.
    function outputPays(bytes calldata raw, uint256 vout, bytes memory script, uint64 sats) internal pure returns (bool found) {
        uint256 o = _outputsStart(raw);
        uint256 outputs;
        (outputs, o) = _varInt(raw, o);
        if (vout >= outputs) return false;
        for (uint256 j = 0; j < outputs; j++) {
            bool pays;
            (pays, o) = _output(raw, o, script, sats);
            if (j == vout) found = pays;
        }
        _end(raw, o);
    }

    /// @notice Whether an input spends output `vout` of transaction `txid`.
    function spends(bytes calldata raw, bytes32 txid, uint32 vout) internal pure returns (bool found) {
        uint256 o = _start(raw);
        uint256 inputs;
        (inputs, o) = _varInt(raw, o);
        for (uint256 i = 0; i < inputs; i++) {
            if (o + 36 > raw.length) revert MalformedTx();
            if (bytes32(raw[o:o + 32]) == txid && _le4(raw, o + 32) == vout) found = true;
            o += 36;
            uint256 len;
            (len, o) = _varInt(raw, o);
            o += len + 4;
            if (o > raw.length) revert MalformedTx();
        }
        uint256 outputs;
        (outputs, o) = _varInt(raw, o);
        for (uint256 j = 0; j < outputs; j++) {
            (, o) = _output(raw, o, "", 0);
        }
        _end(raw, o);
    }

    function _start(bytes calldata raw) private pure returns (uint256) {
        if (raw.length < 10) revert MalformedTx();
        if (raw[4] == 0x00 && raw[5] == 0x01) revert WitnessSerialization();
        return 4;
    }

    /// @dev Skips the version and the inputs.
    function _outputsStart(bytes calldata raw) private pure returns (uint256 o) {
        o = _start(raw);
        uint256 inputs;
        (inputs, o) = _varInt(raw, o);
        if (inputs == 0) revert MalformedTx();
        for (uint256 i = 0; i < inputs; i++) {
            o += 36;
            if (o > raw.length) revert MalformedTx();
            uint256 len;
            (len, o) = _varInt(raw, o);
            o += len + 4;
            if (o > raw.length) revert MalformedTx();
        }
    }

    /// @dev Reads one output at `o`: whether it pays `script` at least `sats`.
    function _output(bytes calldata raw, uint256 o, bytes memory script, uint64 sats) private pure returns (bool pays, uint256 next) {
        if (o + 8 > raw.length) revert MalformedTx();
        uint64 value;
        for (uint256 i = 0; i < 8; i++) {
            value |= uint64(uint8(raw[o + i])) << uint64(8 * i);
        }
        uint256 len;
        (len, next) = _varInt(raw, o + 8);
        if (next + len > raw.length) revert MalformedTx();
        pays = len == script.length && len > 0 && value >= sats && keccak256(raw[next:next + len]) == keccak256(script);
        next += len;
    }

    /// @dev The lock time, and nothing after it.
    function _end(bytes calldata raw, uint256 o) private pure {
        if (o + 4 != raw.length) revert MalformedTx();
    }

    function _le4(bytes calldata b, uint256 o) private pure returns (uint32) {
        return uint32(uint8(b[o])) | (uint32(uint8(b[o + 1])) << 8) | (uint32(uint8(b[o + 2])) << 16) | (uint32(uint8(b[o + 3])) << 24);
    }

    /// @dev Bitcoin's variable length integer, in its shortest form only.
    function _varInt(bytes calldata b, uint256 o) private pure returns (uint256 v, uint256 next) {
        if (o >= b.length) revert MalformedTx();
        uint8 first = uint8(b[o]);
        if (first < 0xFD) return (first, o + 1);
        uint256 size = first == 0xFD ? 2 : first == 0xFE ? 4 : 8;
        if (o + 1 + size > b.length) revert MalformedTx();
        for (uint256 i = 0; i < size; i++) {
            v |= uint256(uint8(b[o + 1 + i])) << (8 * i);
        }
        uint256 least = size == 2 ? 0xFD : size == 4 ? 0x10000 : 0x100000000;
        if (v < least) revert MalformedTx();
        return (v, o + 1 + size);
    }
}
