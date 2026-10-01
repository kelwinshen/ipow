// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

/// @title VaultRecords
/// @notice The records of the vault's pair chains and their bytes, as
/// section 11.9 of docs/design/ipow-protocol.md writes them (D131). Numbers
/// are big-endian; amounts and fees in the receipt's smallest unit.
library VaultRecords {
    /// @notice The networks of the pair, as records name them.
    uint8 internal constant ETHEREUM = 1;
    uint8 internal constant SOLANA = 2;

    uint8 internal constant LOCK = 1;
    uint8 internal constant REQUEST = 2;
    uint8 internal constant CANCEL = 3;
    uint8 internal constant BOND = 4;
    uint8 internal constant EXIT = 5;
    uint8 internal constant ASSET = 6;

    uint256 internal constant LOCK_LEN = 78;
    uint256 internal constant REQUEST_LEN = 78;
    uint256 internal constant CANCEL_LEN = 10;
    uint256 internal constant BOND_LEN = 15;
    uint256 internal constant ASSET_LEN = 39;

    /// @notice A receipt has the token's decimals, at most 9 (section 11.9).
    uint8 internal constant MAX_DECIMALS = 9;

    /// @notice D123: an attester of a lock locks 1.25 times its amount, and a
    /// claim carrying its record must open within 7 days. D126: a lock takes
    /// attests for 7 days from its first.
    uint256 internal constant FAST_COLLATERAL_BPS = 12_500;
    uint256 internal constant FAST_OPEN_WINDOW = 7 days;
    /// @notice D124: an attester's share of a fast fee falls to nothing 8 days
    /// after the lock or burn.
    uint256 internal constant FAST_FEE_DEADLINE = 8 days;
    uint256 internal constant BPS = 10_000;

    /// @notice An asset, by its home network and its number there.
    function key(uint8 home, uint32 asset) internal pure returns (uint40) {
        return (uint40(home) << 32) | uint40(asset);
    }

    function homeOf(uint40 k) internal pure returns (uint8) {
        return uint8(k >> 32);
    }

    function assetOf(uint40 k) internal pure returns (uint32) {
        return uint32(k);
    }

    function u64(bytes calldata b, uint256 o) internal pure returns (uint64) {
        return uint64(bytes8(b[o:o + 8]));
    }

    function u32(bytes calldata b, uint256 o) internal pure returns (uint32) {
        return uint32(bytes4(b[o:o + 4]));
    }

    function b32(bytes calldata b, uint256 o) internal pure returns (bytes32) {
        return bytes32(b[o:o + 32]);
    }

    /// @notice An Ethereum address written in 32 bytes: the last 20, the
    /// first 12 zero. Zero when the bytes do not name one.
    function addressOf(bytes32 b) internal pure returns (address) {
        if (uint256(b) >> 160 != 0) return address(0);
        return address(uint160(uint256(b)));
    }

    function lock(
        uint8 home,
        uint32 asset,
        uint64 id,
        uint64 amount,
        bytes32 recipient,
        uint64 fee,
        uint64 fastFee,
        uint64 at
    ) internal pure returns (bytes memory) {
        return abi.encodePacked(LOCK, home, asset, id, amount, recipient, fee, fastFee, at);
    }

    function request(
        uint8 network,
        uint32 asset,
        uint64 id,
        uint64 amount,
        bytes32 to,
        uint64 fee,
        uint64 fastFee,
        uint64 at
    ) internal pure returns (bytes memory) {
        return abi.encodePacked(REQUEST, network, asset, id, amount, to, fee, fastFee, at);
    }

    function cancel(uint8 network, uint64 lockId) internal pure returns (bytes memory) {
        return abi.encodePacked(CANCEL, network, lockId);
    }

    function assetRecord(uint8 home, uint32 number, bytes32 token, uint8 decimals) internal pure returns (bytes memory) {
        return abi.encodePacked(ASSET, home, number, token, decimals);
    }

    /// @notice D124: an attester's share of `fastFee`, attesting at `at` a
    /// lock or burn made at `madeAt`: the fee times the time left to 8 days
    /// after it, over 8 days.
    function fastShare(uint64 fastFee, uint64 madeAt, uint64 at) internal pure returns (uint64) {
        uint256 end = uint256(madeAt) + FAST_FEE_DEADLINE;
        if (at >= end) return 0;
        uint256 left = end - at;
        if (left > FAST_FEE_DEADLINE) left = FAST_FEE_DEADLINE;
        return uint64((uint256(fastFee) * left) / FAST_FEE_DEADLINE);
    }
}

/// @notice What the vault's parts read of its core.
interface IVaultCore {
    function claimAccepted(uint256 claimId) external view returns (bool);
    function carries(uint256 claimId, bytes32 recordHash) external view returns (bool);
    /// @return operator The chain's operator, `openedAt` when it opened, and
    /// whether it was decided and accepted.
    function claimInfo(uint256 claimId)
        external
        view
        returns (address operator, uint64 openedAt, bool decided, bool accepted);
    /// @notice A registered chain not refused, slashed or ended.
    function operatorActive(address operator) external view returns (bool);
}
