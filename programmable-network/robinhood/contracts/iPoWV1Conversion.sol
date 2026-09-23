// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

import {BitcoinPrimitives} from "./libraries/BitcoinPrimitives.sol";
import {IERC20} from "@openzeppelin/contracts/token/ERC20/IERC20.sol";
import {SafeERC20} from "@openzeppelin/contracts/token/ERC20/utils/SafeERC20.sol";

/// @dev Minimal read-only view into `iPoWV1`'s existing header relay — this
/// contract never writes to it, matching `ipow-conversion`'s read-only
/// cross-program relationship with `ipow` on the Solana side. Both mappings
/// are already `public` on `iPoWV1`, so their auto-generated getters match
/// this interface exactly.
interface IIPoWV1Headers {
    function globalTipHeight() external view returns (uint256);

    function globalHeightToHashLE(uint256 height) external view returns (bytes32);

    function globalHeaders(bytes32 hashLE)
        external
        view
        returns (
            bytes32 prevHashLE,
            bytes32 merkleRootLE,
            uint32 nBits,
            uint32 timestamp,
            bool set,
            uint64 arrivalTime
        );
}

/**
 * @title iPoWV1Conversion
 * @notice Value-moving Native<->Bitcoin conversion, split out as its own contract —
 * see docs/DESIGN_V2.md. Replaces the single-fixed-operator Conversion that still
 * lives, unmodified, inside `iPoWV1`: the claiming role here is a permissionless
 * windowed staked auction, mirroring `ipow-conversion`'s Solana implementation
 * exactly (same field names/semantics, translated to Solidity).
 * @dev Reads `iPoWV1`'s existing header relay directly, read-only, via
 * `IIPoWV1Headers`. `tokenAddr == address(0)` means native ETH; any other
 * address is the ERC20 token a conversion's `nativeAmount` is denominated
 * in — mirrors the Solana side's `token_mint == Pubkey::default()`
 * convention exactly. Auction stakes and the commit fee always move native
 * ETH regardless of `tokenAddr`, same split the Solana side uses.
 */
contract iPoWV1Conversion {
    using SafeERC20 for IERC20;
    // ========= ERRORS =========
    error Unauthorized();
    error ZeroValue();
    error NeedDutyWindow();
    error BadState();
    error WrongConversionType();
    error ClaimNotBetter();
    error BelowAskedRate();
    error RateNotBetter();
    error TooManyTokens();
    error DuplicateTokenMint();
    error InvalidTokenMint();
    error ClaimWindowClosed();
    error ClaimWindowStillOpen();
    error NoClaimsYet();
    error BelowRequiredBond();
    error DutyNotExpired();
    error IncorrectWindow();
    error NoHeadersYet();
    error AlreadyVerified();
    error IncorrectValue();
    error BadBitcoinProgram();
    error ProgramAlreadyUsed();
    error InvalidHeader();
    error IncorrectCommitFee();
    error UnexpectedValue();
    error IncorrectNetwork();
    error IncorrectNetworkAddress();
    error InvalidNetworkConfig();

    // ========= EVENTS =========
    event ConversionCommitted(uint256 indexed txId, address indexed user, bool isNativeToBitcoin);
    event ClaimProposed(uint256 indexed txId, address indexed claimant, uint256 stakeAmount, uint256 proposedRateAmount);
    event ClaimFinalized(uint256 indexed txId, address indexed responsibleOperator);
    event ClaimReclaimed(uint256 indexed txId, bool forfeited);
    event ConversionDeposited(uint256 indexed txId, uint256 nativeAmount);
    event ConversionCompleted(uint256 indexed txId);
    event ConversionRefunded(uint256 indexed txId, uint256 refundNative);

    // ========= CONSTANTS =========
    uint256 public constant BPS_DENOM = 10_000;
    uint256 public constant DEPOSIT_BLOCKS_WINDOW = 10;
    uint256 public constant PROOF_BLOCKS_WINDOW = 40;
    uint256 public constant DIFF_PERIOD = 2016;
    uint256 public constant CLAIM_WINDOW_SEC = 15 minutes;
    /// @dev Kept short (1 minute), matching the Solana side's explicit call —
    /// an uncontested claim should resolve quickly, not sit through a long
    /// quiet period.
    uint256 public constant CLAIM_QUIET_PERIOD_SEC = 1 minutes;

    enum Status {
        None,
        Committed,
        Approved,
        Deposited,
        Completed,
        Refunded
    }

    /// @dev One extra token in a `token->bitcoin` bundle, beyond the
    /// primary `tokenAddr`/`nativeAmount` slot — see `Conversion.
    /// extraTokens`.
    struct TokenAmount {
        address mint;
        uint256 amount;
    }

    struct Conversion {
        address user;
        bool isNativeToBitcoin;
        bytes userProgram;
        bytes ipowReceiveProgram;
        uint256 networkId;
        /// @dev `address(0)` = native ETH; otherwise the ERC20 this
        /// conversion's `nativeAmount` is denominated in.
        address tokenAddr;

        uint256 nativeAmount;
        uint256 bitcoinAmount;
        uint256 commitFee;
        uint256 reservedNative;

        /// @dev `token->bitcoin` only — up to 3 additional (mint, amount)
        /// pairs beyond the primary slot, settled by the same single
        /// `bitcoinAmount` proof. Always empty for `bitcoin->token` and for
        /// any single-token `token->bitcoin` conversion. Solidity's
        /// auto-generated public-mapping getter skips array members, so
        /// `conversions(txId)`'s tuple shape is unaffected by this field —
        /// existing callers (`BetaMint`/`BasketBridge`'s `conversions()`
        /// interface declarations) need no changes.
        TokenAmount[] extraTokens;

        uint256 createdAt;
        uint256 depositedAt;
        uint256 operatorDutyExpiresAt;

        Status status;

        bool windowStarted;
        uint256 windowStartHeight;

        bool proofVerified;
        uint256 proofBlockHeight;

        // --- Auction fields, mirroring `ipow-message-relay`'s claim mechanics ---
        address responsibleOperator;
        uint256 stakedBond;
        uint256 requiredBond;
        uint256 claimStartedAt;
        uint256 lastClaimAt;
        uint256 dutyWindowSeconds;
        uint256 bounty;

        /// @dev `openBundleTunnel`-only destination metadata — `networkId`
        /// existed as a field already (vestigial before this, never
        /// actually stored an address to go with it: `commitTokenToBitcoin`/
        /// `commitBitcoinToToken` are direct-Bitcoin only). Appended at the
        /// end, not inserted alongside `networkId` above, so this stays a
        /// pure addition: `BetaMint.sol`/`BasketBridge.sol`'s locally-
        /// redeclared `conversions()` interfaces only decode a *prefix* of
        /// this tuple and are unaffected by a new trailing field, whereas
        /// inserting it earlier would shift every field after it and break
        /// their positional decode.
        bytes networkAddress;
    }

    address public governance;
    IIPoWV1Headers public immutable ipowHeaders;
    uint256 public commitFeeBps;

    uint256 public nextTxId = 1;
    mapping(uint256 => Conversion) public conversions;

    /// @dev Keyed by `tokenAddr` (`address(0)` = native ETH) — replaces the
    /// old single native-only counters, same reasoning as the Solana side's
    /// per-mint `Pool`. `totalHeldCommitFees` is always keyed by
    /// `address(0)`, since the commit fee is always native ETH regardless
    /// of what a conversion itself moves. `totalReservedAmount` is a pure
    /// aggregate for introspection, not load-bearing for correctness — each
    /// conversion's own reservation is backed by that specific claimant's
    /// own self-escrow in `proposeClaimConversion`, not by this counter (no
    /// more separate governance-funded pool — see that function's comment).
    mapping(address => uint256) public totalLockedDeposits;
    mapping(address => uint256) public totalReservedAmount;
    uint256 public totalHeldCommitFees;

    /// @dev Auction stakes/bounties are always native ETH regardless of what
    /// a conversion itself moves — same convention the Solana side uses.
    /// Tracked implicitly via this contract's own balance minus the buckets
    /// above; no separate escrow needed the way Solana's PDA model requires.

    mapping(bytes => bool) public usedIPoWPrograms;

    /// @dev Same shape as `iPoWV1.sol`'s own `NetworkConfig`
    /// (`base/iPoWV1Types.sol`) — declared locally rather than imported
    /// since this contract has no inheritance relationship with `iPoWV1`,
    /// but the field names/registry pattern (`addNetwork`/`removeNetwork`
    /// below) are a deliberate match, not a reinvention.
    struct NetworkConfig {
        bool enabled;
        uint16 minAddrLen;
        uint16 maxAddrLen;
    }

    /// @dev Registry for `openBundleTunnel`'s `networkId != 0` destination
    /// metadata. This contract has no `SELF_NETWORK_ID` concept (nothing
    /// here ever needs "this chain's own network id" — unlike `iPoWV1`,
    /// only `openBundleTunnel` ever reads this registry), so that guard
    /// isn't ported; everything else mirrors `iPoWV1.sol` directly.
    mapping(uint256 => NetworkConfig) public networkConfigs;

    modifier validTx(uint256 txId) {
        if (txId == 0 || txId >= nextTxId) revert BadState();
        _;
    }

    modifier onlyGovernance() {
        if (msg.sender != governance) revert Unauthorized();
        _;
    }

    constructor(address _governance, address _ipowHeaders, uint256 _commitFeeBps) {
        if (_governance == address(0) || _ipowHeaders == address(0) || _commitFeeBps > BPS_DENOM) {
            revert BadState();
        }
        governance = _governance;
        ipowHeaders = IIPoWV1Headers(_ipowHeaders);
        commitFeeBps = _commitFeeBps;
    }

    /// @notice Registers a destination network for `openBundleTunnel`'s
    /// `networkId`/`networkAddress` fields — governance-gated, same
    /// validation `iPoWV1.sol`'s own `addNetwork` already enforces
    /// (`networkId == 0`, already-enabled, or an inverted/zero length
    /// bound all revert `InvalidNetworkConfig`).
    function addNetwork(uint256 networkId, uint16 minAddrLen, uint16 maxAddrLen)
        external
        onlyGovernance
    {
        if (
            networkId == 0 ||
            networkConfigs[networkId].enabled ||
            minAddrLen == 0 ||
            minAddrLen > maxAddrLen
        ) revert InvalidNetworkConfig();

        networkConfigs[networkId] = NetworkConfig({
            enabled: true,
            minAddrLen: minAddrLen,
            maxAddrLen: maxAddrLen
        });
    }

    /// @notice Mirrors `iPoWV1.sol`'s own `removeNetwork`.
    function removeNetwork(uint256 networkId) external onlyGovernance {
        if (networkId == 0 || !networkConfigs[networkId].enabled) revert InvalidNetworkConfig();
        delete networkConfigs[networkId];
    }

    /// @dev Pulls `amount` of `tokenAddr` from `from` and returns what this
    /// contract's own balance actually increased by — not the nominal
    /// `amount` requested. `safeTransferFrom` only guarantees the *call*
    /// succeeded (handles missing/non-standard return values); it says
    /// nothing about whether the full amount arrived. A fee-on-transfer
    /// ERC20 would otherwise let this contract's accounting silently
    /// overstate its real holdings. Both current callers
    /// (`depositApprovedConversion`, `proposeClaimConversion`'s bitcoin->
    /// native self-escrow) require an exact match and revert on a
    /// shortfall — each moves a value that's already load-bearing for a
    /// promise made elsewhere, so silently accepting less would
    /// under-collateralize it.
    function _pullToken(address tokenAddr, address from, uint256 amount) internal returns (uint256) {
        uint256 before = IERC20(tokenAddr).balanceOf(address(this));
        IERC20(tokenAddr).safeTransferFrom(from, address(this), amount);
        return IERC20(tokenAddr).balanceOf(address(this)) - before;
    }

    /// @dev Native ETH or ERC20 payout, depending on `tokenAddr` — every
    /// release of `nativeAmount`/`reservedNative` goes through this one
    /// place.
    function _payOut(address tokenAddr, address to, uint256 amount) internal {
        if (amount == 0) return;
        if (tokenAddr == address(0)) {
            (bool ok, ) = payable(to).call{value: amount}("");
            if (!ok) revert Unauthorized();
        } else {
            IERC20(tokenAddr).safeTransfer(to, amount);
        }
    }

    /// @notice Renamed from `commitNativeToBitcoin` — "native" only ever
    /// meant "the default case" (`tokenAddr == address(0)`); this already
    /// accepted any ERC20. Also accepts an optional bundle: up to 3
    /// `extraTokens` beyond the primary `tokenAddr`/`nativeAmount` slot,
    /// all locked by this same conversion and settled by the same single
    /// `bitcoinAmount` proof. Every existing single-token caller is
    /// unaffected — pass an empty `extraTokens` array and behavior is
    /// identical to before the rename. Direct-Bitcoin only (`networkId ==
    /// 0`) for this pass — see docs/DESIGN_V2.md; the multi-network
    /// routing `ipow`'s original Conversion supports is orthogonal to the
    /// auction redesign and can be added back the same way if/when it's
    /// needed here.
    function commitTokenToBitcoin(
        uint256 nativeAmount,
        uint256 bitcoinAmount,
        bytes calldata userProgram,
        uint256 requiredBond,
        address tokenAddr,
        TokenAmount[] calldata extraTokens
    ) external payable {
        if (nativeAmount == 0 || bitcoinAmount == 0) revert ZeroValue();
        if (requiredBond == 0) revert ZeroValue();
        if (userProgram.length == 0 || userProgram.length > 80) revert BadBitcoinProgram();

        if (extraTokens.length > 3) revert TooManyTokens();
        for (uint256 i = 0; i < extraTokens.length; i++) {
            if (extraTokens[i].amount == 0) revert ZeroValue();
            // Only the primary slot may be native ETH (`address(0)`) — an
            // extra token slot at `address(0)` would be ambiguous with it.
            if (extraTokens[i].mint == address(0)) revert InvalidTokenMint();
            if (extraTokens[i].mint == tokenAddr) revert DuplicateTokenMint();
            for (uint256 j = i + 1; j < extraTokens.length; j++) {
                if (extraTokens[i].mint == extraTokens[j].mint) revert DuplicateTokenMint();
            }
        }

        // The commit fee is always native ETH, regardless of `tokenAddr` —
        // no value transfer for `nativeAmount` itself happens at commit
        // time either way (native->bitcoin only escrows it later, at
        // `depositApprovedConversion`).
        uint256 requiredFee = (nativeAmount * commitFeeBps) / BPS_DENOM;
        if (msg.value != requiredFee) revert IncorrectCommitFee();

        uint256 txId = nextTxId++;
        Conversion storage c = conversions[txId];
        c.user = msg.sender;
        c.isNativeToBitcoin = true;
        c.userProgram = userProgram;
        c.tokenAddr = tokenAddr;
        c.nativeAmount = nativeAmount;
        c.bitcoinAmount = bitcoinAmount;
        c.commitFee = msg.value;
        c.createdAt = block.timestamp;
        c.status = Status.Committed;
        c.requiredBond = requiredBond;
        c.claimStartedAt = block.timestamp;
        c.lastClaimAt = block.timestamp;
        for (uint256 i = 0; i < extraTokens.length; i++) {
            c.extraTokens.push(extraTokens[i]);
        }

        totalHeldCommitFees += msg.value;
        emit ConversionCommitted(txId, msg.sender, true);
    }

    function commitBitcoinToToken(
        uint256 bitcoinAmount,
        uint256 nativeAmount,
        bytes calldata userProgram,
        uint256 requiredBond,
        address tokenAddr
    ) external payable {
        if (nativeAmount == 0 || bitcoinAmount == 0) revert ZeroValue();
        if (requiredBond == 0) revert ZeroValue();
        if (userProgram.length == 0 || userProgram.length > 80) revert BadBitcoinProgram();

        uint256 requiredFee = (nativeAmount * commitFeeBps) / BPS_DENOM;
        if (msg.value != requiredFee) revert IncorrectCommitFee();

        uint256 txId = nextTxId++;
        Conversion storage c = conversions[txId];
        c.user = msg.sender;
        c.isNativeToBitcoin = false;
        c.userProgram = userProgram;
        c.tokenAddr = tokenAddr;
        c.nativeAmount = nativeAmount;
        c.bitcoinAmount = bitcoinAmount;
        c.commitFee = msg.value;
        c.createdAt = block.timestamp;
        c.status = Status.Committed;
        c.requiredBond = requiredBond;
        c.claimStartedAt = block.timestamp;
        c.lastClaimAt = block.timestamp;

        totalHeldCommitFees += msg.value;
        emit ConversionCommitted(txId, msg.sender, false);
    }

    /// @notice Permissionless, atomic "open + fully fund" path for
    /// `bitcoin->token` conversions that carry a multi-token bundle — see
    /// docs/DESIGN_V2.md §2. Collapses what would otherwise be
    /// `commitBitcoinToToken` + `proposeClaimConversion` +
    /// `finalizeClaimConversion` into one call: the opener self-funds the
    /// *entire* fixed bundle (primary + up to 3 extras) right here,
    /// becomes `responsibleOperator`, and the conversion is immediately
    /// `Approved`/`windowStarted`.
    /// @dev There is deliberately no auction: `commitBitcoinToToken`
    /// already covers the single-token, rate-competed case, and a bundle
    /// can't be rate-competed at all without a price oracle (comparing
    /// "claimant A offers 100 USDC + 5 SOL" against "claimant B offers 90
    /// USDC + 6 SOL" has no natural ordering). Restricted to `networkId
    /// != 0` so this stays purely additive: a direct-Bitcoin, single-token
    /// `bitcoin->token` conversion already has a perfectly good auctioned
    /// path (`commitBitcoinToToken`) — this isn't a backdoor around it.
    /// Mirrors `ipow-conversion`'s Solana `open_bundle_tunnel` exactly.
    function openBundleTunnel(
        uint256 nativeAmount,
        uint256 bitcoinAmount,
        address tokenAddr,
        TokenAmount[] calldata extraTokens,
        address destAddress,
        uint256 networkId,
        bytes calldata networkAddress,
        uint256 dutyWindowSeconds,
        bytes calldata ipowReceiveProgram
    ) external payable {
        if (nativeAmount == 0 || bitcoinAmount == 0) revert ZeroValue();
        if (dutyWindowSeconds == 0) revert NeedDutyWindow();

        if (extraTokens.length > 3) revert TooManyTokens();
        for (uint256 i = 0; i < extraTokens.length; i++) {
            if (extraTokens[i].amount == 0) revert ZeroValue();
            if (extraTokens[i].mint == address(0)) revert InvalidTokenMint();
            if (extraTokens[i].mint == tokenAddr) revert DuplicateTokenMint();
            for (uint256 j = i + 1; j < extraTokens.length; j++) {
                if (extraTokens[i].mint == extraTokens[j].mint) revert DuplicateTokenMint();
            }
        }

        if (ipowReceiveProgram.length == 0 || ipowReceiveProgram.length > 80) revert BadBitcoinProgram();
        if (usedIPoWPrograms[ipowReceiveProgram]) revert ProgramAlreadyUsed();
        usedIPoWPrograms[ipowReceiveProgram] = true;

        // Tunnel-only — see the function's own doc comment. Same two
        // checks as `iPoWV1.sol`'s own `_validateNetwork`/
        // `_validateNetworkAddress` (minus the `SELF_NETWORK_ID` guard,
        // which doesn't apply here — see `networkConfigs`'s own comment).
        if (networkId == 0 || !networkConfigs[networkId].enabled) revert IncorrectNetwork();
        NetworkConfig memory netConfig = networkConfigs[networkId];
        if (networkAddress.length < netConfig.minAddrLen || networkAddress.length > netConfig.maxAddrLen) {
            revert IncorrectNetworkAddress();
        }

        // Self-escrow the primary slot — same pattern `depositApprovedConversion`
        // already uses.
        if (tokenAddr == address(0)) {
            if (msg.value != nativeAmount) revert IncorrectValue();
        } else {
            if (msg.value != 0) revert UnexpectedValue();
            uint256 received = _pullToken(tokenAddr, msg.sender, nativeAmount);
            if (received != nativeAmount) revert IncorrectValue();
        }

        // Self-escrow the bundle — every extra is always ERC20 (enforced
        // above), same reasoning `depositApprovedConversion`'s own bundle
        // loop already documents.
        for (uint256 i = 0; i < extraTokens.length; i++) {
            uint256 receivedExtra = _pullToken(extraTokens[i].mint, msg.sender, extraTokens[i].amount);
            if (receivedExtra != extraTokens[i].amount) revert IncorrectValue();
        }

        uint256 txId = nextTxId++;
        Conversion storage c = conversions[txId];
        c.user = destAddress;
        c.isNativeToBitcoin = false;
        c.tokenAddr = tokenAddr;
        c.nativeAmount = nativeAmount;
        c.bitcoinAmount = bitcoinAmount;
        c.commitFee = 0;
        c.reservedNative = nativeAmount;
        for (uint256 i = 0; i < extraTokens.length; i++) {
            c.extraTokens.push(extraTokens[i]);
        }

        c.ipowReceiveProgram = ipowReceiveProgram;
        c.networkId = networkId;
        c.networkAddress = networkAddress;

        c.createdAt = block.timestamp;
        c.status = Status.Approved;
        c.requiredBond = 0;
        c.stakedBond = 0;
        c.claimStartedAt = block.timestamp;
        c.lastClaimAt = block.timestamp;
        c.dutyWindowSeconds = dutyWindowSeconds;
        c.operatorDutyExpiresAt = block.timestamp + dutyWindowSeconds;
        c.responsibleOperator = msg.sender;

        c.windowStarted = true;
        c.windowStartHeight = ipowHeaders.globalTipHeight();

        totalReservedAmount[tokenAddr] += nativeAmount;

        emit ConversionCommitted(txId, destAddress, false);
        emit ClaimFinalized(txId, msg.sender);
    }

    /// @notice Windowed auction for the claiming role, competing on **rate**
    /// not stake — see docs/DESIGN_V2.md. `requiredBond` is a fixed anti-
    /// griefing deposit every claimant must post (refunded in full to an
    /// out-bid claimant, never itself the deciding factor). What wins a
    /// claim is `proposedRateAmount`: for native->bitcoin, how much real
    /// BTC this claimant will pay the user (`bitcoinAmount`, higher is
    /// better for the user); for bitcoin->native, how much native/ERC20
    /// value they'll front for the fixed required Bitcoin proof
    /// (`nativeAmount`, again higher is better for the user). The
    /// committer's own commit-time value is what they asked for — meeting
    /// it exactly can already win; every later out-bidding claim must
    /// strictly beat the current best. `submitBitcoinMerkleProofWithTx`
    /// already enforces whatever rate wins here (`provedBitcoin >=
    /// c.bitcoinAmount`), so a claimant can never renege on it.
    ///
    /// For bitcoin->native, the claimant also self-escrows their own
    /// proposed `nativeAmount` of `tokenAddr` right here — there's no
    /// separate governance-funded liquidity pool (removed along with
    /// `addLiquidity`/`removeLiquidity`); whoever wins the auction
    /// personally fronts the value they're promising to deliver, symmetric
    /// with how native->bitcoin already has the *user* front it via
    /// `depositApprovedConversion`. This only happens pre-finalize
    /// (`!c.windowStarted`): out-staking during that window refunds the
    /// previous claimant's own escrowed amount (which may be smaller, since
    /// rates only improve) and escrows the new claimant's own (larger)
    /// proposal, exactly mirroring the stake refund below. Once a duty is
    /// locked in by `finalizeClaimConversion`, the escrowed amount is real
    /// and must never move (or its recorded `nativeAmount`) again except
    /// via proof-based payout or force-claim — a later reclaim-and-reopen
    /// round never re-touches it, only the stake changes hands (see
    /// `reclaimExpiredConversion`). Mirrors `ipow-conversion`'s Solana
    /// implementation exactly, including this same "rate only moves while
    /// `!windowStarted`" gating.
    ///
    /// A native-ETH escrow rides along in `msg.value` on top of the stake
    /// (this contract has no separate escrow vault the way Solana's PDA
    /// model needs — it's all one pooled balance, same as `totalLockedDeposits`
    /// already assumes); an ERC20 escrow is pulled separately via
    /// `_pullToken`.
    function proposeClaimConversion(
        uint256 txId,
        uint256 proposedRateAmount,
        uint256 dutyWindowSeconds,
        bytes calldata ipowReceiveProgram
    ) external payable validTx(txId) {
        Conversion storage c = conversions[txId];

        // Claimable in two situations: the normal fresh-commit case
        // (`Committed`), or a round reopened by `reclaimExpiredConversion`
        // — see that function's own comment.
        bool claimable = c.status == Status.Committed ||
            ((c.status == Status.Approved || c.status == Status.Deposited) &&
                c.operatorDutyExpiresAt == 0);
        if (!claimable) revert BadState();
        if (dutyWindowSeconds == 0) revert NeedDutyWindow();
        if (block.timestamp > c.claimStartedAt + CLAIM_WINDOW_SEC) revert ClaimWindowClosed();

        bool isFirstClaim = c.responsibleOperator == address(0);
        uint256 currentRateValue = c.isNativeToBitcoin ? c.bitcoinAmount : c.nativeAmount;
        if (isFirstClaim) {
            if (proposedRateAmount < currentRateValue) revert BelowAskedRate();
        } else {
            if (proposedRateAmount <= currentRateValue) revert RateNotBetter();
        }

        bool selfEscrow = !c.isNativeToBitcoin && !c.windowStarted;
        uint256 nativeEscrowPortion = (selfEscrow && c.tokenAddr == address(0)) ? proposedRateAmount : 0;
        if (msg.value < nativeEscrowPortion) revert BelowRequiredBond();
        uint256 stakeAmount = msg.value - nativeEscrowPortion;
        if (stakeAmount < c.requiredBond) revert BelowRequiredBond();

        if (!isFirstClaim) {
            address previous = c.responsibleOperator;
            uint256 previousStake = c.stakedBond;
            (bool ok, ) = payable(previous).call{value: previousStake}("");
            if (!ok) revert Unauthorized();
            if (selfEscrow) {
                // `c.nativeAmount` still holds the *previous* claimant's
                // own winning proposal here — not yet overwritten by this
                // claim's (necessarily larger) one below.
                uint256 previousNativeAmount = c.nativeAmount;
                totalReservedAmount[c.tokenAddr] -= previousNativeAmount;
                _payOut(c.tokenAddr, previous, previousNativeAmount);
            }
        }

        c.responsibleOperator = msg.sender;
        c.stakedBond = stakeAmount;
        c.lastClaimAt = block.timestamp;
        c.dutyWindowSeconds = dutyWindowSeconds;

        if (selfEscrow) {
            if (c.tokenAddr != address(0)) {
                uint256 received = _pullToken(c.tokenAddr, msg.sender, proposedRateAmount);
                if (received != proposedRateAmount) revert IncorrectValue();
            }
            totalReservedAmount[c.tokenAddr] += proposedRateAmount;
            // Lock in the winning rate here, inside the same
            // `!windowStarted` gate as the self-escrow move itself — this
            // field *is* the real, already-escrowed reservation amount for
            // bitcoin->native, so it must never change once real (same
            // reasoning `reservedNative`/`windowStartHeight` already use).
            c.nativeAmount = proposedRateAmount;
        }

        // native->bitcoin's `bitcoinAmount` is never backed by an on-chain
        // escrow at propose time (only a later-proven promise), so unlike
        // bitcoin->native's `nativeAmount` above, it's always safe to
        // update regardless of `windowStarted`.
        if (c.isNativeToBitcoin) {
            c.bitcoinAmount = proposedRateAmount;
        }

        if (!c.isNativeToBitcoin) {
            if (ipowReceiveProgram.length == 0 || ipowReceiveProgram.length > 80) revert BadBitcoinProgram();
            if (usedIPoWPrograms[ipowReceiveProgram]) revert ProgramAlreadyUsed();
            usedIPoWPrograms[ipowReceiveProgram] = true;
            c.ipowReceiveProgram = ipowReceiveProgram;
        } else if (c.isNativeToBitcoin && c.userProgram.length == 0) {
            if (ipowReceiveProgram.length == 0 || ipowReceiveProgram.length > 80) revert BadBitcoinProgram();
            if (usedIPoWPrograms[ipowReceiveProgram]) revert ProgramAlreadyUsed();
            usedIPoWPrograms[ipowReceiveProgram] = true;
            c.userProgram = ipowReceiveProgram;
        }

        emit ClaimProposed(txId, msg.sender, stakeAmount, proposedRateAmount);
    }

    /// @notice Permissionless: once the quiet period passes with no better
    /// stake, locks in the current highest staker and starts the duty
    /// window. For bitcoin->native, the payout value itself was already
    /// self-escrowed by the (now locked-in) claimant back in
    /// `proposeClaimConversion` — this just records that amount onto
    /// `reservedNative` for later payout/force-claim to read.
    function finalizeClaimConversion(uint256 txId) external validTx(txId) {
        Conversion storage c = conversions[txId];

        bool claimable = c.status == Status.Committed ||
            ((c.status == Status.Approved || c.status == Status.Deposited) &&
                c.operatorDutyExpiresAt == 0);
        if (!claimable) revert BadState();
        if (c.responsibleOperator == address(0)) revert NoClaimsYet();
        if (block.timestamp <= c.lastClaimAt + CLAIM_QUIET_PERIOD_SEC) revert ClaimWindowStillOpen();

        if (c.status == Status.Committed) {
            c.status = Status.Approved;
        }
        c.operatorDutyExpiresAt = block.timestamp + c.dutyWindowSeconds;

        if (!c.windowStarted) {
            c.windowStarted = true;
            c.windowStartHeight = ipowHeaders.globalTipHeight();

            if (!c.isNativeToBitcoin) {
                c.reservedNative = c.nativeAmount;
            }
        }

        emit ClaimFinalized(txId, c.responsibleOperator);
    }

    /// @notice Permissionless. A claimant who wins and then goes dark needs
    /// their stake resolved — see docs/DESIGN_V2.md for the two-outcome
    /// split this mirrors from `reclaim_expired_message`.
    function reclaimExpiredConversion(uint256 txId) external validTx(txId) {
        Conversion storage c = conversions[txId];

        if (c.status != Status.Approved && c.status != Status.Deposited) revert BadState();
        if (c.operatorDutyExpiresAt == 0 || block.timestamp <= c.operatorDutyExpiresAt) {
            revert DutyNotExpired();
        }

        bool realCommitmentHappened = !c.isNativeToBitcoin || c.status == Status.Deposited;

        if (realCommitmentHappened) {
            c.bounty += c.stakedBond;
            c.requiredBond = c.stakedBond;
        } else {
            address previous = c.responsibleOperator;
            uint256 refund = c.stakedBond;
            (bool ok, ) = payable(previous).call{value: refund}("");
            if (!ok) revert Unauthorized();
            c.status = Status.Committed;
        }

        c.stakedBond = 0;
        c.responsibleOperator = address(0);
        c.operatorDutyExpiresAt = 0;
        c.claimStartedAt = block.timestamp;
        c.lastClaimAt = block.timestamp;

        emit ClaimReclaimed(txId, realCommitmentHappened);
    }

    function depositApprovedConversion(uint256 txId) external payable validTx(txId) {
        Conversion storage c = conversions[txId];

        if (!c.isNativeToBitcoin) revert WrongConversionType();
        if (msg.sender != c.user) revert Unauthorized();
        if (c.status != Status.Approved) revert BadState();
        if (!c.windowStarted) revert NoHeadersYet();
        if (ipowHeaders.globalTipHeight() > c.windowStartHeight + (DEPOSIT_BLOCKS_WINDOW - 1)) {
            revert IncorrectWindow();
        }

        if (c.tokenAddr == address(0)) {
            if (msg.value != c.nativeAmount) revert IncorrectValue();
        } else {
            if (msg.value != 0) revert UnexpectedValue();
            // `nativeAmount` is already load-bearing here: the eventual
            // payout and the operator's Bitcoin-side proof requirement were
            // both sized against it back at commit time (mirrors
            // `proposeClaimConversion`'s own self-escrow requirement for the
            // reverse direction). A
            // fee-on-transfer token silently delivering less would
            // under-collateralize a promise already made, so this requires
            // an exact match rather than accepting whatever arrived.
            uint256 received = _pullToken(c.tokenAddr, msg.sender, c.nativeAmount);
            if (received != c.nativeAmount) revert IncorrectValue();
        }

        // Bundle: every extra token is always an ERC20 (never native ETH —
        // enforced at commit), so this is always `_pullToken`, no native
        // branch needed.
        for (uint256 i = 0; i < c.extraTokens.length; i++) {
            TokenAmount storage extra = c.extraTokens[i];
            uint256 receivedExtra = _pullToken(extra.mint, msg.sender, extra.amount);
            if (receivedExtra != extra.amount) revert IncorrectValue();
        }

        c.status = Status.Deposited;
        c.depositedAt = block.timestamp;
        totalLockedDeposits[c.tokenAddr] += c.nativeAmount;

        emit ConversionDeposited(txId, c.nativeAmount);
    }

    /// @notice Header must already be relayed on `iPoWV1` — this pass drops
    /// the two-phase "cache an attempt, complete it once the header
    /// arrives" flow for simplicity, same scope reduction the Solana side
    /// makes. See docs/DESIGN_V2.md.
    function submitBitcoinMerkleProofWithTx(
        uint256 txId,
        bytes calldata txRaw,
        uint256 voutIndex,
        uint256 blockHeight,
        bytes32[] calldata branchLE,
        uint256 index
    ) external validTx(txId) {
        Conversion storage c = conversions[txId];

        if (!c.windowStarted) revert NoHeadersYet();
        if (
            blockHeight < c.windowStartHeight ||
            blockHeight > c.windowStartHeight + (PROOF_BLOCKS_WINDOW - 1)
        ) revert IncorrectWindow();
        if (c.proofVerified) revert AlreadyVerified();

        if (c.isNativeToBitcoin) {
            if (msg.sender != c.responsibleOperator) revert Unauthorized();
        } else {
            if (msg.sender != c.user) revert Unauthorized();
        }

        bytes32 headerHashLE = ipowHeaders.globalHeightToHashLE(blockHeight);
        (, bytes32 merkleRootLE, , , bool set, ) = ipowHeaders.globalHeaders(headerHashLE);
        if (!set) revert InvalidHeader();

        bytes32 txidLE = sha256(abi.encodePacked(sha256(txRaw)));
        (uint64 outValueSats, bytes memory outProgram) = BitcoinPrimitives._parseOutputAt(txRaw, voutIndex);

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
        if (current != merkleRootLE) revert InvalidHeader();

        uint256 feeToOperator = c.commitFee;

        if (c.isNativeToBitcoin) {
            if (c.status != Status.Deposited) revert BadState();
            if (outValueSats < c.bitcoinAmount) revert IncorrectValue();
            if (keccak256(outProgram) != keccak256(c.userProgram)) revert BadBitcoinProgram();

            totalLockedDeposits[c.tokenAddr] -= c.nativeAmount;
            _payOut(c.tokenAddr, c.responsibleOperator, c.nativeAmount);

            // Bundle: pay out every extra token to the same operator, gated
            // by the same single proof already checked above.
            for (uint256 i = 0; i < c.extraTokens.length; i++) {
                TokenAmount storage extra = c.extraTokens[i];
                _payOut(extra.mint, c.responsibleOperator, extra.amount);
            }
        } else {
            bytes memory expectedProg = c.ipowReceiveProgram.length == 0 ? c.userProgram : c.ipowReceiveProgram;
            if (keccak256(outProgram) != keccak256(expectedProg)) revert BadBitcoinProgram();
            if (outValueSats < c.bitcoinAmount) revert IncorrectValue();

            uint256 payout = c.nativeAmount;
            totalReservedAmount[c.tokenAddr] -= payout;
            _payOut(c.tokenAddr, c.user, payout);

            // Bundle: only ever populated by `openBundleTunnel` (no
            // auctioned bitcoin->token commit can carry one). Same shape
            // as the `isNativeToBitcoin` branch's own bundle loop above,
            // paying out to `c.user` instead of the operator.
            for (uint256 i = 0; i < c.extraTokens.length; i++) {
                TokenAmount storage extra = c.extraTokens[i];
                _payOut(extra.mint, c.user, extra.amount);
            }
        }

        if (feeToOperator > 0) {
            c.commitFee = 0;
            totalHeldCommitFees -= feeToOperator;
            (bool ok, ) = payable(c.responsibleOperator).call{value: feeToOperator}("");
            if (!ok) revert Unauthorized();
        }

        // Auction stake + any accumulated bounty release to the operator on
        // success.
        uint256 stakePayout = c.stakedBond + c.bounty;
        c.stakedBond = 0;
        c.bounty = 0;
        (bool okStake, ) = payable(c.responsibleOperator).call{value: stakePayout}("");
        if (!okStake) revert Unauthorized();

        c.proofBlockHeight = blockHeight;
        c.status = Status.Completed;
        c.proofVerified = true;

        emit ConversionCompleted(txId);
    }

    /// @notice Permissionless — unchanged trigger/condition from the
    /// original design. The claimant's forfeited stake is handled
    /// separately by `reclaimExpiredConversion`.
    function refundNoProofNativeToBitcoin(uint256 txId) external validTx(txId) {
        Conversion storage c = conversions[txId];

        if (!c.isNativeToBitcoin) revert WrongConversionType();
        if (c.status != Status.Deposited) revert BadState();

        uint256 proofWindowEnd = c.windowStartHeight + (PROOF_BLOCKS_WINDOW - 1);
        if (ipowHeaders.globalTipHeight() <= proofWindowEnd) revert DutyNotExpired();

        c.status = Status.Refunded;

        uint256 refundedAmount = c.nativeAmount;
        uint256 refundedFee = c.commitFee;
        totalLockedDeposits[c.tokenAddr] -= refundedAmount;
        totalHeldCommitFees -= refundedFee;

        // Two separate transfers: `nativeAmount` follows `tokenAddr`, the
        // commit fee is always native ETH.
        _payOut(c.tokenAddr, c.user, refundedAmount);
        _payOut(address(0), c.user, refundedFee);

        // Bundle: refund every extra token back to the user too.
        for (uint256 i = 0; i < c.extraTokens.length; i++) {
            TokenAmount storage extra = c.extraTokens[i];
            _payOut(extra.mint, c.user, extra.amount);
        }

        emit ConversionRefunded(txId, refundedAmount);
    }

    /// @notice Force-claim, bitcoin->native only: if the winning claimant
    /// never streamed their duty in time, the user takes the
    /// already-reserved real pool capital directly — permissionless.
    function claimNativeOperatorExpired(uint256 txId) external validTx(txId) {
        Conversion storage c = conversions[txId];

        if (c.isNativeToBitcoin) revert WrongConversionType();
        if (c.status != Status.Approved) revert BadState();
        if (c.operatorDutyExpiresAt == 0 || block.timestamp <= c.operatorDutyExpiresAt) {
            revert DutyNotExpired();
        }

        c.status = Status.Completed;

        uint256 amountToClaim = c.reservedNative;
        totalReservedAmount[c.tokenAddr] -= amountToClaim;

        _payOut(c.tokenAddr, c.user, amountToClaim);

        // Bundle: only ever populated by `openBundleTunnel`. Without this,
        // a defaulted bundle tunnel's extra tokens would never resolve
        // even after duty expiry.
        for (uint256 i = 0; i < c.extraTokens.length; i++) {
            TokenAmount storage extra = c.extraTokens[i];
            _payOut(extra.mint, c.user, extra.amount);
        }

        emit ConversionCompleted(txId);
    }
}
