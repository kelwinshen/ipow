// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

/// @dev Same read-only view into `iPoWV1`'s header relay that `BetaVault.sol`
/// and `iPoWV1Conversion.sol` use.
interface IIPoWV1HeadersView {
    function globalTipHeight() external view returns (uint256);
    function globalHeightToHashLE(uint256 height) external view returns (bytes32);
    function globalHeaders(bytes32 hashLE)
        external
        view
        returns (bytes32 prevHashLE, bytes32 merkleRootLE, uint32 nBits, uint32 timestamp, bool set, uint64 arrivalTime);
}

/// @title AnchorChainLib
/// @notice Pure Bitcoin-transaction byte-parsing (DESIGN_V2.md §8.18),
/// extracted near-verbatim from `BetaVault.sol`'s own private
/// `_readVarInt`/`_parseAnchor` (lines 824-882) into a shared library so
/// `BetaHub`'s `HubAnchorJudge` doesn't hand-duplicate the same intricate
/// byte-parsing — a real transcription-risk area. `BetaVault.sol` itself is
/// intentionally left unrefactored: it's already deployed byte-identically
/// on six live networks, and pointing it at this library too would need
/// fresh redeployment everywhere for no behavior change.
///
/// @dev Deliberately pure-functions-only: the storage-touching orchestration
/// around these (`BetaVault._verifyAndAdvance`'s equivalent — checking the
/// `anchors` replay-guard, calling the header relay, walking the Merkle
/// branch, and advancing a party's on-chain pointer) stays directly in
/// `HubAnchorJudge.sol`, copied from `BetaVault.sol` with its own `Party`
/// struct. An earlier draft tried to share that orchestration too via a
/// library-level `StatementPointer storage` parameter aliased onto
/// `HubPartyRegistry.Party`'s storage slot with raw assembly — reverted:
/// `Party`'s actual field layout (`bool exists; address owner; PartyKind
/// kind;` packed into slot 0, `anchorTxidLE` only starting at slot 1) does
/// not match `StatementPointer`'s, so the alias would have silently
/// corrupted `party.exists`/`owner`/`kind` on every write. Not worth the
/// risk for what's a small amount of orchestration code anyway.
library AnchorChainLib {
    error MalformedTx();
    error WitnessSerialization();
    error AlreadyProcessed();
    error InvalidHeader();
    error InvalidMerkleBranch();
    error NotOnStatementChain();
    error BadAnchorPayload();

    uint8 internal constant ANCHOR_VERSION = 1;

    /// @dev The statement-chain txid a raw tx hashes to — `sha256d`, same as
    /// Bitcoin's own txid.
    function txidOf(bytes calldata txRaw) internal pure returns (bytes32) {
        return sha256(abi.encodePacked(sha256(txRaw)));
    }

    function readVarInt(bytes calldata b, uint256 o) internal pure returns (uint64 v, uint256 next) {
        if (o >= b.length) revert MalformedTx();
        uint8 p = uint8(b[o]);
        if (p < 0xFD) return (p, o + 1);
        if (p == 0xFD) {
            if (o + 3 > b.length) revert MalformedTx();
            return (uint64(uint8(b[o + 1])) | (uint64(uint8(b[o + 2])) << 8), o + 3);
        }
        if (p == 0xFE) {
            if (o + 5 > b.length) revert MalformedTx();
            uint64 x;
            for (uint256 i = 0; i < 4; i++) x |= uint64(uint8(b[o + 1 + i])) << uint64(8 * i);
            return (x, o + 5);
        }
        if (o + 9 > b.length) revert MalformedTx();
        uint64 y;
        for (uint256 i = 0; i < 8; i++) y |= uint64(uint8(b[o + 1 + i])) << uint64(8 * i);
        return (y, o + 9);
    }

    /// @dev Mirrors `beta-factory`'s `parse_anchor`: input[0]'s outpoint and
    /// output[1]'s OP_RETURN payload from a witness-stripped tx.
    function parseAnchor(bytes calldata tx_)
        internal
        pure
        returns (bytes32 in0Txid, uint32 in0Vout, bool hasPayload, uint8 kind, bytes32 stmtHash)
    {
        if (tx_.length < 4 + 1 + 36 + 1 + 4 + 1) revert MalformedTx();
        uint256 o = 4;
        if (tx_[o] == 0x00 && tx_[o + 1] == 0x01) revert WitnessSerialization();
        (uint64 inCount, uint256 n) = readVarInt(tx_, o);
        o = n;
        if (inCount == 0 || o + 36 > tx_.length) revert MalformedTx();
        in0Txid = bytes32(tx_[o:o + 32]);
        in0Vout = uint32(uint8(tx_[o + 32])) | (uint32(uint8(tx_[o + 33])) << 8) | (uint32(uint8(tx_[o + 34])) << 16)
            | (uint32(uint8(tx_[o + 35])) << 24);
        for (uint64 i = 0; i < inCount; i++) {
            o += 36;
            (uint64 slen, uint256 n2) = readVarInt(tx_, o);
            o = n2 + slen + 4;
            if (o > tx_.length) revert MalformedTx();
        }
        (uint64 outCount, uint256 n3) = readVarInt(tx_, o);
        o = n3;
        if (outCount == 0) revert MalformedTx();
        for (uint64 j = 0; j < outCount; j++) {
            if (o + 8 > tx_.length) revert MalformedTx();
            o += 8;
            (uint64 slen, uint256 n4) = readVarInt(tx_, o);
            o = n4;
            if (o + slen > tx_.length) revert MalformedTx();
            if (j == 1 && slen == 36 && tx_[o] == 0x6a && uint8(tx_[o + 1]) == 34 && uint8(tx_[o + 2]) == ANCHOR_VERSION) {
                hasPayload = true;
                kind = uint8(tx_[o + 3]);
                stmtHash = bytes32(tx_[o + 4:o + 36]);
            }
            o += slen;
        }
    }
}
