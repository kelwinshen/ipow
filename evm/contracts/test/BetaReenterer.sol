// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

import {BetaBaskets} from "../applications/beta/BetaBaskets.sol";

/// @notice Test-only. Holds BETA and, when a burn pays it ETH, tries to burn
/// again from inside the payment.
contract BetaReenterer {
    BetaBaskets public immutable app;
    bytes32 public immutable key;

    constructor(BetaBaskets app_, bytes32 key_) {
        app = app_;
        key = key_;
    }

    function burn(uint256 amount) external {
        app.burn(key, amount, 0, address(this));
    }

    receive() external payable {
        app.burn(key, 1, 0, address(this));
    }
}
