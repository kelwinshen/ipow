// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

import {iPoWProtocol} from "../protocol/iPoWProtocol.sol";

/// @notice Test-only. An operator or payer that is a contract. It can refuse
/// money, or call back into the protocol when it receives money.
contract ProtocolActor {
    enum Mode {
        Accept,
        Refuse,
        ReenterWithdrawBond,
        ReenterWithdrawCredit
    }

    iPoWProtocol public immutable protocol;
    Mode public mode;

    constructor(iPoWProtocol protocol_) {
        protocol = protocol_;
    }

    function setMode(Mode mode_) external {
        mode = mode_;
    }

    function lockBond() external payable {
        protocol.lockBond{value: msg.value}();
    }

    function withdrawBond(uint256 amount) external {
        protocol.withdrawBond(amount);
    }

    function withdrawCredit() external {
        protocol.withdrawCredit();
    }

    function bid(uint256 jobId, uint256 amount) external {
        protocol.bid(jobId, amount);
    }

    receive() external payable {
        if (mode == Mode.Refuse) revert("refused");
        if (mode == Mode.ReenterWithdrawBond) protocol.withdrawBond(1);
        if (mode == Mode.ReenterWithdrawCredit) protocol.withdrawCredit();
    }
}
