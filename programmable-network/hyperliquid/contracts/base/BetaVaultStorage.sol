// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

import {IERC20} from "@openzeppelin/contracts/token/ERC20/IERC20.sol";
import {SafeERC20} from "@openzeppelin/contracts/token/ERC20/utils/SafeERC20.sol";

/// @dev Same read-only view into `iPoW`'s header relay that
/// `iPoWConversion` uses. Satisfied on this network by `iPoWRouter`'s own
/// inherited public getters (design/ipow-implementation.md §8.16) — no delegatecall involved.
interface IIPoWHeadersView {
    function globalTipHeight() external view returns (uint256);
    function globalHeightToHashLE(uint256 height) external view returns (bytes32);
    function globalHeaders(bytes32 hashLE)
        external
        view
        returns (bytes32 prevHashLE, bytes32 merkleRootLE, uint32 nBits, uint32 timestamp, bool set, uint64 arrivalTime);
}

/**
 * @title BetaVaultStorage
 * @notice Hyperliquid-only split of `BetaVault` (design/ipow-implementation.md §8.17), same
 * reasoning and pattern as `iPoWRouter`'s split (§8.16): HyperEVM
 * testnet's 3,000,000 block gas limit is below what plain `BetaVault` needs
 * to deploy (~4.0M gas, driven by ~16,921 bytes of deployed bytecode — code-
 * deposit cost alone, 200 gas/byte, is ~3.38M), so on this network only, its
 * logic is split across two facets (`BetaVaultCoreFacet`,
 * `BetaVaultAnchorFacet`) that `BetaVaultRouter` dispatches to via
 * `delegatecall`. Every other network keeps the plain, unmodified
 * `BetaVault.sol`.
 *
 * Holds every error, type, event, state variable, and shared internal helper
 * that both facets (and the router itself) need — inherited identically by
 * all three, which is what guarantees identical Solidity storage-slot
 * assignment across them (critical for `delegatecall` safety: a facet only
 * ever executes against the router's own storage). Internal helper functions
 * live here too, rather than being duplicated per-facet: an internal
 * function unreferenced by a given facet's own external entry points is
 * dead-code-eliminated from that facet's deployed bytecode by the optimizer
 * (viaIR), so this costs nothing per-facet while keeping every copy
 * identical by construction — no risk of two hand-duplicated copies drifting.
 *
 * `ipowHeaders` is regular storage here, not `immutable` as in plain
 * `BetaVault` — immutables are baked into a contract's own bytecode at
 * construction and don't propagate through `delegatecall`; each facet would
 * otherwise read its own meaningless zero value instead of the router's.
 * Same fix as `iPoWStorage`'s `NATIVE_DECIMALS`/`SELF_NETWORK_ID`.
 */
abstract contract BetaVaultStorage {
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
    error InvalidConstructor();

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
    // Not immutable — see contract-level comment.
    IIPoWHeadersView public ipowHeaders;
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
    event ReleasePaidFromEscrow(bytes32 indexed txidLE, bytes32 indexed attester, address to, uint256 wei_);
    event ReleaseSettled(bytes32 indexed txidLE, bool reimbursed);
    event Released(bytes32 indexed txidLE, uint64 indexed lockId, address to, uint256 wei_);

    modifier onlyGovernance() {
        if (msg.sender != governance) revert Unauthorized();
        _;
    }

    // ------------------------------------------------------------ shared internal helpers
    function _validate(Params memory p) internal pure {
        if (p.ethWeiPerUnit == 0 || p.tFinSecs == 0 || p.tChallengeSecs == 0) revert InvalidParams();
        if (p.tSkipSecs < 2 * p.tFinSecs || p.unbondDelaySecs == 0 || p.bountyBps > BPS_DENOM) revert InvalidParams();
    }

    function _pay(address to, uint256 amount) internal {
        if (amount == 0) return;
        (bool ok,) = payable(to).call{value: amount}("");
        if (!ok) revert TransferFailed();
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

    function _payOut(address token, address to, uint256 amount) internal {
        if (amount == 0) return;
        if (token == address(0)) {
            _pay(to, amount);
        } else {
            IERC20(token).safeTransfer(to, amount);
        }
    }

    function _refund(uint64 lockId, Lock storage l) internal {
        l.state = LockState.Refunded;
        totalLocked[l.token] -= l.amount;
        _payOut(l.token, l.depositor, l.amount);
        emit Refunded(lockId);
    }

    function _drawVault(uint64 lockId, uint256 amount) internal {
        if (lockId == 0) {
            insuranceReserved -= amount;
            insurance -= amount;
        } else {
            totalLocked[address(0)] -= amount;
        }
    }

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
}
