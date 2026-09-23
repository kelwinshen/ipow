// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

import {ReentrancyGuard} from "@openzeppelin/contracts/utils/ReentrancyGuard.sol";
import {BetaVaultStorage} from "../base/BetaVaultStorage.sol";

/**
 * @title BetaVaultCoreFacet
 * @notice Hyperliquid-only facet (DESIGN_V2.md §8.17): governance, party
 * registration/bonding, deposit/refund, and the v3 slow-path release
 * settlement functions. Only ever executed via `BetaVaultRouter`'s
 * `delegatecall` — direct calls to this contract's own address have no
 * meaningful effect since its own storage is never initialized (governance
 * is always the zero address here).
 */
contract BetaVaultCoreFacet is BetaVaultStorage, ReentrancyGuard {
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

    // ------------------------------------------------------------ v3 slow-path settlement
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

    /// @notice Top up the veto-reward pool (anyone; in practice governance).
    function fundRewards() external payable {
        rewardPool += msg.value;
    }
}
