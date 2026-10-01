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
    function make(uint8 here, uint8 peer, address coin) external returns (VaultHome) {
        return new VaultHome(IVaultCore(msg.sender), here, peer, coin);
    }
}

/// @title VaultReceiptsFactory
/// @notice The same for a vault's `VaultReceipts`.
contract VaultReceiptsFactory {
    function make(uint8 here, uint8 peer) external returns (VaultReceipts) {
        return new VaultReceipts(IVaultCore(msg.sender), here, peer);
    }
}
