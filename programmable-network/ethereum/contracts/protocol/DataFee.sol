// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

/// @title IDataFee
/// @notice D135: on a rollup, what posting `size` bytes of transactions to
/// its parent network costs now, in wei, read from the rollup's own on-chain
/// price. Nobody sets a value (D24, D58).
interface IDataFee {
    function dataFee(uint256 size) external view returns (uint256);
}

interface IGasPriceOracle {
    function getL1FeeUpperBound(uint256 unsignedTxSize) external view returns (uint256);
}

interface IArbGasInfo {
    function getPricesInWei() external view returns (uint256, uint256, uint256, uint256, uint256, uint256);
}

/// @title OpDataFee
/// @notice An OP Stack rollup, such as Base: the `GasPriceOracle` predeploy
/// gives an upper bound of the data fee of a transaction of a given size,
/// for all but about 1 in 10,000 transactions' compression. The size is
/// taken as one transaction's, which leaves out the 68 bytes the oracle adds
/// for each further one; the protocol's byte counts include each
/// transaction's signature, so this stays above what is paid.
contract OpDataFee is IDataFee {
    IGasPriceOracle public constant ORACLE = IGasPriceOracle(0x420000000000000000000000000000000000000F);

    function dataFee(uint256 size) external view returns (uint256) {
        return ORACLE.getL1FeeUpperBound(size);
    }
}

/// @title ArbDataFee
/// @notice An Arbitrum chain, such as Robinhood: the `ArbGasInfo` precompile
/// gives the price of a byte of calldata on the parent network.
contract ArbDataFee is IDataFee {
    IArbGasInfo public constant GAS_INFO = IArbGasInfo(0x000000000000000000000000000000000000006C);

    function dataFee(uint256 size) external view returns (uint256) {
        (, uint256 perByte, , , , ) = GAS_INFO.getPricesInWei();
        return size * perByte;
    }
}
