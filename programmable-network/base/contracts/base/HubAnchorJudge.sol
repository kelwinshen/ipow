// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

import {HubPartyRegistry} from "./HubPartyRegistry.sol";
import {AnchorChainLib, IIPoWHeadersView} from "../libraries/AnchorChainLib.sol";

/// @title HubAnchorJudge
/// @notice Statement-chain anchor judging for a `BetaHub` deployment
/// (design/ipow-implementation.md §8.18). `KIND_VETO`/`KIND_ATTEST`/`KIND_CLEAR`/`KIND_ALIVE`
/// are reused near-verbatim from `BetaVault.sol`'s own handlers (lines
/// 557-711) — they only ever touch `Party`/`ProcessedAnchor`, so retargeting
/// them at a queued MINT *component* instead of a queued *release* needs no
/// change to their actual logic, only to which `AnchorStatus` value means
/// "still pending." `_processRemoteMint` is the one genuinely new predicate.
/// `KIND_RELEASE`/`KIND_CANCEL` are not implemented this pass — redeem/burn
/// generalization for the hub is out of scope, matching Solana's own §8.6
/// design-only status; both revert `UnsupportedKind`, fail-closed.
///
/// The statement-chain verification orchestration (`processAnchor`'s replay
/// guard, header-relay lookup, Merkle-branch walk, party-pointer advance) is
/// copied here directly from `BetaVault._verifyAndAdvance` (adjusted to use
/// this `Party` struct, which lives in `HubPartyRegistry`) rather than
/// shared via `AnchorChainLib`, which holds only pure byte-parsing
/// functions — see that library's header comment for why a storage-sharing
/// version of this was tried and reverted.
abstract contract HubAnchorJudge is HubPartyRegistry {
    error MalformedStatement();
    error KindMismatch();
    error StatementHashMismatch();
    error NotOperator();
    error NotAuditor();
    error NoParty();
    error BadAnchorState();
    error TargetNotProcessed();
    error SkipNotReady();
    error UnsupportedKind();

    uint8 public constant KIND_MINT = 1;
    uint8 public constant KIND_RELEASE = 2;
    uint8 public constant KIND_VETO = 3;
    uint8 public constant KIND_CANCEL = 4;
    uint8 public constant KIND_ATTEST = 5;
    uint8 public constant KIND_CLEAR = 6;
    uint8 public constant KIND_ALIVE = 7;

    enum AnchorStatus {
        None,
        Queued,
        Exercised,
        Slashed,
        Skipped,
        Cancelled
    }

    struct ProcessedAnchor {
        bytes32 partyId;
        uint8 kind;
        AnchorStatus status;
        bytes32 statementHash;
        uint64 blockHeight;
        uint64 processedAt;
        // MINT-component fields, cached for exerciseMint's re-verify.
        uint64 compositionId;
        uint8 componentIndex;
        uint64 lockId;
        uint64 units;
        uint64 challengeUntil;
        bool held;
        bool settled;
        /// @dev Opaque back-reference to the `Pending` this MINT-component
        /// anchor belongs to (a `BetaHub`-defined key `HubAnchorJudge`
        /// doesn't otherwise need to understand) — lets `settleMint` find
        /// and reset the right pending slot when a held component's
        /// challenge window closes.
        bytes32 pendingId;
    }

    mapping(bytes32 => ProcessedAnchor) public anchors;
    /// @dev First attester of an unresolved MINT-component anchor (it may
    /// arrive before the MINT's own anchor is processed on this chain) — a
    /// false MINT fans out to slash its recorded attester too. Only the
    /// FIRST attester is ever recorded (a later attest of the same target
    /// must never overwrite who's actually on the hook) — same fix
    /// `BetaVault.sol` shipped for this exact bug (found live 2026-09-22).
    mapping(bytes32 => bytes32) public mintAttester;
    /// @dev Bookkeeping-only: slash remainders with no identifiable named
    /// victim (wrong-veto, wrong-attest-fan-out, dead-party-ALIVE-veto) land
    /// here rather than being paid out — mirrors `BetaVault.sol`'s own
    /// `insurance` concept. The false-MINT case has a real victim instead
    /// (the pending mint's own user) and is paid there directly — see
    /// `BetaHub._processRemoteMint`.
    uint256 public insurance;

    /// Flat per-veto/dead-party slash amount — mirrors `BetaVault.sol`'s
    /// `params.vetoSlashWei` exactly (a fixed cost, unrelated to any units).
    function _vetoSlashWei() internal view virtual returns (uint256);
    /// Per-unit slash rate for a false MINT-component claim, and for fanning
    /// that same slash out to an attester who vouched for it — mirrors
    /// `BetaVault.sol`'s `params.ethWeiPerUnit` reuse in its own MINT-attest
    /// fan-out (`uint256(ta.units) * params.ethWeiPerUnit`).
    function _slashWeiPerUnit() internal view virtual returns (uint256);
    function _vetoRewardWei() internal view virtual returns (uint256);
    function _tSkipSecs() internal view virtual returns (uint64);
    function _bountyBps() internal view virtual returns (uint16);
    function _bpsDenom() internal view virtual returns (uint256);
    function _rewardPoolSub(uint256 amount) internal virtual;
    function _ipowHeaders() internal view virtual returns (IIPoWHeadersView);

    event AnchorProcessed(bytes32 indexed partyId, bytes32 indexed txidLE, uint8 kind, AnchorStatus status);
    event ComponentHeld(bytes32 indexed txidLE, bool held);

    /// @notice Permissionless. Processes the next anchor on `partyId`'s
    /// statement chain: verifies the Bitcoin tx against the header relay,
    /// advances the pointer, then judges/acts by kind.
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
        if (kind == 0) revert AnchorChainLib.BadAnchorPayload();
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
                (, uint256 remainder) = _slash(partyId, party, _vetoSlashWei(), _bountyBps(), _bpsDenom(), msg.sender, address(0));
                insurance += remainder;
                pa.status = AnchorStatus.Slashed;
            } else {
                pa.status = AnchorStatus.Exercised;
            }
        } else if (kind == KIND_RELEASE || kind == KIND_CANCEL) {
            revert UnsupportedKind();
        } else {
            revert AnchorChainLib.BadAnchorPayload();
        }
        emit AnchorProcessed(partyId, txidLE, kind, pa.status);
    }

    /// @dev The genuinely new predicate — see `BetaHub._processRemoteMint`
    /// (defined on the final contract, since it needs `Pending`, which lives
    /// there). Declared virtual here purely so `processAnchor`'s dispatch
    /// can call it without `HubAnchorJudge` needing to know `Pending`'s shape.
    /// No `headerTs` parameter: unlike `BetaVault._processMint`'s liveness
    /// gate, `beta-factory::process_anchor`'s real `Statement::Mint`
    /// predicate has no timing check at all (verified by reading it) —
    /// timeliness is enforced separately, by `expirePending`'s deadline.
    function _processRemoteMint(bytes32 partyId, Party storage party, ProcessedAnchor storage pa, bytes32 txidLE, bytes calldata s)
        internal
        virtual;

    function _processVeto(bytes32 partyId, Party storage party, ProcessedAnchor storage pa, bytes calldata s) internal {
        if (s.length != 65) revert MalformedStatement();
        bytes32 targetPartyId = bytes32(s[1:33]);
        bytes32 targetTxid = bytes32(s[33:65]);
        Party storage target = parties[targetPartyId];
        if (!target.exists) revert NoParty();
        if (targetTxid == bytes32(0)) {
            // Dead-veto: judged here — "is the operator dead on this chain?"
            if (target.dead) {
                uint256 reward = _vetoRewardWei();
                _rewardPoolSub(reward);
                _pay(party.owner, reward);
                pa.status = AnchorStatus.Exercised;
            } else {
                (, uint256 remainder) = _slash(partyId, party, _vetoSlashWei(), _bountyBps(), _bpsDenom(), msg.sender, address(0));
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
            // Still pending, within its challenge window — hold it, same as
            // `BetaVault._processVeto`'s `QueuedRelease` branch. No expiry;
            // `settleMint` resolves it once the challenge window closes.
            if (ta.settled) revert BadAnchorState();
            ta.held = true;
            pa.status = AnchorStatus.Exercised;
            emit ComponentHeld(targetTxid, true);
            return;
        }
        // Already resolved one way or another — judged here against the
        // MINT's own recorded verdict, same as `BetaVault._processVeto`'s
        // `KIND_MINT` branch.
        if (ta.status == AnchorStatus.Slashed) {
            uint256 reward = _vetoRewardWei();
            _rewardPoolSub(reward);
            _pay(party.owner, reward);
            pa.status = AnchorStatus.Exercised;
        } else {
            (, uint256 remainder) = _slash(partyId, party, _vetoSlashWei(), _bountyBps(), _bpsDenom(), msg.sender, address(0));
            insurance += remainder;
            pa.status = AnchorStatus.Slashed;
        }
    }

    /// @dev ATTEST/CLEAR on a target anchor. The MINT branch is reused
    /// near-verbatim from `BetaVault.sol`'s own KIND_MINT handling (lines
    /// 679-688) — that branch never escrowed or paid anyone even in the
    /// spoke, since it's a fan-out-record/punish mechanism, not a
    /// fast-payout one. (BetaVault's fast-payout ATTEST behavior for
    /// KIND_RELEASE has no equivalent here — a MINT component's actual
    /// payout, the mint itself, only ever happens collectively at
    /// `exerciseMint`, so there's nothing to fast-pay per component.) CLEAR
    /// is new — it lifts a hold placed by `_processVeto`'s `Queued` branch
    /// above, which has no equivalent in `BetaVault`'s own flat MINT (which
    /// never has a "held" MINT at all).
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
        // KIND_ATTEST
        if (ta.status == AnchorStatus.Slashed) {
            bytes32 att = mintAttester[target];
            if (att != bytes32(0) && parties[att].exists && !parties[att].dead) {
                (, uint256 remainder) = _slash(att, parties[att], uint256(ta.units) * _slashWeiPerUnit(), _bountyBps(), _bpsDenom(), msg.sender, address(0));
                insurance += remainder;
            }
            pa.status = AnchorStatus.Slashed;
        } else {
            if (mintAttester[target] == bytes32(0)) mintAttester[target] = partyId;
            pa.status = AnchorStatus.Exercised;
        }
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
        if (block.timestamp < uint256(headerTs) + _tSkipSecs()) revert SkipNotReady();
        ProcessedAnchor storage pa = anchors[txidLE];
        pa.partyId = partyId;
        pa.kind = kind;
        pa.statementHash = stmtHash;
        pa.blockHeight = uint64(blockHeight);
        pa.processedAt = uint64(block.timestamp);
        pa.status = AnchorStatus.Skipped;
        emit AnchorProcessed(partyId, txidLE, kind, AnchorStatus.Skipped);
    }

    /// @dev txid, inclusion, statement-chain membership; advances the
    /// party's pointer. Copied from `BetaVault._verifyAndAdvance`, using
    /// `AnchorChainLib`'s pure `parseAnchor` for the byte-parsing — see this
    /// contract's header comment for why the storage-touching part isn't
    /// itself shared via the library.
    function _verifyAndAdvance(
        Party storage party,
        bytes calldata txRaw,
        uint256 blockHeight,
        bytes32[] calldata branchLE,
        uint256 index,
        bool requirePayload
    ) internal returns (bytes32 txidLE, uint32 headerTs, uint8 kind, bytes32 stmtHash) {
        txidLE = AnchorChainLib.txidOf(txRaw);
        if (anchors[txidLE].status != AnchorStatus.None) revert AnchorChainLib.AlreadyProcessed();

        IIPoWHeadersView ipowHeaders = _ipowHeaders();
        bytes32 headerHashLE = ipowHeaders.globalHeightToHashLE(blockHeight);
        (, bytes32 merkleRootLE, , uint32 ts, bool set, ) = ipowHeaders.globalHeaders(headerHashLE);
        if (!set) revert AnchorChainLib.InvalidHeader();
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
        if (current != merkleRootLE) revert AnchorChainLib.InvalidMerkleBranch();

        (bytes32 in0Txid, uint32 in0Vout, bool hasPayload, uint8 k, bytes32 h) = AnchorChainLib.parseAnchor(txRaw);
        if (in0Txid != party.anchorTxidLE || in0Vout != party.anchorVout) revert AnchorChainLib.NotOnStatementChain();
        if (requirePayload && !hasPayload) revert AnchorChainLib.BadAnchorPayload();
        party.anchorTxidLE = txidLE;
        party.anchorVout = 0;
        party.seq += 1;
        kind = hasPayload ? k : 0;
        stmtHash = h;
    }
}
