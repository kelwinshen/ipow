// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

import {BitcoinPaymentLib} from "../apps/BitcoinPaymentLib.sol";

/// @notice Test-only: exposes the library's reads.
contract BitcoinPaymentLibHarness {
    function paysAtLeast(bytes calldata raw, bytes calldata script, uint64 sats) external pure returns (bool) {
        return BitcoinPaymentLib.paysAtLeast(raw, script, sats);
    }

    function outputPays(bytes calldata raw, uint256 vout, bytes calldata script, uint64 sats) external pure returns (bool) {
        return BitcoinPaymentLib.outputPays(raw, vout, script, sats);
    }

    function spends(bytes calldata raw, bytes32 txid, uint32 vout) external pure returns (bool) {
        return BitcoinPaymentLib.spends(raw, txid, vout);
    }
}
