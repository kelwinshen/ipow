// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

import {BetaHubStorage} from "../base/BetaHubStorage.sol";

/**
 * @title BetaHubGovernanceFacet
 * @notice Hyperliquid-only facet (design/ipow-implementation.md §8.19): governance
 * (`setParams`), party/bond registration (mirrors `HubPartyRegistry`
 * verbatim), and composition registration (mirrors `HubCompositionRegistry`
 * verbatim). Only ever executed via `BetaHubRouter`'s `delegatecall`.
 */
contract BetaHubGovernanceFacet is BetaHubStorage {
    function setParams(Params calldata p, bool _paused) external onlyGovernance {
        _validateParams(p);
        params = p;
        paused = _paused;
    }

    function fundRewards() external payable {
        rewardPool += msg.value;
    }

    // ------------------------------------------------------------ party/bond
    function approveOperator(bytes32 partyId, address owner) external onlyGovernance {
        approvedOperators[partyId] = owner;
    }

    function registerParty(bytes32 partyId, PartyKind kind, bytes32 anchorTxidLE, uint32 anchorVout) external payable {
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

    // ------------------------------------------------------------ composition registry
    function registerComposition(uint64 id, Component[] calldata components) external onlyGovernance {
        if (_compositions[id].exists) revert CompositionExists();
        if (components.length == 0 || components.length > MAX_COMPONENTS) revert InvalidComponents();

        uint256 localCount;
        for (uint256 i = 0; i < components.length; i++) {
            Component calldata c = components[i];
            if (c.amountPerUnit == 0) revert InvalidComponents();
            if (c.networkId == SOLANA_NETWORK_ID) revert UnsupportedNetwork();
            if (c.networkId == SELF_NETWORK_ID) localCount++;
            for (uint256 j = i + 1; j < components.length; j++) {
                if (components[j].networkId == c.networkId && components[j].tokenId == c.tokenId) {
                    revert DuplicateComponent();
                }
            }
        }
        if (localCount == 0) revert NoLocalComponent();
        if (localCount > MAX_LOCAL_COMPONENTS) revert TooManyLocal();

        Composition storage comp = _compositions[id];
        comp.exists = true;
        for (uint256 i = 0; i < components.length; i++) {
            comp.components.push(components[i]);
        }
        emit CompositionRegistered(id, components.length);
    }
}
