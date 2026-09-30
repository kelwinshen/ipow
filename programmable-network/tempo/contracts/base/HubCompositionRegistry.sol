// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

import {HubPartyRegistry} from "./HubPartyRegistry.sol";

/// @title HubCompositionRegistry
/// @notice Governance-registered mint "recipes" for a `BetaHub` deployment
/// (design/ipow-implementation.md §8.18) — mirrors Solana `beta-factory`'s `Composition`/
/// `register_composition` (`state/composition.rs`, `register_composition.rs`)
/// field-for-field.
/// @dev `Component.amountPerUnit` is deliberately the only rate table this
/// contract needs — unlike `BetaVault.sol`'s separate `setTokenParams`
/// registry (needed there because `BetaVault.deposit` is composition-less
/// and accepts any governance-registered token at any time), a hub's local
/// deposit is always composition-scoped, and `registerComposition` is
/// already governance-gated and immutable-per-id — exactly mirroring
/// `lock_sol`'s own `amount = units * component.amount_per_unit`.
abstract contract HubCompositionRegistry is HubPartyRegistry {
    error InvalidComponents();
    error DuplicateComponent();
    error TooManyLocal();
    error NoLocalComponent();
    error UnsupportedNetwork();
    error CompositionExists();
    error NoComposition();

    /// Total components (local + remote) any one composition may have —
    /// mirrors Solana's `MAX_COMPONENTS`. A governance/product tuning knob,
    /// not a technical constraint (no block-gas ceiling on this network).
    uint256 public constant MAX_COMPONENTS = 8;
    /// Local (this-chain) components a composition may have — mirrors
    /// Solana's `MAX_LOCAL_COMPONENTS`.
    uint256 public constant MAX_LOCAL_COMPONENTS = 4;
    /// iPoW protocol network-ID registry (docs/design/ipow-implementation.md): Solana is
    /// reserved and, for this pass, unsupported as a remote leg — a spoke
    /// there needs a new Solana "spoke" program this repo doesn't have yet
    /// (design/ipow-implementation.md §8.18's "deliberately deferred" list). Reject rather
    /// than silently registering an unjudgeable leg.
    uint256 public constant SOLANA_NETWORK_ID = 3;

    struct Component {
        uint256 networkId; // SELF_NETWORK_ID = local (this hub's own chain); else a remote BetaVault-shaped leg.
        address tokenId; // address(0) = native on that component's chain; else that chain's ERC20 address (descriptive only for remote legs).
        uint256 amountPerUnit; // in that leg's own smallest unit.
    }

    struct Composition {
        bool exists;
        Component[] components;
    }

    /// Set once by `BetaHub`'s constructor.
    uint256 public immutable SELF_NETWORK_ID;

    mapping(uint64 => Composition) private _compositions;

    event CompositionRegistered(uint64 indexed id, uint256 componentCount);

    constructor(uint256 selfNetworkId) {
        SELF_NETWORK_ID = selfNetworkId;
    }

    function getComposition(uint64 id) external view returns (Component[] memory) {
        return _compositions[id].components;
    }

    function compositionExists(uint64 id) public view returns (bool) {
        return _compositions[id].exists;
    }

    /// @notice Governance-only, immutable once registered (a new recipe
    /// needs a new `id`) — mirrors `register_composition.rs`'s `init`-only
    /// PDA exactly.
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
