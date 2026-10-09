// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

/// @notice Test-only. Stands in for an OP Stack rollup's `GasPriceOracle`,
/// its code placed at the predeploy's address: a fee per byte the test sets.
contract MockGasPriceOracle {
    uint256 public perByte;

    function setPerByte(uint256 value) external {
        perByte = value;
    }

    function getL1FeeUpperBound(uint256 unsignedTxSize) external view returns (uint256) {
        return unsignedTxSize * perByte;
    }
}

/// @notice Test-only. A reader of the data fee whose answer is not one word.
contract LongDataFee {
    function dataFee(uint256) external pure returns (uint256, uint256) {
        return (1, 2);
    }
}

/// @notice Test-only. A reader of the data fee that reads within its gas at
/// deployment, then burns all the gas it is given once told to.
contract BurningDataFee {
    bool public burning;

    function burn() external {
        burning = true;
    }

    function dataFee(uint256) external view returns (uint256) {
        if (burning) {
            uint256 i;
            while (true) i++;
        }
        return 1;
    }
}

/// @notice Test-only. Stands in for Arbitrum's `ArbGasInfo` precompile, its
/// code placed at the precompile's address.
contract MockArbGasInfo {
    uint256 public perByte;

    function setPerByte(uint256 value) external {
        perByte = value;
    }

    function getPricesInWei() external view returns (uint256, uint256, uint256, uint256, uint256, uint256) {
        return (1, perByte, 2, 3, 4, 5);
    }
}
