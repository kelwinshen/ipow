// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

import {ReentrancyGuard} from "@openzeppelin/contracts/utils/ReentrancyGuard.sol";

/// @dev Same read-only view into `iPoWV1`'s header relay that
/// `iPoWV1Conversion` uses.
interface IIPoWV1HeadersView {
    function globalTipHeight() external view returns (uint256);
    function globalHeightToHashLE(uint256 height) external view returns (bytes32);
    function globalHeaders(bytes32 hashLE)
        external
        view
        returns (bytes32 prevHashLE, bytes32 merkleRootLE, uint32 nBits, uint32 timestamp, bool set, uint64 arrivalTime);
}

/// @title BetaVault — the Ethereum half of BETA v2 (docs/DESIGN_V2.md §6)
/// @notice Holds the ETH backing BETA minted on Solana by `beta-factory`.
/// Cross-chain claims arrive as Bitcoin transactions on each registered
/// party's statement chain. This contract judges the claims that are about
/// Ethereum facts (does `lock[N]` exist and match? is it FINAL?) and slashes
/// the ETH bonds it holds; it acts provisionally — delayed, rate-limited,
/// vetoable — on RELEASE, whose burn half is judged on Solana.
///
/// Statement encodings and the anchor layout are byte-identical to the
/// Solana program's (`beta-factory/src/statement.rs`, `anchor_verify.rs`):
///   OP_RETURN payload = ver(1)=1 | kind(1) | sha256(statement)(32)
///   MINT    = 0x01 | lockId u64 | solUser 32 | nonce u64 | units u64 | deadline i64
///   RELEASE = 0x02 | lockId u64 | burnId u64 | to 20 | units u64
///   VETO    = 0x03 | targetPartyId 32 | targetTxidLE 32 (zero = dead-veto)
///   CANCEL  = 0x04 | lockId u64
contract BetaVault is ReentrancyGuard {
    // ------------------------------------------------------------ errors
    error Unauthorized();
    error Paused();
    error InvalidParams();
    error BondTooSmall();
    error PartyDead();
    error PartyExists();
    error NoParty();
    error NotOperator();
    error NotAuditor();
    error NotApproved();
    error UnbondNotRequested();
    error UnbondNotReady();
    error BadLockState();
    error RefundNotReady();
    error TxidMismatch();
    error InvalidHeader();
    error InvalidMerkleBranch();
    error MalformedTx();
    error WitnessSerialization();
    error NotOnStatementChain();
    error BadAnchorPayload();
    error KindMismatch();
    error StatementHashMismatch();
    error MalformedStatement();
    error AlreadyProcessed();
    error BadAnchorState();
    error Held();
    error NotReady();
    error RateLimited();
    error SkipNotReady();
    error TargetNotProcessed();
    error TransferFailed();

    // ------------------------------------------------------------ types
    uint8 public constant KIND_MINT = 1;
    uint8 public constant KIND_RELEASE = 2;
    uint8 public constant KIND_VETO = 3;
    uint8 public constant KIND_CANCEL = 4;
    uint8 public constant KIND_ATTEST = 5;
    uint8 public constant KIND_CLEAR = 6;
    uint8 public constant KIND_ALIVE = 7;
    uint8 public constant ANCHOR_VERSION = 1;
    uint256 public constant BPS_DENOM = 10_000;

    struct Params {
        uint256 ethWeiPerUnit;
        /// A MINT anchor only finalizes a lock if processed within this
        /// many seconds of its Bitcoin block time (Solana's `t_skip` is
        /// set to twice this, so an anchor is either live on both chains
        /// or dead on both).
        uint64 tFinSecs;
        /// v3 challenge window (DESIGN_V2 §7): a queued RELEASE pays early
        /// only on an ATTEST (from the attester's escrow), otherwise after
        /// this long if no VETO stands; escrows settle at its end.
        uint64 tChallengeSecs;
        uint64 tSkipSecs;
        uint64 refundMarginSecs;
        uint64 unbondDelaySecs;
        uint256 minOperatorBond;
        uint256 minAuditorBond;
        uint256 vetoSlashWei;
        uint256 vetoRewardWei;
        uint16 bountyBps;
    }

    enum PartyKind {
        Operator,
        Auditor
    }

    struct Party {
        bool exists;
        address owner;
        PartyKind kind;
        bytes32 anchorTxidLE;
        uint32 anchorVout;
        uint64 seq;
        uint256 bond;
        bool dead;
        uint64 unbondRequestedAt;
    }

    enum LockState {
        None,
        Pending,
        Final,
        Released,
        Refunded
    }

    struct Lock {
        bytes32 solUser;
        uint64 nonce;
        uint64 units;
        uint64 deadline;
        LockState state;
        address depositor;
    }

    enum AnchorStatus {
        None,
        Exercised,
        Slashed,
        Skipped,
        QueuedRelease,
        /// v3: a queued RELEASE still held at the end of its window — never paid by the vault.
        Cancelled
    }

    struct ProcessedAnchor {
        bytes32 partyId;
        uint8 kind;
        AnchorStatus status;
        bytes32 statementHash;
        uint64 blockHeight;
        uint64 processedAt;
        // RELEASE
        uint64 lockId;
        address to;
        uint64 units;
        /// v3: end of the challenge window.
        uint64 challengeUntil;
        /// v3: a VETO stands (lifted only by a CLEAR).
        bool held;
        /// v3: party whose ATTEST covers this anchor (zero = none); for a
        /// RELEASE, `paid` means the user was paid from that party's escrow.
        bytes32 attester;
        uint256 escrow;
        bool paid;
        bool settled;
    }

    // ------------------------------------------------------------ state
    IIPoWV1HeadersView public immutable ipowHeaders;
    address public governance;
    Params public params;
    bool public paused;

    mapping(bytes32 => Party) public parties;
    mapping(bytes32 => address) public approvedOperators;
    mapping(uint64 => Lock) public locks;
    uint64 public nextLockId = 1;
    mapping(bytes32 => ProcessedAnchor) public anchors;
    /// v3: ATTESTs of MINT anchors recorded here (they may arrive before the
    /// MINT itself); a false MINT slashes its recorded attester too.
    mapping(bytes32 => bytes32) public mintAttester;

    uint256 public insurance;
    /// @dev Insurance already promised to queued insurance-releases.
    uint256 public insuranceReserved;
    /// @dev Governance-funded pool that pays veto rewards, kept apart from
    /// insurance so rewards never eat the backing of units minted against lies.
    uint256 public rewardPool;
    uint256 public totalBonds;
    uint256 public totalLocked;

    // ------------------------------------------------------------ events
    event Deposited(uint64 indexed lockId, bytes32 indexed solUser, uint64 nonce, uint64 units, uint64 deadline, address depositor);
    event Finalized(uint64 indexed lockId, bytes32 anchorTxidLE);
    event Refunded(uint64 indexed lockId);
    event PartyRegistered(bytes32 indexed partyId, PartyKind kind, address owner, uint256 bond);
    event AnchorProcessed(bytes32 indexed partyId, bytes32 indexed txidLE, uint8 kind, AnchorStatus status);
    event Slashed(bytes32 indexed partyId, uint256 amount, address submitter);
    event ReleaseQueued(bytes32 indexed txidLE, uint64 indexed lockId, address to, uint64 units, uint64 releaseAfter);
    event ReleaseHeld(bytes32 indexed txidLE, bool held);
    event ReleasePaidFromEscrow(bytes32 indexed txidLE, bytes32 indexed attester, address to, uint256 wei_);
    event ReleaseSettled(bytes32 indexed txidLE, bool reimbursed);
    event Released(bytes32 indexed txidLE, uint64 indexed lockId, address to, uint256 wei_);

    modifier onlyGovernance() {
        if (msg.sender != governance) revert Unauthorized();
        _;
    }

    constructor(address _governance, address _ipowHeaders, Params memory _params) {
        if (_governance == address(0) || _ipowHeaders == address(0)) revert InvalidParams();
        _validate(_params);
        governance = _governance;
        ipowHeaders = IIPoWV1HeadersView(_ipowHeaders);
        params = _params;
    }

    function _validate(Params memory p) internal pure {
        if (p.ethWeiPerUnit == 0 || p.tFinSecs == 0 || p.tChallengeSecs == 0) revert InvalidParams();
        if (p.tSkipSecs < 2 * p.tFinSecs || p.unbondDelaySecs == 0 || p.bountyBps > BPS_DENOM) revert InvalidParams();
    }

    // ------------------------------------------------------------ governance
    function setParams(Params calldata p, bool _paused) external onlyGovernance {
        _validate(p);
        params = p;
        paused = _paused;
    }

    /// @notice Governance pre-approves who may register as an operator.
    function approveOperator(bytes32 partyId, address owner) external onlyGovernance {
        approvedOperators[partyId] = owner;
    }

    // ------------------------------------------------------------ parties
    function registerParty(bytes32 partyId, PartyKind kind, bytes32 anchorTxidLE, uint32 anchorVout)
        external
        payable
    {
        if (parties[partyId].exists) revert PartyExists();
        if (kind == PartyKind.Operator) {
            if (approvedOperators[partyId] != msg.sender) revert NotApproved();
            if (msg.value < params.minOperatorBond) revert BondTooSmall();
        } else {
            if (msg.value < params.minAuditorBond) revert BondTooSmall();
        }
        parties[partyId] = Party({
            exists: true,
            owner: msg.sender,
            kind: kind,
            anchorTxidLE: anchorTxidLE,
            anchorVout: anchorVout,
            seq: 0,
            bond: msg.value,
            dead: false,
            unbondRequestedAt: 0
        });
        totalBonds += msg.value;
        emit PartyRegistered(partyId, kind, msg.sender, msg.value);
    }

    function topUpBond(bytes32 partyId) external payable {
        Party storage p = parties[partyId];
        if (!p.exists || p.owner != msg.sender) revert Unauthorized();
        p.bond += msg.value;
        totalBonds += msg.value;
    }

    function requestUnbond(bytes32 partyId) external {
        Party storage p = parties[partyId];
        if (!p.exists || p.owner != msg.sender) revert Unauthorized();
        if (p.dead) revert PartyDead();
        p.unbondRequestedAt = uint64(block.timestamp);
    }

    function withdrawBond(bytes32 partyId) external nonReentrant {
        Party storage p = parties[partyId];
        if (!p.exists || p.owner != msg.sender) revert Unauthorized();
        if (p.dead) revert PartyDead();
        if (p.unbondRequestedAt == 0) revert UnbondNotRequested();
        if (block.timestamp < p.unbondRequestedAt + params.unbondDelaySecs) revert UnbondNotReady();
        uint256 amount = p.bond;
        p.bond = 0;
        p.dead = true;
        totalBonds -= amount;
        _pay(msg.sender, amount);
    }

    // ------------------------------------------------------------ user side
    /// @notice Lock the ETH half of `units` BETA for Solana user `solUser`'s
    /// pending lock `nonce`. Refundable after `deadline + refundMargin` if
    /// never finalized.
    function deposit(bytes32 solUser, uint64 nonce, uint64 units, uint64 deadline)
        external
        payable
        returns (uint64 lockId)
    {
        if (paused) revert Paused();
        if (units == 0 || deadline <= block.timestamp) revert InvalidParams();
        if (msg.value != uint256(units) * params.ethWeiPerUnit) revert InvalidParams();
        lockId = nextLockId++;
        locks[lockId] = Lock({
            solUser: solUser,
            nonce: nonce,
            units: units,
            deadline: deadline,
            state: LockState.Pending,
            depositor: msg.sender
        });
        totalLocked += msg.value;
        emit Deposited(lockId, solUser, nonce, units, deadline, msg.sender);
    }

    function refund(uint64 lockId) external nonReentrant {
        Lock storage l = locks[lockId];
        if (l.state != LockState.Pending) revert BadLockState();
        if (block.timestamp <= uint256(l.deadline) + params.refundMarginSecs) revert RefundNotReady();
        _refund(lockId, l);
    }

    function _refund(uint64 lockId, Lock storage l) internal {
        l.state = LockState.Refunded;
        uint256 amount = uint256(l.units) * params.ethWeiPerUnit;
        totalLocked -= amount;
        _pay(l.depositor, amount);
        emit Refunded(lockId);
    }

    // ------------------------------------------------------------ anchors
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

    function _processMint(
        bytes32 partyId,
        Party storage party,
        ProcessedAnchor storage pa,
        bytes32 txidLE,
        bytes calldata s,
        uint32 headerTs
    ) internal {
        if (s.length != 65) revert MalformedStatement();
        uint64 lockId = uint64(bytes8(s[1:9]));
        bytes32 solUser = bytes32(s[9:41]);
        uint64 nonce = uint64(bytes8(s[41:49]));
        uint64 units = uint64(bytes8(s[49:57]));
        uint64 deadline = uint64(bytes8(s[57:65]));
        Lock storage l = locks[lockId];
        pa.units = units;
        pa.lockId = lockId;
        bool matches = l.state != LockState.None && l.solUser == solUser && l.nonce == nonce && l.units == units
            && l.deadline == deadline;
        if (!matches) {
            // A lie about Ethereum: slash in ETH, to insurance — the operator
            // and (v3) whoever attested to it.
            uint256 amount = uint256(units) * params.ethWeiPerUnit;
            _slash(partyId, party, amount, msg.sender);
            bytes32 att = mintAttester[txidLE];
            if (att != bytes32(0) && parties[att].exists && !parties[att].dead) {
                _slash(att, parties[att], amount, msg.sender);
            }
            pa.status = AnchorStatus.Slashed;
            return;
        }
        // True statement. Finalize only if still Pending, anchored before
        // the deadline, and revealed within the finalize window.
        if (
            l.state == LockState.Pending && headerTs < deadline
                && block.timestamp <= uint256(headerTs) + params.tFinSecs
        ) {
            l.state = LockState.Final;
            emit Finalized(lockId, txidLE);
        }
        pa.status = AnchorStatus.Exercised;
    }

    function _processRelease(
        bytes32 partyId,
        Party storage party,
        ProcessedAnchor storage pa,
        bytes32 txidLE,
        bytes calldata s
    ) internal {
        if (s.length != 45) revert MalformedStatement();
        uint64 lockId = uint64(bytes8(s[1:9]));
        address to = address(bytes20(s[17:37]));
        uint64 units = uint64(bytes8(s[37:45]));
        uint256 amount = uint256(units) * params.ethWeiPerUnit;
        if (lockId == 0) {
            // Insurance release (§6.6): redeems a unit that no FINAL lock
            // backs — one minted against a lie, whose slashed bond now sits
            // in `insurance`. The Ethereum half of the predicate is simply
            // "insurance can cover it"; the burn is judged on Solana.
            if (insurance < insuranceReserved + amount) {
                _slash(partyId, party, amount, msg.sender);
                pa.status = AnchorStatus.Slashed;
                return;
            }
            insuranceReserved += amount;
        } else {
            Lock storage l = locks[lockId];
            // The Ethereum half of the predicate: a FINAL, unreleased lock of this size.
            if (l.state != LockState.Final || l.units != units) {
                _slash(partyId, party, amount, msg.sender);
                pa.status = AnchorStatus.Slashed;
                return;
            }
            // The Solana half (the burn) is judged on Solana; act provisionally.
            l.state = LockState.Released; // reserved for this release; paid by executeRelease
        }
        pa.lockId = lockId;
        pa.to = to;
        pa.units = units;
        pa.challengeUntil = uint64(block.timestamp + params.tChallengeSecs);
        pa.status = AnchorStatus.QueuedRelease;
        emit ReleaseQueued(txidLE, lockId, to, units, pa.challengeUntil);
    }

    function _processVeto(bytes32 partyId, Party storage party, ProcessedAnchor storage pa, bytes calldata s) internal {
        if (s.length != 65) revert MalformedStatement();
        bytes32 targetPartyId = bytes32(s[1:33]);
        bytes32 targetTxid = bytes32(s[33:65]);
        Party storage target = parties[targetPartyId];
        if (!target.exists) revert NoParty();
        if (targetTxid == bytes32(0)) {
            // Dead-veto: judged here — "is the operator dead on Ethereum?"
            if (target.dead) {
                uint256 reward = params.vetoRewardWei < rewardPool ? params.vetoRewardWei : rewardPool;
                rewardPool -= reward;
                _pay(party.owner, reward);
                pa.status = AnchorStatus.Exercised;
            } else {
                _slash(partyId, party, params.vetoSlashWei, msg.sender);
                pa.status = AnchorStatus.Slashed;
            }
            return;
        }
        ProcessedAnchor storage ta = anchors[targetTxid];
        if (ta.status == AnchorStatus.None) revert TargetNotProcessed();
        if (ta.partyId != targetPartyId) revert BadAnchorState();
        if (ta.kind == KIND_RELEASE && ta.status == AnchorStatus.QueuedRelease) {
            // Judged on Solana (the burn). Here: hold, with no expiry (§7.4).
            if (ta.settled) revert BadAnchorState();
            ta.held = true;
            pa.status = AnchorStatus.Exercised;
            emit ReleaseHeld(targetTxid, true);
            return;
        }
        if (ta.kind == KIND_MINT) {
            // Judged here: the MINT's own verdict is already on record.
            if (ta.status == AnchorStatus.Slashed) {
                uint256 reward = params.vetoRewardWei < rewardPool ? params.vetoRewardWei : rewardPool;
                rewardPool -= reward;
                _pay(party.owner, reward);
                pa.status = AnchorStatus.Exercised;
            } else {
                _slash(partyId, party, params.vetoSlashWei, msg.sender);
                pa.status = AnchorStatus.Slashed;
            }
            return;
        }
        revert BadAnchorState();
    }

    /// @notice Permissionless. v3 slow path: pays a queued, unattested
    /// release from the vault once its challenge window has closed with no
    /// veto standing (§7.4).
    function executeRelease(bytes32 txidLE) external nonReentrant {
        ProcessedAnchor storage pa = anchors[txidLE];
        if (pa.status != AnchorStatus.QueuedRelease) revert BadAnchorState();
        if (pa.paid) revert BadAnchorState();
        if (block.timestamp < pa.challengeUntil) revert NotReady();
        if (pa.held) revert Held();
        if (paused) revert Paused();
        Party storage party = parties[pa.partyId];
        if (party.dead) revert PartyDead();
        uint256 amount = uint256(pa.units) * params.ethWeiPerUnit;
        _drawVault(pa.lockId, amount);
        pa.status = AnchorStatus.Exercised;
        pa.settled = true;
        _pay(pa.to, amount);
        emit Released(txidLE, pa.lockId, pa.to, amount);
    }

    /// @notice Permissionless. Settles a release once its window has closed
    /// (§7.4). Paid-from-escrow and not held → the vault reimburses the
    /// attester (credited to its bond). Held → reimbursement is cancelled:
    /// the lock returns to FINAL (or the insurance reservation is released)
    /// and the anchor is Cancelled; the vault never paid.
    function settleRelease(bytes32 txidLE) external nonReentrant {
        ProcessedAnchor storage pa = anchors[txidLE];
        if (pa.status != AnchorStatus.QueuedRelease || pa.settled) revert BadAnchorState();
        if (block.timestamp < pa.challengeUntil) revert NotReady();
        uint256 amount = uint256(pa.units) * params.ethWeiPerUnit;
        pa.settled = true;
        if (pa.held) {
            if (pa.lockId == 0) insuranceReserved -= amount;
            else locks[pa.lockId].state = LockState.Final;
            pa.status = AnchorStatus.Cancelled;
            emit ReleaseSettled(txidLE, false);
            return;
        }
        if (pa.paid) {
            _drawVault(pa.lockId, amount);
            Party storage att = parties[pa.attester];
            att.bond += amount;
            totalBonds += amount;
            pa.status = AnchorStatus.Exercised;
            emit ReleaseSettled(txidLE, true);
        }
    }

    function _drawVault(uint64 lockId, uint256 amount) internal {
        if (lockId == 0) {
            insuranceReserved -= amount;
            insurance -= amount;
        } else {
            totalLocked -= amount;
        }
    }

    /// @dev ATTEST / CLEAR on a target anchor (§7.2). RELEASE targets act
    /// here (pay from escrow / lift hold); MINT targets are judged here
    /// against the MINT's own verdict and recorded for fan-out slashing.
    function _processAttestOrClear(bytes32 partyId, Party storage party, ProcessedAnchor storage pa, uint8 kind, bytes32 target)
        internal
    {
        ProcessedAnchor storage ta = anchors[target];
        if (ta.status == AnchorStatus.None) {
            // The MINT may not be on Ethereum yet: record the FIRST attester
            // for later fan-out — a later attest of the same target must
            // never overwrite who's actually on the hook (found live
            // 2026-09-22: without this guard, whoever attested *last* took
            // sole responsibility, letting every earlier attester off).
            if (kind == KIND_ATTEST && mintAttester[target] == bytes32(0)) mintAttester[target] = partyId;
            pa.status = AnchorStatus.Exercised;
            return;
        }
        if (ta.kind == KIND_MINT) {
            if (ta.status == AnchorStatus.Slashed) {
                _slash(partyId, party, uint256(ta.units) * params.ethWeiPerUnit, msg.sender);
                pa.status = AnchorStatus.Slashed;
            } else {
                if (kind == KIND_ATTEST && mintAttester[target] == bytes32(0)) mintAttester[target] = partyId;
                pa.status = AnchorStatus.Exercised;
            }
            return;
        }
        if (ta.kind == KIND_RELEASE && ta.status == AnchorStatus.QueuedRelease && !ta.settled) {
            if (kind == KIND_CLEAR) {
                ta.held = false;
                pa.status = AnchorStatus.Exercised;
                emit ReleaseHeld(target, false);
                return;
            }
            if (ta.paid || ta.held) revert BadAnchorState();
            if (block.timestamp >= ta.challengeUntil) revert BadAnchorState();
            uint256 amount = uint256(ta.units) * params.ethWeiPerUnit;
            if (party.bond < amount) revert BondTooSmall();
            party.bond -= amount;
            totalBonds -= amount;
            ta.attester = partyId;
            ta.escrow = amount;
            ta.paid = true;
            pa.status = AnchorStatus.Exercised;
            _pay(ta.to, amount);
            emit ReleasePaidFromEscrow(target, partyId, ta.to, amount);
            return;
        }
        revert BadAnchorState();
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

    // ------------------------------------------------------------ internals
    function _slash(bytes32 partyId, Party storage party, uint256 amount, address submitter) internal {
        uint256 slashed = amount < party.bond ? amount : party.bond;
        party.bond -= slashed;
        party.dead = true;
        totalBonds -= slashed;
        uint256 bounty = (slashed * params.bountyBps) / BPS_DENOM;
        insurance += slashed - bounty;
        emit Slashed(partyId, slashed, submitter);
        _pay(submitter, bounty);
    }

    function _pay(address to, uint256 amount) internal {
        if (amount == 0) return;
        (bool ok,) = payable(to).call{value: amount}("");
        if (!ok) revert TransferFailed();
    }

    /// @dev txid, inclusion, statement-chain membership; advances the party's
    /// pointer. Returns the header timestamp and the parsed payload (kind 0
    /// = no well-formed payload, only allowed when `requirePayload` is false).
    function _verifyAndAdvance(
        Party storage party,
        bytes calldata txRaw,
        uint256 blockHeight,
        bytes32[] calldata branchLE,
        uint256 index,
        bool requirePayload
    ) internal returns (bytes32 txidLE, uint32 headerTs, uint8 kind, bytes32 stmtHash) {
        txidLE = sha256(abi.encodePacked(sha256(txRaw)));
        if (anchors[txidLE].status != AnchorStatus.None) revert AlreadyProcessed();

        bytes32 headerHashLE = ipowHeaders.globalHeightToHashLE(blockHeight);
        (, bytes32 merkleRootLE, , uint32 ts, bool set, ) = ipowHeaders.globalHeaders(headerHashLE);
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

        (bytes32 in0Txid, uint32 in0Vout, bool hasPayload, uint8 k, bytes32 h) = _parseAnchor(txRaw);
        if (in0Txid != party.anchorTxidLE || in0Vout != party.anchorVout) revert NotOnStatementChain();
        if (requirePayload && !hasPayload) revert BadAnchorPayload();
        party.anchorTxidLE = txidLE;
        party.anchorVout = 0;
        party.seq += 1;
        kind = hasPayload ? k : 0;
        stmtHash = h;
    }

    function _readVarInt(bytes calldata b, uint256 o) internal pure returns (uint64 v, uint256 next) {
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
    function _parseAnchor(bytes calldata tx_)
        internal
        pure
        returns (bytes32 in0Txid, uint32 in0Vout, bool hasPayload, uint8 kind, bytes32 stmtHash)
    {
        if (tx_.length < 4 + 1 + 36 + 1 + 4 + 1) revert MalformedTx();
        uint256 o = 4;
        if (tx_[o] == 0x00 && tx_[o + 1] == 0x01) revert WitnessSerialization();
        (uint64 inCount, uint256 n) = _readVarInt(tx_, o);
        o = n;
        if (inCount == 0 || o + 36 > tx_.length) revert MalformedTx();
        in0Txid = bytes32(tx_[o:o + 32]);
        in0Vout = uint32(uint8(tx_[o + 32])) | (uint32(uint8(tx_[o + 33])) << 8) | (uint32(uint8(tx_[o + 34])) << 16)
            | (uint32(uint8(tx_[o + 35])) << 24);
        for (uint64 i = 0; i < inCount; i++) {
            o += 36;
            (uint64 slen, uint256 n2) = _readVarInt(tx_, o);
            o = n2 + slen + 4;
            if (o > tx_.length) revert MalformedTx();
        }
        (uint64 outCount, uint256 n3) = _readVarInt(tx_, o);
        o = n3;
        if (outCount == 0) revert MalformedTx();
        for (uint64 j = 0; j < outCount; j++) {
            if (o + 8 > tx_.length) revert MalformedTx();
            o += 8;
            (uint64 slen, uint256 n4) = _readVarInt(tx_, o);
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

    /// @notice Top up the veto-reward pool (anyone; in practice governance).
    function fundRewards() external payable {
        rewardPool += msg.value;
    }

    /// @notice Plain transfers top up insurance.
    receive() external payable {
        insurance += msg.value;
    }
}
