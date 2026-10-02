// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

/// @notice Test-only. Records what a contract sees on a network, to measure
/// a network before the protocol is deployed there (V5, stage 7): the price
/// of gas, the base fee, and the units of the native coin, from an amount
/// sent with a call and the caller's balance.
contract NetworkProbe {
    uint256 public gasPrice;
    uint256 public baseFee;
    uint256 public value;
    uint256 public ownBalance;
    uint256 public callerBalance;
    uint256 public chainId;
    uint256 public gasLeft;

    function look() external payable {
        gasPrice = tx.gasprice;
        baseFee = block.basefee;
        value = msg.value;
        ownBalance = address(this).balance;
        callerBalance = msg.sender.balance;
        chainId = block.chainid;
        gasLeft = gasleft();
    }
}
