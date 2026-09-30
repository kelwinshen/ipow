// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

import {iPoWProtocol} from "../protocol/iPoWProtocol.sol";
import {iPoWLightClient} from "../protocol/iPoWLightClient.sol";

/// @notice Test-only. Lets a test lower the limit of questions for a parent,
/// so that reaching it does not take 2,016 rounds. Until a test sets a value,
/// the rule of the real contract applies.
contract iPoWProtocolHarness is iPoWProtocol {
    uint256 private _testMaxParentQuestions;

    constructor(iPoWLightClient lightClient_) iPoWProtocol(lightClient_) {}

    function setMaxParentQuestions(uint256 max) external {
        _testMaxParentQuestions = max;
    }

    function _maxParentQuestions() internal view override returns (uint256) {
        return _testMaxParentQuestions == 0 ? super._maxParentQuestions() : _testMaxParentQuestions;
    }
}
