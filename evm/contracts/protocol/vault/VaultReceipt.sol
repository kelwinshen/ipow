// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

import {ERC20} from "@openzeppelin/contracts/token/ERC20/ERC20.sol";

/// @title VaultReceipt
/// @notice The receipt on Ethereum of an asset locked on Solana (section
/// 11.9): one per asset, made by the vault, which alone mints. Anyone burns
/// their own; a burn for the asset goes through the vault.
contract VaultReceipt is ERC20 {
    address public immutable minter;
    uint8 private immutable _decimals;

    error NotMinter();

    constructor(string memory name_, string memory symbol_, uint8 decimals_) ERC20(name_, symbol_) {
        minter = msg.sender;
        _decimals = decimals_;
    }

    function decimals() public view override returns (uint8) {
        return _decimals;
    }

    function mint(address to, uint256 amount) external {
        if (msg.sender != minter) revert NotMinter();
        _mint(to, amount);
    }

    /// @notice The vault burns what a holder hands it for a burn request.
    function burnFrom(address from, uint256 amount) external {
        if (msg.sender != minter) revert NotMinter();
        _burn(from, amount);
    }

    /// @notice Anyone burns its own: a slashed bond is burned this way.
    function burn(uint256 amount) external {
        _burn(msg.sender, amount);
    }
}
