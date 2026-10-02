// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

import {ReentrancyGuard} from "@openzeppelin/contracts/utils/ReentrancyGuard.sol";

import {iPoWLightClient} from "./iPoWLightClient.sol";
import {BitcoinTxLib} from "./BitcoinTxLib.sol";
import {IDataFee} from "./DataFee.sol";

/// @title iPoWProtocol
/// @notice Operators, jobs, fees and the auction of the iPoW protocol. Spec:
/// docs/design/ipow-protocol.md, sections 3 to 5. The `D` numbers below are
/// the decisions of that document.
///
/// There is no owner and no key, and nothing here can be changed after
/// deployment (D59). Anyone can become an operator (D12) and anyone can
/// register an application (D63).
///
/// @dev Built: the bond, the registration of an application, opening a
/// job, the fees, the auction, the chain head, the duty and its proof, the
/// slash of a missed duty, the guardian's reward, paying the operator when
/// the lock ends, the challenge of a proof, and claims and attest.
///
/// Money is never pushed to an address. It is credited and the address
/// withdraws it.
///
/// This is the shared logic of two builds (D137): `iPoWProtocolNative`, whose
/// money is the network's native coin, and `iPoWProtocolToken`, whose money
/// is a token, for a network without a native coin (D136). They differ only
/// in how money moves (`_take`, `_send`) and in the unit of the price.
abstract contract iPoWProtocol is ReentrancyGuard {
    // ------------------------------------------------------------------
    // Rules fixed by the protocol
    // ------------------------------------------------------------------

    /// @notice D41: confirmations a job requires by default, and at least.
    uint16 public constant DEFAULT_CONFIRMATIONS = 6;
    /// @notice D15: the proof is in blocks 1 to 25 of the window.
    uint16 public constant PROOF_RANGE = 25;
    /// @notice D70: a window is at most 100 blocks.
    uint16 public constant MAX_WINDOW = 100;
    /// @notice D69: the deadline is 48 minutes for each block of the window,
    /// which is 1 day for 30 blocks (D14).
    uint256 public constant DEADLINE_PER_BLOCK = 48 minutes;

    /// @notice D37: bidding is open for 15 minutes.
    uint256 public constant AUCTION_DURATION = 15 minutes;
    /// @notice D37: the winner is locked in once 1 minute passes with no
    /// better bid.
    uint256 public constant AUCTION_QUIET = 1 minutes;
    /// @notice D76: a better bid is at least 0.1% above the best bid.
    uint256 public constant BID_STEP_DIVISOR = 1000;

    /// @notice D25: the escrow fee is 0.5% of x by default.
    uint16 public constant DEFAULT_ESCROW_FEE_BPS = 50;
    /// @notice D77: the escrow fee is at most 100% of x.
    uint16 public constant BPS = 10_000;
    /// @notice D56: the minimum escrow is 5 times the commitment fee.
    uint256 public constant MIN_ESCROW_MULTIPLE = 5;

    /// @notice D24: the safety margin of the commitment fee is 1.5.
    uint256 public constant MARGIN_NUMERATOR = 3;
    uint256 public constant MARGIN_DENOMINATOR = 2;
    /// @notice The work of streaming one block, in gas. Measured on the light
    /// client on 2026-09-28: about 76,000 when blocks are streamed together.
    uint256 public constant WORK_PER_BLOCK = 80_000;
    /// @notice The work of a job that does not depend on its window, in gas.
    /// Measured on 2026-09-28: about 175,000 for the jump, 111,000 for naming
    /// the anchor and 216,000 for the proof, which is 502,000. The proof was
    /// measured with a Merkle proof of 1 level; a real block has about 12.
    uint256 public constant WORK_FIXED = 520_000;
    /// @notice D135: the bytes of the work's transactions, each with its
    /// envelope and signature (about 116 bytes), for a rollup's data fee.
    /// Measured on 2026-10-02 from the calls' encoding: 228 bytes of calldata
    /// to stream one block, one transaction per block, as an operator streams
    /// blocks as they come (streamed together, a further block adds 96); 228
    /// for the jump, 132 for naming the anchor and 1,092 for a proof of 12
    /// levels of a 168-byte transaction. A larger transaction or proof is
    /// above this; the oracles' prices are above what is paid (DataFee.sol).
    uint256 internal constant WORK_BYTES_PER_BLOCK = 350;
    uint256 internal constant WORK_BYTES_FIXED = 1_800;

    /// @notice D39: an anchor may be at most 2 hours old.
    uint256 public constant MAX_ANCHOR_AGE = 2 hours;
    /// @notice D50: the escrow stays locked 36 hours after a job.
    uint256 public constant MIN_LOCK = 36 hours;
    /// @notice D71: the lock and the challenge period are at least 1.5 times
    /// the deadline of the job.
    uint256 public constant LOCK_NUMERATOR = 3;
    uint256 public constant LOCK_DENOMINATOR = 2;
    /// @notice D81: the time to show a parent. D99: no challenge opens in
    /// the last 12 hours of the lock.
    uint256 public constant RESPONSE_TIME = 12 hours;
    /// @notice D92: at most 2,016 questions for a parent per job.
    uint256 public constant MAX_PARENT_QUESTIONS = 2016;
    /// @notice D35, D36: 80% of a slash goes to the application, 20% to the
    /// guardian.
    uint256 public constant APPLICATION_SHARE_BPS = 8000;
    /// @notice D42: an attester earns up to 40% of the escrow fee, in
    /// proportion to how long it kept x locked (D96).
    uint256 public constant ATTESTER_SHARE_BPS = 4000;

    /// @notice D28: the challenge period is between 36 hours and 7 days.
    uint32 public constant MIN_CHALLENGE_PERIOD = 36 hours;
    uint32 public constant MAX_CHALLENGE_PERIOD = 7 days;
    /// @notice D78: an application registers at most 32 kinds of claim.
    uint256 public constant MAX_CLAIM_KINDS = 32;

    iPoWLightClient public immutable lightClient;

    // ------------------------------------------------------------------
    // Storage
    // ------------------------------------------------------------------

    struct Operator {
        /// Everything the operator has locked in the protocol (D31).
        uint256 bond;
        /// The part of the bond that is locked for jobs, as escrow or as a
        /// bid that is still the best.
        uint256 locked;
    }

    struct Application {
        bool registered;
        /// D17: the challenge period of each kind of claim, set once.
        uint32[] challengePeriods;
    }

    enum JobStatus {
        None,
        /// Bidding is open.
        Auction,
        /// The auction closed with no bid. The fees were or can be returned.
        Expired,
        /// An operator won. Its deadline runs from `lockedInAt`.
        Assigned,
        /// The proof was accepted. The duty has ended and the lock runs.
        Proven,
        /// The operator failed. Its escrow was slashed and the fees returned.
        Slashed,
        /// The lock ended. The operator was paid and its bond is free.
        Settled
    }

    /// @notice Where a block is in the light client.
    struct BlockRef {
        bytes32 hash;
        uint32 height;
        uint32 epochTime;
    }

    /// @notice The coin an operator's next tagged transaction must spend (D30).
    struct ChainHead {
        bytes32 txid;
        uint32 vout;
        bool set;
    }

    struct Duty {
        BlockRef anchor;
        BlockRef proofBlock;
        /// The block on top of the proof that was shown with it.
        BlockRef tip;
        /// The oldest block of the operator's branch that was asked for and
        /// shown (D81). The anchor until a parent is shown.
        BlockRef deepest;
        /// How many parents were shown. The next question is number
        /// `parentsShown + 1` (D91).
        uint32 parentsShown;
        /// D42: who locked x in the operator's place. Zero if nobody did.
        /// It may be the operator itself (D95).
        address attester;
        uint40 attestedAt;
        bytes32 txid;
        uint40 anchoredAt;
        uint40 provenAt;
        uint40 lockEnd;
        bool slashed;
        bool settled;
    }

    enum ChallengeKind {
        None,
        /// A guardian asked for the parent of the oldest block.
        Parent,
        /// A guardian showed a competing branch.
        Fork
    }

    /// @notice An open challenge of a proof (D49, D81). A job can have
    /// several at the same time, from different guardians (D88).
    struct Challenge {
        ChallengeKind kind;
        uint256 jobId;
        address guardian;
        /// D81, D91: the commitment fee of the job, times the number of the
        /// question for a parent.
        uint256 deposit;
        uint40 openedAt;
        /// Parent only: the block whose parent is asked for.
        BlockRef asked;
        /// Fork only: the most mining work shown on each branch, counted from
        /// the block where they part.
        uint256 operatorWork;
        uint256 guardianWork;
    }

    /// @notice What is submitted to prove a tagged transaction.
    struct Proof {
        /// The block that holds the transaction.
        BlockRef proofBlock;
        /// A block on top of it, far enough for the confirmations.
        BlockRef tip;
        /// Only used when the window crossed into a new epoch: the epoch time
        /// of the older epoch. Zero otherwise.
        uint32 prevEpochTime;
        /// The transaction, without witness data.
        bytes rawTx;
        bytes32[] siblings;
        uint256 txIndex;
        /// The input that spends the operator's chain head. The output with
        /// the same number is the next chain head.
        uint32 headIndex;
        /// The output that carries the tag.
        uint32 tagIndex;
    }

    struct Job {
        address application;
        /// Who paid the fees and receives them back (D62, D65).
        address payer;
        bytes32 tag;
        /// x, the escrow the application requests (D40).
        uint256 escrow;
        /// What the job holds as commitment fee: the fee at the price of the
        /// moment (D24), and what was sent above the fees (D79).
        uint256 commitmentFee;
        uint256 escrowFee;
        /// The best bid so far, at least x (D32).
        uint256 bid;
        address operator;
        uint16 confirmations;
        /// 0 for a settlement. Otherwise the number of the application's kind
        /// of claim, counted from 1.
        uint16 claimKind;
        uint40 openedAt;
        uint40 lastBidAt;
        bool feesReturned;
    }

    mapping(address => Operator) private _operators;
    mapping(address => Application) private _applications;
    mapping(uint256 => Job) private _jobs;
    /// @dev keccak256(application, tag) => job. An application uses a tag once.
    mapping(bytes32 => uint256) private _jobOfTag;
    mapping(uint256 => Duty) private _duties;
    /// @dev Challenge number => challenge. Numbers start at 1.
    mapping(uint256 => Challenge) private _challenges;
    uint256 public challengeCount;
    /// @notice How many challenges of a job are open. The operator is not
    /// paid while any is open.
    mapping(uint256 => uint256) public openChallengesOf;
    /// @dev A block shown on one branch of a fork challenge => the mining
    /// work of that branch up to and with it. Any of them can be built on, so
    /// nobody can freeze a branch by showing a block that goes nowhere.
    mapping(bytes32 => uint256) private _checkpoints;
    mapping(address => ChainHead) private _chainHeads;
    /// @notice D80: a Bitcoin transaction settles one job on this network.
    mapping(bytes32 => bool) public usedTx;
    /// @dev A guardian's sealed note => the block it was sealed in (D47).
    mapping(bytes32 => uint256) private _noteBlock;
    /// @notice Money an address can withdraw.
    mapping(address => uint256) public credit;
    uint256 public jobCount;

    // ------------------------------------------------------------------
    // Events and errors
    // ------------------------------------------------------------------

    event BondLocked(address indexed operator, uint256 amount);
    event BondWithdrawn(address indexed operator, uint256 amount);
    event ApplicationRegistered(address indexed application, uint32[] challengePeriods);
    event JobOpened(
        uint256 indexed jobId,
        address indexed application,
        bytes32 indexed tag,
        uint256 escrow,
        uint256 commitmentFee,
        uint256 escrowFee,
        uint16 confirmations,
        uint16 claimKind,
        address payer
    );
    event BidPlaced(uint256 indexed jobId, address indexed operator, uint256 amount);
    event JobExpired(uint256 indexed jobId);
    event ChainHeadSet(address indexed operator, bytes32 txid, uint32 vout);
    event JobAnchored(uint256 indexed jobId, bytes32 anchorHash, uint32 height, uint32 epochTime);
    event JobProven(uint256 indexed jobId, bytes32 indexed txid, uint256 lockEnd);
    event JobSlashed(
        uint256 indexed jobId,
        address indexed operator,
        address indexed guardian,
        uint256 toApplication,
        uint256 toGuardian
    );
    event JobSettled(uint256 indexed jobId, address indexed operator, uint256 paid);
    event NoteSealed(bytes32 indexed note);
    event ParentAsked(
        uint256 indexed challengeId,
        uint256 indexed jobId,
        address indexed guardian,
        bytes32 childHash,
        uint256 question
    );
    event ParentShown(uint256 indexed challengeId, bytes32 parentHash);
    event ForkChallenged(
        uint256 indexed challengeId,
        uint256 indexed jobId,
        address indexed guardian,
        bytes32 operatorBlock,
        bytes32 guardianBlock
    );
    event BranchExtended(uint256 indexed challengeId, bool guardianSide, bytes32 tip, uint256 work);
    event ChallengeWon(uint256 indexed challengeId, address indexed guardian);
    event ChallengeFailed(uint256 indexed challengeId, address indexed guardian);
    event ChallengeRefunded(uint256 indexed challengeId, address indexed guardian);
    event Attested(uint256 indexed jobId, address indexed attester, uint256 amount);
    event Credited(address indexed to, uint256 amount);
    event CreditWithdrawn(address indexed to, uint256 amount);

    error ZeroAmount();
    error ZeroAddress();
    error BondNotFree();
    error TransferFailed();
    error AlreadyRegistered();
    error NotRegistered();
    error TooManyClaimKinds();
    error ChallengePeriodOutOfRange();
    error UnknownClaimKind();
    error TagUsed();
    error ConfirmationsOutOfRange();
    error EscrowFeeOutOfRange();
    error EscrowTooLow();
    error FeesNotPaid();
    error UnknownJob();
    error AuctionClosed();
    error AuctionOpen();
    error BidTooLow();
    error JobHasBid();
    error FeesAlreadyReturned();
    error ChainHeadExists();
    error NoChainHead();
    error NotInBlock();
    error NoCoin();
    error WrongTag();
    error WrongChainHead();
    error TransactionUsed();
    error NotOperator();
    error NotAssigned();
    error AlreadyAnchored();
    error NotAnchored();
    error UnknownBlock();
    error AnchorTooOld();
    error DeadlinePassed();
    error DeadlineNotPassed();
    error OutsideProofRange();
    error NotLinked();
    error NotEnoughConfirmations();
    error NotProven();
    error LockNotEnded();
    error NoteExists();
    error NoNote();
    error LockEnded();
    error ChallengeWindowClosed();
    error ChallengeOpen();
    error NoChallenge();
    error WrongValue();
    error ResponseTimeNotOver();
    error ResponseTimeOver();
    error NotOnOperatorBranch();
    error NotCompeting();
    error NotHeavier();
    error NoParent();
    error NotShown();
    error TooManyQuestions();
    error AlreadyAttested();

    constructor(iPoWLightClient lightClient_) {
        if (address(lightClient_) == address(0)) revert ZeroAddress();
        lightClient = lightClient_;
    }

    // ------------------------------------------------------------------
    // Operators (D12, D31, D43)
    // ------------------------------------------------------------------

    /// @notice Locks a bond of `amount`. This is all it takes to be an
    /// operator (D12). There is no minimum (D43). A native coin sends exactly
    /// `amount` (D138).
    function lockBond(uint256 amount) external payable {
        if (amount == 0) revert ZeroAmount();
        _take(amount);
        _operators[msg.sender].bond += amount;
        emit BondLocked(msg.sender, amount);
    }

    /// @notice Withdraws free bond. Bond that is locked for a job stays (D43).
    function withdrawBond(uint256 amount) external nonReentrant {
        if (amount == 0) revert ZeroAmount();
        Operator storage op = _operators[msg.sender];
        if (amount > op.bond - op.locked) revert BondNotFree();
        op.bond -= amount;
        emit BondWithdrawn(msg.sender, amount);
        _send(msg.sender, amount);
    }

    function bondOf(address operator) external view returns (uint256 bond, uint256 locked) {
        Operator storage op = _operators[operator];
        return (op.bond, op.locked);
    }

    // ------------------------------------------------------------------
    // Applications (D17, D28, D63)
    // ------------------------------------------------------------------

    /// @notice Registers the caller as an application. Anyone may (D63).
    /// @param challengePeriods The challenge period of each kind of claim the
    /// application makes, in seconds. They cannot be changed afterwards (D17).
    /// Empty for an application that makes no claims.
    function registerApplication(uint32[] calldata challengePeriods) external {
        Application storage app = _applications[msg.sender];
        if (app.registered) revert AlreadyRegistered();
        if (challengePeriods.length > MAX_CLAIM_KINDS) revert TooManyClaimKinds();
        for (uint256 i = 0; i < challengePeriods.length; i++) {
            if (challengePeriods[i] < MIN_CHALLENGE_PERIOD || challengePeriods[i] > MAX_CHALLENGE_PERIOD) {
                revert ChallengePeriodOutOfRange();
            }
        }
        app.registered = true;
        app.challengePeriods = challengePeriods;
        emit ApplicationRegistered(msg.sender, challengePeriods);
    }

    function isRegistered(address application) external view returns (bool) {
        return _applications[application].registered;
    }

    function challengePeriodsOf(address application) external view returns (uint32[] memory) {
        return _applications[application].challengePeriods;
    }

    // ------------------------------------------------------------------
    // Fees (D20, D22, D24, D25, D58, D69)
    // ------------------------------------------------------------------

    /// @notice The window of a job: the proof range plus the blocks that only
    /// confirm (D15, D68).
    function windowOf(uint16 confirmations) public pure returns (uint16) {
        if (confirmations < DEFAULT_CONFIRMATIONS) revert ConfirmationsOutOfRange();
        uint256 window = uint256(PROOF_RANGE) + confirmations - 1;
        if (window > MAX_WINDOW) revert ConfirmationsOutOfRange();
        return uint16(window);
    }

    /// @notice D24, D69: amount of work x current price x 1.5, where the work
    /// grows with the window. No person sets the price (D58).
    /// @dev Many nodes run a call that is not a transaction at a price of
    /// zero, and this then returns too little: zero, or on a rollup only the
    /// data part. To know the fee before sending, use `commitmentFeeAt` with
    /// the price of the network.
    function commitmentFeeFor(uint16 confirmations) public view returns (uint256) {
        return commitmentFeeAt(confirmations, _price());
    }

    /// @notice The commitment fee at a given price of work, in the network's
    /// coin: the price is the network's gas price as a contract sees it (the
    /// base fee; on Hedera the network's gas price in tinybars, D139),
    /// divided by the units of it per unit of the coin (D136) or, on
    /// Polkadot, by its gas per unit of Ethereum's (D140). On a rollup it adds the
    /// cost of posting the work's data, at the rollup's price now (D135).
    function commitmentFeeAt(uint16 confirmations, uint256 price) public view returns (uint256) {
        uint256 window = windowOf(confirmations);
        uint256 cost = (WORK_PER_BLOCK * window + WORK_FIXED) * price
            + _dataFee(WORK_BYTES_PER_BLOCK * window + WORK_BYTES_FIXED);
        return (cost * MARGIN_NUMERATOR) / MARGIN_DENOMINATOR / _priceScale();
    }

    /// @notice D69: the time an operator has, from the moment it is locked in.
    function dutyTimeFor(uint16 confirmations) public pure returns (uint256) {
        return uint256(windowOf(confirmations)) * DEADLINE_PER_BLOCK;
    }

    // ------------------------------------------------------------------
    // Jobs (D19, D40, D52, D55, D56, D65)
    // ------------------------------------------------------------------

    /// @notice Opens a job. Called by a registered application, which passes
    /// on the fees (D65).
    /// @param tag What the operator's Bitcoin transaction must carry (D6).
    /// @param escrow x, already 125% of what the application needs back (D40).
    /// @param escrowFeeBps The escrow fee in hundredths of a percent of x.
    /// 50 is the default of 0.5% (D25).
    /// @param confirmations 6 or more (D41).
    /// @param claimKind 0 for a settlement, otherwise the number of the kind
    /// of claim, counted from 1.
    /// @param payer Who receives the fees back when the job expires (D61) or
    /// the operator fails (D62).
    /// @dev The price is read when the transaction runs, so the sender cannot
    /// know the exact fee before and sends more. What it sends above the fees
    /// is kept for the operator as part of the commitment fee (D79).
    function openJob(
        bytes32 tag,
        uint256 escrow,
        uint16 escrowFeeBps,
        uint16 confirmations,
        uint16 claimKind,
        address payer,
        uint256 paid
    ) external payable returns (uint256 jobId) {
        Application storage app = _applications[msg.sender];
        if (!app.registered) revert NotRegistered();
        if (payer == address(0)) revert ZeroAddress();
        if (claimKind > app.challengePeriods.length) revert UnknownClaimKind();
        if (escrowFeeBps > BPS) revert EscrowFeeOutOfRange();

        bytes32 tagKey = keccak256(abi.encode(msg.sender, tag));
        if (_jobOfTag[tagKey] != 0) revert TagUsed();

        uint256 commitmentFee = commitmentFeeFor(confirmations);
        // D55, D56.
        if (escrow == 0 || escrow < MIN_ESCROW_MULTIPLE * commitmentFee) revert EscrowTooLow();
        // D33: on x, never on a bid.
        uint256 escrowFee = (escrow * escrowFeeBps) / BPS;

        // D138: the amount paid is named; a native coin sends exactly it.
        if (paid < commitmentFee + escrowFee) revert FeesNotPaid();
        // D79.
        commitmentFee = paid - escrowFee;

        jobId = ++jobCount;
        _jobOfTag[tagKey] = jobId;
        _jobs[jobId] = Job({
            application: msg.sender,
            payer: payer,
            tag: tag,
            escrow: escrow,
            commitmentFee: commitmentFee,
            escrowFee: escrowFee,
            bid: 0,
            operator: address(0),
            confirmations: confirmations,
            claimKind: claimKind,
            openedAt: uint40(block.timestamp),
            lastBidAt: 0,
            feesReturned: false
        });
        // Taken after the job is recorded: a token build calls the coin.
        _take(paid);
        emit JobOpened(
            jobId,
            msg.sender,
            tag,
            escrow,
            commitmentFee,
            escrowFee,
            confirmations,
            claimKind,
            payer
        );
    }

    function getJob(uint256 jobId) external view returns (Job memory) {
        return _jobs[jobId];
    }

    function jobOfTag(address application, bytes32 tag) external view returns (uint256) {
        return _jobOfTag[keccak256(abi.encode(application, tag))];
    }

    function statusOf(uint256 jobId) public view returns (JobStatus) {
        Job storage job = _jobs[jobId];
        if (job.openedAt == 0) return JobStatus.None;
        if (block.timestamp < _auctionEnd(job)) return JobStatus.Auction;
        if (job.operator == address(0)) return JobStatus.Expired;

        Duty storage duty = _duties[jobId];
        if (duty.slashed) return JobStatus.Slashed;
        if (duty.settled) return JobStatus.Settled;
        if (duty.provenAt != 0) return JobStatus.Proven;
        return JobStatus.Assigned;
    }

    function getDuty(uint256 jobId) external view returns (Duty memory) {
        return _duties[jobId];
    }

    function getChallenge(uint256 challengeId) external view returns (Challenge memory) {
        return _challenges[challengeId];
    }

    /// @notice The deposit of the next question for a parent of a job (D91).
    function parentDepositOf(uint256 jobId) public view returns (uint256) {
        return _jobs[jobId].commitmentFee * (uint256(_duties[jobId].parentsShown) + 1);
    }

    /// @notice When the winner was or will be locked in (D37). The deadline
    /// starts here (D60).
    function auctionEndOf(uint256 jobId) external view returns (uint256) {
        Job storage job = _jobs[jobId];
        if (job.openedAt == 0) revert UnknownJob();
        return _auctionEnd(job);
    }

    /// @notice D60, D69: the moment a proof is no longer accepted (D27).
    /// Zero while no winner is locked in.
    function deadlineOf(uint256 jobId) external view returns (uint256) {
        Job storage job = _jobs[jobId];
        if (job.openedAt == 0) revert UnknownJob();
        if (job.operator == address(0) || block.timestamp < _auctionEnd(job)) return 0;
        return _deadline(job);
    }

    /// @notice Everything the protocol holds for a job that was not returned
    /// or paid: its two fees.
    function feesHeldOf(uint256 jobId) external view returns (uint256) {
        Job storage job = _jobs[jobId];
        if (job.openedAt == 0) revert UnknownJob();
        return job.feesReturned ? 0 : job.commitmentFee + job.escrowFee;
    }

    /// @notice How long the escrow stays locked after the proof (D50, D66,
    /// D71, D74).
    function lockTimeOf(uint256 jobId) public view returns (uint256 lockTime) {
        Job storage job = _jobs[jobId];
        if (job.openedAt == 0) revert UnknownJob();

        // D71: at least 1.5 times the deadline of the job.
        uint256 floor = (dutyTimeFor(job.confirmations) * LOCK_NUMERATOR) / LOCK_DENOMINATOR;
        // D50.
        lockTime = floor > MIN_LOCK ? floor : MIN_LOCK;

        if (job.claimKind != 0) {
            // D66: the whole challenge period, which starts at the proof (D74).
            uint256 period = _applications[job.application].challengePeriods[job.claimKind - 1];
            if (period < floor) period = floor;
            if (period > lockTime) lockTime = period;
        }
    }

    // ------------------------------------------------------------------
    // Auction (D29, D32, D33, D37, D61)
    // ------------------------------------------------------------------

    /// @notice Bids for a job by locking bond. The amount is at least x (D32)
    /// and at least 0.1% above the best bid (D76). The operator that was
    /// outbid gets its bond back at once.
    function bid(uint256 jobId, uint256 amount) external {
        Job storage job = _jobs[jobId];
        if (job.openedAt == 0) revert UnknownJob();
        if (block.timestamp >= _auctionEnd(job)) revert AuctionClosed();
        if (amount < job.escrow || amount < minimumBidOf(jobId)) revert BidTooLow();

        if (job.operator != address(0)) {
            _operators[job.operator].locked -= job.bid;
        }

        Operator storage op = _operators[msg.sender];
        // D19: an operator needs that much free bond.
        if (amount > op.bond - op.locked) revert BondNotFree();
        op.locked += amount;

        job.operator = msg.sender;
        job.bid = amount;
        job.lastBidAt = uint40(block.timestamp);
        emit BidPlaced(jobId, msg.sender, amount);
    }

    /// @notice The lowest amount the next bid must lock: x for the first bid
    /// (D32), and 0.1% above the best bid after that (D76).
    function minimumBidOf(uint256 jobId) public view returns (uint256) {
        Job storage job = _jobs[jobId];
        if (job.openedAt == 0) revert UnknownJob();
        if (job.operator == address(0)) return job.escrow;
        uint256 step = job.bid / BID_STEP_DIVISOR;
        // An amount too small to have a 0.1% still needs a better bid.
        if (step == 0) step = 1;
        return job.bid + step;
    }

    /// @notice D61: a job with no bid when the auction closes expires and its
    /// fees return. Anyone may call.
    function expire(uint256 jobId) external {
        Job storage job = _jobs[jobId];
        if (job.openedAt == 0) revert UnknownJob();
        if (block.timestamp < _auctionEnd(job)) revert AuctionOpen();
        if (job.operator != address(0)) revert JobHasBid();
        if (job.feesReturned) revert FeesAlreadyReturned();

        job.feesReturned = true;
        emit JobExpired(jobId);
        _credit(job.payer, job.commitmentFee + job.escrowFee);
    }

    // ------------------------------------------------------------------
    // Chain head (D30, D34)
    // ------------------------------------------------------------------

    /// @notice What the first transaction of an operator's chain must carry
    /// in its `OP_RETURN`, so that nobody can register a coin of someone else.
    /// @dev It can never equal what a tagged transaction carries, whatever
    /// tag an application chooses: the two start from different words.
    function chainHeadCommitment(address operator) public view returns (bytes32) {
        return sha256(abi.encodePacked("iPoW chain head", block.chainid, address(this), operator));
    }

    /// @notice What the tagged transaction of a job carries in its
    /// `OP_RETURN`: the application's tag, marked as the tag of a job. It is
    /// the same on every network (D80).
    function tagPayload(bytes32 tag) public pure returns (bytes32) {
        return sha256(abi.encodePacked("iPoW job", tag));
    }

    function chainHeadOf(address operator) external view returns (ChainHead memory) {
        return _chainHeads[operator];
    }

    /// @notice Registers the caller's chain head for this network (D34): a
    /// coin made by a Bitcoin transaction that names the caller.
    /// @param blockRef A stored block that holds the transaction.
    /// @param coinIndex The output that is the chain head.
    /// @param tagIndex The output that carries `chainHeadCommitment(caller)`.
    function registerChainHead(
        BlockRef calldata blockRef,
        bytes calldata rawTx,
        bytes32[] calldata siblings,
        uint256 txIndex,
        uint32 coinIndex,
        uint32 tagIndex
    ) external {
        if (_chainHeads[msg.sender].set) revert ChainHeadExists();
        _requireInBlock(blockRef, rawTx, siblings, txIndex);

        BitcoinTxLib.View memory v = BitcoinTxLib.read(rawTx, 0, coinIndex, tagIndex);
        if (!v.hasCoin) revert NoCoin();
        if (!v.hasTag || v.tag != chainHeadCommitment(msg.sender)) revert WrongTag();

        bytes32 txid = BitcoinTxLib.txid(rawTx);
        if (usedTx[txid]) revert TransactionUsed();
        // It can never settle a job.
        usedTx[txid] = true;
        _setChainHead(msg.sender, txid, coinIndex);
    }

    /// @notice Moves the caller's chain head past a transaction that spent it
    /// and did not settle a job, for example one that came after its deadline.
    /// Only the operator itself may do this: anyone else could move the chain
    /// head past a transaction the operator is about to prove.
    function advanceChainHead(
        BlockRef calldata blockRef,
        bytes calldata rawTx,
        bytes32[] calldata siblings,
        uint256 txIndex,
        uint32 headIndex
    ) external {
        ChainHead storage head = _chainHeads[msg.sender];
        if (!head.set) revert NoChainHead();
        _requireInBlock(blockRef, rawTx, siblings, txIndex);

        BitcoinTxLib.View memory v = BitcoinTxLib.read(rawTx, headIndex, headIndex, type(uint256).max);
        if (v.spentTxid != head.txid || v.spentVout != head.vout) revert WrongChainHead();
        if (!v.hasCoin) revert NoCoin();

        bytes32 txid = BitcoinTxLib.txid(rawTx);
        if (usedTx[txid]) revert TransactionUsed();
        // It can no longer settle a job.
        usedTx[txid] = true;
        _setChainHead(msg.sender, txid, headIndex);
    }

    // ------------------------------------------------------------------
    // Duty (D6, D9, D15, D27, D39, D54, D80)
    // ------------------------------------------------------------------

    /// @notice The operator of a job names its anchor. The window of the job
    /// is the blocks that follow it.
    function anchorJob(uint256 jobId, BlockRef calldata anchor) external {
        Job storage job = _jobs[jobId];
        if (statusOf(jobId) != JobStatus.Assigned) revert NotAssigned();
        if (msg.sender != job.operator) revert NotOperator();

        Duty storage duty = _duties[jobId];
        if (duty.anchoredAt != 0) revert AlreadyAnchored();
        if (block.timestamp > _deadline(job)) revert DeadlinePassed();

        iPoWLightClient.Node memory node = lightClient.getNode(
            lightClient.nodeId(anchor.hash, anchor.height, anchor.epochTime)
        );
        if (node.storedAt == 0) revert UnknownBlock();
        // D39, applied to this job at this moment.
        if (block.timestamp > uint256(node.time) + MAX_ANCHOR_AGE) revert AnchorTooOld();

        duty.anchor = anchor;
        duty.anchoredAt = uint40(block.timestamp);
        emit JobAnchored(jobId, anchor.hash, anchor.height, anchor.epochTime);
    }

    /// @notice Proves the operator's tagged transaction. Only the operator of
    /// the job submits it: the proof names the branch that is judged when a
    /// guardian challenges (D81), so nobody else may choose it.
    function proveJob(uint256 jobId, Proof calldata proof) external {
        Job storage job = _jobs[jobId];
        if (statusOf(jobId) != JobStatus.Assigned) revert NotAssigned();
        if (msg.sender != job.operator) revert NotOperator();
        Duty storage duty = _duties[jobId];
        if (duty.anchoredAt == 0) revert NotAnchored();
        // D27.
        if (block.timestamp > _deadline(job)) revert DeadlinePassed();

        _requireWindow(duty.anchor, proof, job.confirmations);
        _requireInBlock(proof.proofBlock, proof.rawTx, proof.siblings, proof.txIndex);

        bytes32 txid = BitcoinTxLib.txid(proof.rawTx);
        if (usedTx[txid]) revert TransactionUsed();

        BitcoinTxLib.View memory v = BitcoinTxLib.read(
            proof.rawTx,
            proof.headIndex,
            proof.headIndex,
            proof.tagIndex
        );
        // D30: it comes from the operator.
        ChainHead storage head = _chainHeads[job.operator];
        if (!head.set) revert NoChainHead();
        if (v.spentTxid != head.txid || v.spentVout != head.vout) revert WrongChainHead();
        if (!v.hasCoin) revert NoCoin();
        // D6.
        if (!v.hasTag || v.tag != tagPayload(job.tag)) revert WrongTag();

        usedTx[txid] = true;
        _setChainHead(job.operator, txid, proof.headIndex);

        // D54: the duty ends here.
        duty.proofBlock = proof.proofBlock;
        duty.tip = proof.tip;
        duty.deepest = duty.anchor;
        duty.txid = txid;
        duty.provenAt = uint40(block.timestamp);
        duty.lockEnd = uint40(block.timestamp + lockTimeOf(jobId));
        emit JobProven(jobId, txid, duty.lockEnd);
    }

    /// @notice Pays the operator and frees its bond when the lock has ended
    /// (D23, D50, D67). Anyone may call.
    function settle(uint256 jobId) external {
        Job storage job = _jobs[jobId];
        if (statusOf(jobId) != JobStatus.Proven) revert NotProven();
        Duty storage duty = _duties[jobId];
        if (block.timestamp < duty.lockEnd) revert LockNotEnded();
        if (openChallengesOf[jobId] != 0) revert ChallengeOpen();

        duty.settled = true;
        job.feesReturned = true;

        uint256 paid = job.commitmentFee + job.escrowFee;
        address attester = duty.attester;
        if (attester == address(0)) {
            _operators[job.operator].locked -= job.bid;
        } else {
            // D42, D96: the attester's lock ends. It earns 40% of the escrow
            // fee for the whole lock, and less the later it stepped in. The
            // operator's bond was freed when the attester stepped in.
            _operators[attester].locked -= job.escrow;
            uint256 attesterShare = (job.escrowFee * ATTESTER_SHARE_BPS * (duty.lockEnd - duty.attestedAt))
                / (BPS * (duty.lockEnd - duty.provenAt));
            paid -= attesterShare;
            _credit(attester, attesterShare);
        }
        emit JobSettled(jobId, job.operator, paid);
        _credit(job.operator, paid);
    }

    // ------------------------------------------------------------------
    // Punishment (D7, D26, D35, D36, D47, D53, D62)
    // ------------------------------------------------------------------

    /// @notice The first of a guardian's two steps (D47): it registers a
    /// sealed note. Nobody can read from it what the guardian will show.
    /// @param note `noteFor(guardian, jobId, evidence, salt)`.
    function sealNote(bytes32 note) external {
        if (_noteBlock[note] != 0) revert NoteExists();
        _noteBlock[note] = block.number;
        emit NoteSealed(note);
    }

    function noteFor(
        address guardian,
        uint256 jobId,
        bytes32 evidence,
        bytes32 salt
    ) public pure returns (bytes32) {
        return keccak256(abi.encode(guardian, jobId, evidence, salt));
    }

    /// @notice The second step, for a missed duty: the deadline has passed
    /// and no proof was accepted. The full escrow of the job is slashed
    /// (D53), and both fees return to the user (D62).
    /// @param salt The salt of the caller's sealed note for this job. The
    /// evidence of a missed duty is empty.
    function reportMissedDuty(uint256 jobId, bytes32 salt) external {
        Job storage job = _jobs[jobId];
        if (statusOf(jobId) != JobStatus.Assigned) revert NotAssigned();
        if (block.timestamp <= _deadline(job)) revert DeadlineNotPassed();
        _requireNote(jobId, bytes32(0), salt);
        _slash(jobId, job, msg.sender);
    }

    // ------------------------------------------------------------------
    // Challenge of a proof (D49, D81)
    // ------------------------------------------------------------------

    /// @notice What a guardian's note holds as evidence when it asks for the
    /// parent of a block.
    function parentEvidence(bytes32 childHash) public pure returns (bytes32) {
        return keccak256(abi.encode("parent", childHash));
    }

    /// @notice What a guardian's note holds as evidence when it shows a
    /// competing branch that starts with `guardianBlock`.
    function forkEvidence(bytes32 guardianBlock) public pure returns (bytes32) {
        return keccak256(abi.encode("fork", guardianBlock));
    }

    /// @notice A guardian asks for the parent of the oldest block of the
    /// operator's branch. Anyone may show it within 12 hours. If nobody does,
    /// the proof is false (D81).
    /// @dev Sent with the deposit: the commitment fee times the number of the
    /// question (D91). At most 2,016 questions per job (D92).
    function askParent(uint256 jobId, bytes32 salt) external payable returns (uint256 challengeId) {
        uint256 deposit = parentDepositOf(jobId);
        Duty storage duty;
        (challengeId, duty) = _openChallenge(jobId, ChallengeKind.Parent, deposit);
        if (duty.parentsShown >= _maxParentQuestions()) revert TooManyQuestions();
        // A block stated at height 0 may be asked about too. Nobody can show a
        // parent below height 0, so the proof is false: a real anchor is never
        // that low, only a forger's that tries to escape the questions.
        _requireNote(jobId, parentEvidence(duty.deepest.hash), salt);
        _challenges[challengeId].asked = duty.deepest;
        emit ParentAsked(challengeId, jobId, msg.sender, duty.deepest.hash, uint256(duty.parentsShown) + 1);
    }

    /// @notice Answers a guardian: the parent is in the light client. Anyone
    /// may call. The guardian's deposit goes to the operator (D90).
    /// @param prevEpochTime Only used when the block asked about is the first
    /// of its epoch: the epoch time of the epoch before. Zero otherwise.
    function showParent(uint256 challengeId, uint32 prevEpochTime) external {
        Challenge storage challenge = _challenges[challengeId];
        if (challenge.kind != ChallengeKind.Parent) revert NoChallenge();
        if (statusOf(challenge.jobId) != JobStatus.Proven) revert NotProven();
        if (block.timestamp >= uint256(challenge.openedAt) + RESPONSE_TIME) revert ResponseTimeOver();

        Duty storage duty = _duties[challenge.jobId];
        BlockRef memory child = challenge.asked;
        if (child.height == 0) revert NoParent();
        iPoWLightClient.Node memory node = lightClient.getNode(
            lightClient.nodeId(child.hash, child.height, child.epochTime)
        );
        BlockRef memory parent = BlockRef({
            hash: node.prevHash,
            height: child.height - 1,
            epochTime: child.height % 2016 == 0 ? prevEpochTime : child.epochTime
        });
        // The parent is stored, and the child has the difficulty that
        // follows from it.
        if (!_linked(parent, child, prevEpochTime)) revert NoParent();

        // The next question is about the parent. Several guardians may have
        // asked the same question; the first answer moves on.
        if (_same(duty.deepest, child)) {
            duty.deepest = parent;
            duty.parentsShown += 1;
        }
        emit ParentShown(challengeId, parent.hash);
        _failChallenge(challengeId, challenge);
    }

    /// @notice A guardian shows a competing branch: a block that names the
    /// same parent as a block of the operator's branch, with more mining work
    /// on top of it than the operator's branch has from there (D81).
    /// @param operatorBlock A block of the operator's branch, at or below the
    /// block of the proof.
    /// @param guardianBlock Another block with the same parent.
    /// @param guardianTip The best block on top of `guardianBlock`.
    /// @dev Sent with the deposit, which equals the commitment fee.
    function challengeFork(
        uint256 jobId,
        BlockRef calldata operatorBlock,
        BlockRef calldata guardianBlock,
        BlockRef calldata guardianTip,
        uint32 prevEpochTime,
        bytes32 salt
    ) external payable returns (uint256 challengeId) {
        Duty storage duty;
        (challengeId, duty) = _openChallenge(jobId, ChallengeKind.Fork, _jobs[jobId].commitmentFee);
        _requireNote(jobId, forkEvidence(guardianBlock.hash), salt);

        // D81: the branches part at the anchor or at a block after it.
        if (!_linked(duty.anchor, operatorBlock, prevEpochTime)) revert NotOnOperatorBranch();
        _requireSameParent(operatorBlock, guardianBlock);

        Challenge storage challenge = _challenges[challengeId];
        uint256 toProof = _branchWork(operatorBlock, duty.proofBlock, prevEpochTime, true);
        challenge.operatorWork = toProof + _branchWork(duty.proofBlock, duty.tip, prevEpochTime, false);
        _setCheckpoint(challengeId, false, duty.proofBlock, toProof);
        _setCheckpoint(challengeId, false, duty.tip, challenge.operatorWork);

        uint256 first = _branchWork(guardianBlock, guardianBlock, prevEpochTime, true);
        challenge.guardianWork = first + _branchWork(guardianBlock, guardianTip, prevEpochTime, false);
        _setCheckpoint(challengeId, true, guardianBlock, first);
        _setCheckpoint(challengeId, true, guardianTip, challenge.guardianWork);

        if (challenge.guardianWork <= challenge.operatorWork) revert NotHeavier();
        emit ForkChallenged(challengeId, jobId, msg.sender, operatorBlock.hash, guardianBlock.hash);
    }

    /// @notice Adds blocks to one of the two branches of an open challenge.
    /// Anyone may call, for either side, until the lock ends (D81, D99). The
    /// new blocks go on top of any block shown before on that side.
    /// @param from A block shown before on that side.
    function extendBranch(
        uint256 challengeId,
        bool guardianSide,
        BlockRef calldata from,
        BlockRef calldata newTip,
        uint32 prevEpochTime
    ) external {
        Challenge storage challenge = _challenges[challengeId];
        if (challenge.kind != ChallengeKind.Fork) revert NoChallenge();
        if (statusOf(challenge.jobId) != JobStatus.Proven) revert NotProven();
        Duty storage duty = _duties[challenge.jobId];
        if (block.timestamp >= duty.lockEnd) revert LockEnded();

        uint256 base = _checkpoints[_checkpointKey(challengeId, guardianSide, from)];
        if (base == 0) revert NotShown();
        uint256 work = base + _branchWork(from, newTip, prevEpochTime, false);
        _setCheckpoint(challengeId, guardianSide, newTip, work);

        if (guardianSide) {
            if (work > challenge.guardianWork) challenge.guardianWork = work;
        } else {
            if (work > challenge.operatorWork) challenge.operatorWork = work;
        }
        emit BranchExtended(challengeId, guardianSide, newTip.hash, work);
    }

    /// @notice Ends an open challenge. Anyone may call.
    /// A parent that was asked for and not shown in 12 hours: the proof is
    /// false. A competing branch: the branch with more work when the lock has
    /// ended wins, and the operator's branch wins when they are equal.
    /// When another challenge has already slashed the job, the guardian gets
    /// its deposit back (D88).
    function resolveChallenge(uint256 challengeId) external {
        Challenge storage challenge = _challenges[challengeId];
        if (challenge.kind == ChallengeKind.None) revert NoChallenge();
        uint256 jobId = challenge.jobId;
        Duty storage duty = _duties[jobId];
        address guardian = challenge.guardian;
        uint256 deposit = challenge.deposit;

        if (duty.slashed) {
            _close(challengeId);
            emit ChallengeRefunded(challengeId, guardian);
            _credit(guardian, deposit);
            return;
        }

        bool proofIsFalse;
        if (challenge.kind == ChallengeKind.Parent) {
            if (block.timestamp < uint256(challenge.openedAt) + RESPONSE_TIME) revert ResponseTimeNotOver();
            proofIsFalse = true;
        } else {
            if (block.timestamp < duty.lockEnd) revert LockNotEnded();
            proofIsFalse = challenge.guardianWork > challenge.operatorWork;
        }

        if (!proofIsFalse) {
            _failChallenge(challengeId, challenge);
            return;
        }

        _close(challengeId);
        emit ChallengeWon(challengeId, guardian);
        _slash(jobId, _jobs[jobId], guardian);
        _credit(guardian, deposit);
    }

    // ------------------------------------------------------------------
    // Claims and attest (D11, D42, D66, D73, D74, D94)
    // ------------------------------------------------------------------

    /// @notice An attester takes the operator's place: it locks x from its
    /// own bond, and the operator's bond for the job becomes free (D42, D73).
    /// When the lock ends it earns 40% of the escrow fee, less the later it
    /// stepped in (D96). It is slashed as the operator would have been if the
    /// proof is proven fake. The first attester is the only one. Anyone with
    /// a bond may attest, the operator too (D95); an application sees who did
    /// in `getDuty`.
    function attest(uint256 jobId) external {
        Job storage job = _jobs[jobId];
        if (statusOf(jobId) != JobStatus.Proven) revert NotProven();
        Duty storage duty = _duties[jobId];
        if (block.timestamp >= duty.lockEnd) revert LockEnded();
        if (duty.attester != address(0)) revert AlreadyAttested();

        Operator storage att = _operators[msg.sender];
        if (job.escrow > att.bond - att.locked) revert BondNotFree();
        att.locked += job.escrow;
        _operators[job.operator].locked -= job.bid;

        duty.attester = msg.sender;
        duty.attestedAt = uint40(block.timestamp);
        emit Attested(jobId, msg.sender, job.escrow);
    }

    /// @notice The challenge period of a job, from the moment its proof was
    /// accepted (D74). Zero for a settlement. For a claim, the period the
    /// application registered, and at least 1.5 times the deadline (D71).
    function challengePeriodOf(uint256 jobId) public view returns (uint256) {
        Job storage job = _jobs[jobId];
        if (job.openedAt == 0) revert UnknownJob();
        if (job.claimKind == 0) return 0;
        uint256 period = _applications[job.application].challengePeriods[job.claimKind - 1];
        uint256 floor = (dutyTimeFor(job.confirmations) * LOCK_NUMERATOR) / LOCK_DENOMINATOR;
        return period > floor ? period : floor;
    }

    /// @notice Whether the application's message counts as official (D11).
    /// A settlement is official once its proof is accepted. A claim is
    /// official once it is attested, or once its lock has ended with no
    /// challenge open (D97). The lock covers the whole challenge period
    /// (D66), and no new challenge can open after it, so such a claim never
    /// stops being official. Never after a slash. What the application does
    /// with it is the application's matter.
    function isOfficial(uint256 jobId) external view returns (bool) {
        JobStatus status = statusOf(jobId);
        if (status != JobStatus.Proven && status != JobStatus.Settled) return false;
        if (_jobs[jobId].claimKind == 0) return true;

        Duty storage duty = _duties[jobId];
        if (duty.attester != address(0)) return true;
        if (block.timestamp < duty.lockEnd) return false;
        return openChallengesOf[jobId] == 0;
    }

    // ------------------------------------------------------------------
    // Withdrawing
    // ------------------------------------------------------------------

    function withdrawCredit() external nonReentrant {
        uint256 amount = credit[msg.sender];
        if (amount == 0) revert ZeroAmount();
        credit[msg.sender] = 0;
        emit CreditWithdrawn(msg.sender, amount);
        _send(msg.sender, amount);
    }

    // ------------------------------------------------------------------
    // Internal
    // ------------------------------------------------------------------

    /// @dev D37: bidding ends 15 minutes after the job was opened, or 1
    /// minute after the last bid when that is earlier.
    function _auctionEnd(Job storage job) private view returns (uint256) {
        uint256 end = uint256(job.openedAt) + AUCTION_DURATION;
        if (job.operator != address(0)) {
            uint256 quiet = uint256(job.lastBidAt) + AUCTION_QUIET;
            if (quiet < end) end = quiet;
        }
        return end;
    }

    function _deadline(Job storage job) private view returns (uint256) {
        return _auctionEnd(job) + dutyTimeFor(job.confirmations);
    }

    function _setChainHead(address operator, bytes32 txid, uint32 vout) private {
        _chainHeads[operator] = ChainHead({txid: txid, vout: vout, set: true});
        emit ChainHeadSet(operator, txid, vout);
    }

    function _nodeId(BlockRef calldata ref) private view returns (bytes32) {
        return lightClient.nodeId(ref.hash, ref.height, ref.epochTime);
    }

    function _requireInBlock(
        BlockRef calldata blockRef,
        bytes calldata rawTx,
        bytes32[] calldata siblings,
        uint256 txIndex
    ) private view {
        if (!lightClient.txInBlock(_nodeId(blockRef), rawTx, siblings, txIndex)) revert NotInBlock();
    }

    /// @dev D15, D41, D68: the proof is in blocks 1 to 25 after the anchor,
    /// and enough blocks are on top of it.
    function _requireWindow(
        BlockRef storage anchor,
        Proof calldata proof,
        uint16 confirmations
    ) private view {
        BlockRef calldata inBlock = proof.proofBlock;
        if (inBlock.height <= anchor.height || inBlock.height - anchor.height > PROOF_RANGE) {
            revert OutsideProofRange();
        }
        if (
            !lightClient.isAncestor(
                anchor.hash,
                anchor.height,
                anchor.epochTime,
                inBlock.hash,
                inBlock.height,
                inBlock.epochTime,
                proof.prevEpochTime
            )
        ) revert NotLinked();

        BlockRef calldata tip = proof.tip;
        // The proof's own block is the first confirmation.
        if (tip.height < inBlock.height || uint256(tip.height - inBlock.height) + 1 < confirmations) {
            revert NotEnoughConfirmations();
        }
        if (
            !lightClient.isAncestor(
                inBlock.hash,
                inBlock.height,
                inBlock.epochTime,
                tip.hash,
                tip.height,
                tip.epochTime,
                proof.prevEpochTime
            )
        ) revert NotLinked();
    }

    /// @dev D47: the note was sealed by the caller, in an earlier block.
    function _requireNote(uint256 jobId, bytes32 evidence, bytes32 salt) private {
        bytes32 note = noteFor(msg.sender, jobId, evidence, salt);
        uint256 sealedIn = _noteBlock[note];
        if (sealedIn == 0 || sealedIn >= block.number) revert NoNote();
        delete _noteBlock[note];
    }

    /// @dev D26: the escrow x of the job, not the whole bond, and not what
    /// the operator locked above x. D36: 80% to the application, 20% to the
    /// guardian. D62: both fees back to the user.
    function _slash(uint256 jobId, Job storage job, address guardian) private {
        _duties[jobId].slashed = true;
        job.feesReturned = true;

        address attester = _duties[jobId].attester;
        if (attester == address(0)) {
            Operator storage op = _operators[job.operator];
            op.locked -= job.bid;
            op.bond -= job.escrow;
        } else {
            // D42: the attester is slashed as the operator would have been.
            Operator storage att = _operators[attester];
            att.locked -= job.escrow;
            att.bond -= job.escrow;
        }

        uint256 toApplication = (job.escrow * APPLICATION_SHARE_BPS) / BPS;
        uint256 toGuardian = job.escrow - toApplication;
        emit JobSlashed(jobId, job.operator, guardian, toApplication, toGuardian);

        _credit(job.application, toApplication);
        _credit(guardian, toGuardian);
        _credit(job.payer, job.commitmentFee + job.escrowFee);
    }

    /// @dev The checks every challenge starts with, and its record. Several
    /// challenges of one job may be open at the same time (D88).
    function _openChallenge(uint256 jobId, ChallengeKind kind, uint256 deposit)
        private
        returns (uint256 challengeId, Duty storage duty)
    {
        if (statusOf(jobId) != JobStatus.Proven) revert NotProven();
        duty = _duties[jobId];
        // D99: the lock never moves, and the last challenge opens 12 hours
        // before it ends, so every challenge is decided within the lock.
        if (block.timestamp + RESPONSE_TIME > duty.lockEnd) revert ChallengeWindowClosed();
        challengeId = ++challengeCount;
        Challenge storage challenge = _challenges[challengeId];
        challenge.kind = kind;
        challenge.jobId = jobId;
        challenge.guardian = msg.sender;
        challenge.deposit = deposit;
        challenge.openedAt = uint40(block.timestamp);
        openChallengesOf[jobId] += 1;
        // D81, D91: taken after the record; a token build calls the coin.
        _take(deposit);
    }

    function _close(uint256 challengeId) private {
        openChallengesOf[_challenges[challengeId].jobId] -= 1;
        delete _challenges[challengeId];
    }

    /// @dev D81, D90: the deposit of a challenge that failed goes to the
    /// operator.
    function _failChallenge(uint256 challengeId, Challenge storage challenge) private {
        uint256 jobId = challenge.jobId;
        address guardian = challenge.guardian;
        uint256 deposit = challenge.deposit;
        _close(challengeId);
        emit ChallengeFailed(challengeId, guardian);
        _credit(_jobs[jobId].operator, deposit);
    }

    function _checkpointKey(
        uint256 challengeId,
        bool guardianSide,
        BlockRef memory ref
    ) private pure returns (bytes32) {
        return keccak256(abi.encode(challengeId, guardianSide, ref.hash, ref.height, ref.epochTime));
    }

    function _setCheckpoint(
        uint256 challengeId,
        bool guardianSide,
        BlockRef memory ref,
        uint256 work
    ) private {
        bytes32 key = _checkpointKey(challengeId, guardianSide, ref);
        if (work > _checkpoints[key]) _checkpoints[key] = work;
    }

    function _same(BlockRef storage a, BlockRef memory b) private view returns (bool) {
        return a.hash == b.hash && a.height == b.height && a.epochTime == b.epochTime;
    }

    function _linked(
        BlockRef memory from,
        BlockRef memory to,
        uint32 prevEpochTime
    ) private view returns (bool linked) {
        (linked, ) = lightClient.walk(
            from.hash,
            from.height,
            from.epochTime,
            to.hash,
            to.height,
            to.epochTime,
            prevEpochTime
        );
    }

    /// @dev The mining work of the blocks after `from`, up to and with `to`,
    /// and of `from` itself when `withFirst`. The two must be linked.
    function _branchWork(
        BlockRef memory from,
        BlockRef memory to,
        uint32 prevEpochTime,
        bool withFirst
    ) private view returns (uint256 work) {
        bool linked;
        (linked, work) = lightClient.walk(
            from.hash,
            from.height,
            from.epochTime,
            to.hash,
            to.height,
            to.epochTime,
            prevEpochTime
        );
        if (!linked) revert NotOnOperatorBranch();
        if (withFirst) {
            iPoWLightClient.Node memory node = lightClient.getNode(
                lightClient.nodeId(from.hash, from.height, from.epochTime)
            );
            work += lightClient.workOf(node.bits);
        }
    }

    /// @dev Two different blocks that name the same parent. The height and
    /// epoch time each side stated do not matter: the parent's hash fixes the
    /// place, and each side's work is its own mining.
    function _requireSameParent(BlockRef calldata a, BlockRef calldata b) private view {
        if (a.hash == b.hash) revert NotCompeting();
        iPoWLightClient.Node memory nodeA = lightClient.getNode(_nodeId(a));
        iPoWLightClient.Node memory nodeB = lightClient.getNode(_nodeId(b));
        if (nodeA.storedAt == 0 || nodeB.storedAt == 0) revert UnknownBlock();
        if (nodeA.prevHash != nodeB.prevHash) revert NotCompeting();
    }

    function _credit(address to, uint256 amount) private {
        credit[to] += amount;
        emit Credited(to, amount);
    }

    /// @dev D137: takes exactly `amount` of the network's coin from the
    /// caller.
    function _take(uint256 amount) internal virtual;

    /// @dev D137: pays `amount` of the network's coin.
    function _send(address to, uint256 amount) internal virtual;

    /// @dev D135: the cost of posting `size` bytes to a rollup's parent
    /// network, in the units of the price. None by default.
    function _dataFee(uint256) internal view virtual returns (uint256) {
        return 0;
    }

    /// @dev D136: units of the price of work per unit of the coin.
    function _priceScale() internal view virtual returns (uint256) {
        return 1;
    }

    /// @dev D92. Virtual only so that a test can lower it.
    function _maxParentQuestions() internal view virtual returns (uint256) {
        return MAX_PARENT_QUESTIONS;
    }

    /// @dev D58: the price of work on this network, read by the contract.
    function _price() internal view virtual returns (uint256) {
        return block.basefee;
    }
}

/// @title iPoWProtocolNative
/// @notice The protocol on a network with a native coin (D137): bonds, fees
/// and deposits are sent with the call, exactly the amount named. On a
/// rollup, `dataFee` reads the price of posting its data (D135); zero
/// elsewhere.
contract iPoWProtocolNative is iPoWProtocol {
    IDataFee public immutable dataFee;
    /// @dev The gas the reader of the data fee is given, and the gas a call
    /// must have left to give it all of it (64/63 of it and the call's own
    /// cost), so that only the reader can make it fail, not a caller's low
    /// gas limit.
    uint256 internal constant DATA_FEE_GAS = 100_000;
    uint256 internal constant DATA_FEE_GAS_NEEDED = 107_000;

    error DataFeeGas();

    constructor(iPoWLightClient lightClient_, IDataFee dataFee_) iPoWProtocol(lightClient_) {
        dataFee = dataFee_;
        // A reader that cannot read, within the gas it will be given, is
        // found here, not at the first job.
        if (address(dataFee_) != address(0)) dataFee_.dataFee{gas: DATA_FEE_GAS}(1);
    }

    /// @dev A reader that stops reading counts as no data fee, so that the
    /// network's jobs can still open: priced too low, a job may draw no bid
    /// and expire with its fees returned (D61). Failing would end them for
    /// good, as nobody can change the reader (D59).
    function _dataFee(uint256 size) internal view override returns (uint256 fee) {
        address reader = address(dataFee);
        if (reader == address(0)) return 0;
        if (gasleft() < DATA_FEE_GAS_NEEDED) revert DataFeeGas();
        bytes4 selector = IDataFee.dataFee.selector;
        // In scratch memory, and only the first word of the answer copied.
        assembly ("memory-safe") {
            mstore(0, selector)
            mstore(4, size)
            let ok := staticcall(DATA_FEE_GAS, reader, 0, 0x24, 0, 0x20)
            fee := mload(0)
            if or(iszero(ok), or(xor(returndatasize(), 0x20), gt(fee, 0xffffffffffffffffffffffffffffffff))) {
                fee := 0
            }
        }
    }

    function _take(uint256 amount) internal override {
        if (msg.value != amount) revert WrongValue();
    }

    function _send(address to, uint256 amount) internal override {
        (bool ok, ) = to.call{value: amount}("");
        if (!ok) revert TransferFailed();
    }
}

/// @title iPoWProtocolGasPrice
/// @notice The native build on Hedera, whose base fee reads as zero to a
/// contract (D139): the price of work is the gas price a contract sees,
/// which is the network's own, in tinybars, whatever price the sender
/// offers (measured on its testnet on 2026-10-02).
contract iPoWProtocolGasPrice is iPoWProtocolNative {
    constructor(iPoWLightClient lightClient_) iPoWProtocolNative(lightClient_, IDataFee(address(0))) {}

    function _price() internal view override returns (uint256) {
        return tx.gasprice;
    }
}

/// @title iPoWProtocolPolkadot
/// @notice The native build on Polkadot, whose gas is not Ethereum's (D140):
/// the work's gas constants are Ethereum's, so the cost of the work at the
/// base fee is divided by Ethereum's gas per unit of Polkadot's, 8, measured
/// on its testnet on 2026-10-02 (the light client's work took 0.08 to 0.124
/// of Ethereum's gas). Dividing the cost, not the base fee, loses nothing to
/// rounding, and `commitmentFeeAt` then takes the base fee as it is.
contract iPoWProtocolPolkadot is iPoWProtocolNative {
    /// @notice Units of Ethereum's gas per unit of Polkadot's.
    uint256 public constant WORK_SCALE = 8;

    constructor(iPoWLightClient lightClient_) iPoWProtocolNative(lightClient_, IDataFee(address(0))) {}

    function _priceScale() internal pure override returns (uint256) {
        return WORK_SCALE;
    }
}
