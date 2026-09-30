// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

import {iPoWTypes} from "./iPoWTypes.sol";

/**
 * @title iPoWStorage
 * @notice Shared storage layout for the Hyperliquid-only facet/router split of
 * `iPoW` (see `../iPoWRouter.sol`). Every facet and the router itself
 * inherit this SAME contract, in this SAME form, so the Solidity compiler
 * assigns every variable an identical storage slot everywhere — the one
 * property a `delegatecall`-based split absolutely depends on. Never edit
 * this file to add, remove, or reorder a variable without recompiling and
 * re-verifying every facet and the router together; doing so would silently
 * shift slots and corrupt state.
 * @dev Only Hyperliquid needs this split (its testnet block gas limit,
 * 3,000,000, is below the ~4.79M `iPoW` needs to deploy as one contract —
 * see docs/design/ipow-implementation.md §8.16). Every other network keeps the plain,
 * monolithic `iPoW.sol` unchanged.
 *
 * Two deliberate differences from the monolithic contract, both required by
 * the split, neither changing external behavior:
 * - `NATIVE_DECIMALS`/`SELF_NETWORK_ID` move from `immutable` (baked into
 *   each contract's own bytecode, so a delegatecalled facet would read its
 *   own meaningless copy, not the router's) to plain storage, set once by
 *   the router's constructor like everything else here.
 * - `windows` moves from `private` to `internal` — Solidity's `private`
 *   isn't visible to inheriting contracts at all, and both facets need it.
 */
abstract contract iPoWStorage is iPoWTypes {
    address public operator;

    modifier onlyOperator() {
        if (msg.sender != operator) revert Unauthorized();
        _;
    }

    uint256 public NATIVE_DECIMALS;
    uint256 public SELF_NETWORK_ID;

    /// @dev Basis points, e.g. 50 = 0.5%.
    uint256 public commitFeeBps;

    uint256 public nativeLiquidity;

    // --- Safety buckets: liabilities carved out of the contract's raw balance ---
    uint256 public totalLockedDeposits;
    uint256 public totalReservedNative;
    uint256 public totalHeldCommitFees;

    uint256 public minAnchorHeight;

    uint256 public nextTxId = 1;
    mapping(uint256 => Conversion) public conversions;

    // --- Global Bitcoin header relay ---
    mapping(uint256 => bytes32) public globalHeightToHashLE;
    mapping(bytes32 => GlobalHeaderMeta) public globalHeaders;
    uint256 public globalTipHeight;

    /// @dev Keyed by keccak256(txidLE || headerHashLE) — stops a single proof from
    /// settling more than one conversion.
    mapping(bytes32 => bool) public usedProofs;

    mapping(uint256 => NetworkConfig) public networkConfigs;

    /// @dev Bitcoin height -> txIds whose proofs are waiting on a header at that height.
    mapping(uint256 => uint256[]) internal pendingProofsAtHeight;

    /// @dev Guards against reusing the same protocol-owned Bitcoin script across
    /// conversions.
    mapping(bytes => bool) public usedIPoWPrograms;

    mapping(uint256 => HeaderWindow) internal windows;

    /// @dev Gates whether the header relay may jump ahead or must stream contiguously.
    uint256 public activeOpenConversions;

    modifier validTx(uint256 txId) {
        if (txId == 0 || txId >= nextTxId) revert BadTxId();
        _;
    }
}
