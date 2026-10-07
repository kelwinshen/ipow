// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

import {IVaultCore} from "./VaultRecords.sol";
import {VaultHome} from "./VaultHome.sol";
import {VaultReceipts} from "./VaultReceipts.sol";

/// @title VaultHomeFactory
/// @notice Makes a vault's `VaultHome` for the core it names, before that
/// core is deployed, and records what it made: the core's deployment takes
/// the part and checks that the factory made it, so whoever checks a vault
/// compares the factory's code with the source. Made in its own transaction
/// so that no transaction creates the core and its parts together, which
/// passes Tempo's limit of 50 million gas (2026-10-03). One factory serves
/// every vault of a network (one per pair, D132). Anyone may make a part;
/// one made for another core serves only that core. `made` shows a part's
/// code, not that its vault exists: one whose vault was never deployed
/// holds whatever is locked into it for good, so whoever checks a part also
/// reads that its core has code and names it.
contract VaultHomeFactory {
    /// @notice The native coin's decimals as a contract sees them on this
    /// network: 18, or 8 on Hedera (D139).
    uint8 public immutable nativeDecimals;

    mapping(address => bool) public made;

    error BadDecimals();

    constructor(uint8 nativeDecimals_) {
        if (nativeDecimals_ == 0 || nativeDecimals_ > 18) revert BadDecimals();
        nativeDecimals = nativeDecimals_;
    }

    function make(IVaultCore core, uint8 here, uint8 peer, address coin) external returns (VaultHome part) {
        part = new VaultHome(core, here, peer, coin, nativeDecimals);
        made[address(part)] = true;
    }
}

/// @title VaultReceiptsFactory
/// @notice The same for a vault's `VaultReceipts`.
contract VaultReceiptsFactory {
    mapping(address => bool) public made;

    /// @notice A part with no genesis (D59 whole).
    function make(IVaultCore core, uint8 here, uint8 peer) external returns (VaultReceipts part) {
        part = new VaultReceipts(core, here, peer, address(0), 0);
        made[address(part)] = true;
    }

    /// @notice A part that starts in genesis: `genesisKey` may make receipts
    /// and issue named locks until it finalizes or `genesisEnd` (D142, D144).
    function makeGenesis(IVaultCore core, uint8 here, uint8 peer, address genesisKey, uint64 genesisEnd) external returns (VaultReceipts part) {
        part = new VaultReceipts(core, here, peer, genesisKey, genesisEnd);
        made[address(part)] = true;
    }
}
