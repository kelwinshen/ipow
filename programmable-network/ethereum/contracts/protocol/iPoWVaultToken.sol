// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

import {IERC20} from "@openzeppelin/contracts/token/ERC20/IERC20.sol";
import {SafeERC20} from "@openzeppelin/contracts/token/ERC20/utils/SafeERC20.sol";

import {iPoWProtocol} from "./iPoWProtocol.sol";
import {iPoWProtocolToken} from "./iPoWProtocolToken.sol";
import {iPoWVault} from "./iPoWVault.sol";
import {VaultHomeFactory, VaultReceiptsFactory} from "./vault/VaultFactories.sol";

/// @title iPoWVaultToken
/// @notice The vault on a network without a native coin (D136, D137), such
/// as Tempo: its coin is a token, asset 0 of its home part. Deposits and a
/// checkpoint's fees are taken from the caller with its approval; the
/// protocol, its token build, takes the fees from the vault. Native value is
/// refused. The coin must be the protocol's coin, and must move exactly the
/// amount named and call nothing on a transfer, as PathUSD does.
contract iPoWVaultToken is iPoWVault {
    using SafeERC20 for IERC20;

    /// @notice The network's coin. In storage: the core is near the size
    /// limit, and an immutable is copied into the code at every use.
    IERC20 public coin;

    constructor(
        iPoWProtocol protocol_,
        uint8 here_,
        uint8 peer_,
        bytes32 peerVault_,
        uint256 deposit_,
        uint256 minCertifyingEscrow_,
        VaultHomeFactory homeFactory_,
        VaultReceiptsFactory receiptsFactory_,
        IERC20 coin_
    ) iPoWVault(protocol_, here_, peer_, peerVault_, deposit_, minCertifyingEscrow_, homeFactory_, receiptsFactory_, address(coin_)) {
        if (address(coin_) != address(iPoWProtocolToken(address(protocol_)).coin())) revert WrongCoin();
        coin = coin_;
        coin_.forceApprove(address(protocol_), type(uint256).max);
    }

    function _take(uint256 amount) internal override {
        if (msg.value != 0) revert WrongValue();
        coin.safeTransferFrom(msg.sender, address(this), amount);
    }

    function _sendCoin(address to, uint256 amount) internal override {
        if (amount == 0) return;
        coin.safeTransfer(to, amount);
    }

    function _coinBalance() internal view override returns (uint256) {
        return coin.balanceOf(address(this));
    }

    function _jobValue(uint256) internal pure override returns (uint256) {
        return 0;
    }
}
