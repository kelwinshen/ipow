// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

import {ReentrancyGuard} from "@openzeppelin/contracts/utils/ReentrancyGuard.sol";
import {BetaVaultStorage} from "../base/BetaVaultStorage.sol";

/**
 * @title BetaVaultAnchorFacet
 * @notice Hyperliquid-only facet (design/ipow-implementation.md §8.17): the statement-chain
 * anchor pipeline — verifying a Bitcoin tx against the header relay,
 * advancing a party's pointer, and judging/acting on the parsed payload by
 * kind (MINT/RELEASE/VETO/CANCEL/ATTEST/CLEAR/ALIVE, §6.4–6.7). Only ever
 * executed via `BetaVaultRouter`'s `delegatecall`, same as
 * `BetaVaultCoreFacet` — see that contract's header comment.
 */
contract BetaVaultAnchorFacet is BetaVaultStorage, ReentrancyGuard {
    /// @notice Permissionless. Processes the next anchor on `partyId`'s
    /// statement chain: verifies the Bitcoin tx against the header relay,
    /// advances the pointer, then judges/acts by kind (§6.4–6.7).
    function processAnchor(
        bytes32 partyId,
        bytes calldata statement,
        bytes calldata txRaw,
        uint256 blockHeight,
        bytes32[] calldata branchLE,
        uint256 index
    ) external nonReentrant {
        Party storage party = parties[partyId];
        if (!party.exists) revert NoParty();
        (bytes32 txidLE, uint32 headerTs, uint8 kind, bytes32 stmtHash) =
            _verifyAndAdvance(party, txRaw, blockHeight, branchLE, index, true);
        if (kind == 0) revert BadAnchorPayload();
        if (statement.length == 0 || uint8(statement[0]) != kind) revert KindMismatch();
        if (sha256(statement) != stmtHash) revert StatementHashMismatch();

        ProcessedAnchor storage pa = anchors[txidLE];
        pa.partyId = partyId;
        pa.kind = kind;
        pa.statementHash = stmtHash;
        pa.blockHeight = uint64(blockHeight);
        pa.processedAt = uint64(block.timestamp);

        if (kind == KIND_MINT) {
            if (party.kind != PartyKind.Operator) revert NotOperator();
            _processMint(partyId, party, pa, txidLE, statement, headerTs);
        } else if (kind == KIND_RELEASE) {
            if (party.kind != PartyKind.Operator) revert NotOperator();
            _processRelease(partyId, party, pa, txidLE, statement);
        } else if (kind == KIND_VETO) {
            if (party.kind != PartyKind.Auditor) revert NotAuditor();
            if (party.dead) revert PartyDead();
            _processVeto(partyId, party, pa, statement);
        } else if (kind == KIND_CANCEL) {
            if (party.kind != PartyKind.Operator) revert NotOperator();
            if (statement.length != 9) revert MalformedStatement();
            uint64 lockId = uint64(bytes8(statement[1:9]));
            Lock storage l = locks[lockId];
            if (l.state == LockState.Pending) _refund(lockId, l);
            pa.status = AnchorStatus.Exercised;
        } else if (kind == KIND_ATTEST || kind == KIND_CLEAR) {
            if (party.dead) revert PartyDead();
            if (statement.length != 33) revert MalformedStatement();
            _processAttestOrClear(partyId, party, pa, kind, bytes32(statement[1:33]));
        } else if (kind == KIND_ALIVE) {
            if (party.dead) revert PartyDead();
            if (statement.length != 33) revert MalformedStatement();
            // Judged here: "that operator is not dead on Ethereum".
            Party storage target = parties[bytes32(statement[1:33])];
            if (!target.exists) revert NoParty();
            if (target.dead) {
                _slash(partyId, party, params.vetoSlashWei, msg.sender);
                pa.status = AnchorStatus.Slashed;
            } else {
                pa.status = AnchorStatus.Exercised;
            }
        } else {
            revert BadAnchorPayload();
        }
        emit AnchorProcessed(partyId, txidLE, kind, pa.status);
    }

    /// @notice Permissionless. Advances past an anchor whose preimage nobody
    /// revealed once its header is `tSkipSecs` old.
    function skipAnchor(bytes32 partyId, bytes calldata txRaw, uint256 blockHeight, bytes32[] calldata branchLE, uint256 index)
        external
    {
        Party storage party = parties[partyId];
        if (!party.exists) revert NoParty();
        (bytes32 txidLE, uint32 headerTs, uint8 kind, bytes32 stmtHash) =
            _verifyAndAdvance(party, txRaw, blockHeight, branchLE, index, false);
        if (block.timestamp < uint256(headerTs) + params.tSkipSecs) revert SkipNotReady();
        ProcessedAnchor storage pa = anchors[txidLE];
        pa.partyId = partyId;
        pa.kind = kind;
        pa.statementHash = stmtHash;
        pa.blockHeight = uint64(blockHeight);
        pa.processedAt = uint64(block.timestamp);
        pa.status = AnchorStatus.Skipped;
        emit AnchorProcessed(partyId, txidLE, kind, AnchorStatus.Skipped);
    }
}
