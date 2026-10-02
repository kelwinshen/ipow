// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

import {iPoWProtocolNative} from "../protocol/iPoWProtocol.sol";
import {iPoWLightClient} from "../protocol/iPoWLightClient.sol";
import {IDataFee} from "../protocol/DataFee.sol";

/// @notice Test-only. Lets a test lower the limit of questions for a parent,
/// so that reaching it does not take 2,016 rounds. Until a test sets a value,
/// the rule of the real contract applies.
contract iPoWProtocolHarness is iPoWProtocolNative {
    uint256 private _testMaxParentQuestions;

    constructor(iPoWLightClient lightClient_) iPoWProtocolNative(lightClient_, IDataFee(address(0))) {}

    function setMaxParentQuestions(uint256 max) external {
        _testMaxParentQuestions = max;
    }

    function _maxParentQuestions() internal view override returns (uint256) {
        return _testMaxParentQuestions == 0 ? super._maxParentQuestions() : _testMaxParentQuestions;
    }
}
