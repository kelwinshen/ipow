// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

import {IERC20} from "@openzeppelin/contracts/token/ERC20/IERC20.sol";
import {SafeERC20} from "@openzeppelin/contracts/token/ERC20/utils/SafeERC20.sol";

import {HubPartyRegistry} from "./base/HubPartyRegistry.sol";
import {HubCompositionRegistry} from "./base/HubCompositionRegistry.sol";
import {HubAnchorJudge} from "./base/HubAnchorJudge.sol";
import {IIPoWHeadersView} from "./libraries/AnchorChainLib.sol";
import {HubToken} from "./HubToken.sol";

/// @title BetaHub — an EVM chain acting as a mint hub, not just a spoke
/// @notice design/ipow-implementation.md §8.18. Every EVM chain in this repo (via
/// `BetaVault.sol`) only ever locks value and judges its own leg's MINT
/// claim — it never mints anything itself; only Solana's `beta-factory` can
/// mint BETA. `BetaHub` lets THIS chain also mint its own token (`HubToken`,
/// deployed as "iBETA" for this pilot — deliberately not named "Beta"/
/// "BETA", which is already `BetaToken.sol`'s name for an unrelated
/// mechanism), backed by a governance-registered composition of legs on
/// this chain (local, permissionless, judged directly) and on other EVM
/// chains (remote, judged via the same Bitcoin-anchored-statement +
/// bonding/challenge-window pattern `BetaVault.sol` already uses for its
/// own leg — no new cross-chain trust primitive, no message passing:
/// `BetaVault.sol` already works unmodified as a remote leg here, since its
/// `Lock.solUser` is an opaque `bytes32` never compared except for
/// byte-equality).
///
/// Per-chain token, not one global fungible supply, for now — a future
/// bridging layer could unify per-chain hub tokens later; that's out of
/// scope here. Solana-as-a-remote-leg is also out of scope (would need a
/// new Solana "spoke" program mirroring `BetaVault.sol`'s judging logic);
/// `HubCompositionRegistry.registerComposition` fails closed on
/// `networkId == SOLANA_NETWORK_ID`.
contract BetaHub is HubCompositionRegistry, HubAnchorJudge {
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
        /// Per-unit rate for both a false MINT-component claim's slash and
        /// an ATTEST fan-out slash — mirrors `BetaVault.sol`'s reuse of
        /// `params.ethWeiPerUnit` for both of its own equivalents.
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
        uint256 _selfNetworkId,
        Params memory _params,
        string memory tokenName,
        string memory tokenSymbol
    ) HubCompositionRegistry(_selfNetworkId) {
        if (_governance == address(0) || _ipowHeadersAddr == address(0)) revert InvalidParams();
        _validateParams(_params);
        governance = _governance;
        ipowHeaders = IIPoWHeadersView(_ipowHeadersAddr);
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

    function fundRewards() external payable {
        rewardPool += msg.value;
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
    function _pendingKey(address user, uint64 nonce) internal pure returns (bytes32) {
        return keccak256(abi.encodePacked(user, nonce));
    }

    /// @dev Same before/after-balance defensive pattern as `BetaVault.
    /// _pullToken`/`iPoWConversion._pullToken`. Unlike `BetaVault.deposit`
    /// (a single flat `Lock`, which can afford to freeze whatever amount
    /// was actually received), a composition may lock several local legs at
    /// once with no per-leg "amount actually received" field to freeze —
    /// so a fee-on-transfer token that shorts the expected amount is
    /// rejected outright here rather than accepted at a silently-reduced
    /// rate.
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

    /// @notice Permissionless. Locks every LOCAL leg of `compositionId`
    /// directly in this contract (native via `msg.value`, ERC20 via
    /// `_pullToken`) and creates a `Pending` mint request. Mirrors
    /// `lock_sol`: no operator/governance involvement for this step at all.
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

    /// @notice `Pending.remoteLockId`/`remoteAnchorTxid` — Solidity's
    /// auto-generated public getter for a mapping-to-struct silently omits
    /// array-typed members, so callers (including this test suite) need an
    /// explicit accessor to observe them at all.
    function pendingRemoteLockId(bytes32 pendingId) external view returns (uint64[] memory) {
        return pending[pendingId].remoteLockId;
    }

    function pendingRemoteAnchorTxid(bytes32 pendingId) external view returns (bytes32[] memory) {
        return pending[pendingId].remoteAnchorTxid;
    }

    /// @notice The depositing user binds the `remoteLockId[]` they obtained
    /// by separately depositing on each remote leg's own `BetaVault`. One-
    /// shot — mirrors `approve_pending.rs` exactly.
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

    /// @dev The genuinely new predicate, translated field-for-field from
    /// `beta-factory::process_anchor`'s verified `Statement::Mint` branch.
    /// Unlike Solana (whose account model needs every touched account named
    /// up front, forcing `exercise_mint`'s `remote_0..remote_6` optional
    /// slots), this can just read `pending`/`parties` directly — no
    /// equivalent API-surface complexity needed on EVM.
    function _processRemoteMint(bytes32 partyId, Party storage party, ProcessedAnchor storage pa, bytes32 txidLE, bytes calldata s) internal override {
        // 74 bytes: kind(1) | compositionId(8) | componentIndex(1) | lockId(8)
        // | hubUser(32, zero-padded address) | nonce(8) | units(8) | deadline(8) —
        // identical wire format BetaVault.sol's own doc comment already
        // defines and parses past.
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
                // No matching pending at all (bad componentIndex/user/nonce
                // in the statement, or none ever created) — no identifiable
                // victim to compensate; park the remainder in insurance
                // rather than silently discarding it.
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

    /// @notice Permissionless. Once every remote component of `pendingId` is
    /// `Queued`, unheld, and either attested or past its own challenge
    /// window, mints `units * 10**18` `HubToken` to the user and clears the
    /// pending slot. A composition with zero remote components (all-local)
    /// mints as soon as `approved`, with no operator involved at all —
    /// mirrors Solana's own zero-remote-leg case.
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

    /// @notice Permissionless. Resolves the challenge-window outcome of a
    /// queued MINT-component anchor once its window has closed. Mirrors
    /// `BetaVault.settleRelease`: if held, un-queues it (resets the whole
    /// pending slot's claim so a live operator can re-claim from scratch, or
    /// the user can eventually `expirePending`) and marks the anchor
    /// `Cancelled`. If NOT held, this is a genuine no-op — unlike
    /// `BetaVault`'s RELEASE-ATTEST (which fast-pays the recipient and so
    /// needs `settleRelease` to later reimburse the attester from the
    /// vault), a MINT-component's ATTEST never escrows or pays anyone (see
    /// `HubAnchorJudge._processAttestOrClear`'s comment) — there is nothing
    /// left to resolve once a clean (never-held) component's challenge
    /// window closes; `exerciseMint` is what actually finalizes it.
    function settleMint(bytes32 txidLE) external nonReentrant {
        ProcessedAnchor storage pa = anchors[txidLE];
        if (pa.kind != KIND_MINT || pa.status != AnchorStatus.Queued || pa.settled) revert BadAnchorState();
        if (block.timestamp < pa.challengeUntil) revert ComponentHeldOrNotReady();
        if (!pa.held) return;
        pa.settled = true;
        pa.status = AnchorStatus.Cancelled;
        Pending storage p = pending[pa.pendingId];
        // Only reset if this component's anchor is still the currently-
        // claimed operator's — a takeover may already have superseded it.
        if (p.user != address(0) && p.queuedBy == pa.partyId) {
            for (uint256 i = 0; i < p.remoteAnchorTxid.length; i++) {
                p.remoteAnchorTxid[i] = bytes32(0);
            }
            p.queuedBy = bytes32(0);
        }
    }

    /// @notice Permissionless refund fallback, past `deadline +
    /// refundMarginSecs`: refunds every local leg back to the user at the
    /// composition's frozen `amountPerUnit` rate and closes the pending
    /// slot. Blocked while claimed by a still-live operator — mirrors
    /// `expire_pending.rs`'s `PendingQueued`-unless-dead guard. Unlike
    /// `beta-factory::expire_pending` (no margin at all), this keeps a
    /// `refundMarginSecs` — consistent with `BetaVault.refund`'s own
    /// existing margin, avoiding a race between a just-anchored true MINT
    /// and an expire landing in the same block.
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
