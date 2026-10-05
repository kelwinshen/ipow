// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

import {ReentrancyGuard} from "@openzeppelin/contracts/utils/ReentrancyGuard.sol";
import {IERC20} from "@openzeppelin/contracts/token/ERC20/IERC20.sol";
import {SafeERC20} from "@openzeppelin/contracts/token/ERC20/utils/SafeERC20.sol";

import {iPoWLightClient} from "./iPoWLightClient.sol";
import {iPoWProtocol} from "./iPoWProtocol.sol";
import {BitcoinTxLib} from "./BitcoinTxLib.sol";
import {VaultRecords as R, IVaultCore} from "./vault/VaultRecords.sol";
import {VaultHome} from "./vault/VaultHome.sol";
import {VaultReceipts} from "./vault/VaultReceipts.sol";
import {VaultHomeFactory, VaultReceiptsFactory} from "./vault/VaultFactories.sol";
import {VaultReceipt} from "./vault/VaultReceipt.sol";

/// @title iPoWVault
/// @notice The protocol's vault on an EVM network, for one pair of networks:
/// this one (`here`) and a peer, deployed once per peer (D132). Spec:
/// docs/design/ipow-protocol.md, section 11, decisions D104 to D136. It is
/// part of the protocol, beside the protocol contract (D104). Comments name
/// the first pair, Ethereum and Solana: read "here" and "the peer".
///
/// This core keeps the operators' pair chains and bonds, processes their
/// messages, judges the records whose facts live here (D109), and holds the
/// records from the peer as claims for 7 days of objections (D111). Two
/// parts, made for it by their factories, act on accepted claims: `home`,
/// for the assets whose home is here, and `receipts`, for the receipts here
/// of the peer's (section 11.9).
///
/// There is no owner and no key, and nothing here can be changed after
/// deployment (D59). Money is never pushed to an address. It is credited and
/// the address withdraws it.
abstract contract iPoWVault is ReentrancyGuard, IVaultCore {
    using SafeERC20 for IERC20;

    // ------------------------------------------------------------------
    // Rules
    // ------------------------------------------------------------------

    /// @notice D111: a claim is decided when the last objection or answer has
    /// stood for 7 days.
    uint256 public constant OBJECTION_WINDOW = 7 days;
    /// @notice D110, D129: a chain's open claims in an asset are at most 80% of
    /// its bond in that asset.
    uint256 public constant COVER_BPS = 8000;
    /// @notice D109: 80% of a slash backs the receipt, 20% goes to whoever
    /// submitted the message.
    uint256 public constant BACKING_SHARE_BPS = 8000;
    uint256 public constant BPS = 10_000;

    /// @notice D119: a message is false when its Bitcoin transaction or batch
    /// is longer than these, or it carries more LOCK, REQUEST, CANCEL and
    /// ASSET records, on both networks together, than this: Solana reads an
    /// account for most of them.
    uint256 public constant MAX_RAW_TX = 1024;
    uint256 public constant MAX_BATCH = 2048;
    uint256 public constant MAX_RECORDS = 32;
    /// @notice The assets a chain keeps bonds in, and a claim moves, at most
    /// (section 11.9).
    uint256 public constant MAX_ASSETS = 8;

    iPoWProtocol public immutable protocol;
    iPoWLightClient public immutable lightClient;
    /// @notice The vault program on Solana, as named in a registration.
    bytes32 public immutable peerVault;
    /// @notice D121: the flat deposit to object or answer, and the operator's
    /// deposit for a claim, in wei.
    uint256 public immutable deposit;
    /// @notice D118: the least escrow of a job whose proof block counts as
    /// real.
    uint256 public immutable minCertifyingEscrow;
    VaultHome public immutable home;
    VaultReceipts public immutable receipts;

    // ------------------------------------------------------------------
    // Storage
    // ------------------------------------------------------------------

    struct Chain {
        bytes32 peerOperator; // the same operator on Solana
        bytes32 coinTxid;
        uint32 coinVout;
        bool registered;
        bool exited;
        /// A false record was proven here: every bond is gone (D109).
        bool slashed;
        /// A claim of the chain was refused here: no other claim acts (D111).
        bool refused;
        uint64 messages;
        /// Wei for the flat deposits of its claims, apart from the bonds.
        uint256 deposits;
        uint32 openClaims;
        /// The block of the chain's last message here, where its
        /// `MessageBatch` event is: each names the block of the one before.
        uint64 lastMessageBlock;
    }

    /// A chain's place in one asset (D129), in record units.
    struct Position {
        /// Held here: the asset itself when its home is Ethereum, its receipt
        /// otherwise.
        uint64 bond;
        /// The part its BOND record stated.
        uint64 stated;
        /// Its bond in the asset on Solana, counted once its BOND claim is
        /// official.
        uint64 peerBond;
        bool peerBondCarried;
        /// Open claims here in the asset.
        uint64 openValue;
    }

    struct Claim {
        address operator;
        uint40 lastAt;
        uint40 openedAt;
        /// An objection stands.
        bool held;
        bool decided;
        bool accepted;
        /// Deposits put down on each side, the operator's among the answers.
        uint32 answers;
        uint32 objections;
        /// Set when decided: what each deposit of the winning side collects.
        uint256 payout;
        /// The block it was opened in, where `ClaimBatch` names its records.
        uint64 openedBlock;
    }

    mapping(address => Chain) private _chains;
    mapping(address => mapping(uint40 => Position)) private _positions;
    mapping(address => uint40[]) private _assetsOf;

    uint256 public claimCount;
    mapping(uint256 => Claim) private _claims;
    /// @dev Claim => the assets it moves, and in each, its value and a BOND it
    /// carries.
    mapping(uint256 => uint40[]) private _claimAssets;
    mapping(uint256 => mapping(uint40 => uint64)) private _claimValue;
    mapping(uint256 => mapping(uint40 => uint64)) private _claimPeerBond;
    /// @notice Claim => the hashes of the records it acts on.
    mapping(uint256 => mapping(bytes32 => bool)) public carries;
    mapping(uint256 => mapping(address => uint32)) public answersOf;
    mapping(uint256 => mapping(address => uint32)) public objectionsOf;

    /// @notice Blocks known to be on real Bitcoin (D108), by light client id.
    mapping(bytes32 => bool) public isReal;
    uint256 public checkpointCount;
    /// @notice The backing share of a slashed bond not yet settled, by
    /// operator and asset, in record units.
    mapping(address => mapping(uint40 => uint64)) public slashBacking;
    /// @notice What each address can withdraw, by asset, in its native units:
    /// wei or token units for an asset of Ethereum, receipts for Solana's.
    mapping(address => mapping(uint40 => uint256)) public credit;

    // ------------------------------------------------------------------
    // Events and errors
    // ------------------------------------------------------------------

    event ChainRegistered(address indexed operator, bytes32 peerOperator, bytes32 coinTxid, uint32 coinVout);
    event BondChanged(address indexed operator, uint40 indexed asset, uint64 bond);
    event RealBlock(bytes32 indexed id);
    event MessageProcessed(address indexed operator, uint64 index, bytes32 txid, bool truthful, uint256 claimId);
    /// @notice Every message's batch, so that anyone can bring the message to
    /// the other vault (D107, D120). `prevBlock` is the block of the chain's
    /// message before it, zero for the first.
    event MessageBatch(address indexed operator, uint64 indexed index, uint64 prevBlock, bytes batch);
    event Slashed(address indexed operator, address submitter);
    event ClaimOpened(uint256 indexed claimId, address indexed operator);
    /// @notice The batch whose acting records a claim carries.
    event ClaimBatch(uint256 indexed claimId, bytes batch);
    event Objected(uint256 indexed claimId, address indexed by);
    event Answered(uint256 indexed claimId, address indexed by);
    event ClaimDecided(uint256 indexed claimId, bool accepted);
    event DepositsChanged(address indexed operator, uint256 deposits);
    event CreditWithdrawn(address indexed to, uint40 indexed asset, uint256 amount);

    error ZeroAmount();
    error ChainExists();
    error NoChain();
    error NotReal();
    error NotInBlock();
    error WrongCoin();
    error NoCoin();
    error WrongTag();
    error ChainEnded();
    error NotCertified();
    error BondNotFree();
    error NotOpen();
    error AlreadyHeld();
    error NotHeld();
    error WindowOver();
    error WindowNotOver();
    error AlreadyDecided();
    error NotDecided();
    error NothingToCollect();
    error EscrowTooLow();
    error TooManyAssets();
    error UnknownAsset();
    error WrongValue();
    error TransferFailed();
    error BadNetworks();

    /// @notice This network's number, and its peer's (D133). Kept in
    /// storage, set once here: an immutable's value is copied into the code
    /// at every use, and the core is near the size limit.
    uint8 public here;
    uint8 public peer;
    /// @notice The factories that made `home` and `receipts`.
    address public homeFactory;
    address public receiptsFactory;

    /// @param here_ This network's number, `peer_` the other network's, and
    /// `peerVault_` the vault there (D132, D133). The factories make its two
    /// parts, bound to it.
    constructor(
        iPoWProtocol protocol_,
        uint8 here_,
        uint8 peer_,
        bytes32 peerVault_,
        uint256 deposit_,
        uint256 minCertifyingEscrow_,
        VaultHomeFactory homeFactory_,
        VaultReceiptsFactory receiptsFactory_,
        VaultHome home_,
        VaultReceipts receipts_,
        address coin_
    ) {
        if (deposit_ == 0 || minCertifyingEscrow_ == 0) revert ZeroAmount();
        // D133, D141: two different networks of the list, 1 to 9.
        if (here_ == 0 || peer_ == 0 || here_ > 9 || peer_ > 9 || here_ == peer_) revert BadNetworks();
        here = here_;
        peer = peer_;
        minCertifyingEscrow = minCertifyingEscrow_;
        protocol = protocol_;
        lightClient = protocol_.lightClient();
        peerVault = peerVault_;
        deposit = deposit_;
        home = home_;
        receipts = receipts_;
        // Made by the factories, before this vault, for this vault and its
        // pair. What the parts' code is, the factories' code shows: whoever
        // checks a vault reads `homeFactory` and `receiptsFactory` and
        // compares their code with the source.
        if (
            !homeFactory_.made(address(home_)) || !receiptsFactory_.made(address(receipts_))
                || address(home_.core()) != address(this) || address(receipts_.core()) != address(this) || home_.here() != here_
                || home_.peer() != peer_ || receipts_.here() != here_ || receipts_.peer() != peer_
        ) revert BadNetworks();
        homeFactory = address(homeFactory_);
        receiptsFactory = address(receiptsFactory_);
        if (home.getAsset(0).token != coin_) revert BadNetworks();
        // D116: the vault opens checkpoint jobs, as an application does.
        protocol_.registerApplication(new uint32[](0));
    }

    // ------------------------------------------------------------------
    // Reading
    // ------------------------------------------------------------------

    function getChain(address operator) external view returns (Chain memory) {
        return _chains[operator];
    }

    function getPosition(address operator, uint40 asset) external view returns (Position memory) {
        return _positions[operator][asset];
    }

    function assetsOf(address operator) external view returns (uint40[] memory) {
        return _assetsOf[operator];
    }

    function getClaim(uint256 claimId) external view returns (Claim memory) {
        return _claims[claimId];
    }

    /// @notice What claim `claimId` moves in `asset`, and the BOND it carries.
    function claimAsset(uint256 claimId, uint40 asset) external view returns (uint64 value, uint64 peerBond) {
        return (_claimValue[claimId][asset], _claimPeerBond[claimId][asset]);
    }

    /// @inheritdoc IVaultCore
    function claimAccepted(uint256 claimId) external view returns (bool) {
        return _claims[claimId].accepted;
    }

    /// @inheritdoc IVaultCore
    function claimInfo(uint256 claimId) external view returns (address, uint64, bool, bool) {
        Claim storage cl = _claims[claimId];
        return (cl.operator, cl.openedAt, cl.decided, cl.accepted);
    }

    /// @inheritdoc IVaultCore
    function operatorActive(address operator) external view returns (bool) {
        Chain storage c = _chains[operator];
        return c.registered && !c.refused && !c.slashed && !c.exited;
    }

    /// @notice What a registration carries in its `OP_RETURN` (D106).
    function pairCommitment(address operator, bytes32 peerOperator) public view returns (bytes32) {
        // The pair's two sides, the lower network number first, each as its
        // number, its vault and its operator in 32 bytes (section 11.3).
        bytes memory mine = abi.encodePacked(here, bytes32(uint256(uint160(address(this)))), bytes32(uint256(uint160(operator))));
        bytes memory theirs = abi.encodePacked(peer, peerVault, peerOperator);
        return sha256(here < peer ? abi.encodePacked("iPoW pair", mine, theirs) : abi.encodePacked("iPoW pair", theirs, mine));
    }

    /// @notice What a message carries in its `OP_RETURN` (D107).
    function messagePayload(bytes calldata batch) public pure returns (bytes32) {
        return sha256(abi.encodePacked("iPoW vault", batch));
    }

    /// @notice The part of an operator's bond in `asset` it may withdraw.
    function freeBond(address operator, uint40 asset) public view returns (uint64) {
        Chain storage c = _chains[operator];
        Position storage p = _positions[operator][asset];
        if (c.exited && c.openClaims == 0) return p.bond;
        return p.bond - p.stated;
    }

    // ------------------------------------------------------------------
    // Real Bitcoin (D108, D116)
    // ------------------------------------------------------------------

    /// @notice Records the proof block of a job as real: its escrow is at
    /// least `minCertifyingEscrow` (D118), and its lock has ended with no
    /// challenge won and no slash. Anyone may call.
    function recordRealFromJob(uint256 jobId) external {
        if (protocol.getJob(jobId).escrow < minCertifyingEscrow) revert EscrowTooLow();
        iPoWProtocol.JobStatus status = protocol.statusOf(jobId);
        iPoWProtocol.Duty memory duty = protocol.getDuty(jobId);
        bool certified = status == iPoWProtocol.JobStatus.Settled ||
            (status == iPoWProtocol.JobStatus.Proven &&
                block.timestamp >= duty.lockEnd &&
                protocol.openChallengesOf(jobId) == 0);
        if (!certified) revert NotCertified();
        _markReal(_id(duty.proofBlock));
    }

    /// @notice Records a block below a real block as real. Anyone may call.
    function recordReal(
        iPoWProtocol.BlockRef calldata low,
        iPoWProtocol.BlockRef calldata high,
        uint32 prevEpochTime
    ) external {
        _requireBelowReal(low, high, prevEpochTime);
        _markReal(_id(low));
    }

    /// @notice Opens a job whose only use is to make a real block (D116),
    /// with the escrow D118 asks, or the protocol's lowest if that is more.
    /// The sender pays its fees and receives them back if nobody takes it.
    function openCheckpoint(uint16 confirmations, uint256 paid) external payable returns (uint256 jobId) {
        uint256 fee = protocol.commitmentFeeFor(confirmations);
        uint256 escrow = protocol.MIN_ESCROW_MULTIPLE() * fee;
        if (escrow < minCertifyingEscrow) escrow = minCertifyingEscrow;
        bytes32 tag = keccak256(abi.encode("iPoW checkpoint", address(this), ++checkpointCount));
        _take(paid);
        jobId = protocol.openJob{value: _jobValue(paid)}(tag, escrow, protocol.DEFAULT_ESCROW_FEE_BPS(), confirmations, 0, msg.sender, paid);
    }

    /// @notice Takes what the protocol owes the vault, the application's share
    /// of a slashed checkpoint job, into the backing of vETH. Anyone may call.
    function collectProtocolCredit() external nonReentrant {
        uint256 before = _coinBalance();
        protocol.withdrawCredit();
        _toBacking(_coinBalance() - before);
    }

    // ------------------------------------------------------------------
    // Pair chain, bonds and deposits (D106, D107, D110, D129)
    // ------------------------------------------------------------------

    struct BitcoinTx {
        iPoWProtocol.BlockRef block;
        bytes rawTx;
        bytes32[] siblings;
        uint256 txIndex;
        /// A block already recorded as real, with `block` below it (D108).
        iPoWProtocol.BlockRef real;
        uint32 prevEpochTime;
    }

    /// @notice Registers the caller's pair chain: the coin made by a Bitcoin
    /// transaction that names the caller here and `peerOperator` on Solana.
    function registerChain(
        bytes32 peerOperator,
        BitcoinTx calldata btc,
        uint32 coinIndex,
        uint32 tagIndex
    ) external {
        Chain storage c = _chains[msg.sender];
        if (c.registered) revert ChainExists();
        _requireReal(btc);
        BitcoinTxLib.View memory v = BitcoinTxLib.read(btc.rawTx, 0, coinIndex, tagIndex);
        if (!v.hasCoin) revert NoCoin();
        if (!v.hasTag || v.tag != pairCommitment(msg.sender, peerOperator)) revert WrongTag();

        c.registered = true;
        c.peerOperator = peerOperator;
        c.coinTxid = BitcoinTxLib.txid(btc.rawTx);
        c.coinVout = coinIndex;
        emit ChainRegistered(msg.sender, peerOperator, c.coinTxid, coinIndex);
    }

    /// @notice Adds to the caller's bond in `asset`, in record units: ETH sent
    /// with the call, a token or a receipt taken from the caller. A token
    /// that takes a fee on transfer counts what arrived.
    function addBond(uint40 asset, uint64 amount) external payable nonReentrant {
        Chain storage c = _chains[msg.sender];
        if (!c.registered) revert NoChain();
        if (c.slashed) revert ChainEnded();
        if (amount == 0) revert ZeroAmount();
        Position storage p = _position(msg.sender, asset);
        uint32 number = R.assetOf(asset);
        if (R.homeOf(asset) == here) {
            VaultHome.Asset memory a = home.getAsset(number);
            if (a.token == address(0)) {
                if (msg.value != uint256(amount) * a.unit) revert WrongValue();
            } else {
                if (msg.value != 0) revert WrongValue();
                IERC20 t = IERC20(a.token);
                uint256 before = t.balanceOf(address(this));
                t.safeTransferFrom(msg.sender, address(this), uint256(amount) * a.unit);
                amount = uint64((t.balanceOf(address(this)) - before) / a.unit);
            }
        } else {
            if (msg.value != 0) revert WrongValue();
            receipts.take(number, msg.sender, amount);
        }
        p.bond += amount;
        emit BondChanged(msg.sender, asset, p.bond);
    }

    /// @notice Withdraws free bond in `asset`: all of it once the chain has
    /// exited and every claim of it here has ended (D112).
    function withdrawBond(uint40 asset, uint64 amount) external nonReentrant {
        if (amount == 0) revert ZeroAmount();
        if (amount > freeBond(msg.sender, asset)) revert BondNotFree();
        Position storage p = _positions[msg.sender][asset];
        p.bond -= amount;
        if (p.stated > p.bond) p.stated = p.bond;
        emit BondChanged(msg.sender, asset, p.bond);
        _payNative(msg.sender, asset, _native(asset, amount));
    }

    /// @notice Adds `amount` of the network's coin for the flat deposits of
    /// the caller's claims; a native coin sends exactly that much (D138).
    function addDeposits(uint256 amount) external payable {
        Chain storage c = _chains[msg.sender];
        if (!c.registered) revert NoChain();
        _take(amount);
        c.deposits += amount;
        emit DepositsChanged(msg.sender, c.deposits);
    }

    function withdrawDeposits(uint256 amount) external nonReentrant {
        Chain storage c = _chains[msg.sender];
        if (amount == 0 || amount > c.deposits) revert ZeroAmount();
        c.deposits -= amount;
        emit DepositsChanged(msg.sender, c.deposits);
        _sendCoin(msg.sender, amount);
    }

    // ------------------------------------------------------------------
    // Messages (D107, D108, D109, D114)
    // ------------------------------------------------------------------

    /// @notice Processes the next message of an operator's pair chain, with
    /// its batch. Anyone may submit it (D108).
    /// @param inputIndex The input that spends the chain's coin. The output
    /// with the same number is the next coin (N23).
    /// @param tagIndex The output that carries `messagePayload(batch)`.
    function submitMessage(
        address operator,
        BitcoinTx calldata btc,
        uint32 inputIndex,
        uint32 tagIndex,
        bytes calldata batch
    ) external nonReentrant {
        Chain storage c = _chains[operator];
        if (!c.registered) revert NoChain();
        if (c.exited) revert ChainEnded();
        _requireReal(btc);

        BitcoinTxLib.View memory v = BitcoinTxLib.read(btc.rawTx, inputIndex, inputIndex, tagIndex);
        if (v.spentTxid != c.coinTxid || v.spentVout != c.coinVout) revert WrongCoin();
        if (!v.hasCoin) revert NoCoin();
        if (!v.hasTag || v.tag != messagePayload(batch)) revert WrongTag();

        bytes32 txid = BitcoinTxLib.txid(btc.rawTx);
        c.coinTxid = txid;
        c.coinVout = inputIndex;
        uint64 index = c.messages++;
        _markReal(_id(btc.block));

        bool truthful;
        uint256 claimId;
        if (btc.rawTx.length <= MAX_RAW_TX && batch.length <= MAX_BATCH) {
            (truthful, claimId) = _process(operator, c, batch);
        }
        if (!truthful) _slash(operator, c);
        emit MessageProcessed(operator, index, txid, truthful, claimId);
        emit MessageBatch(operator, index, c.lastMessageBlock, batch);
        c.lastMessageBlock = uint64(block.number);
    }

    // ------------------------------------------------------------------
    // Objections (D111)
    // ------------------------------------------------------------------

    /// @notice Objects to a claim: it is held. Sent with the deposit.
    function object(uint256 claimId) external payable {
        _take(deposit);
        Claim storage cl = _openClaim(claimId);
        if (cl.held) revert AlreadyHeld();
        cl.held = true;
        cl.lastAt = uint40(block.timestamp);
        cl.objections += 1;
        objectionsOf[claimId][msg.sender] += 1;
        emit Objected(claimId, msg.sender);
    }

    /// @notice Answers the objection that holds a claim: the hold is lifted
    /// and the 7 days restart. Sent with the deposit.
    function answer(uint256 claimId) external payable {
        _take(deposit);
        Claim storage cl = _openClaim(claimId);
        if (!cl.held) revert NotHeld();
        cl.held = false;
        cl.lastAt = uint40(block.timestamp);
        cl.answers += 1;
        answersOf[claimId][msg.sender] += 1;
        emit Answered(claimId, msg.sender);
    }

    /// @notice Decides a claim whose last statement has stood for 7 days.
    /// Anyone may call. A claim of a chain that a proof here showed false, or
    /// that lost another claim here, is refused (D109, D111). Each winner
    /// collects its own share afterwards.
    function decide(uint256 claimId) external nonReentrant {
        Claim storage cl = _claims[claimId];
        if (cl.operator == address(0)) revert NotOpen();
        if (cl.decided) revert AlreadyDecided();
        if (block.timestamp < uint256(cl.lastAt) + OBJECTION_WINDOW) revert WindowNotOver();

        Chain storage c = _chains[cl.operator];
        bool accepted = !cl.held && !c.slashed && !c.refused;
        cl.decided = true;
        cl.accepted = accepted;
        c.openClaims -= 1;
        uint40[] storage keys = _claimAssets[claimId];
        for (uint256 i; i < keys.length; i++) {
            Position storage p = _positions[cl.operator][keys[i]];
            p.openValue -= _claimValue[claimId][keys[i]];
            uint64 peerBond = _claimPeerBond[claimId][keys[i]];
            if (accepted && peerBond != 0) p.peerBond = peerBond;
        }
        if (!accepted) c.refused = true;
        // D111: the losing side's deposits are shared by the winning side.
        // What does not divide evenly, or has nobody to go to, backs vETH.
        uint256 winners = accepted ? cl.answers : cl.objections;
        uint256 pot = (accepted ? cl.objections : cl.answers) * deposit;
        uint256 rest = pot;
        if (winners != 0) {
            uint256 share = pot / winners;
            rest = pot - share * winners;
            cl.payout = deposit + share;
        }
        _toBacking(rest);
        emit ClaimDecided(claimId, accepted);
    }

    /// @notice Credits the caller's deposits on the winning side of a
    /// decided claim, each with its share of the losing side's.
    function collect(uint256 claimId) external {
        Claim storage cl = _claims[claimId];
        if (!cl.decided) revert NotDecided();
        uint256 entries;
        if (cl.accepted) {
            entries = answersOf[claimId][msg.sender];
            answersOf[claimId][msg.sender] = 0;
        } else {
            entries = objectionsOf[claimId][msg.sender];
            objectionsOf[claimId][msg.sender] = 0;
        }
        if (entries == 0) revert NothingToCollect();
        credit[msg.sender][R.key(here, 0)] += entries * cl.payout;
    }

    function withdrawCredit(uint40 asset) external nonReentrant {
        uint256 amount = credit[msg.sender][asset];
        if (amount == 0) revert ZeroAmount();
        credit[msg.sender][asset] = 0;
        emit CreditWithdrawn(msg.sender, asset, amount);
        _payNative(msg.sender, asset, amount);
    }

    // ------------------------------------------------------------------
    // Inside
    // ------------------------------------------------------------------

    /// @dev What one message asks here, gathered while judging it.
    struct Plan {
        uint40[] keys;
        uint64[] values;
        uint64[] peerBonds;
        uint256 count;
        /// More assets than a claim holds, or more than an asset counts: no
        /// claim opens.
        bool overflow;
        bool acting;
    }

    /// @dev Judges the records about Ethereum and gathers the others into one
    /// claim. A batch that does not parse is false: its hash is what the
    /// operator wrote on Bitcoin.
    function _process(address operator, Chain storage c, bytes calldata batch)
        private
        returns (bool truthful, uint256 claimId)
    {
        Plan memory plan = Plan(new uint40[](MAX_ASSETS), new uint64[](MAX_ASSETS), new uint64[](MAX_ASSETS), 0, false, false);
        // First pass: judge, and measure the claim.
        if (!_judge(operator, batch, plan)) return (false, 0);
        // Second pass: nothing false was found. Apply.
        if (plan.acting) claimId = _openClaimFor(operator, c, plan);
        if (claimId != 0) emit ClaimBatch(claimId, batch);
        uint256 o;
        while (o < batch.length) {
            uint8 kind = uint8(batch[o]);
            uint256 len = _len(kind);
            bytes calldata r = batch[o:o + len];
            o += len;
            if (kind == R.EXIT) {
                c.exited = true;
                continue;
            }
            bool isHere = uint8(r[1]) == here;
            if (kind == R.BOND) {
                uint40 k = R.key(uint8(r[2]), R.u32(r, 3));
                if (isHere) _positions[operator][k].stated = R.u64(r, 7);
                // Only a claim that opened carries it: otherwise it can be
                // carried again. A later one was not added to the claim.
                else if (claimId != 0) {
                    uint256 i = _find(plan, k);
                    if (i < MAX_ASSETS && plan.peerBonds[i] != 0) _positions[operator][k].peerBondCarried = true;
                }
            } else if (isHere) {
                // A slashed operator earns nothing; the fee waits for another.
                if (!c.slashed && kind == R.LOCK) home.earnFee(R.u64(r, 6), operator);
                if (!c.slashed && kind == R.REQUEST) receipts.earnFee(R.u64(r, 6), operator);
            } else if (claimId != 0) {
                carries[claimId][keccak256(r)] = true;
            }
        }
        return (true, claimId);
    }

    function _len(uint8 kind) private pure returns (uint256) {
        if (kind == R.LOCK) return R.LOCK_LEN;
        if (kind == R.REQUEST) return R.REQUEST_LEN;
        if (kind == R.CANCEL) return R.CANCEL_LEN;
        if (kind == R.BOND) return R.BOND_LEN;
        if (kind == R.ASSET) return R.ASSET_LEN;
        if (kind == R.EXIT) return 1;
        return 0;
    }

    /// @dev The first pass: false when a record here is not so, or the batch
    /// does not parse.
    function _judge(address operator, bytes calldata batch, Plan memory plan) private view returns (bool) {
        uint256 o;
        uint256 counted;
        uint40[] memory stated = new uint40[](MAX_ASSETS);
        uint256 statedCount;
        while (o < batch.length) {
            uint8 kind = uint8(batch[o]);
            uint256 len = _len(kind);
            if (len == 0 || o + len > batch.length) return false;
            bytes calldata r = batch[o:o + len];
            o += len;
            if (kind == R.EXIT) {
                if (o != batch.length) return false;
                continue;
            }
            uint8 net = uint8(r[1]);
            if (net != here && net != peer) return false;
            if (kind != R.BOND && ++counted > MAX_RECORDS) return false;
            if (net == here) {
                if (kind != R.BOND) {
                    if (!_true(kind, r)) return false;
                    continue;
                }
                // BOND, D110: once per chain and asset, from the bond. The same
                // BOND again is true and changes nothing.
                uint40 k = _bondKey(r);
                if (k == 0) return false;
                uint64 amount = R.u64(r, 7);
                Position storage p = _positions[operator][k];
                if (amount == 0 || (p.stated != 0 ? amount != p.stated : amount > p.bond)) return false;
                for (uint256 i; i < statedCount; i++) if (stated[i] == k) return false;
                if (statedCount == MAX_ASSETS) return false;
                stated[statedCount++] = k;
                continue;
            }
            // A record from Solana: acted on here. A BOND already counted
            // is not acted on, so alone it opens no claim.
            if (kind != R.BOND) plan.acting = true;
            if (kind == R.LOCK) {
                _add(plan, R.key(peer, R.u32(r, 2)), uint256(R.u64(r, 14)) + R.u64(r, 62), 0);
            } else if (kind == R.REQUEST) {
                // Paid here, to a real Ethereum address, in a registered asset:
                // Solana writes no other.
                if (R.addressOf(R.b32(r, 22)) == address(0)) return false;
                uint32 a = R.u32(r, 2);
                if (a >= home.assetCount()) return false;
                _add(plan, R.key(here, a), uint256(R.u64(r, 14)) + R.u64(r, 62), 0);
            } else if (kind == R.CANCEL) {
                // Solana gives up only a lock it learned from a true LOCK
                // record, so a lock that does not exist here is a lie.
                (uint32 a, uint64 value) = home.lockValue(R.u64(r, 2));
                if (value == 0) return false;
                _add(plan, R.key(here, a), value, 0);
            } else if (kind == R.BOND) {
                // A fact of Solana, judged there (D109). Here the first one
                // carried in a claim counts.
                uint40 k = _bondKey(r);
                if (k == 0) return false;
                uint64 amount = R.u64(r, 7);
                if (!_positions[operator][k].peerBondCarried && amount != 0) {
                    _add(plan, k, 0, amount);
                    plan.acting = true;
                }
            }
            // ASSET from Solana: acted on by making its receipt; no value.
        }
        return true;
    }

    /// @dev Whether a record whose fact lives on Ethereum, other than BOND, is
    /// so.
    function _true(uint8 kind, bytes calldata r) private view returns (bool) {
        bytes32 h = keccak256(r);
        if (kind == R.LOCK) return h == home.lockRecordHash(R.u64(r, 6));
        if (kind == R.REQUEST) return h == receipts.burnRecordHash(R.u64(r, 6));
        if (kind == R.CANCEL) return receipts.givenUp(R.u64(r, 2));
        return h == home.assetRecordHash(R.u32(r, 2));
    }

    /// @dev The asset a BOND record names; zero when its home is neither
    /// network.
    function _bondKey(bytes calldata r) private view returns (uint40) {
        uint8 h = uint8(r[2]);
        if (h != here && h != peer) return 0;
        return R.key(h, R.u32(r, 3));
    }

    function _find(Plan memory plan, uint40 k) private pure returns (uint256) {
        for (uint256 i; i < plan.count; i++) if (plan.keys[i] == k) return i;
        return MAX_ASSETS;
    }

    function _add(Plan memory plan, uint40 k, uint256 value, uint64 peerBond) private pure {
        uint256 i = _find(plan, k);
        if (i == MAX_ASSETS) {
            if (plan.count == MAX_ASSETS) {
                plan.overflow = true;
                return;
            }
            i = plan.count++;
            plan.keys[i] = k;
        }
        uint256 v = plan.values[i] + value;
        if (v > type(uint64).max) {
            plan.overflow = true;
            return;
        }
        plan.values[i] = uint64(v);
        if (peerBond != 0 && plan.peerBonds[i] == 0) plan.peerBonds[i] = peerBond;
    }

    /// @dev The acting records of one message become one claim, unless the
    /// chain was refused, its open claims in an asset would pass 80% of its
    /// bond there on Solana, it moves more assets than a claim holds, or its
    /// deposit money cannot pay the deposit. Then nothing acts, and the
    /// records can be carried again.
    function _openClaimFor(address operator, Chain storage c, Plan memory plan) private returns (uint256 claimId) {
        if (c.refused || c.slashed || plan.overflow) return 0;
        if (c.deposits < deposit) return 0;
        for (uint256 i; i < plan.count; i++) {
            if (plan.values[i] == 0) continue;
            Position storage p = _positions[operator][plan.keys[i]];
            if ((uint256(p.openValue) + plan.values[i]) * BPS > uint256(p.peerBond) * COVER_BPS) return 0;
        }
        c.deposits -= deposit;
        c.openClaims += 1;
        claimId = ++claimCount;
        Claim storage cl = _claims[claimId];
        cl.operator = operator;
        cl.lastAt = uint40(block.timestamp);
        cl.openedAt = uint40(block.timestamp);
        cl.answers = 1;
        cl.openedBlock = uint64(block.number);
        answersOf[claimId][operator] = 1;
        for (uint256 i; i < plan.count; i++) {
            uint40 k = plan.keys[i];
            _claimAssets[claimId].push(k);
            _claimValue[claimId][k] = plan.values[i];
            _claimPeerBond[claimId][k] = plan.peerBonds[i];
            _positions[operator][k].openValue += plan.values[i];
        }
        emit ClaimOpened(claimId, operator);
    }

    /// @dev D109, D129: every bond of the chain here. 80% backs its asset's
    /// receipts, 20% goes to the submitter, or to the backing when the
    /// operator submitted its own false message.
    function _slash(address operator, Chain storage c) private {
        c.slashed = true;
        emit Slashed(operator, msg.sender);
        uint40[] storage keys = _assetsOf[operator];
        for (uint256 i; i < keys.length; i++) {
            uint40 k = keys[i];
            Position storage p = _positions[operator][k];
            uint64 amount = p.bond;
            if (amount == 0) continue;
            p.bond = 0;
            p.stated = 0;
            uint64 toSubmitter = msg.sender == operator ? 0 : uint64((uint256(amount) * (BPS - BACKING_SHARE_BPS)) / BPS);
            if (toSubmitter != 0) credit[msg.sender][k] += _native(k, toSubmitter);
            // Settled per asset by `settleSlash`: a token that will not move
            // cannot keep the slash from happening.
            slashBacking[operator][k] += amount - toSubmitter;
            emit BondChanged(operator, k, 0);
        }
    }

    /// @notice Moves a slashed bond's backing share in `asset` into the
    /// backing of its receipts (D109, D129): an asset of Ethereum joins the
    /// reserve, counted as it arrived; a receipt bond is burned, fewer
    /// receipts for the same asset. Anyone may call.
    function settleSlash(address operator, uint40 asset) external nonReentrant {
        uint64 backing = slashBacking[operator][asset];
        if (backing == 0) revert NothingToCollect();
        slashBacking[operator][asset] = 0;
        uint32 number = R.assetOf(asset);
        if (R.homeOf(asset) == here) {
            VaultHome.Asset memory a = home.getAsset(number);
            if (a.token == address(0)) {
                _sendCoin(address(home), uint256(backing) * a.unit);
                home.addBacking(number, backing);
            } else {
                IERC20 t = IERC20(a.token);
                uint256 before = t.balanceOf(address(home));
                t.safeTransfer(address(home), uint256(backing) * a.unit);
                home.addBacking(number, (t.balanceOf(address(home)) - before) / a.unit);
            }
        } else {
            VaultReceipt(address(receipts.receiptOf(number))).burn(backing);
        }
    }

    /// @dev The chain's position in `asset`, made on first use.
    function _position(address operator, uint40 asset) private returns (Position storage) {
        uint8 h = R.homeOf(asset);
        if (h != here && h != peer) revert UnknownAsset();
        uint40[] storage keys = _assetsOf[operator];
        for (uint256 i; i < keys.length; i++) if (keys[i] == asset) return _positions[operator][asset];
        if (keys.length == MAX_ASSETS) revert TooManyAssets();
        if (h == peer && address(receipts.receiptOf(R.assetOf(asset))) == address(0)) revert UnknownAsset();
        keys.push(asset);
        return _positions[operator][asset];
    }

    /// @dev Record units in an asset's native units.
    function _native(uint40 asset, uint64 amount) private view returns (uint256) {
        if (R.homeOf(asset) != here) return amount;
        return uint256(amount) * home.getAsset(R.assetOf(asset)).unit;
    }

    function _payNative(address to, uint40 asset, uint256 amount) private {
        if (amount == 0) return;
        uint32 number = R.assetOf(asset);
        if (R.homeOf(asset) == here) {
            address token = home.getAsset(number).token;
            if (token == address(0)) _sendCoin(to, amount);
            else IERC20(token).safeTransfer(to, amount);
        } else {
            IERC20(address(receipts.receiptOf(number))).safeTransfer(to, amount);
        }
    }

    /// @dev The coin into the backing of its receipt: what the vault gains
    /// in its coin, asset 0.
    function _toBacking(uint256 amount) private {
        if (amount == 0) return;
        _sendCoin(address(home), amount);
        home.addBacking(0, amount / home.getAsset(0).unit);
    }

    function _openClaim(uint256 claimId) private view returns (Claim storage cl) {
        cl = _claims[claimId];
        if (cl.operator == address(0) || cl.decided) revert NotOpen();
        if (block.timestamp >= uint256(cl.lastAt) + OBJECTION_WINDOW) revert WindowOver();
    }

    function _requireReal(BitcoinTx calldata btc) private view {
        _requireBelowReal(btc.block, btc.real, btc.prevEpochTime);
        if (!lightClient.txInBlock(_id(btc.block), btc.rawTx, btc.siblings, btc.txIndex)) revert NotInBlock();
    }

    function _requireBelowReal(
        iPoWProtocol.BlockRef calldata low,
        iPoWProtocol.BlockRef calldata high,
        uint32 prevEpochTime
    ) private view {
        if (!isReal[_id(high)]) revert NotReal();
        bool linked = lightClient.isAncestor(
            low.hash,
            low.height,
            low.epochTime,
            high.hash,
            high.height,
            high.epochTime,
            prevEpochTime
        );
        if (!linked) revert NotReal();
    }

    function _markReal(bytes32 id) private {
        if (isReal[id]) return;
        isReal[id] = true;
        emit RealBlock(id);
    }

    function _id(iPoWProtocol.BlockRef memory b) private view returns (bytes32) {
        return lightClient.nodeId(b.hash, b.height, b.epochTime);
    }

    // ------------------------------------------------------------------
    // The network's coin: by build (D136, D137)
    // ------------------------------------------------------------------

    /// @dev Takes exactly `amount` of the network's coin from the caller.
    function _take(uint256 amount) internal virtual;

    /// @dev Pays `amount` of the network's coin.
    function _sendCoin(address to, uint256 amount) internal virtual;

    function _coinBalance() internal view virtual returns (uint256);

    /// @dev The native value a job's fees are sent to the protocol with.
    function _jobValue(uint256 paid) internal pure virtual returns (uint256);
}

/// @title iPoWVaultNative
/// @notice The vault on a network with a native coin (D137): deposits and a
/// checkpoint's fees are sent with the call, exactly the amount named.
contract iPoWVaultNative is iPoWVault {
    constructor(
        iPoWProtocol protocol_,
        uint8 here_,
        uint8 peer_,
        bytes32 peerVault_,
        uint256 deposit_,
        uint256 minCertifyingEscrow_,
        VaultHomeFactory homeFactory_,
        VaultReceiptsFactory receiptsFactory_,
        VaultHome home_,
        VaultReceipts receipts_
    ) iPoWVault(protocol_, here_, peer_, peerVault_, deposit_, minCertifyingEscrow_, homeFactory_, receiptsFactory_, home_, receipts_, address(0)) {}

    function _take(uint256 amount) internal override {
        if (msg.value != amount) revert WrongValue();
    }

    function _sendCoin(address to, uint256 amount) internal override {
        if (amount == 0) return;
        (bool ok, ) = to.call{value: amount}("");
        if (!ok) revert TransferFailed();
    }

    function _coinBalance() internal view override returns (uint256) {
        return address(this).balance;
    }

    function _jobValue(uint256 paid) internal pure override returns (uint256) {
        return paid;
    }

    /// @dev Only the protocol sends the coin here unasked, when the vault
    /// collects its credit.
    receive() external payable {
        if (msg.sender != address(protocol)) revert();
    }
}
