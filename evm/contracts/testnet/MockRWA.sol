// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

import {ERC20} from "@openzeppelin/contracts/token/ERC20/ERC20.sol";

/// @title MockRWA
/// @notice A test-network stand-in for a tokenized real-world asset (a
/// share, gold): an ordinary ERC-20 with no backing and no value, so that
/// Greatwall.finance can be tried end to end, converting into it, locking it
/// in the vault and putting it in a BETA basket. Never deploy it on a main
/// network.
///
/// The deployer mints any amount (an operator's inventory for swaps). Anyone
/// takes `faucetAmount` from the faucet, once per `FAUCET_COOLDOWN`.
contract MockRWA is ERC20 {
    /// @notice How long an address waits between two faucet claims.
    uint256 public constant FAUCET_COOLDOWN = 6 hours;

    address public immutable minter;
    uint256 public immutable faucetAmount;
    /// @notice When each address last used the faucet.
    mapping(address => uint256) public lastClaim;

    error NotMinter();
    error FaucetCoolingDown(uint256 nextAt);

    event FaucetClaimed(address indexed to, uint256 amount);

    constructor(string memory name_, string memory symbol_, uint256 faucetAmount_) ERC20(name_, symbol_) {
        minter = msg.sender;
        faucetAmount = faucetAmount_;
    }

    /// @notice The deployer mints, for an operator's inventory.
    function mint(address to, uint256 amount) external {
        if (msg.sender != minter) revert NotMinter();
        _mint(to, amount);
    }

    /// @notice Gives the caller `faucetAmount`, once per cooldown.
    function faucet() external {
        uint256 last = lastClaim[msg.sender];
        if (last != 0 && block.timestamp < last + FAUCET_COOLDOWN) revert FaucetCoolingDown(last + FAUCET_COOLDOWN);
        lastClaim[msg.sender] = block.timestamp;
        _mint(msg.sender, faucetAmount);
        emit FaucetClaimed(msg.sender, faucetAmount);
    }
}
