// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

import {BetaHubStorage} from "../base/BetaHubStorage.sol";

/**
 * @title BetaHubMintFacet
 * @notice Hyperliquid-only facet (design/ipow-implementation.md §8.19): the `Pending` mint
 * lifecycle — `lockLocal`/`approvePending`/`exerciseMint`/`settleMint`/
 * `expirePending`. Reused near-verbatim from `BetaHub.sol`'s own body —
 * see that file's comments for the design reasoning.
 */
contract BetaHubMintFacet is BetaHubStorage {
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
