// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

import {iPoWLightClient} from "../protocol/iPoWLightClient.sol";
import {BitcoinHeaderLib} from "../protocol/BitcoinHeaderLib.sol";

/// @notice Test-only. Lets a test set the network's time, and lower the minimum
/// difficulty so that a test can mine its own blocks. Until a test sets a value,
/// the rules of the real contract apply.
contract iPoWLightClientHarness is iPoWLightClient {
    uint256 private _testNow;
    uint256 private _testMaxTarget;
    uint256 private _testPowLimit;

    constructor(uint32 minHeight_) iPoWLightClient(minHeight_) {}

    function setNow(uint256 t) external {
        _testNow = t;
    }

    function setLimits(uint256 maxTarget, uint256 powLimit) external {
        _testMaxTarget = maxTarget;
        _testPowLimit = powLimit;
    }

    function retargetBits(uint32 lastBits, uint256 firstTime, uint256 lastTime) external view returns (uint32) {
        return BitcoinHeaderLib.retarget(lastBits, firstTime, lastTime, _powLimit());
    }

    function targetFromBits(uint32 bits) external pure returns (uint256) {
        return BitcoinHeaderLib.targetFromBits(bits);
    }

    function bitsFromTarget(uint256 target) external pure returns (uint32) {
        return BitcoinHeaderLib.bitsFromTarget(target);
    }

    function _now() internal view override returns (uint256) {
        return _testNow == 0 ? super._now() : _testNow;
    }

    function _maxTarget() internal view override returns (uint256) {
        return _testMaxTarget == 0 ? super._maxTarget() : _testMaxTarget;
    }

    function _powLimit() internal view override returns (uint256) {
        return _testPowLimit == 0 ? super._powLimit() : _testPowLimit;
    }
}
