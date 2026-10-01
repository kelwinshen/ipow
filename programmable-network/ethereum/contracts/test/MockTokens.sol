// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

import {ERC20} from "@openzeppelin/contracts/token/ERC20/ERC20.sol";

/// @notice A mintable token with any number of decimals, for the vault's
/// tests — not part of the protocol.
contract MockToken is ERC20 {
    uint8 private immutable _decimals;

    constructor(string memory name_, string memory symbol_, uint8 decimals_) ERC20(name_, symbol_) {
        _decimals = decimals_;
    }

    function decimals() public view override returns (uint8) {
        return _decimals;
    }

    function mint(address to, uint256 amount) external {
        _mint(to, amount);
    }
}

/// @notice A token that burns 1% of every transfer, for the vault's tests.
contract FeeOnTransferToken is ERC20 {
    constructor() ERC20("Fee token", "FEE") {}

    function decimals() public pure override returns (uint8) {
        return 6;
    }

    function mint(address to, uint256 amount) external {
        _mint(to, amount);
    }

    function _update(address from, address to, uint256 value) internal override {
        if (from == address(0) || to == address(0)) return super._update(from, to, value);
        uint256 fee = value / 100;
        super._update(from, address(0), fee);
        super._update(from, to, value - fee);
    }
}

/// @notice A token whose issuer can freeze an account and seize tokens from
/// any, as a regulated token's issuer can, for BETA's tests.
contract IssuerToken is ERC20 {
    address public immutable issuer;
    mapping(address => bool) public frozen;

    error Frozen();

    constructor() ERC20("Issuer token", "ISS") {
        issuer = msg.sender;
    }

    function mint(address to, uint256 amount) external {
        _mint(to, amount);
    }

    function setFrozen(address who, bool value) external {
        require(msg.sender == issuer);
        frozen[who] = value;
    }

    function seize(address from, uint256 amount) external {
        require(msg.sender == issuer);
        _burn(from, amount);
    }

    function _update(address from, address to, uint256 value) internal override {
        if (frozen[from] || frozen[to]) revert Frozen();
        super._update(from, to, value);
    }
}
