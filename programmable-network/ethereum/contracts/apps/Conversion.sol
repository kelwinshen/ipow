// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

import {IERC20} from "@openzeppelin/contracts/token/ERC20/IERC20.sol";
import {SafeERC20} from "@openzeppelin/contracts/token/ERC20/utils/SafeERC20.sol";
import {ReentrancyGuard} from "@openzeppelin/contracts/utils/ReentrancyGuard.sol";

import {iPoWProtocol} from "../protocol/iPoWProtocol.sol";
import {iPoWLightClient} from "../protocol/iPoWLightClient.sol";
import {BitcoinTxLib} from "../protocol/BitcoinTxLib.sol";
import {BitcoinPaymentLib} from "./BitcoinPaymentLib.sol";

/// @title Conversion
/// @notice An application on the iPoW protocol (D5, D8): a user swaps this
/// network's coin, or a token, for real BTC, or real BTC for them, at a
/// price the user sets. The operator that wins the job is the other side of
/// the swap. Design: docs/drafts/ipow-conversion-app.md.
///
/// Sell (coin to BTC): the user locks the coin. The operator's tagged
/// Bitcoin transaction pays the user. The operator receives the coin once
/// the proof's lock has ended with no successful challenge, so a proof on
/// made-up blocks never takes the user's coin.
///
/// Buy (BTC to coin): the operator locks the coin and names a Bitcoin
/// address; the user pays it. The operator's tagged transaction spends the
/// payment (a receipt) and the user receives the coin, or says no payment
/// arrived (a close, a claim) and the operator takes the coin back. The
/// user may prove the payment themselves (D10), on blocks on top of the
/// job's anchor.
///
/// Every job asks the protocol's lowest escrow: the swap's value is kept
/// safe here, not by the escrow. No key can change this contract (D59).
contract Conversion is ReentrancyGuard {
    using SafeERC20 for IERC20;

    enum Side {
        None,
        Sell,
        Buy
    }

    enum State {
        None,
        /// Sell: waiting for the operator's payment. Buy: waiting for the
        /// operator to lock the coin.
        Open,
        /// Buy only: the operator locked the coin.
        Funded,
        /// The coin went to the operator (sell) or to the user (buy).
        Done,
        /// Sell: the coin went back to the user.
        Refunded,
        /// Buy: the operator did not lock the coin in time.
        Cancelled,
        /// Buy: the coin went back to the operator.
        Reclaimed
    }

    struct Swap {
        Side side;
        State state;
        address user;
        /// Zero for this network's own coin.
        address token;
        uint256 amount;
        /// Sell: the least the user accepts. Buy: what the user pays.
        uint64 sats;
        uint256 jobId;
        /// Sell: the user's script, to be paid. Buy: the operator's script,
        /// set when it locks the coin.
        bytes script;
    }

    /// @notice The challenge period of a close: the one kind of claim.
    uint32 public constant CLOSE_PERIOD = 36 hours;
    uint16 public constant CLOSE = 1;
    /// @notice Buy: how long the operator has to lock the coin, from the end
    /// of the auction.
    uint256 public constant FUNDING_TIME = 30 minutes;
    /// @notice Buy: the user pays in one of the 12 blocks after the anchor.
    /// A close counts only when mined after them.
    uint32 public constant PAY_BLOCKS = 12;
    /// @notice Buy: the operator may lock the coin, and so reveal the script
    /// the user pays, only while its anchor is at most this old: the user
    /// then has most of the payment blocks left.
    uint256 public constant ANCHOR_AGE = 30 minutes;
    uint16 public constant ESCROW_FEE_BPS = 50;
    uint256 public constant MAX_SCRIPT_LENGTH = 100;

    iPoWProtocol public immutable protocol;
    iPoWLightClient public immutable lightClient;
    /// @notice Buy: the largest payment, in satoshis. A user who proves
    /// their own payment on made-up blocks must mine at Bitcoin's full
    /// difficulty on top of the anchor; this keeps that unprofitable.
    uint64 public immutable maxSats;

    uint256 public swapCount;
    mapping(uint256 => Swap) private _swaps;
    /// @notice Buy: an operator's script is used for one swap only, so a
    /// payment to it belongs to that swap.
    mapping(bytes32 => bool) public scriptUsed;
    /// @notice The escrow shares the protocol paid this application for
    /// slashed jobs, withdrawn and not yet passed on.
    uint256 public compensation;
    /// @notice Whether a swap's share of its slashed job's escrow was passed
    /// on to its user.
    mapping(uint256 => bool) public compensated;

    event Sold(uint256 indexed swapId, uint256 indexed jobId, address indexed user, address token, uint256 amount, uint64 sats, bytes script);
    event Bought(uint256 indexed swapId, uint256 indexed jobId, address indexed user, address token, uint256 amount, uint64 sats);
    event Funded(uint256 indexed swapId, address indexed operator, bytes script);
    event Completed(uint256 indexed swapId, address indexed to);
    event Refunded(uint256 indexed swapId);
    event Cancelled(uint256 indexed swapId);
    event Reclaimed(uint256 indexed swapId);

    error InvalidAmount();
    error InvalidScript();
    error TooLarge();
    error WrongState();
    error NotOperator();
    error FundingTimeOver();
    error FundingTimeNotOver();
    error NotProven();
    error LockNotEnded();
    error WrongTransaction();
    error NotPaid();
    error NotRefundable();
    error NotReclaimable();
    error OutsidePaymentBlocks();
    error TooFewConfirmations();
    error NotLinked();
    error NotInBlock();
    error ScriptUsed();
    error NotFromProtocol();
    error TransferFailed();
    error NotAnchored();
    error AnchorTooOld();
    error NotSlashed();

    constructor(iPoWProtocol protocol_, uint64 maxSats_) {
        if (maxSats_ == 0) revert InvalidAmount();
        protocol = protocol_;
        lightClient = protocol_.lightClient();
        maxSats = maxSats_;
        uint32[] memory periods = new uint32[](1);
        periods[0] = CLOSE_PERIOD;
        protocol_.registerApplication(periods);
    }

    /// @notice Only the protocol pays this contract in the coin directly:
    /// the escrow shares of slashed jobs.
    receive() external payable {
        if (msg.sender != address(protocol)) revert NotFromProtocol();
    }

    function getSwap(uint256 swapId) external view returns (Swap memory) {
        return _swaps[swapId];
    }

    /// @notice The tag of a swap's job (D80).
    function tagOf(uint256 swapId) public view returns (bytes32) {
        return keccak256(abi.encode("iPoW conversion", block.chainid, address(this), swapId));
    }

    /// @notice The fees of a swap's job at this block's base fee: the
    /// commitment fee and the escrow fee on the lowest escrow. The base fee
    /// can rise before the swap's transaction runs, so send a little more;
    /// what is sent above the fees goes to the operator (D79). Some nodes
    /// answer a call at a base fee of zero, and this is then zero.
    function feesFor(uint16 confirmations) external view returns (uint256) {
        (, uint256 escrowFee, uint256 fee) = _escrow(confirmations);
        return fee + escrowFee;
    }

    // ------------------------------------------------------------------
    // Sell: coin to BTC
    // ------------------------------------------------------------------

    /// @notice Locks `amount` of `token` (zero for the coin, sent with the
    /// call) for at least `sats` paid to `script`. The job's fees are the
    /// rest of the coin sent.
    function sell(
        address token,
        uint256 amount,
        uint64 sats,
        bytes calldata script,
        uint16 confirmations
    ) external payable nonReentrant returns (uint256 swapId) {
        if (amount == 0 || sats == 0) revert InvalidAmount();
        if (script.length == 0 || script.length > MAX_SCRIPT_LENGTH) revert InvalidScript();
        uint256 fees = msg.value;
        if (token == address(0)) {
            if (msg.value < amount) revert InvalidAmount();
            fees = msg.value - amount;
        } else {
            amount = _pull(token, amount);
        }
        swapId = ++swapCount;
        Swap storage s = _swaps[swapId];
        s.side = Side.Sell;
        s.state = State.Open;
        s.user = msg.sender;
        s.token = token;
        s.amount = amount;
        s.sats = sats;
        s.script = script;
        s.jobId = _openJob(swapId, confirmations, 0, fees);
        emit Sold(swapId, s.jobId, msg.sender, token, amount, sats, script);
    }

    /// @notice Pays the operator once its transaction, which pays the user
    /// enough, is proven and the proof's lock has ended with no challenge
    /// won. Anyone may call.
    /// @param rawTx The job's transaction, without witness data.
    function completeSell(uint256 swapId, bytes calldata rawTx) external nonReentrant {
        Swap storage s = _swaps[swapId];
        if (s.side != Side.Sell || s.state != State.Open) revert WrongState();
        iPoWProtocol.Duty memory duty = _provenDuty(s.jobId, rawTx);
        if (duty.slashed) revert NotProven();
        if (block.timestamp < duty.lockEnd || protocol.openChallengesOf(s.jobId) != 0) revert LockNotEnded();
        if (!BitcoinPaymentLib.paysAtLeast(rawTx, s.script, s.sats)) revert NotPaid();

        s.state = State.Done;
        address operator = protocol.getJob(s.jobId).operator;
        emit Completed(swapId, operator);
        _send(s.token, operator, s.amount);
    }

    /// @notice Gives the user their coin back when the operator did not pay:
    /// nobody took the job, the deadline passed with no proof, the proof was
    /// shown false, or the proven transaction does not pay the user enough
    /// (then `rawTx` is that transaction). Anyone may call.
    function refundSell(uint256 swapId, bytes calldata rawTx) external nonReentrant {
        Swap storage s = _swaps[swapId];
        if (s.side != Side.Sell || s.state != State.Open) revert WrongState();
        uint256 jobId = s.jobId;
        iPoWProtocol.JobStatus status = protocol.statusOf(jobId);

        bool refundable;
        if (status == iPoWProtocol.JobStatus.Expired || status == iPoWProtocol.JobStatus.Slashed) {
            refundable = true;
        } else if (status == iPoWProtocol.JobStatus.Assigned) {
            refundable = block.timestamp > protocol.deadlineOf(jobId);
        } else if (status == iPoWProtocol.JobStatus.Proven || status == iPoWProtocol.JobStatus.Settled) {
            _provenDuty(jobId, rawTx);
            refundable = !BitcoinPaymentLib.paysAtLeast(rawTx, s.script, s.sats);
        }
        if (!refundable) revert NotRefundable();

        s.state = State.Refunded;
        emit Refunded(swapId);
        _send(s.token, s.user, s.amount);
        if (status == iPoWProtocol.JobStatus.Slashed) _compensate(swapId, s);
    }

    /// @notice Passes on to the swap's user this application's share of its
    /// job's escrow, once the job is slashed: 80% of x (D7, D36). Once per
    /// swap. Anyone may call, whenever the slash happened.
    function compensate(uint256 swapId) external nonReentrant {
        Swap storage s = _swaps[swapId];
        if (s.side == Side.None) revert WrongState();
        if (protocol.statusOf(s.jobId) != iPoWProtocol.JobStatus.Slashed) revert NotSlashed();
        _compensate(swapId, s);
    }

    // ------------------------------------------------------------------
    // Buy: BTC to coin
    // ------------------------------------------------------------------

    /// @notice Asks for `amount` of `token` (zero for the coin) for `sats`
    /// paid on Bitcoin. The coin sent pays the job's fees.
    function buy(address token, uint256 amount, uint64 sats, uint16 confirmations) external payable nonReentrant returns (uint256 swapId) {
        if (amount == 0 || sats == 0) revert InvalidAmount();
        if (sats > maxSats) revert TooLarge();
        swapId = ++swapCount;
        Swap storage s = _swaps[swapId];
        s.side = Side.Buy;
        s.state = State.Open;
        s.user = msg.sender;
        s.token = token;
        s.amount = amount;
        s.sats = sats;
        s.jobId = _openJob(swapId, confirmations, CLOSE, msg.value);
        emit Bought(swapId, s.jobId, msg.sender, token, amount, sats);
    }

    /// @notice The operator of the job locks the coin the user will receive,
    /// and names the script the user pays. The script must be new.
    function fund(uint256 swapId, bytes calldata script) external payable nonReentrant {
        Swap storage s = _swaps[swapId];
        if (s.side != Side.Buy || s.state != State.Open) revert WrongState();
        if (script.length == 0 || script.length > MAX_SCRIPT_LENGTH) revert InvalidScript();
        if (protocol.statusOf(s.jobId) != iPoWProtocol.JobStatus.Assigned) revert WrongState();
        if (protocol.getJob(s.jobId).operator != msg.sender) revert NotOperator();
        if (block.timestamp > protocol.auctionEndOf(s.jobId) + FUNDING_TIME) revert FundingTimeOver();
        // The script is revealed only once the payment blocks are fixed, and
        // while most of them are still ahead.
        iPoWProtocol.BlockRef memory anchor = protocol.getDuty(s.jobId).anchor;
        if (anchor.hash == bytes32(0)) revert NotAnchored();
        uint256 anchorTime = lightClient.getNode(lightClient.nodeId(anchor.hash, anchor.height, anchor.epochTime)).time;
        if (block.timestamp > anchorTime + ANCHOR_AGE) revert AnchorTooOld();
        bytes32 key = keccak256(script);
        if (scriptUsed[key]) revert ScriptUsed();
        scriptUsed[key] = true;
        s.state = State.Funded;
        s.script = script;

        if (s.token == address(0)) {
            if (msg.value != s.amount) revert InvalidAmount();
        } else {
            if (msg.value != 0) revert InvalidAmount();
            // What arrives must be what the user was promised.
            if (_pull(s.token, s.amount) != s.amount) revert InvalidAmount();
        }
        emit Funded(swapId, msg.sender, script);
    }

    /// @notice Ends a swap whose operator did not lock the coin in time, or
    /// that nobody took. The user has paid nothing on Bitcoin. Anyone may
    /// call.
    function cancel(uint256 swapId) external {
        Swap storage s = _swaps[swapId];
        if (s.side != Side.Buy || s.state != State.Open) revert WrongState();
        iPoWProtocol.JobStatus status = protocol.statusOf(s.jobId);
        if (status == iPoWProtocol.JobStatus.Auction) revert FundingTimeNotOver();
        if (status != iPoWProtocol.JobStatus.Expired && block.timestamp <= protocol.auctionEndOf(s.jobId) + FUNDING_TIME) {
            revert FundingTimeNotOver();
        }
        s.state = State.Cancelled;
        emit Cancelled(swapId);
    }

    /// @notice Gives the user the coin once the operator's proven
    /// transaction spends the user's payment: the receipt. Anyone may call.
    /// @param receiptRaw The job's transaction, without witness data.
    /// @param paymentRaw The user's payment, without witness data.
    /// @param vout The output of the payment that pays the operator's script.
    function completeBuy(uint256 swapId, bytes calldata receiptRaw, bytes calldata paymentRaw, uint32 vout) external nonReentrant {
        Swap storage s = _swaps[swapId];
        if (s.side != Side.Buy || s.state != State.Funded) revert WrongState();
        _provenDuty(s.jobId, receiptRaw);
        if (!BitcoinPaymentLib.spends(receiptRaw, BitcoinTxLib.txid(paymentRaw), vout)) revert WrongTransaction();
        if (!BitcoinPaymentLib.outputPays(paymentRaw, vout, s.script, s.sats)) revert NotPaid();
        _complete(swapId, s);
    }

    /// @notice The user proves their payment themselves (D10). Anyone may
    /// call; the coin goes to the user. The payment is in one of the payment
    /// blocks, and:
    /// - when the operator proved a close mined after the payment blocks, in
    ///   a block below that close, on the chain the operator proved, which
    ///   the protocol's guardians watch; `top` is not used;
    /// - otherwise, once the operator can no longer prove (its deadline
    ///   passed, it was slashed, or its close came within the payment
    ///   blocks), in a block on top of the anchor's parent (so a
    ///   reorganisation that took the anchor away does not strand it), with
    ///   `top` giving the job's confirmations.
    /// @param payBlock The block of the payment, stored in the light client.
    /// @param prevEpochTime The epoch time of the older epoch when the blocks
    /// walked lie in two epochs, zero otherwise.
    function proveMyPayment(
        uint256 swapId,
        bytes calldata paymentRaw,
        uint32 vout,
        iPoWProtocol.BlockRef calldata payBlock,
        bytes32[] calldata siblings,
        uint256 txIndex,
        iPoWProtocol.BlockRef calldata top,
        uint32 prevEpochTime
    ) external nonReentrant {
        Swap storage s = _swaps[swapId];
        if (s.side != Side.Buy || s.state != State.Funded) revert WrongState();
        iPoWProtocol.Duty memory duty = protocol.getDuty(s.jobId);
        iPoWProtocol.BlockRef memory anchor = duty.anchor;
        if (payBlock.height <= anchor.height || payBlock.height > anchor.height + PAY_BLOCKS) revert OutsidePaymentBlocks();

        iPoWProtocol.JobStatus status = protocol.statusOf(s.jobId);
        bool proven = status == iPoWProtocol.JobStatus.Proven || status == iPoWProtocol.JobStatus.Settled;
        if (proven && !duty.slashed && duty.proofBlock.height > anchor.height + PAY_BLOCKS) {
            // Below the operator's close, on its proven chain.
            if (!_linked(anchor, payBlock, prevEpochTime) || !_linked(payBlock, duty.proofBlock, prevEpochTime)) revert NotLinked();
        } else {
            bool failed = status == iPoWProtocol.JobStatus.Slashed || proven ||
                (status == iPoWProtocol.JobStatus.Assigned && block.timestamp > protocol.deadlineOf(s.jobId));
            if (!failed) revert NotLinked();
            uint16 confirmations = protocol.getJob(s.jobId).confirmations;
            if (top.height < payBlock.height || top.height - payBlock.height + 1 < confirmations) revert TooFewConfirmations();
            iPoWLightClient.Node memory anchorNode = lightClient.getNode(lightClient.nodeId(anchor.hash, anchor.height, anchor.epochTime));
            iPoWProtocol.BlockRef memory parent = iPoWProtocol.BlockRef({
                hash: anchorNode.prevHash,
                height: anchor.height - 1,
                epochTime: anchor.height % 2016 == 0 ? prevEpochTime : anchor.epochTime
            });
            if (!_linked(parent, payBlock, prevEpochTime) || !_linked(payBlock, top, prevEpochTime)) revert NotLinked();
        }
        bytes32 id = lightClient.nodeId(payBlock.hash, payBlock.height, payBlock.epochTime);
        if (!lightClient.txInBlock(id, paymentRaw, siblings, txIndex)) revert NotInBlock();
        if (!BitcoinPaymentLib.outputPays(paymentRaw, vout, s.script, s.sats)) revert NotPaid();
        _complete(swapId, s);
    }

    /// @notice Gives the operator its coin back when the user did not pay.
    /// Either its close was mined after the payment blocks and its lock has
    /// ended with no challenge won, or 36 hours have passed since the job's
    /// deadline. Until then the user may still prove a payment. Anyone may
    /// call.
    function reclaim(uint256 swapId) external nonReentrant {
        Swap storage s = _swaps[swapId];
        if (s.side != Side.Buy || s.state != State.Funded) revert WrongState();
        uint256 jobId = s.jobId;
        iPoWProtocol.JobStatus status = protocol.statusOf(jobId);
        iPoWProtocol.Duty memory duty = protocol.getDuty(jobId);

        bool closed = (status == iPoWProtocol.JobStatus.Proven || status == iPoWProtocol.JobStatus.Settled) &&
            !duty.slashed &&
            duty.proofBlock.height > duty.anchor.height + PAY_BLOCKS &&
            block.timestamp >= duty.lockEnd &&
            protocol.openChallengesOf(jobId) == 0;
        bool late = block.timestamp >= protocol.deadlineOf(jobId) + CLOSE_PERIOD;
        if (!closed && !late) revert NotReclaimable();

        s.state = State.Reclaimed;
        address operator = protocol.getJob(jobId).operator;
        emit Reclaimed(swapId);
        _send(s.token, operator, s.amount);
    }

    // ------------------------------------------------------------------
    // Inside
    // ------------------------------------------------------------------

    function _complete(uint256 swapId, Swap storage s) private {
        s.state = State.Done;
        emit Completed(swapId, s.user);
        _send(s.token, s.user, s.amount);
    }

    /// @dev The duty of a proven job whose transaction is `rawTx`.
    function _provenDuty(uint256 jobId, bytes calldata rawTx) private view returns (iPoWProtocol.Duty memory duty) {
        iPoWProtocol.JobStatus status = protocol.statusOf(jobId);
        if (status != iPoWProtocol.JobStatus.Proven && status != iPoWProtocol.JobStatus.Settled) revert NotProven();
        duty = protocol.getDuty(jobId);
        if (BitcoinTxLib.txid(rawTx) != duty.txid) revert WrongTransaction();
    }

    /// @dev The lowest escrow the protocol accepts, its fee, and the
    /// commitment fee, at this moment's price.
    function _escrow(uint16 confirmations) private view returns (uint256 escrow, uint256 escrowFee, uint256 fee) {
        fee = protocol.commitmentFeeFor(confirmations);
        escrow = protocol.MIN_ESCROW_MULTIPLE() * fee;
        // At a base fee of zero (a gas estimate on some nodes) the lowest
        // escrow is zero, which the protocol refuses (D55).
        if (escrow == 0) escrow = 1;
        escrowFee = (escrow * ESCROW_FEE_BPS) / protocol.BPS();
    }

    function _openJob(uint256 swapId, uint16 confirmations, uint16 claimKind, uint256 fees) private returns (uint256) {
        (uint256 escrow, , ) = _escrow(confirmations);
        // The user receives the fees back if the operator fails (D62).
        return protocol.openJob{value: fees}(tagOf(swapId), escrow, ESCROW_FEE_BPS, confirmations, claimKind, msg.sender);
    }

    /// @dev Passes on the application's share of a slashed job's escrow.
    function _compensate(uint256 swapId, Swap storage s) private {
        if (compensated[swapId]) return;
        compensated[swapId] = true;
        if (protocol.credit(address(this)) != 0) {
            uint256 before = address(this).balance;
            protocol.withdrawCredit();
            compensation += address(this).balance - before;
        }
        uint256 share = (protocol.getJob(s.jobId).escrow * protocol.APPLICATION_SHARE_BPS()) / protocol.BPS();
        if (share > compensation) share = compensation;
        compensation -= share;
        _send(address(0), s.user, share);
    }

    function _linked(iPoWProtocol.BlockRef memory low, iPoWProtocol.BlockRef memory high, uint32 prevEpochTime) private view returns (bool) {
        return lightClient.isAncestor(low.hash, low.height, low.epochTime, high.hash, high.height, high.epochTime, prevEpochTime);
    }

    /// @dev Takes a token, and returns what arrived: a token that takes a
    /// fee on transfer delivers less than asked.
    function _pull(address token, uint256 amount) private returns (uint256) {
        uint256 before = IERC20(token).balanceOf(address(this));
        IERC20(token).safeTransferFrom(msg.sender, address(this), amount);
        return IERC20(token).balanceOf(address(this)) - before;
    }

    function _send(address token, address to, uint256 amount) private {
        if (amount == 0) return;
        if (token == address(0)) {
            (bool ok, ) = to.call{value: amount}("");
            if (!ok) revert TransferFailed();
        } else {
            IERC20(token).safeTransfer(to, amount);
        }
    }
}
