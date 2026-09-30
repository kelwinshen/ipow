// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

import {ReentrancyGuard} from "@openzeppelin/contracts/utils/ReentrancyGuard.sol";
import {IERC20} from "@openzeppelin/contracts/token/ERC20/IERC20.sol";
import {SafeERC20} from "@openzeppelin/contracts/token/ERC20/utils/SafeERC20.sol";

/// @dev Same read-only view into `iPoW`'s header relay that
/// `iPoWConversion` uses.
interface IIPoWHeadersView {
    function globalTipHeight() external view returns (uint256);
    function globalHeightToHashLE(uint256 height) external view returns (bytes32);
    function globalHeaders(bytes32 hashLE)
        external
        view
        returns (bytes32 prevHashLE, bytes32 merkleRootLE, uint32 nBits, uint32 timestamp, bool set, uint64 arrivalTime);
}

/// @title BetaVaultPathUSD — Tempo-only `BetaVault` variant (design/ipow-implementation.md §8.21)
/// @notice Confirmed live (2026-09-23): Tempo's custom `0x76` transaction type
/// unconditionally rejects any transaction carrying native `value` — not a
/// nonce/gas quirk like the earlier §8.15 findings, but a hard chain-level
/// rule ("Revm error: value transfer in Tempo Transaction not allowed"),
/// verified against both a plain transfer and a contract call carrying
/// value. Tempo has no native-ETH-style value at all; every real value
/// movement (fees included) goes through a real ERC20, `PathUSD`
/// (`0x20c0000000000000000000000000000000000000`, 6 decimals — the exact
/// token Tempo's own `feeToken` field names on every transaction).
///
/// The plain `BetaVault.sol`'s local-leg ERC20 deposit path (§8.12) already
/// works unmodified on Tempo (`transferFrom`, no `msg.value`) — this
/// variant only changes what plain `BetaVault.sol` gets structurally wrong
/// for Tempo: **bonds** (`registerParty`/`topUpBond`/`withdrawBond`) and
/// every **internal payout** (`_pay`, used by slashing, veto rewards,
/// escrow payouts, and the v3 slow-path release) were all hardcoded to
/// native `msg.value`/`.call{value:}`. Every one of those now moves
/// `BOND_TOKEN` (PathUSD) instead. Everything else — statement parsing,
/// Merkle verification, anchor judging, the whole v3 challenge-window
/// mechanism — is unchanged from `BetaVault.sol`, since none of it is
/// native-value-specific; `params.ethWeiPerUnit`/`vetoSlashWei`/
/// `vetoRewardWei` etc. keep their field names (renaming every reference
/// risked introducing bugs for no behavioral gain) but now denote PathUSD's
/// own smallest unit (6 decimals), not wei.
///
/// The plain-native `deposit(address(0), ...)` path is left in place
/// unmodified rather than removed: since `msg.value` can never be nonzero
/// on Tempo, its own `msg.value != amount` check already makes it
/// permanently unreachable there — a passive, provably-safe dead path,
/// not a live footgun.
contract BetaVaultPathUSD is ReentrancyGuard {
    using SafeERC20 for IERC20;
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
    error TokenNotRegistered();
    error ReleaseTokenUnsupported();

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
        /// PathUSD's own smallest unit (6 decimals) per unit — not wei,
        /// despite the field name kept for minimal diff against BetaVault.sol.
        uint256 ethWeiPerUnit;
        uint64 tFinSecs;
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
        /// address(0) = native (permanently unreachable on Tempo — see
        /// contract-level comment); else an ERC20 token registered via
        /// `setTokenParams`, e.g. PathUSD itself for the local leg too.
        address token;
        uint256 amount;
    }

    struct TokenParams {
        uint256 amountPerUnit;
        uint256 slashWeiPerUnit;
    }

    enum AnchorStatus {
        None,
        Exercised,
        Slashed,
        Skipped,
        QueuedRelease,
        Cancelled
    }

    struct ProcessedAnchor {
        bytes32 partyId;
        uint8 kind;
        AnchorStatus status;
        bytes32 statementHash;
        uint64 blockHeight;
        uint64 processedAt;
        uint64 lockId;
        address to;
        uint64 units;
        uint64 challengeUntil;
        bool held;
        bytes32 attester;
        uint256 escrow;
        bool paid;
        bool settled;
    }

    // ------------------------------------------------------------ state
    IIPoWHeadersView public immutable ipowHeaders;
    /// PathUSD, Tempo's real value-bearing ERC20 — every bond and every
    /// internal payout (`_pay`) moves this, never native `msg.value`.
    IERC20 public immutable BOND_TOKEN;
    address public governance;
    Params public params;
    bool public paused;

    mapping(bytes32 => Party) public parties;
    mapping(bytes32 => address) public approvedOperators;
    mapping(uint64 => Lock) public locks;
    uint64 public nextLockId = 1;
    mapping(bytes32 => ProcessedAnchor) public anchors;
    mapping(bytes32 => bytes32) public mintAttester;
    mapping(address => TokenParams) public tokenParams;

    uint256 public insurance;
    uint256 public insuranceReserved;
    uint256 public rewardPool;
    uint256 public totalBonds;
    mapping(address => uint256) public totalLocked;

    // ------------------------------------------------------------ events
    event Deposited(uint64 indexed lockId, bytes32 indexed solUser, uint64 nonce, uint64 units, uint64 deadline, address depositor, address token);
    event TokenParamsSet(address indexed token, uint256 amountPerUnit, uint256 slashWeiPerUnit);
    event Finalized(uint64 indexed lockId, bytes32 anchorTxidLE);
    event Refunded(uint64 indexed lockId);
    event PartyRegistered(bytes32 indexed partyId, PartyKind kind, address owner, uint256 bond);
    event AnchorProcessed(bytes32 indexed partyId, bytes32 indexed txidLE, uint8 kind, AnchorStatus status);
    event Slashed(bytes32 indexed partyId, uint256 amount, address submitter);
    event ReleaseQueued(bytes32 indexed txidLE, uint64 indexed lockId, address to, uint64 units, uint64 releaseAfter);
    event ReleaseHeld(bytes32 indexed txidLE, bool held);
    event ReleasePaidFromEscrow(bytes32 indexed txidLE, bytes32 indexed attester, address to, uint256 amount);
    event ReleaseSettled(bytes32 indexed txidLE, bool reimbursed);
    event Released(bytes32 indexed txidLE, uint64 indexed lockId, address to, uint256 amount);

    modifier onlyGovernance() {
        if (msg.sender != governance) revert Unauthorized();
        _;
    }

    constructor(address _governance, address _ipowHeaders, address _bondToken, Params memory _params) {
        if (_governance == address(0) || _ipowHeaders == address(0) || _bondToken == address(0)) revert InvalidParams();
        _validate(_params);
        governance = _governance;
        ipowHeaders = IIPoWHeadersView(_ipowHeaders);
        BOND_TOKEN = IERC20(_bondToken);
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

    function approveOperator(bytes32 partyId, address owner) external onlyGovernance {
        approvedOperators[partyId] = owner;
    }

    function setTokenParams(address token, TokenParams calldata p) external onlyGovernance {
        if (token == address(0)) revert InvalidParams();
        tokenParams[token] = p;
        emit TokenParamsSet(token, p.amountPerUnit, p.slashWeiPerUnit);
    }

    // ------------------------------------------------------------ parties
    /// @notice Same as `BetaVault.registerParty` except the bond is pulled
    /// via `BOND_TOKEN.transferFrom` (an explicit `bond` argument), not
    /// `msg.value` — Tempo rejects any transaction carrying native value
    /// outright, so `payable` here would never be callable at all.
    function registerParty(bytes32 partyId, PartyKind kind, bytes32 anchorTxidLE, uint32 anchorVout, uint256 bond) external {
        if (parties[partyId].exists) revert PartyExists();
        if (kind == PartyKind.Operator) {
            if (approvedOperators[partyId] != msg.sender) revert NotApproved();
            if (bond < params.minOperatorBond) revert BondTooSmall();
        } else {
            if (bond < params.minAuditorBond) revert BondTooSmall();
        }
        uint256 pulled = _pullBond(msg.sender, bond);
        parties[partyId] = Party({
            exists: true,
            owner: msg.sender,
            kind: kind,
            anchorTxidLE: anchorTxidLE,
            anchorVout: anchorVout,
            seq: 0,
            bond: pulled,
            dead: false,
            unbondRequestedAt: 0
        });
        totalBonds += pulled;
        emit PartyRegistered(partyId, kind, msg.sender, pulled);
    }

    function topUpBond(bytes32 partyId, uint256 amount) external {
        Party storage p = parties[partyId];
        if (!p.exists || p.owner != msg.sender) revert Unauthorized();
        uint256 pulled = _pullBond(msg.sender, amount);
        p.bond += pulled;
        totalBonds += pulled;
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
    /// @notice Same as `BetaVault.deposit`. The `token == address(0)`
    /// (native) branch is unreachable on Tempo (`msg.value` can never be
    /// nonzero there), left in place unmodified rather than removed — see
    /// contract-level comment. Real deposits use an ERC20, e.g. PathUSD.
    function deposit(address token, bytes32 solUser, uint64 nonce, uint64 units, uint64 deadline)
        external
        payable
        returns (uint64 lockId)
    {
        if (paused) revert Paused();
        if (units == 0 || deadline <= block.timestamp) revert InvalidParams();
        uint256 amount;
        if (token == address(0)) {
            amount = uint256(units) * params.ethWeiPerUnit;
            if (msg.value != amount) revert InvalidParams();
        } else {
            if (msg.value != 0) revert InvalidParams();
            TokenParams memory tp = tokenParams[token];
            if (tp.amountPerUnit == 0) revert TokenNotRegistered();
            amount = uint256(units) * tp.amountPerUnit;
            amount = _pullToken(token, msg.sender, amount);
        }
        lockId = nextLockId++;
        locks[lockId] = Lock({
            solUser: solUser,
            nonce: nonce,
            units: units,
            deadline: deadline,
            state: LockState.Pending,
            depositor: msg.sender,
            token: token,
            amount: amount
        });
        totalLocked[token] += amount;
        emit Deposited(lockId, solUser, nonce, units, deadline, msg.sender, token);
    }

    function refund(uint64 lockId) external nonReentrant {
        Lock storage l = locks[lockId];
        if (l.state != LockState.Pending) revert BadLockState();
        if (block.timestamp <= uint256(l.deadline) + params.refundMarginSecs) revert RefundNotReady();
        _refund(lockId, l);
    }

    function _refund(uint64 lockId, Lock storage l) internal {
        l.state = LockState.Refunded;
        totalLocked[l.token] -= l.amount;
        _payOut(l.token, l.depositor, l.amount);
        emit Refunded(lockId);
    }

    // ------------------------------------------------------------ anchors
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
        if (s.length != 74) revert MalformedStatement();
        uint64 lockId = uint64(bytes8(s[10:18]));
        bytes32 solUser = bytes32(s[18:50]);
        uint64 nonce = uint64(bytes8(s[50:58]));
        uint64 units = uint64(bytes8(s[58:66]));
        uint64 deadline = uint64(bytes8(s[66:74]));
        Lock storage l = locks[lockId];
        pa.units = units;
        pa.lockId = lockId;
        bool matches = l.state != LockState.None && l.solUser == solUser && l.nonce == nonce && l.units == units
            && l.deadline == deadline;
        if (!matches) {
            uint256 slashRate = l.state == LockState.None ? params.ethWeiPerUnit : _slashRateFor(l.token);
            uint256 amount = uint256(units) * slashRate;
            _slash(partyId, party, amount, msg.sender);
            bytes32 att = mintAttester[txidLE];
            if (att != bytes32(0) && parties[att].exists && !parties[att].dead) {
                _slash(att, parties[att], amount, msg.sender);
            }
            pa.status = AnchorStatus.Slashed;
            return;
        }
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
            if (insurance < insuranceReserved + amount) {
                _slash(partyId, party, amount, msg.sender);
                pa.status = AnchorStatus.Slashed;
                return;
            }
            insuranceReserved += amount;
        } else {
            Lock storage l = locks[lockId];
            if (l.token != address(0)) revert ReleaseTokenUnsupported();
            if (l.state != LockState.Final || l.units != units) {
                _slash(partyId, party, amount, msg.sender);
                pa.status = AnchorStatus.Slashed;
                return;
            }
            l.state = LockState.Released;
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
            if (ta.settled) revert BadAnchorState();
            ta.held = true;
            pa.status = AnchorStatus.Exercised;
            emit ReleaseHeld(targetTxid, true);
            return;
        }
        if (ta.kind == KIND_MINT) {
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
            totalLocked[address(0)] -= amount;
        }
    }

    function _processAttestOrClear(bytes32 partyId, Party storage party, ProcessedAnchor storage pa, uint8 kind, bytes32 target)
        internal
    {
        ProcessedAnchor storage ta = anchors[target];
        if (ta.status == AnchorStatus.None) {
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

    /// @dev Every bond/payout moves PathUSD, not native value — see
    /// contract-level comment.
    function _pay(address to, uint256 amount) internal {
        if (amount == 0) return;
        BOND_TOKEN.safeTransfer(to, amount);
    }

    function _pullBond(address from, uint256 amount) internal returns (uint256) {
        uint256 before = BOND_TOKEN.balanceOf(address(this));
        BOND_TOKEN.safeTransferFrom(from, address(this), amount);
        return BOND_TOKEN.balanceOf(address(this)) - before;
    }

    function _slashRateFor(address token) internal view returns (uint256) {
        if (token == address(0)) return params.ethWeiPerUnit;
        uint256 r = tokenParams[token].slashWeiPerUnit;
        if (r == 0) revert TokenNotRegistered();
        return r;
    }

    function _pullToken(address token, address from, uint256 amount) internal returns (uint256) {
        uint256 before = IERC20(token).balanceOf(address(this));
        IERC20(token).safeTransferFrom(from, address(this), amount);
        return IERC20(token).balanceOf(address(this)) - before;
    }

    /// @dev The native branch (`token == address(0)`) is unreachable on
    /// Tempo — nothing can ever fund it — left in place for structural
    /// parity with `BetaVault.sol` rather than special-cased away.
    function _payOut(address token, address to, uint256 amount) internal {
        if (amount == 0) return;
        if (token == address(0)) {
            _pay(to, amount);
        } else {
            IERC20(token).safeTransfer(to, amount);
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

    /// @notice Top up the veto-reward pool with PathUSD (anyone; in
    /// practice governance) — `payable` removed, needs an explicit amount
    /// and a prior `approve`.
    function fundRewards(uint256 amount) external {
        uint256 pulled = _pullBond(msg.sender, amount);
        rewardPool += pulled;
    }

    // No `receive()` — Tempo rejects native value unconditionally, so a
    // plain-transfer entry point would never be callable there anyway.
}
