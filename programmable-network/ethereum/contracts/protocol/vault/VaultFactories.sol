// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

import {IVaultCore} from "./VaultRecords.sol";
import {VaultHome} from "./VaultHome.sol";
import {VaultReceipts} from "./VaultReceipts.sol";

/// @title VaultHomeFactory
/// @notice Makes a vault's `VaultHome` for the core that calls it, so that
/// the core's own deployment code does not hold the part's: it would pass
/// Ethereum's limit on it. One factory serves every vault of a network (one
/// per pair, D132). A part made for any other caller is bound to that caller
/// only, and serves no vault.
contract VaultHomeFactory {
    /// @notice The native coin's decimals as a contract sees them on this
    /// network: 18, or 8 on Hedera (D139).
    uint8 public immutable nativeDecimals;

    error BadDecimals();

    constructor(uint8 nativeDecimals_) {
        if (nativeDecimals_ == 0 || nativeDecimals_ > 18) revert BadDecimals();
        nativeDecimals = nativeDecimals_;
    }

    function make(uint8 here, uint8 peer, address coin) external returns (VaultHome) {
        return new VaultHome(IVaultCore(msg.sender), here, peer, coin, nativeDecimals);
    }
}

/// @title VaultReceiptsFactory
/// @notice The same for a vault's `VaultReceipts`.
contract VaultReceiptsFactory {
    function make(uint8 here, uint8 peer) external returns (VaultReceipts) {
        return new VaultReceipts(IVaultCore(msg.sender), here, peer);
    }
}
