// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

import {IERC20} from "@openzeppelin/contracts/token/ERC20/IERC20.sol";
import {SafeERC20} from "@openzeppelin/contracts/token/ERC20/utils/SafeERC20.sol";

import {iPoWLightClient} from "./iPoWLightClient.sol";
import {iPoWProtocol} from "./iPoWProtocol.sol";

/// @title iPoWProtocolToken
/// @notice The protocol on a network without a native coin (D136, D137),
/// such as Tempo: bonds, fees and deposits are a token, taken from the
/// caller with its approval, and paid out in it. Native value is refused.
/// The commitment fee divides the price of work by `priceScale`, the units
/// of the network's gas price per unit of the token: 10^12 on Tempo, whose
/// gas price is in attodollars and whose PathUSD has 6 decimals.
/// The coin must move exactly the amount named and call nothing on a
/// transfer, as PathUSD does: what is taken is credited as named, not
/// measured.
contract iPoWProtocolToken is iPoWProtocol {
    using SafeERC20 for IERC20;

    /// @notice The network's coin, and the units of the price per unit of
    /// it. Set once, in storage: an immutable's value is copied into the code
    /// at every use, and this build is near the size limit.
    IERC20 public coin;
    uint256 public priceScale;

    constructor(iPoWLightClient lightClient_, IERC20 coin_, uint256 priceScale_) iPoWProtocol(lightClient_) {
        if (address(coin_) == address(0) || priceScale_ == 0) revert ZeroAddress();
        coin = coin_;
        priceScale = priceScale_;
    }

    function _take(uint256 amount) internal override {
        if (msg.value != 0) revert WrongValue();
        coin.safeTransferFrom(msg.sender, address(this), amount);
    }

    function _send(address to, uint256 amount) internal override {
        coin.safeTransfer(to, amount);
    }

    function _priceScale() internal view override returns (uint256) {
        return priceScale;
    }
}
