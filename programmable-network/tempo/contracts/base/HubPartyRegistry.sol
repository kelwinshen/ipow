// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

import {ReentrancyGuard} from "@openzeppelin/contracts/utils/ReentrancyGuard.sol";

/// @title HubPartyRegistry
/// @notice Bonded statement-maker registry for a `BetaHub` deployment
/// (design/ipow-implementation.md §8.18) — copied from `BetaVault.sol`'s own `Party`/bond
/// lifecycle (lines 201-334, 734-743), since it's the identical trust
/// primitive: a governance-approved, bonded party posts Bitcoin-anchored
/// statements about facts this contract can't otherwise observe.
/// @dev Deliberately a fresh, hub-local registry: NOT shared with any
/// `BetaVault` deployment's own `parties` (even one on the same chain), and
/// NOT a single global pool the way Solana's `beta-factory` uses. Every
/// `BetaVault` deployment already has its own independent registry — this
/// keeps that same per-deployment trust-isolation property for hubs, so an
/// operator's failure on one hub can never affect an unrelated hub's trust.
abstract contract HubPartyRegistry is ReentrancyGuard {
    error Unauthorized();
    error BondTooSmall();
    error PartyDead();
    error PartyExists();
    error NotApproved();
    error UnbondNotRequested();
    error UnbondNotReady();
    error TransferFailed();

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

    /// @dev Set once by `BetaHub`'s constructor (see that contract) — declared
    /// here since both this registry's `approveOperator` and
    /// `HubCompositionRegistry.registerComposition` need `onlyGovernance`.
    address public governance;

    mapping(bytes32 => Party) public parties;
    mapping(bytes32 => address) public approvedOperators;
    uint256 public totalBonds;

    event PartyRegistered(bytes32 indexed partyId, PartyKind kind, address owner, uint256 bond);
    event Slashed(bytes32 indexed partyId, uint256 amount, address submitter, address remainderTo);

    modifier onlyGovernance() {
        if (msg.sender != governance) revert Unauthorized();
        _;
    }

    function _minOperatorBond() internal view virtual returns (uint256);
    function _minAuditorBond() internal view virtual returns (uint256);
    function _unbondDelaySecs() internal view virtual returns (uint64);

    /// @notice Governance pre-approves who may register as an operator.
    function approveOperator(bytes32 partyId, address owner) external onlyGovernance {
        approvedOperators[partyId] = owner;
    }

    function registerParty(bytes32 partyId, PartyKind kind, bytes32 anchorTxidLE, uint32 anchorVout) external payable {
        if (parties[partyId].exists) revert PartyExists();
        if (kind == PartyKind.Operator) {
            if (approvedOperators[partyId] != msg.sender) revert NotApproved();
            if (msg.value < _minOperatorBond()) revert BondTooSmall();
        } else {
            if (msg.value < _minAuditorBond()) revert BondTooSmall();
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
        if (block.timestamp < p.unbondRequestedAt + _unbondDelaySecs()) revert UnbondNotReady();
        uint256 amount = p.bond;
        p.bond = 0;
        p.dead = true;
        totalBonds -= amount;
        _pay(msg.sender, amount);
    }

    /// @dev Generalized from `BetaVault._slash`: pays the bounty to
    /// `submitter` same as `BetaVault._slash` always does, but leaves the
    /// non-bounty remainder for the CALLER to route — either paid out to a
    /// named victim (`_pay`) or added to an `insurance` counter, matching
    /// whichever `BetaVault._slash` (always insurance, no identifiable
    /// victim) or `beta-factory::utils::slash()`'s own `remainder_to`
    /// parameter (a real, external transfer to the victim) fits the case.
    /// `HubAnchorJudge`'s false-MINT case has a named victim (the specific
    /// pending mint's user) to compensate; its veto/attest-fan-out/dead
    /// cases don't, and route the remainder to insurance instead — see that
    /// contract's call sites.
    function _slash(bytes32 partyId, Party storage party, uint256 amount, uint16 bountyBps, uint256 bpsDenom, address submitter, address remainderTarget)
        internal
        returns (uint256 slashed, uint256 remainder)
    {
        slashed = amount < party.bond ? amount : party.bond;
        party.bond -= slashed;
        party.dead = true;
        totalBonds -= slashed;
        uint256 bounty = (slashed * bountyBps) / bpsDenom;
        remainder = slashed - bounty;
        emit Slashed(partyId, slashed, submitter, remainderTarget);
        _pay(submitter, bounty);
    }

    /// @dev `virtual` purely so a network-specific variant (e.g.
    /// `BetaHubPathUSD` on Tempo, which can't use native value at all) can
    /// redirect every inherited payout path through one choke point — a
    /// no-op for every other network's `BetaHub`, which doesn't override
    /// it.
    function _pay(address to, uint256 amount) internal virtual {
        if (amount == 0) return;
        (bool ok,) = payable(to).call{value: amount}("");
        if (!ok) revert TransferFailed();
    }
}
