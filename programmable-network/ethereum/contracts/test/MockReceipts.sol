// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

/// @dev Stands in for the vault's receipts in BetaBaskets' tests (E5): a
/// receipt exists once `make` names it.
contract MockReceipts {
    mapping(uint32 => address) public receiptOf;

    function make(uint32 asset, address token) external {
        receiptOf[asset] = token;
    }
}
