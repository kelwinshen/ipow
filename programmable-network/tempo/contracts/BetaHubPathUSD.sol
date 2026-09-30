// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

import {IERC20} from "@openzeppelin/contracts/token/ERC20/IERC20.sol";
import {SafeERC20} from "@openzeppelin/contracts/token/ERC20/utils/SafeERC20.sol";

import {HubPartyRegistry} from "./base/HubPartyRegistry.sol";
import {HubCompositionRegistry} from "./base/HubCompositionRegistry.sol";
import {HubAnchorJudge} from "./base/HubAnchorJudge.sol";
import {IIPoWHeadersView} from "./libraries/AnchorChainLib.sol";
import {HubToken} from "./HubToken.sol";

/// @title BetaHubPathUSD — Tempo-only `BetaHub` variant (mirrors BetaVaultPathUSD.sol)
/// @notice Tempo's custom `0x76` transaction type unconditionally rejects
/// any transaction carrying native value (confirmed for both plain
/// transfers and payable calls — see `BetaVaultPathUSD.sol`'s own header
/// comment). Plain `BetaHub.sol` structurally requires native value for
/// bonds (`registerParty`/`topUpBond`, inherited from `HubPartyRegistry`)
/// and every internal payout (`_pay`, used by slashing/veto rewards/
/// `withdrawBond`) — confirmed live: no party can ever be registered on
/// Tempo's already-deployed plain `BetaHub`, since `registerParty`'s
/// `msg.value >= minOperatorBond` can never be satisfied there (any
/// nonzero-value transaction reverts at the chain level before this
/// contract ever runs).
///
/// The fix mirrors `BetaVaultPathUSD.sol` exactly in spirit — every bond
/// and every internal payout now moves `BOND_TOKEN` (PathUSD,
/// `0x20c0000000000000000000000000000000000000`, 6 decimals) instead of
/// native value — but via inheritance + `virtual`/`override` rather than
/// full duplication, since `BetaHub` already factors its bonded-party/
/// payout logic into a reusable base (`HubPartyRegistry`) that every other
/// network's already-live `BetaHub` deployment also inherits unchanged:
/// only `HubPartyRegistry._pay` was marked `virtual` (a no-op change for
/// every network that doesn't override it) so this contract can redirect
/// every inherited payout path through one choke point.
/// `registerParty`/`topUpBond` gain a new PathUSD-taking overload here
/// (different arity, so this is overloading, not overriding) alongside —
/// not replacing — the inherited payable originals; those stay present but
/// permanently unreachable on Tempo, the same "dead path, not a live
/// footgun" choice `BetaVaultPathUSD.sol` already made for its own native
/// deposit branch. `processAnchor`/`exerciseMint`/composition judging/
/// statement verification (`HubCompositionRegistry`, `HubAnchorJudge`) are
/// entirely unchanged, inherited as-is — none of it is native-value-
/// specific, confirmed by reading both base contracts in full.
contract BetaHubPathUSD is HubCompositionRegistry, HubAnchorJudge {
    using SafeERC20 for IERC20;

    error InvalidParams();
    error PendingExists();
    error NoPending();
    error AlreadyApproved();
    error RemoteCountMismatch();
    error Paused();
    error NotPendingUser();
    error IncompleteRemoteComponents();
    error ComponentHeldOrNotReady();
    error QueuedByLiveOperator();
    error RefundNotReady();

    struct Params {
        uint64 tChallengeSecs;
        uint64 tSkipSecs;
        uint64 refundMarginSecs;
        uint64 unbondDelaySecs;
        uint256 minOperatorBond;
        uint256 minAuditorBond;
        uint256 slashWeiPerUnit;
        uint256 vetoSlashWei;
        uint256 vetoRewardWei;
        uint16 bountyBps;
    }

    struct Pending {
        address user;
        uint64 nonce;
        uint64 compositionId;
        uint64 units;
        uint64 deadline;
        bool approved;
        uint64[] remoteLockId;
        bytes32[] remoteAnchorTxid;
        bytes32 queuedBy;
        uint64 createdAt;
    }

    uint256 public constant BPS_DENOM = 10_000;

    IIPoWHeadersView public ipowHeaders;
    /// PathUSD, Tempo's real value-bearing ERC20 — every bond and every
    /// internal payout (`_pay`) moves this, never native `msg.value`. Same
    /// role as `BetaVaultPathUSD.BOND_TOKEN`.
    IERC20 public immutable BOND_TOKEN;
    Params public params;
    bool public paused;
    uint256 public rewardPool;
    HubToken public token;

    mapping(bytes32 => Pending) public pending;

    event PendingCreated(bytes32 indexed pendingId, address indexed user, uint64 nonce, uint64 compositionId, uint64 units, uint64 deadline);
    event PendingApproved(bytes32 indexed pendingId, uint64[] remoteLockId);
    event Minted(bytes32 indexed pendingId, address indexed user, uint256 amount);
    event PendingExpired(bytes32 indexed pendingId);

    constructor(
        address _governance,
        address _ipowHeadersAddr,
        address _bondToken,
        uint256 _selfNetworkId,
        Params memory _params,
        string memory tokenName,
        string memory tokenSymbol
    ) HubCompositionRegistry(_selfNetworkId) {
        if (_governance == address(0) || _ipowHeadersAddr == address(0) || _bondToken == address(0)) {
            revert InvalidParams();
        }
        _validateParams(_params);
        governance = _governance;
        ipowHeaders = IIPoWHeadersView(_ipowHeadersAddr);
        BOND_TOKEN = IERC20(_bondToken);
        params = _params;
        token = new HubToken(tokenName, tokenSymbol, address(this));
    }

    function _validateParams(Params memory p) internal pure {
        if (p.tChallengeSecs == 0 || p.unbondDelaySecs == 0 || p.bountyBps > BPS_DENOM) revert InvalidParams();
    }

    function setParams(Params calldata p, bool _paused) external onlyGovernance {
        _validateParams(p);
        params = p;
        paused = _paused;
    }

    /// @notice Same as `BetaHub.fundRewards` except PathUSD, not native
    /// value — `payable` removed, needs an explicit amount and a prior
    /// `approve`. Mirrors `BetaVaultPathUSD.fundRewards` exactly.
    function fundRewards(uint256 amount) external {
        uint256 pulled = _pullBond(msg.sender, amount);
        rewardPool += pulled;
    }

    // ---------------------------------------------------------- bonds (PathUSD)
    /// @notice Same as the inherited `HubPartyRegistry.registerParty`
    /// except the bond is pulled via `BOND_TOKEN.transferFrom` (an
    /// explicit `bond` argument), not `msg.value` — an overload, not an
    /// override (different arity); the inherited original stays present
    /// and permanently unreachable on Tempo — see contract-level comment.
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

    function _pullBond(address from, uint256 amount) internal returns (uint256) {
        uint256 before = BOND_TOKEN.balanceOf(address(this));
        BOND_TOKEN.safeTransferFrom(from, address(this), amount);
        return BOND_TOKEN.balanceOf(address(this)) - before;
    }

    /// @dev Overrides `HubPartyRegistry._pay` (now `virtual` there) so
    /// every inherited payout path (`withdrawBond`, `_slash`'s bounty,
    /// `_processVeto`'s reward) moves PathUSD instead of native value —
    /// the single choke point that makes the rest of the inherited
    /// judging logic (`HubAnchorJudge`) work completely unmodified.
    function _pay(address to, uint256 amount) internal override {
        if (amount == 0) return;
        BOND_TOKEN.safeTransfer(to, amount);
    }

    // ---------------------------------------------------------- virtual hooks
    function _minOperatorBond() internal view override returns (uint256) {
        return params.minOperatorBond;
    }

    function _minAuditorBond() internal view override returns (uint256) {
        return params.minAuditorBond;
    }

    function _unbondDelaySecs() internal view override returns (uint64) {
        return params.unbondDelaySecs;
    }

    function _vetoSlashWei() internal view override returns (uint256) {
        return params.vetoSlashWei;
    }

    function _slashWeiPerUnit() internal view override returns (uint256) {
        return params.slashWeiPerUnit;
    }

    function _vetoRewardWei() internal view override returns (uint256) {
        return params.vetoRewardWei;
    }

    function _tSkipSecs() internal view override returns (uint64) {
        return params.tSkipSecs;
    }

    function _bountyBps() internal view override returns (uint16) {
        return params.bountyBps;
    }

    function _bpsDenom() internal pure override returns (uint256) {
        return BPS_DENOM;
    }

    function _rewardPoolSub(uint256 amount) internal override {
        uint256 reward = amount < rewardPool ? amount : rewardPool;
        rewardPool -= reward;
    }

    function _ipowHeaders() internal view override returns (IIPoWHeadersView) {
        return ipowHeaders;
    }

    // ---------------------------------------------------------- Pending lifecycle
    // Unchanged from BetaHub.sol below this point -- none of it is
    // native-value-specific; a local leg can already be registered as
    // PathUSD (or any ERC20) like any other component, and `_payOut`'s
    // native branch is left in place unreachable, same choice
    // `BetaVaultPathUSD._payOut` already made for structural parity.
    function _pendingKey(address user, uint64 nonce) internal pure returns (bytes32) {
        return keccak256(abi.encodePacked(user, nonce));
    }

    function _pullTokenExact(address tokenAddr, address from, uint256 amount) internal {
        uint256 before = IERC20(tokenAddr).balanceOf(address(this));
        IERC20(tokenAddr).safeTransferFrom(from, address(this), amount);
        if (IERC20(tokenAddr).balanceOf(address(this)) - before != amount) revert InvalidParams();
    }

    function _payOut(address tokenAddr, address to, uint256 amount) internal {
        if (amount == 0) return;
        if (tokenAddr == address(0)) {
            _pay(to, amount);
        } else {
            IERC20(tokenAddr).safeTransfer(to, amount);
        }
    }

    function lockLocal(uint64 compositionId, uint64 nonce, uint64 units, uint64 deadline) external payable returns (bytes32 pendingId) {
        if (paused) revert Paused();
        if (units == 0 || deadline <= block.timestamp) revert InvalidParams();
        if (!compositionExists(compositionId)) revert NoComposition();
        pendingId = _pendingKey(msg.sender, nonce);
        if (pending[pendingId].user != address(0)) revert PendingExists();

        Component[] memory components = this.getComposition(compositionId);
        uint256 nativeNeeded;
        uint256 remoteCount;
        for (uint256 i = 0; i < components.length; i++) {
            Component memory c = components[i];
            if (c.networkId != SELF_NETWORK_ID) {
                remoteCount++;
                continue;
            }
            uint256 amount = uint256(units) * c.amountPerUnit;
            if (c.tokenId == address(0)) {
                nativeNeeded += amount;
            } else {
                _pullTokenExact(c.tokenId, msg.sender, amount);
            }
        }
        if (msg.value != nativeNeeded) revert InvalidParams();

        Pending storage p = pending[pendingId];
        p.user = msg.sender;
        p.nonce = nonce;
        p.compositionId = compositionId;
        p.units = units;
        p.deadline = deadline;
        p.approved = false;
        p.remoteLockId = new uint64[](remoteCount);
        p.remoteAnchorTxid = new bytes32[](remoteCount);
        p.createdAt = uint64(block.timestamp);
        emit PendingCreated(pendingId, msg.sender, nonce, compositionId, units, deadline);
    }

    function pendingRemoteLockId(bytes32 pendingId) external view returns (uint64[] memory) {
        return pending[pendingId].remoteLockId;
    }

    function pendingRemoteAnchorTxid(bytes32 pendingId) external view returns (bytes32[] memory) {
        return pending[pendingId].remoteAnchorTxid;
    }

    function approvePending(uint64 nonce, uint64[] calldata remoteLockId) external {
        bytes32 pendingId = _pendingKey(msg.sender, nonce);
        Pending storage p = pending[pendingId];
        if (p.user != msg.sender) revert NotPendingUser();
        if (p.approved) revert AlreadyApproved();
        if (remoteLockId.length != p.remoteLockId.length) revert RemoteCountMismatch();
        for (uint256 i = 0; i < remoteLockId.length; i++) {
            p.remoteLockId[i] = remoteLockId[i];
        }
        p.approved = true;
        emit PendingApproved(pendingId, remoteLockId);
    }

    function _processRemoteMint(bytes32 partyId, Party storage party, ProcessedAnchor storage pa, bytes32 txidLE, bytes calldata s)
        internal
        override
    {
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
            (, uint256 remainder) = _slash(partyId, party, amount, params.bountyBps, BPS_DENOM, msg.sender, p.user);
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

    function exerciseMint(address user, uint64 nonce) external nonReentrant returns (uint256 minted) {
        if (paused) revert Paused();
        bytes32 pendingId = _pendingKey(user, nonce);
        Pending storage p = pending[pendingId];
        if (p.user == address(0)) revert NoPending();
        if (!p.approved) revert NotPendingUser();

        uint256 remoteCount = p.remoteLockId.length;
        if (remoteCount > 0) {
            if (p.queuedBy == bytes32(0) || parties[p.queuedBy].dead) revert IncompleteRemoteComponents();
            for (uint256 i = 0; i < remoteCount; i++) {
                bytes32 txid = p.remoteAnchorTxid[i];
                if (txid == bytes32(0)) revert IncompleteRemoteComponents();
                ProcessedAnchor storage pa = anchors[txid];
                bool ready = pa.kind == KIND_MINT && pa.status == AnchorStatus.Queued && pa.compositionId == p.compositionId
                    && pa.componentIndex == i && pa.units == p.units && !pa.held;
                if (!ready) revert IncompleteRemoteComponents();
                bool attested = mintAttester[txid] != bytes32(0);
                if (!attested && block.timestamp < pa.challengeUntil) revert ComponentHeldOrNotReady();
            }
            for (uint256 i = 0; i < remoteCount; i++) {
                anchors[p.remoteAnchorTxid[i]].status = AnchorStatus.Exercised;
            }
        }

        minted = uint256(p.units) * 1e18;
        address recipient = p.user;
        delete pending[pendingId];
        token.mint(recipient, minted);
        emit Minted(pendingId, recipient, minted);
    }

    function settleMint(bytes32 txidLE) external nonReentrant {
        ProcessedAnchor storage pa = anchors[txidLE];
        if (pa.kind != KIND_MINT || pa.status != AnchorStatus.Queued || pa.settled) revert BadAnchorState();
        if (block.timestamp < pa.challengeUntil) revert ComponentHeldOrNotReady();
        if (!pa.held) return;
        pa.settled = true;
        pa.status = AnchorStatus.Cancelled;
        Pending storage p = pending[pa.pendingId];
        if (p.user != address(0) && p.queuedBy == pa.partyId) {
            for (uint256 i = 0; i < p.remoteAnchorTxid.length; i++) {
                p.remoteAnchorTxid[i] = bytes32(0);
            }
            p.queuedBy = bytes32(0);
        }
    }

    function expirePending(address user, uint64 nonce) external nonReentrant {
        bytes32 pendingId = _pendingKey(user, nonce);
        Pending storage p = pending[pendingId];
        if (p.user == address(0)) revert NoPending();
        if (p.queuedBy != bytes32(0) && !parties[p.queuedBy].dead) revert QueuedByLiveOperator();
        if (block.timestamp <= uint256(p.deadline) + params.refundMarginSecs) revert RefundNotReady();

        Component[] memory components = this.getComposition(p.compositionId);
        uint64 units = p.units;
        address refundUser = p.user;
        delete pending[pendingId];
        for (uint256 i = 0; i < components.length; i++) {
            Component memory c = components[i];
            if (c.networkId != SELF_NETWORK_ID) continue;
            _payOut(c.tokenId, refundUser, uint256(units) * c.amountPerUnit);
        }
        emit PendingExpired(pendingId);
    }
}
