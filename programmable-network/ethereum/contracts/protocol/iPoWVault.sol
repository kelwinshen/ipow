// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

import {ReentrancyGuard} from "@openzeppelin/contracts/utils/ReentrancyGuard.sol";

import {iPoWLightClient} from "./iPoWLightClient.sol";
import {iPoWProtocol} from "./iPoWProtocol.sol";
import {BitcoinTxLib} from "./BitcoinTxLib.sol";

/// @title iPoWVault
/// @notice The protocol's vault on Ethereum, for the pair Ethereum and
/// Solana. Spec: docs/design/ipow-protocol.md, section 11, decisions D104 to
/// D126. It is part of the protocol, beside the protocol contract (D104).
///
/// On Ethereum it holds ETH locked for vETH on Solana. It judges LOCK and
/// BOND records about itself (D109), and acts on REQUEST, CANCEL and BOND
/// records from Solana after 7 days of objections (D111). An attester may pay
/// a burn's ETH at once and be repaid when its claim is accepted (D122).
///
/// There is no owner and no key, and nothing here can be changed after
/// deployment (D59). The first version holds ETH only (D117).
///
/// Money is never pushed to an address. It is credited and the address
/// withdraws it.
contract iPoWVault is ReentrancyGuard {
    // ------------------------------------------------------------------
    // Rules
    // ------------------------------------------------------------------

    /// @notice D111: a claim is decided when the last objection or answer has
    /// stood for 7 days.
    uint256 public constant OBJECTION_WINDOW = 7 days;
    /// @notice D110: a chain's open claims are at most 80% of its bond.
    uint256 public constant COVER_BPS = 8000;
    /// @notice D109: 80% of a slash backs the receipt, 20% goes to whoever
    /// submitted the message.
    uint256 public constant BACKING_SHARE_BPS = 8000;
    uint256 public constant BPS = 10_000;
    /// @notice Amounts in records are in gwei: vETH has 9 decimals.
    uint256 public constant GWEI = 1e9;

    /// @notice Section 11.3: a message is false when its Bitcoin transaction
    /// or its batch is longer than these, or it carries more REQUEST and
    /// CANCEL records than this, so that every message that counts can be
    /// judged on every network.
    uint256 public constant MAX_RAW_TX = 1024;
    uint256 public constant MAX_BATCH = 2048;
    uint256 public constant MAX_HOME_RECORDS = 32;

    /// @notice The networks of the pair, as records name them.
    uint8 public constant ETHEREUM = 1;
    uint8 public constant SOLANA = 2;

    uint8 internal constant LOCK = 1;
    uint8 internal constant REQUEST = 2;
    uint8 internal constant CANCEL = 3;
    uint8 internal constant BOND = 4;
    uint8 internal constant EXIT = 5;
    uint256 internal constant LOCK_LEN = 73;
    uint256 internal constant REQUEST_LEN = 61;
    /// @notice D124: an attester's share of a fast fee falls to nothing 8 days
    /// after the lock or burn, about the slow path.
    uint256 public constant FAST_FEE_DEADLINE = 8 days;

    iPoWProtocol public immutable protocol;
    iPoWLightClient public immutable lightClient;
    /// @notice The vault program on Solana, as named in a registration.
    bytes32 public immutable peerVault;
    /// @notice V1: the flat deposit to object or answer, and the operator's
    /// deposit for a claim.
    uint256 public immutable deposit;
    /// @notice D118: the least escrow of a job whose proof block counts as
    /// real. A forger risks at least this much, and a guardian who catches it
    /// earns 20% of it.
    uint256 public immutable minCertifyingEscrow;

    // ------------------------------------------------------------------
    // Storage
    // ------------------------------------------------------------------

    struct Lock {
        address owner;
        uint64 amount; // gwei
        uint64 fee; // gwei
        /// For the attester of the fast path, or the recipient (D122).
        uint64 fastFee; // gwei
        /// Its block's time, in the LOCK record (D124).
        uint64 lockedAt;
        bytes32 recipient; // on Solana
        bool feePaid;
        bool returned;
    }

    struct Chain {
        bytes32 peerOperator; // the same operator on Solana
        bytes32 coinTxid;
        uint32 coinVout;
        bool registered;
        bool exited;
        /// A false record was proven here: the bond is gone (D109).
        bool slashed;
        /// A claim of the chain was refused here: no other claim acts (D111).
        bool refused;
        uint64 messages;
        /// What the operator holds in the vault, in wei.
        uint256 bond;
        /// The part of the bond its BOND message stated for Ethereum (D110).
        uint256 stated;
        /// Money for the flat deposits of its claims, apart from the bond, so
        /// that a deposit never changes what a BOND record can state.
        uint256 deposits;
        /// The vETH bond on Solana, counted once its BOND claim is official.
        uint256 peerBond; // wei
        /// A BOND record for Solana was carried: once per chain (D110).
        bool peerBondCarried;
        /// The value of the chain's open claims here, in wei.
        uint256 openValue;
        uint32 openClaims;
        /// The block of the chain's last message here, where its
        /// `MessageBatch` event is: each names the block of the one before.
        uint64 lastMessageBlock;
    }

    struct Claim {
        address operator;
        uint40 lastAt;
        /// An objection stands.
        bool held;
        bool decided;
        bool accepted;
        uint256 value; // wei
        /// A BOND for Solana carried in this claim, in wei, zero if none.
        uint256 peerBond;
        /// Deposits put down on each side, the operator's among the answers.
        uint32 answers;
        uint32 objections;
        /// Set when decided: what each deposit of the winning side collects.
        uint256 payout;
        /// The block it was opened in, where `ClaimBatch` names its records.
        uint64 openedBlock;
    }

    struct Request {
        address to;
        uint64 amount; // gwei
        uint64 fastFee; // gwei
        /// The burn's time on Solana (D124).
        uint64 requestedAt;
    }

    /// A burn on Solana paid at once by an attester (section 11.7), under
    /// the record it stated.
    struct FastPay {
        address attester;
        uint64 paidAt;
    }

    uint256 public lockCount;
    mapping(uint256 => Lock) private _locks;
    mapping(address => Chain) private _chains;
    uint256 public claimCount;
    mapping(uint256 => Claim) private _claims;
    /// @dev Claim => request number on Solana => the request it carries.
    /// Several claims may carry the same request; it is paid once.
    mapping(uint256 => mapping(uint64 => Request)) private _requests;
    mapping(uint64 => bool) public requestPaid;
    /// @dev Request => hash of the record stated => its payment. Several
    /// may be made; only the true record's is repaid (D126).
    mapping(uint64 => mapping(bytes32 => FastPay)) private _fastPays;
    /// @dev Claim => lock => whether the claim carries a CANCEL of it.
    mapping(uint256 => mapping(uint64 => bool)) public cancels;
    /// @dev Claim => address => deposits it put down on each side.
    mapping(uint256 => mapping(address => uint32)) public answersOf;
    mapping(uint256 => mapping(address => uint32)) public objectionsOf;

    /// @notice The ETH that backs vETH: the locks not returned, less what was
    /// paid out, plus the backing share of slashes.
    uint256 public reserve;
    /// @notice Blocks known to be on real Bitcoin (D108), by light client id.
    mapping(bytes32 => bool) public isReal;
    uint256 public checkpointCount;
    mapping(address => uint256) public credit;

    // ------------------------------------------------------------------
    // Events and errors
    // ------------------------------------------------------------------

    event Locked(uint256 indexed lockId, address indexed owner, uint64 amount, uint64 fee, uint64 fastFee, uint64 lockedAt, bytes32 recipient);
    event ChainRegistered(address indexed operator, bytes32 peerOperator, bytes32 coinTxid, uint32 coinVout);
    event BondChanged(address indexed operator, uint256 bond);
    event RealBlock(bytes32 indexed id);
    event MessageProcessed(address indexed operator, uint64 index, bytes32 txid, bool truthful, uint256 claimId);
    /// @notice Every message's batch, so that anyone can bring the message to
    /// the other vault (D107). `prevBlock` is the block of the chain's
    /// message before it, zero for the first.
    event MessageBatch(address indexed operator, uint64 indexed index, uint64 prevBlock, bytes batch);
    event Slashed(address indexed operator, uint256 amount, address submitter);
    event FeeEarned(uint256 indexed lockId, address indexed operator, uint256 amount);
    event ClaimOpened(uint256 indexed claimId, address indexed operator, uint256 value);
    /// @notice The batch whose acting records a claim carries, for guardians
    /// to check against the other network.
    event ClaimBatch(uint256 indexed claimId, bytes batch);
    event Objected(uint256 indexed claimId, address indexed by);
    event Answered(uint256 indexed claimId, address indexed by);
    event ClaimDecided(uint256 indexed claimId, bool accepted);
    event RequestPaid(uint64 indexed requestId, uint256 indexed claimId, address to, uint256 amount);
    event FastPaid(uint64 indexed requestId, address indexed attester, address to, uint256 amount);
    event DepositsChanged(address indexed operator, uint256 deposits);
    event LockReturned(uint256 indexed lockId, address owner, uint256 amount);
    event CreditWithdrawn(address indexed to, uint256 amount);

    error ZeroAmount();
    error NotGwei();
    error TooLarge();
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
    error WrongDeposit();
    error NotOpen();
    error AlreadyHeld();
    error NotHeld();
    error WindowOver();
    error WindowNotOver();
    error AlreadyDecided();
    error NotAccepted();
    error AlreadyDone();
    error Underfunded();
    error UnknownLock();
    error TransferFailed();
    error NotDecided();
    error NothingToCollect();
    error NoCancel();
    error EscrowTooLow();
    error AlreadyPaid();

    constructor(iPoWProtocol protocol_, bytes32 peerVault_, uint256 deposit_, uint256 minCertifyingEscrow_) {
        if (deposit_ == 0 || minCertifyingEscrow_ == 0) revert ZeroAmount();
        minCertifyingEscrow = minCertifyingEscrow_;
        protocol = protocol_;
        lightClient = protocol_.lightClient();
        peerVault = peerVault_;
        deposit = deposit_;
        // D116: the vault opens checkpoint jobs, as an application does.
        protocol_.registerApplication(new uint32[](0));
    }

    // ------------------------------------------------------------------
    // Reading
    // ------------------------------------------------------------------

    function getLock(uint256 lockId) external view returns (Lock memory) {
        return _locks[lockId];
    }

    function getChain(address operator) external view returns (Chain memory) {
        return _chains[operator];
    }

    function getClaim(uint256 claimId) external view returns (Claim memory) {
        return _claims[claimId];
    }

    function getRequest(uint256 claimId, uint64 requestId) external view returns (Request memory) {
        return _requests[claimId][requestId];
    }

    /// @notice The payment made at once of burn `requestId` under the record
    /// it stated, if any.
    function getFastPay(uint64 requestId, address to, uint64 amount, uint64 fastFee, uint64 requestedAt)
        external
        view
        returns (FastPay memory)
    {
        return _fastPays[requestId][_stated(to, amount, fastFee, requestedAt)];
    }

    /// @notice D124: an attester's share of `fastFee`, attesting at `at` a
    /// lock or burn made at `madeAt`: the fee times the time left to 8 days
    /// after it, over 8 days.
    function fastShare(uint64 fastFee, uint64 madeAt, uint64 at) public pure returns (uint64) {
        uint256 end = uint256(madeAt) + FAST_FEE_DEADLINE;
        if (at >= end) return 0;
        uint256 left = end - at;
        if (left > FAST_FEE_DEADLINE) left = FAST_FEE_DEADLINE;
        return uint64((uint256(fastFee) * left) / FAST_FEE_DEADLINE);
    }

    /// @notice What a registration carries in its `OP_RETURN` (D106).
    function pairCommitment(address operator, bytes32 peerOperator) public view returns (bytes32) {
        return sha256(abi.encodePacked("iPoW pair", address(this), operator, peerVault, peerOperator));
    }

    /// @notice What a message carries in its `OP_RETURN` (D107).
    function messagePayload(bytes calldata batch) public pure returns (bytes32) {
        return sha256(abi.encodePacked("iPoW vault", batch));
    }

    /// @notice The part of an operator's bond it may withdraw.
    function freeBond(address operator) public view returns (uint256) {
        Chain storage c = _chains[operator];
        if (c.exited && c.openClaims == 0) return c.bond;
        return c.bond - c.stated;
    }

    // ------------------------------------------------------------------
    // Locks (section 11.5)
    // ------------------------------------------------------------------

    /// @notice Locks ETH for vETH on Solana. `msg.value` is the amount, the
    /// fee and the fast fee; the fee goes to the first operator whose message
    /// carrying the lock is judged true (D113). The fast fee is minted in
    /// vETH on Solana, to the attester who issued the receipt at once, or to
    /// the recipient (D122); until then it backs vETH. All are whole gwei.
    function lock(bytes32 recipient, uint256 fee, uint256 fastFee) external payable returns (uint256 lockId) {
        if (msg.value <= fee + fastFee) revert ZeroAmount();
        uint256 amount = msg.value - fee - fastFee;
        if (amount % GWEI != 0 || fee % GWEI != 0 || fastFee % GWEI != 0) revert NotGwei();
        if (amount / GWEI > type(uint64).max || fee / GWEI > type(uint64).max || fastFee / GWEI > type(uint64).max) {
            revert TooLarge();
        }
        lockId = ++lockCount;
        _locks[lockId] = Lock({
            owner: msg.sender,
            amount: uint64(amount / GWEI),
            fee: uint64(fee / GWEI),
            fastFee: uint64(fastFee / GWEI),
            lockedAt: uint64(block.timestamp),
            recipient: recipient,
            feePaid: false,
            returned: false
        });
        reserve += amount + fastFee;
        emit Locked(
            lockId,
            msg.sender,
            uint64(amount / GWEI),
            uint64(fee / GWEI),
            uint64(fastFee / GWEI),
            uint64(block.timestamp),
            recipient
        );
    }

    /// @notice Pays a burn on Solana at once, from the sender's own ETH:
    /// `msg.value` is its amount, sent to `to` (section 11.7). The sender
    /// states the burn's record. When a claim carrying the same REQUEST
    /// record is accepted, the vault pays the sender the amount and its
    /// share of the fast fee (D124). Stated wrongly, the sender's ETH is
    /// lost: it checks the burn on Solana first. Anyone may call; each
    /// record stated once, so nobody blocks the true one (D126).
    function fastPay(uint64 requestId, address to, uint64 fastFee, uint64 requestedAt) external payable nonReentrant {
        if (to == address(0) || msg.value == 0) revert ZeroAmount();
        if (msg.value % GWEI != 0) revert NotGwei();
        if (msg.value / GWEI > type(uint64).max) revert TooLarge();
        FastPay storage f = _fastPays[requestId][_stated(to, uint64(msg.value / GWEI), fastFee, requestedAt)];
        if (requestPaid[requestId] || f.attester != address(0)) revert AlreadyPaid();
        f.attester = msg.sender;
        f.paidAt = uint64(block.timestamp);
        emit FastPaid(requestId, msg.sender, to, msg.value);
        _send(to, msg.value);
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
    function openCheckpoint(uint16 confirmations) external payable returns (uint256 jobId) {
        uint256 fee = protocol.commitmentFeeFor(confirmations);
        uint256 escrow = protocol.MIN_ESCROW_MULTIPLE() * fee;
        if (escrow < minCertifyingEscrow) escrow = minCertifyingEscrow;
        bytes32 tag = keccak256(abi.encode("iPoW checkpoint", address(this), ++checkpointCount));
        jobId = protocol.openJob{value: msg.value}(tag, escrow, protocol.DEFAULT_ESCROW_FEE_BPS(), confirmations, 0, msg.sender);
    }

    // ------------------------------------------------------------------
    // Pair chain and bond (D106, D107, D110)
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

    function addBond() external payable {
        Chain storage c = _chains[msg.sender];
        if (!c.registered) revert NoChain();
        if (c.slashed) revert ChainEnded();
        c.bond += msg.value;
        emit BondChanged(msg.sender, c.bond);
    }

    /// @notice Adds money for the flat deposits of the caller's claims.
    function addDeposits() external payable {
        Chain storage c = _chains[msg.sender];
        if (!c.registered) revert NoChain();
        c.deposits += msg.value;
        emit DepositsChanged(msg.sender, c.deposits);
    }

    /// @notice Withdraws deposit money not put down in a claim.
    function withdrawDeposits(uint256 amount) external nonReentrant {
        Chain storage c = _chains[msg.sender];
        if (amount == 0) revert ZeroAmount();
        if (amount > c.deposits) revert BondNotFree();
        c.deposits -= amount;
        emit DepositsChanged(msg.sender, c.deposits);
        _send(msg.sender, amount);
    }

    /// @notice Withdraws free bond: all of it once the chain has exited and
    /// every claim of it here has ended (D112).
    function withdrawBond(uint256 amount) external nonReentrant {
        if (amount == 0) revert ZeroAmount();
        if (amount > freeBond(msg.sender)) revert BondNotFree();
        Chain storage c = _chains[msg.sender];
        c.bond -= amount;
        if (c.stated > c.bond) c.stated = c.bond;
        emit BondChanged(msg.sender, c.bond);
        _send(msg.sender, amount);
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
    /// collects its own share afterwards, so no number of deposits can make
    /// this too costly to run.
    function decide(uint256 claimId) external {
        Claim storage cl = _claims[claimId];
        if (cl.operator == address(0)) revert NotOpen();
        if (cl.decided) revert AlreadyDecided();
        if (block.timestamp < uint256(cl.lastAt) + OBJECTION_WINDOW) revert WindowNotOver();

        Chain storage c = _chains[cl.operator];
        bool accepted = !cl.held && !c.slashed && !c.refused;
        cl.decided = true;
        cl.accepted = accepted;
        c.openValue -= cl.value;
        c.openClaims -= 1;
        if (accepted) {
            if (cl.peerBond != 0) c.peerBond = cl.peerBond;
        } else {
            c.refused = true;
        }
        // D111: the losing side's deposits are shared by the winning side.
        // What does not divide evenly, or has nobody to go to, backs vETH.
        uint256 winners = accepted ? cl.answers : cl.objections;
        uint256 pot = (accepted ? cl.objections : cl.answers) * deposit;
        if (winners == 0) {
            reserve += pot;
        } else {
            uint256 share = pot / winners;
            reserve += pot - share * winners;
            cl.payout = deposit + share;
        }
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
        _credit(msg.sender, entries * cl.payout);
    }

    /// @notice Pays a request carried by an accepted claim, once, with its
    /// fast fee: to the attester that paid it at once with the same record,
    /// its amount and share of the fast fee, the rest to the request's
    /// address; or else all to the address (section 11.7). Anyone may call.
    function payRequest(uint256 claimId, uint64 requestId) external nonReentrant {
        if (!_claims[claimId].accepted) revert NotAccepted();
        Request storage r = _requests[claimId][requestId];
        if (r.to == address(0)) revert NotAccepted();
        if (requestPaid[requestId]) revert AlreadyDone();
        uint256 amount = (uint256(r.amount) + r.fastFee) * GWEI;
        if (amount > reserve) revert Underfunded();
        requestPaid[requestId] = true;
        reserve -= amount;
        FastPay storage f = _fastPays[requestId][_stated(r.to, r.amount, r.fastFee, r.requestedAt)];
        if (f.attester == address(0)) {
            emit RequestPaid(requestId, claimId, r.to, amount);
            _credit(r.to, amount);
        } else {
            uint256 share = uint256(fastShare(r.fastFee, r.requestedAt, f.paidAt)) * GWEI;
            uint256 toAttester = uint256(r.amount) * GWEI + share;
            emit RequestPaid(requestId, claimId, f.attester, toAttester);
            _credit(f.attester, toAttester);
            if (amount > toAttester) _credit(r.to, amount - toAttester);
        }
    }

    /// @notice Returns a lock whose CANCEL an accepted claim carries, with
    /// its fee if no message earned it. Anyone may call.
    function returnLock(uint256 claimId, uint64 lockId) external nonReentrant {
        if (!_claims[claimId].accepted) revert NotAccepted();
        if (!cancels[claimId][lockId]) revert NoCancel();
        Lock storage l = _locks[lockId];
        if (l.returned) revert AlreadyDone();
        uint256 amount = (uint256(l.amount) + l.fastFee) * GWEI;
        if (amount > reserve) revert Underfunded();
        l.returned = true;
        reserve -= amount;
        uint256 total = amount;
        if (!l.feePaid) {
            // The fee is settled: no later message can earn it.
            l.feePaid = true;
            total += uint256(l.fee) * GWEI;
        }
        emit LockReturned(lockId, l.owner, total);
        _credit(l.owner, total);
    }

    /// @notice Takes what the protocol owes the vault, the application's share
    /// of a slashed checkpoint job, into the backing of vETH. Anyone may call.
    function collectProtocolCredit() external nonReentrant {
        uint256 before = address(this).balance;
        protocol.withdrawCredit();
        reserve += address(this).balance - before;
    }

    function withdrawCredit() external nonReentrant {
        uint256 amount = credit[msg.sender];
        if (amount == 0) revert ZeroAmount();
        credit[msg.sender] = 0;
        emit CreditWithdrawn(msg.sender, amount);
        _send(msg.sender, amount);
    }

    // ------------------------------------------------------------------
    // Inside
    // ------------------------------------------------------------------

    /// @dev Judges the records about Ethereum and gathers the others into one
    /// claim. A batch that does not parse is false: its hash is what the
    /// operator wrote on Bitcoin.
    function _process(address operator, Chain storage c, bytes calldata batch)
        private
        returns (bool truthful, uint256 claimId)
    {
        uint256 o;
        uint256 home;
        uint256 value;
        uint256 peerBond;
        bool stating;
        bool acting;
        // First pass: judge, and measure the claim.
        while (o < batch.length) {
            uint8 kind = uint8(batch[o]);
            if ((kind == REQUEST || kind == CANCEL) && ++home > MAX_HOME_RECORDS) return (false, 0);
            if (kind == LOCK) {
                if (o + LOCK_LEN > batch.length) return (false, 0);
                uint64 lockId = _u64(batch, o + 1);
                Lock storage l = _locks[lockId];
                if (
                    // Returned or not: Solana issues nothing for a lock it
                    // marked never usable (section 11.5).
                    l.owner == address(0) ||
                    l.amount != _u64(batch, o + 9) ||
                    l.recipient != bytes32(batch[o + 17:o + 49]) ||
                    l.fee != _u64(batch, o + 49) ||
                    l.fastFee != _u64(batch, o + 57) ||
                    l.lockedAt != _u64(batch, o + 65)
                ) return (false, 0);
                o += LOCK_LEN;
            } else if (kind == REQUEST) {
                if (o + REQUEST_LEN > batch.length) return (false, 0);
                // Solana writes no request to address zero.
                if (bytes20(batch[o + 17:o + 37]) == bytes20(0)) return (false, 0);
                value += (uint256(_u64(batch, o + 9)) + _u64(batch, o + 45)) * GWEI;
                acting = true;
                o += REQUEST_LEN;
            } else if (kind == CANCEL) {
                if (o + 9 > batch.length) return (false, 0);
                // Solana gives up only a lock it learned from a true LOCK
                // record, so a lock that does not exist here is a lie.
                Lock storage l = _locks[_u64(batch, o + 1)];
                if (l.owner == address(0)) return (false, 0);
                value += (uint256(l.amount) + l.fastFee) * GWEI;
                acting = true;
                o += 9;
            } else if (kind == BOND) {
                if (o + 10 > batch.length) return (false, 0);
                uint8 net = uint8(batch[o + 1]);
                uint256 amount = uint256(_u64(batch, o + 2)) * GWEI;
                if (net == ETHEREUM) {
                    // D110: judged here. Once per chain, from the bond. The
                    // same BOND again is true and changes nothing: it is
                    // carried again when its claim could not open on Solana.
                    if (stating || amount == 0) return (false, 0);
                    if (c.stated != 0 ? amount != c.stated : amount > c.bond) return (false, 0);
                    stating = true;
                } else if (net == SOLANA) {
                    // A fact of Solana, judged there (D109). Here the first
                    // one carried in a claim counts, and a later one is not
                    // acted on.
                    if (!c.peerBondCarried && peerBond == 0 && amount != 0) {
                        peerBond = amount;
                        acting = true;
                    }
                } else {
                    return (false, 0);
                }
                o += 10;
            } else if (kind == EXIT) {
                if (o + 1 != batch.length) return (false, 0);
                o += 1;
            } else {
                return (false, 0);
            }
        }

        // Second pass: nothing false was found. Apply.
        o = 0;
        if (acting) claimId = _openClaimFor(operator, c, value, peerBond);
        if (claimId != 0) emit ClaimBatch(claimId, batch);
        while (o < batch.length) {
            uint8 kind = uint8(batch[o]);
            if (kind == LOCK) {
                uint64 lockId = _u64(batch, o + 1);
                Lock storage l = _locks[lockId];
                // A slashed operator earns nothing; the fee waits for another.
                if (!l.feePaid && !c.slashed) {
                    l.feePaid = true;
                    uint256 fee = uint256(l.fee) * GWEI;
                    emit FeeEarned(lockId, operator, fee);
                    if (fee != 0) _credit(operator, fee);
                }
                o += LOCK_LEN;
            } else if (kind == REQUEST) {
                if (claimId != 0) {
                    _requests[claimId][_u64(batch, o + 1)] = Request({
                        to: address(bytes20(batch[o + 17:o + 37])),
                        amount: _u64(batch, o + 9),
                        fastFee: _u64(batch, o + 45),
                        requestedAt: _u64(batch, o + 53)
                    });
                }
                o += REQUEST_LEN;
            } else if (kind == CANCEL) {
                if (claimId != 0) cancels[claimId][_u64(batch, o + 1)] = true;
                o += 9;
            } else if (kind == BOND) {
                if (uint8(batch[o + 1]) == ETHEREUM) c.stated = uint256(_u64(batch, o + 2)) * GWEI;
                // Only a claim that opened carries it: otherwise it can be
                // carried again.
                else if (claimId != 0 && peerBond != 0) c.peerBondCarried = true;
                o += 10;
            } else {
                c.exited = true;
                o += 1;
            }
        }
        return (true, claimId);
    }

    /// @dev The acting records of one message become one claim, unless the
    /// chain was refused, its open claims would pass 80% of its bond on
    /// Solana, or its deposit money cannot pay the deposit. Then nothing
    /// acts, and the records can be carried again.
    function _openClaimFor(address operator, Chain storage c, uint256 value, uint256 peerBond)
        private
        returns (uint256 claimId)
    {
        if (c.refused || c.slashed) return 0;
        if (value != 0 && (c.openValue + value) * BPS > c.peerBond * COVER_BPS) return 0;
        if (c.deposits < deposit) return 0;
        c.deposits -= deposit;
        c.openValue += value;
        c.openClaims += 1;
        claimId = ++claimCount;
        Claim storage cl = _claims[claimId];
        cl.operator = operator;
        cl.lastAt = uint40(block.timestamp);
        cl.value = value;
        cl.peerBond = peerBond;
        cl.answers = 1;
        cl.openedBlock = uint64(block.number);
        answersOf[claimId][operator] = 1;
        emit ClaimOpened(claimId, operator, value);
    }

    /// @dev D109: the whole bond. 80% backs vETH, 20% to the submitter, or to
    /// the backing when the operator submitted its own false message.
    function _slash(address operator, Chain storage c) private {
        uint256 amount = c.bond;
        c.bond = 0;
        c.stated = 0;
        c.slashed = true;
        uint256 toSubmitter = msg.sender == operator ? 0 : (amount * (BPS - BACKING_SHARE_BPS)) / BPS;
        reserve += amount - toSubmitter;
        emit Slashed(operator, amount, msg.sender);
        if (toSubmitter != 0) _credit(msg.sender, toSubmitter);
    }

    function _openClaim(uint256 claimId) private view returns (Claim storage cl) {
        if (msg.value != deposit) revert WrongDeposit();
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

    function _stated(address to, uint64 amount, uint64 fastFee, uint64 requestedAt) private pure returns (bytes32) {
        return keccak256(abi.encode(to, amount, fastFee, requestedAt));
    }

    function _u64(bytes calldata b, uint256 o) private pure returns (uint64) {
        return uint64(bytes8(b[o:o + 8]));
    }

    function _credit(address to, uint256 amount) private {
        credit[to] += amount;
    }

    function _send(address to, uint256 amount) private {
        (bool ok, ) = to.call{value: amount}("");
        if (!ok) revert TransferFailed();
    }

    /// @dev Only the protocol sends ETH here, when the vault collects its
    /// credit.
    receive() external payable {
        if (msg.sender != address(protocol)) revert();
    }
}
