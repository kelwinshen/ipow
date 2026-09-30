// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

import {ReentrancyGuard} from "@openzeppelin/contracts/utils/ReentrancyGuard.sol";
import {IERC20} from "@openzeppelin/contracts/token/ERC20/IERC20.sol";
import {SafeERC20} from "@openzeppelin/contracts/token/ERC20/utils/SafeERC20.sol";

import {IIPoWHeadersView} from "../libraries/AnchorChainLib.sol";
import {HubToken} from "../HubToken.sol";

/**
 * @title BetaHubStorage
 * @notice Hyperliquid-only split of `BetaHub` (design/ipow-implementation.md §8.18/§8.19),
 * same reasoning and pattern as `iPoWRouter`/`BetaVaultRouter`'s splits
 * (§8.16/§8.17): `BetaHub`'s deployed bytecode (~19,118 bytes on the
 * reference `ethereum/` build) needs ~3.82M gas of code-deposit cost alone
 * (200 gas/byte) — above HyperEVM testnet's 3,000,000 block gas limit — so
 * on this network only, its logic is split across facets that
 * `BetaHubRouter` dispatches to via `delegatecall`. Every other network
 * keeps the plain, unmodified `BetaHub.sol` (composed from
 * `HubPartyRegistry`/`HubCompositionRegistry`/`HubAnchorJudge` via normal
 * inheritance, which costs nothing extra there since there's no per-tx gas
 * ceiling to work around).
 *
 * Unlike that base-contract chain (built for a single monolithic
 * deployment, where an abstract base's virtual hooks let `BetaHub`'s own
 * `Params` stay private to the final contract), a delegatecall split needs
 * one FLAT shared storage layout every facet and the router inherit
 * identically — so this file inlines everything directly: every error,
 * type, event, state variable, and shared internal helper from
 * `HubPartyRegistry` + `HubCompositionRegistry` + `HubAnchorJudge` +
 * `BetaHub`'s own additions, with no virtual-hook indirection (each facet
 * just reads `params.xyz` directly). `SELF_NETWORK_ID` moves from
 * `immutable` to regular storage — immutables don't propagate through
 * `delegatecall`, same fix `iPoWStorage`/`BetaVaultStorage` needed.
 */
abstract contract BetaHubStorage is ReentrancyGuard {
    using SafeERC20 for IERC20;

    // ------------------------------------------------------------ errors
    error Unauthorized();
    error BondTooSmall();
    error PartyDead();
    error PartyExists();
    error NotApproved();
    error UnbondNotRequested();
    error UnbondNotReady();
    error TransferFailed();
    error InvalidComponents();
    error DuplicateComponent();
    error TooManyLocal();
    error NoLocalComponent();
    error UnsupportedNetwork();
    error CompositionExists();
    error NoComposition();
    error MalformedStatement();
    error KindMismatch();
    error StatementHashMismatch();
    error NotOperator();
    error NotAuditor();
    error NoParty();
    error BadAnchorState();
    error TargetNotProcessed();
    error SkipNotReady();
    error UnsupportedKind();
    error MalformedTx();
    error WitnessSerialization();
    error AlreadyProcessed();
    error InvalidHeader();
    error InvalidMerkleBranch();
    error NotOnStatementChain();
    error BadAnchorPayload();
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
    error InvalidConstructor();

    // ------------------------------------------------------------ party/bond
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

    // ------------------------------------------------------------ composition registry
    uint256 public constant MAX_COMPONENTS = 8;
    uint256 public constant MAX_LOCAL_COMPONENTS = 4;
    uint256 public constant SOLANA_NETWORK_ID = 3;

    struct Component {
        uint256 networkId;
        address tokenId;
        uint256 amountPerUnit;
    }

    struct Composition {
        bool exists;
        Component[] components;
    }

    // Not immutable — see contract-level comment.
    uint256 public SELF_NETWORK_ID;

    mapping(uint64 => Composition) internal _compositions;

    event CompositionRegistered(uint64 indexed id, uint256 componentCount);

    // ------------------------------------------------------------ anchor judging
    uint8 public constant KIND_MINT = 1;
    uint8 public constant KIND_RELEASE = 2;
    uint8 public constant KIND_VETO = 3;
    uint8 public constant KIND_CANCEL = 4;
    uint8 public constant KIND_ATTEST = 5;
    uint8 public constant KIND_CLEAR = 6;
    uint8 public constant KIND_ALIVE = 7;
    uint8 internal constant ANCHOR_VERSION = 1;

    enum AnchorStatus {
        None,
        Queued,
        Exercised,
        Slashed,
        Skipped,
        Cancelled
    }

    struct ProcessedAnchor {
        bytes32 partyId;
        uint8 kind;
        AnchorStatus status;
        bytes32 statementHash;
        uint64 blockHeight;
        uint64 processedAt;
        uint64 compositionId;
        uint8 componentIndex;
        uint64 lockId;
        uint64 units;
        uint64 challengeUntil;
        bool held;
        bool settled;
        bytes32 pendingId;
    }

    mapping(bytes32 => ProcessedAnchor) public anchors;
    mapping(bytes32 => bytes32) public mintAttester;
    uint256 public insurance;

    event AnchorProcessed(bytes32 indexed partyId, bytes32 indexed txidLE, uint8 kind, AnchorStatus status);
    event ComponentHeld(bytes32 indexed txidLE, bool held);

    // ------------------------------------------------------------ BetaHub's own additions
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
    Params public params;
    bool public paused;
    uint256 public rewardPool;
    HubToken public token;

    mapping(bytes32 => Pending) public pending;

    event PendingCreated(bytes32 indexed pendingId, address indexed user, uint64 nonce, uint64 compositionId, uint64 units, uint64 deadline);
    event PendingApproved(bytes32 indexed pendingId, uint64[] remoteLockId);
    event Minted(bytes32 indexed pendingId, address indexed user, uint256 amount);
    event PendingExpired(bytes32 indexed pendingId);

    // ------------------------------------------------------------ shared internal helpers
    function _validateParams(Params memory p) internal pure {
        if (p.tChallengeSecs == 0 || p.unbondDelaySecs == 0 || p.bountyBps > BPS_DENOM) revert InvalidParams();
    }

    function _pay(address to, uint256 amount) internal {
        if (amount == 0) return;
        (bool ok,) = payable(to).call{value: amount}("");
        if (!ok) revert TransferFailed();
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

    /// @dev Generalized from `BetaVault._slash` — see `HubPartyRegistry._slash`'s
    /// comment (the reference this mirrors) for why the remainder is
    /// returned rather than paid out here.
    function _slash(bytes32 partyId, Party storage party, uint256 amount, address submitter)
        internal
        returns (uint256 slashed, uint256 remainder)
    {
        slashed = amount < party.bond ? amount : party.bond;
        party.bond -= slashed;
        party.dead = true;
        totalBonds -= slashed;
        uint256 bounty = (slashed * params.bountyBps) / BPS_DENOM;
        remainder = slashed - bounty;
        emit Slashed(partyId, slashed, submitter, address(0));
        _pay(submitter, bounty);
    }

    function _pendingKey(address user, uint64 nonce) internal pure returns (bytes32) {
        return keccak256(abi.encodePacked(user, nonce));
    }

    function getComposition(uint64 id) external view returns (Component[] memory) {
        return _compositions[id].components;
    }

    function compositionExists(uint64 id) public view returns (bool) {
        return _compositions[id].exists;
    }

    function pendingRemoteLockId(bytes32 pendingId) external view returns (uint64[] memory) {
        return pending[pendingId].remoteLockId;
    }

    function pendingRemoteAnchorTxid(bytes32 pendingId) external view returns (bytes32[] memory) {
        return pending[pendingId].remoteAnchorTxid;
    }
}
