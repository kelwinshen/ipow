// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

import {BetaHubStorage} from "../base/BetaHubStorage.sol";
import {AnchorChainLib, IIPoWHeadersView} from "../libraries/AnchorChainLib.sol";

/**
 * @title BetaHubAnchorFacet
 * @notice Hyperliquid-only facet (design/ipow-implementation.md §8.19): the statement-chain
 * anchor pipeline — `processAnchor`/`skipAnchor`, `_verifyAndAdvance`, and
 * the kind handlers (`_processRemoteMint`, `_processVeto`,
 * `_processAttestOrClear`). Reused near-verbatim from `HubAnchorJudge.sol`
 * and `BetaHub._processRemoteMint` — see those files' own comments for the
 * design reasoning (this facet just inlines them directly against
 * `BetaHubStorage` instead of going through virtual hooks, since a
 * delegatecall split has no use for the "swap `Params` source" flexibility
 * those hooks existed for).
 */
contract BetaHubAnchorFacet is BetaHubStorage {
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
        (bytes32 txidLE, , uint8 kind, bytes32 stmtHash) =
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
            _processRemoteMint(partyId, party, pa, txidLE, statement);
        } else if (kind == KIND_VETO) {
            if (party.kind != PartyKind.Auditor) revert NotAuditor();
            if (party.dead) revert PartyDead();
            _processVeto(partyId, party, pa, statement);
        } else if (kind == KIND_ATTEST || kind == KIND_CLEAR) {
            if (party.dead) revert PartyDead();
            if (statement.length != 33) revert MalformedStatement();
            _processAttestOrClear(partyId, pa, kind, bytes32(statement[1:33]));
        } else if (kind == KIND_ALIVE) {
            if (party.dead) revert PartyDead();
            if (statement.length != 33) revert MalformedStatement();
            Party storage target = parties[bytes32(statement[1:33])];
            if (!target.exists) revert NoParty();
            if (target.dead) {
                (, uint256 remainder) = _slash(partyId, party, params.vetoSlashWei, msg.sender);
                insurance += remainder;
                pa.status = AnchorStatus.Slashed;
            } else {
                pa.status = AnchorStatus.Exercised;
            }
        } else if (kind == KIND_RELEASE || kind == KIND_CANCEL) {
            revert UnsupportedKind();
        } else {
            revert BadAnchorPayload();
        }
        emit AnchorProcessed(partyId, txidLE, kind, pa.status);
    }

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

    /// @dev Translated field-for-field from `beta-factory::process_anchor`'s
    /// verified `Statement::Mint` branch — see `BetaHub._processRemoteMint`'s
    /// own comment.
    function _processRemoteMint(bytes32 partyId, Party storage party, ProcessedAnchor storage pa, bytes32 txidLE, bytes calldata s) internal {
        if (s.length != 74) revert MalformedStatement();
        uint64 compositionId = uint64(bytes8(s[1:9]));
        uint8 componentIndex = uint8(s[9]);
        uint64 lockId = uint64(bytes8(s[10:18]));
        address user = address(uint160(uint256(bytes32(s[18:50]))));
        uint64 nonce = uint64(bytes8(s[50:58]));
        uint64 units = uint64(bytes8(s[58:66]));
        uint64 deadline = uint64(bytes8(s[66:74]));

        pa.compositionId = compositionId;
        pa.componentIndex = componentIndex;
        pa.lockId = lockId;
        pa.units = units;

        bytes32 pendingId = _pendingKey(user, nonce);
        Pending storage p = pending[pendingId];
        pa.pendingId = pendingId;

        bool takeover = p.queuedBy != bytes32(0) && p.queuedBy != partyId && parties[p.queuedBy].dead;
        bool claimable = p.queuedBy == bytes32(0) || p.queuedBy == partyId || takeover;
        bool matches = p.user != address(0) && p.approved && p.compositionId == compositionId
            && componentIndex < p.remoteLockId.length && p.remoteLockId[componentIndex] == lockId
            && p.units == units && p.deadline == deadline && claimable
            && (takeover || p.remoteAnchorTxid[componentIndex] == bytes32(0));

        if (!matches) {
            uint256 amount = uint256(units) * params.slashWeiPerUnit;
            (, uint256 remainder) = _slash(partyId, party, amount, msg.sender);
            if (p.user != address(0)) {
                _pay(p.user, remainder);
            } else {
                insurance += remainder;
            }
            pa.status = AnchorStatus.Slashed;
            return;
        }

        if (p.queuedBy != partyId) {
            for (uint256 i = 0; i < p.remoteAnchorTxid.length; i++) {
                p.remoteAnchorTxid[i] = bytes32(0);
            }
            p.queuedBy = partyId;
        }
        p.remoteAnchorTxid[componentIndex] = txidLE;
        pa.challengeUntil = uint64(block.timestamp + params.tChallengeSecs);
        pa.status = AnchorStatus.Queued;
    }

    function _processVeto(bytes32 partyId, Party storage party, ProcessedAnchor storage pa, bytes calldata s) internal {
        if (s.length != 65) revert MalformedStatement();
        bytes32 targetPartyId = bytes32(s[1:33]);
        bytes32 targetTxid = bytes32(s[33:65]);
        Party storage target = parties[targetPartyId];
        if (!target.exists) revert NoParty();
        if (targetTxid == bytes32(0)) {
            if (target.dead) {
                uint256 reward = params.vetoRewardWei < rewardPool ? params.vetoRewardWei : rewardPool;
                rewardPool -= reward;
                _pay(party.owner, reward);
                pa.status = AnchorStatus.Exercised;
            } else {
                (, uint256 remainder) = _slash(partyId, party, params.vetoSlashWei, msg.sender);
                insurance += remainder;
                pa.status = AnchorStatus.Slashed;
            }
            return;
        }
        ProcessedAnchor storage ta = anchors[targetTxid];
        if (ta.status == AnchorStatus.None) revert TargetNotProcessed();
        if (ta.partyId != targetPartyId) revert BadAnchorState();
        if (ta.kind != KIND_MINT) revert BadAnchorState();
        if (ta.status == AnchorStatus.Queued) {
            if (ta.settled) revert BadAnchorState();
            ta.held = true;
            pa.status = AnchorStatus.Exercised;
            emit ComponentHeld(targetTxid, true);
            return;
        }
        if (ta.status == AnchorStatus.Slashed) {
            uint256 reward = params.vetoRewardWei < rewardPool ? params.vetoRewardWei : rewardPool;
            rewardPool -= reward;
            _pay(party.owner, reward);
            pa.status = AnchorStatus.Exercised;
        } else {
            (, uint256 remainder) = _slash(partyId, party, params.vetoSlashWei, msg.sender);
            insurance += remainder;
            pa.status = AnchorStatus.Slashed;
        }
    }

    function _processAttestOrClear(bytes32 partyId, ProcessedAnchor storage pa, uint8 kind, bytes32 target) internal {
        ProcessedAnchor storage ta = anchors[target];
        if (ta.status == AnchorStatus.None) {
            if (kind == KIND_ATTEST && mintAttester[target] == bytes32(0)) mintAttester[target] = partyId;
            pa.status = AnchorStatus.Exercised;
            return;
        }
        if (ta.kind != KIND_MINT) revert BadAnchorState();
        if (kind == KIND_CLEAR) {
            if (!ta.held) revert BadAnchorState();
            ta.held = false;
            pa.status = AnchorStatus.Exercised;
            emit ComponentHeld(target, false);
            return;
        }
        if (ta.status == AnchorStatus.Slashed) {
            bytes32 att = mintAttester[target];
            if (att != bytes32(0) && parties[att].exists && !parties[att].dead) {
                (, uint256 remainder) = _slash(att, parties[att], uint256(ta.units) * params.slashWeiPerUnit, msg.sender);
                insurance += remainder;
            }
            pa.status = AnchorStatus.Slashed;
        } else {
            if (mintAttester[target] == bytes32(0)) mintAttester[target] = partyId;
            pa.status = AnchorStatus.Exercised;
        }
    }

    function _verifyAndAdvance(
        Party storage party,
        bytes calldata txRaw,
        uint256 blockHeight,
        bytes32[] calldata branchLE,
        uint256 index,
        bool requirePayload
    ) internal returns (bytes32 txidLE, uint32 headerTs, uint8 kind, bytes32 stmtHash) {
        txidLE = AnchorChainLib.txidOf(txRaw);
        if (anchors[txidLE].status != AnchorStatus.None) revert AlreadyProcessed();

        IIPoWHeadersView headers = ipowHeaders;
        bytes32 headerHashLE = headers.globalHeightToHashLE(blockHeight);
        (, bytes32 merkleRootLE, , uint32 ts, bool set, ) = headers.globalHeaders(headerHashLE);
        if (!set) revert InvalidHeader();
        headerTs = ts;
        bytes32 current = txidLE;
        uint256 idx = index;
        for (uint256 i = 0; i < branchLE.length; i++) {
            if (idx % 2 == 0) {
                current = sha256(abi.encodePacked(sha256(abi.encodePacked(current, branchLE[i]))));
            } else {
                current = sha256(abi.encodePacked(sha256(abi.encodePacked(branchLE[i], current))));
            }
            idx /= 2;
        }
        if (current != merkleRootLE) revert InvalidMerkleBranch();

        (bytes32 in0Txid, uint32 in0Vout, bool hasPayload, uint8 k, bytes32 h) = AnchorChainLib.parseAnchor(txRaw);
        if (in0Txid != party.anchorTxidLE || in0Vout != party.anchorVout) revert NotOnStatementChain();
        if (requirePayload && !hasPayload) revert BadAnchorPayload();
        party.anchorTxidLE = txidLE;
        party.anchorVout = 0;
        party.seq += 1;
        kind = hasPayload ? k : 0;
        stmtHash = h;
    }
}
