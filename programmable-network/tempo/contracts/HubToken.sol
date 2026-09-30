// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

import {ERC20} from "@openzeppelin/contracts/token/ERC20/ERC20.sol";
import {ERC20Burnable} from "@openzeppelin/contracts/token/ERC20/extensions/ERC20Burnable.sol";

/// @title HubToken
/// @notice The ERC20 a `BetaHub` deployment mints — design/ipow-implementation.md §8.18.
/// Deliberately not named "Beta"/"BETA": that's already `BetaToken.sol`'s
/// name, for the unrelated single-chain `BetaMint` mechanism. Each `BetaHub`
/// deploys and owns its own `HubToken` instance with its own name/symbol
/// (this repo's pilot deployment uses "iBETA"), so per-chain hubs never
/// collide on-chain even though several may share the same symbol.
/// @dev Same shape as `BetaToken.sol`: `minter` is set once at deploy time
/// and is immutable thereafter — no owner/admin who could ever change it.
/// Burning is unrestricted (`ERC20Burnable`'s own `burn`/`burnFrom`).
contract HubToken is ERC20, ERC20Burnable {
    address public immutable minter;

    error Unauthorized();

    constructor(string memory name_, string memory symbol_, address _minter) ERC20(name_, symbol_) {
        if (_minter == address(0)) revert Unauthorized();
        minter = _minter;
    }

    function mint(address to, uint256 amount) external {
        if (msg.sender != minter) revert Unauthorized();
        _mint(to, amount);
    }
}
