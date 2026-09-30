// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

import {BitcoinHeaderLib} from "./BitcoinHeaderLib.sol";

/// @title iPoWLightClient
/// @notice The store of Bitcoin blocks of the iPoW protocol, one per network,
/// shared by everyone. Spec: docs/design/ipow-protocol.md, section 2. The
/// `D` numbers below are the decisions of that document.
///
/// Anyone can add a block that passes the checks (D64). There is no owner and no
/// key, and nothing here can be changed after deployment (D59).
///
/// @dev How a block is stored. The contract cannot prove a block number, so the
/// number is stated by whoever submits an epoch start or a jump. A block is
/// stored under (block hash, height, epoch time), where epoch time is the time
/// of the first block of its epoch. Honest submitters state the same values and
/// share one entry (D48). A submitter that states other values gets an entry of
/// its own and cannot disturb theirs (D64).
///
/// An entry does not store a link to its parent. The parent is found from the
/// entry's own previous-block hash, so an entry is the same whether it was
/// stored by a jump, by an epoch start or by streaming.
contract iPoWLightClient {
    using BitcoinHeaderLib for bytes;

    // ------------------------------------------------------------------
    // Rules fixed by the protocol
    // ------------------------------------------------------------------

    /// @notice D44: an epoch start is the first 6 blocks of an epoch.
    uint256 public constant EPOCH_START_BLOCKS = 6;
    /// @notice D39: an anchor may be at most 2 hours old.
    uint256 public constant MAX_ANCHOR_AGE = 2 hours;
    /// @notice D57: an epoch start may be at most 4 weeks older than the anchor.
    /// D45 compares epoch starts over the same 4 weeks.
    uint256 public constant MAX_EPOCH_START_AGE = 4 weeks;
    /// @notice Bitcoin's own rule: a block's time is at most 2 hours ahead.
    uint256 public constant MAX_FUTURE_TIME = 2 hours;
    /// @notice D70: a window is at most 100 blocks, so no walk is longer.
    uint256 public constant MAX_WALK = 100;
    /// @notice D45 compares epoch starts by the day of their first block.
    uint256 public constant COMPARE_STEP = 1 days;

    /// @dev D38: minimum difficulty 2^45. Difficulty 1 is the target
    /// 0xFFFF * 2^208, so difficulty 2^45 is 0xFFFF * 2^163.
    uint256 internal constant MIN_DIFFICULTY_TARGET = uint256(0xFFFF) << 163;
    /// @dev The easiest target Bitcoin allows (difficulty 1).
    uint256 internal constant POW_LIMIT = uint256(0xFFFF) << 208;

    uint256 internal constant EPOCH_BLOCKS = 2016;
    uint256 internal constant HEADER_LENGTH = 80;

    /// @notice D93: no block is stored under a lower number. Fixed at
    /// deployment, about 4 weeks below Bitcoin's height at that moment, so
    /// that an honest operator can always answer the 2,016 questions for a
    /// parent (D92) and a label such as 0 is refused from the start.
    uint32 public immutable minHeight;

    // ------------------------------------------------------------------
    // Storage
    // ------------------------------------------------------------------

    struct Node {
        bytes32 prevHash;
        bytes32 merkleRoot;
        uint32 bits;
        uint32 time;
        uint32 height;
        /// The time of the first block of this block's epoch.
        uint32 epochTime;
        /// Network time when the entry was stored. Zero means "not stored".
        uint40 storedAt;
        /// Network time when the block first passed the jump guards, by
        /// anyone. Zero means it was never jumped to. It says nothing about a
        /// later use of the block: a job applies D39 itself.
        uint40 anchoredAt;
    }

    struct EpochStart {
        uint32 bits;
        /// The time of the first of the 6 blocks.
        uint32 firstTime;
        /// The stated height of the first of the 6 blocks.
        uint32 height;
        uint40 recordedAt;
    }

    mapping(bytes32 => Node) private _nodes;
    /// @dev Keyed by the node id of the first of the 6 blocks.
    mapping(bytes32 => EpochStart) private _epochStarts;
    /// @dev Day number of an epoch start's first block => the lowest target
    /// (highest difficulty) among the epoch starts of that day. Zero means
    /// none. D45 reads a fixed number of days, so its cost does not grow with
    /// the number of epoch starts recorded.
    mapping(uint256 => uint256) private _lowestTargetOfDay;

    // ------------------------------------------------------------------
    // Events and errors
    // ------------------------------------------------------------------

    event BlockStored(
        bytes32 indexed id,
        bytes32 indexed blockHash,
        uint32 height,
        uint32 epochTime,
        bytes32 prevHash
    );
    event EpochStartRecorded(bytes32 indexed id, uint32 bits, uint32 height, uint32 firstTime);
    event Jumped(bytes32 indexed id, bytes32 indexed epochStartId, address indexed by);

    error InvalidLength();
    error InvalidHeight();
    error InsufficientWork();
    error DifficultyTooLow();
    error DifficultyMismatch();
    error NotLinked();
    error TimeTooNew();
    error AnchorTooOld();
    error EpochStartTooOld();
    error EpochStartOutOfRange();
    error EpochStartTooWeak();
    error UnknownEpochStart();
    error UnknownParent();
    error UnknownBlock();
    error WalkTooLong();
    error InvalidTransaction();
    error InvalidIndex();
    error EpochTimeMismatch();
    error BelowMinHeight();

    constructor(uint32 minHeight_) {
        minHeight = minHeight_;
    }

    // ------------------------------------------------------------------
    // Adding blocks
    // ------------------------------------------------------------------

    /// @notice Records an epoch start: 6 blocks that name each other in order
    /// and have one difficulty (D44). Anyone may call (D45). Recording one that
    /// is recorded already changes nothing.
    /// @dev The contract cannot check that the 6 blocks are the first of an
    /// epoch. Any 6 blocks in a row that Bitcoin mined in the last 4 weeks
    /// pass. What an epoch start pins is a difficulty that Bitcoin used.
    /// @param headers The 6 headers, 80 bytes each, oldest first.
    /// @param height The stated height of the first one. Not proven.
    function addEpochStart(bytes calldata headers, uint32 height) external returns (bytes32 id) {
        if (headers.length != EPOCH_START_BLOCKS * HEADER_LENGTH) revert InvalidLength();
        if (height % EPOCH_BLOCKS != 0) revert InvalidHeight();
        // D93. A jump is never below its epoch start, and streaming only goes
        // up, so this and `extendBack` keep every block at or above it.
        if (height < minHeight) revert BelowMinHeight();

        uint256 nowTime = _now();
        bytes calldata first = headers[0:HEADER_LENGTH];
        uint32 firstTime = first.time();
        uint32 firstBits = first.bits();

        // An epoch start older than this can serve no anchor (D39, D57).
        if (nowTime > uint256(firstTime) + MAX_EPOCH_START_AGE + MAX_ANCHOR_AGE) revert EpochStartTooOld();

        bytes32 prevHash;
        for (uint256 i = 0; i < EPOCH_START_BLOCKS; i++) {
            bytes calldata header = headers[i * HEADER_LENGTH:(i + 1) * HEADER_LENGTH];
            if (header.bits() != firstBits) revert DifficultyMismatch();
            if (i > 0 && header.prevLE() != prevHash) revert NotLinked();

            bytes32 blockHash = _checkHeader(header, nowTime);
            bytes32 nodeKey = _store(blockHash, header, height + uint32(i), firstTime, nowTime);
            if (i == 0) id = nodeKey;
            prevHash = blockHash;
        }

        if (_epochStarts[id].recordedAt != 0) return id;
        _epochStarts[id] = EpochStart({
            bits: firstBits,
            firstTime: firstTime,
            height: height,
            recordedAt: uint40(nowTime)
        });

        uint256 target = BitcoinHeaderLib.targetFromBits(firstBits);
        uint256 day = uint256(firstTime) / COMPARE_STEP;
        uint256 lowest = _lowestTargetOfDay[day];
        if (lowest == 0 || target < lowest) _lowestTargetOfDay[day] = target;

        emit EpochStartRecorded(id, firstBits, height, firstTime);
    }

    /// @notice Jumps to an anchor. The guards are D38, D39, D44, D45 and D57.
    /// @param header The anchor's header.
    /// @param epochStartId The epoch start the anchor belongs to.
    /// @param height The stated height of the anchor. Not proven.
    function jump(
        bytes calldata header,
        bytes32 epochStartId,
        uint32 height
    ) external returns (bytes32 id) {
        EpochStart memory es = _epochStarts[epochStartId];
        if (es.recordedAt == 0) revert UnknownEpochStart();

        uint256 nowTime = _now();
        bytes32 blockHash = _checkHeader(header, nowTime);
        uint32 time = header.time();

        // D44: the anchor has the difficulty of the epoch start.
        if (header.bits() != es.bits) revert DifficultyMismatch();
        // D39.
        if (nowTime > uint256(time) + MAX_ANCHOR_AGE) revert AnchorTooOld();
        // D57.
        if (time < es.firstTime || uint256(time) - es.firstTime > MAX_EPOCH_START_AGE) {
            revert EpochStartOutOfRange();
        }
        if (height < es.height || uint256(height) >= uint256(es.height) + EPOCH_BLOCKS) revert InvalidHeight();
        // The first block of an epoch gives the epoch its time, as in `extend`.
        if (height == es.height && time != es.firstTime) revert EpochTimeMismatch();
        // D45.
        if (!_counts(es, time)) revert EpochStartTooWeak();

        id = _store(blockHash, header, height, es.firstTime, nowTime);
        if (_nodes[id].anchoredAt == 0) _nodes[id].anchoredAt = uint40(nowTime);
        emit Jumped(id, epochStartId, msg.sender);
    }

    /// @notice Streams blocks on top of a stored block. Each block names the one
    /// before it and has the difficulty Bitcoin's rules give it (D72).
    /// @param headers One or more headers, 80 bytes each, oldest first.
    /// @param parentHeight The height of the stored block the first header names.
    /// @param parentEpochTime The epoch time of that stored block.
    /// @return id The entry of the last header.
    function extend(
        bytes calldata headers,
        uint32 parentHeight,
        uint32 parentEpochTime
    ) external returns (bytes32 id) {
        if (headers.length == 0 || headers.length % HEADER_LENGTH != 0) revert InvalidLength();
        if (headers.length / HEADER_LENGTH > MAX_WALK) revert InvalidLength();

        bytes calldata first = headers[0:HEADER_LENGTH];
        bytes32 parentHash = first.prevLE();
        Node memory parent = _nodes[nodeId(parentHash, parentHeight, parentEpochTime)];
        if (parent.storedAt == 0) revert UnknownParent();

        uint256 nowTime = _now();
        uint32 parentBits = parent.bits;
        uint32 parentTime = parent.time;
        uint32 height = parentHeight;
        uint32 epochTime = parentEpochTime;

        for (uint256 o = 0; o < headers.length; o += HEADER_LENGTH) {
            bytes calldata header = headers[o:o + HEADER_LENGTH];
            if (header.prevLE() != parentHash) revert NotLinked();

            height += 1;
            uint32 expectedBits = parentBits;
            if (height % EPOCH_BLOCKS == 0) {
                // D72: the first block of a new epoch.
                expectedBits = BitcoinHeaderLib.retarget(parentBits, epochTime, parentTime, _powLimit());
                epochTime = header.time();
            }
            if (header.bits() != expectedBits) revert DifficultyMismatch();

            bytes32 blockHash = _checkHeader(header, nowTime);
            id = _store(blockHash, header, height, epochTime, nowTime);

            parentHash = blockHash;
            parentBits = expectedBits;
            parentTime = header.time();
        }
    }

    /// @notice Stores the parent of a stored block: the block whose hash the
    /// stored block names as the one before it. Anyone may call (D64). This is
    /// how the parent of an anchor is shown when a guardian asks for it (D81).
    /// @param header The parent's header.
    /// @param childHash The stored block, with its height and epoch time.
    /// @param prevEpochTime Only used when the stored block is the first of
    /// its epoch: the epoch time of the epoch before. Zero otherwise.
    /// @return id The entry of the parent.
    function extendBack(
        bytes calldata header,
        bytes32 childHash,
        uint32 childHeight,
        uint32 childEpochTime,
        uint32 prevEpochTime
    ) external returns (bytes32 id) {
        Node memory child = _nodes[nodeId(childHash, childHeight, childEpochTime)];
        if (child.storedAt == 0) revert UnknownBlock();
        if (childHeight == 0) revert InvalidHeight();
        if (childHeight - 1 < minHeight) revert BelowMinHeight();

        uint256 nowTime = _now();
        bytes32 blockHash = _checkHeader(header, nowTime);
        if (blockHash != child.prevHash) revert NotLinked();

        uint32 epochTime = childEpochTime;
        if (childHeight % EPOCH_BLOCKS == 0) {
            // D72, read backward: the stored block is the first of its epoch.
            epochTime = prevEpochTime;
            uint32 expected = BitcoinHeaderLib.retarget(header.bits(), prevEpochTime, header.time(), _powLimit());
            if (expected != child.bits) revert DifficultyMismatch();
        } else if (header.bits() != child.bits) {
            revert DifficultyMismatch();
        }
        // The first block of an epoch gives the epoch its time.
        if ((childHeight - 1) % EPOCH_BLOCKS == 0 && epochTime != header.time()) revert EpochTimeMismatch();

        id = _store(blockHash, header, childHeight - 1, epochTime, nowTime);
    }

    // ------------------------------------------------------------------
    // Reading
    // ------------------------------------------------------------------

    /// @notice The key a block is stored under.
    function nodeId(bytes32 blockHash, uint32 height, uint32 epochTime) public pure returns (bytes32) {
        return keccak256(abi.encode(blockHash, height, epochTime));
    }

    function getNode(bytes32 id) external view returns (Node memory) {
        return _nodes[id];
    }

    function isStored(bytes32 id) public view returns (bool) {
        return _nodes[id].storedAt != 0;
    }

    function getEpochStart(bytes32 id) external view returns (EpochStart memory) {
        return _epochStarts[id];
    }

    /// @notice D45: whether an epoch start counts for an anchor with this time.
    function epochStartCounts(bytes32 epochStartId, uint32 anchorTime) external view returns (bool) {
        EpochStart memory es = _epochStarts[epochStartId];
        if (es.recordedAt == 0) revert UnknownEpochStart();
        return _counts(es, anchorTime);
    }

    /// @notice Whether `ancestor` is reached by following previous-block hashes
    /// down from `descendant`. Every block on the way must be stored, and every
    /// block must have the difficulty that Bitcoin's rules give it after the
    /// block before it (D72), however it was stored.
    /// @param prevEpochTime Only used when the walk crosses into an older epoch:
    /// the epoch time of that older epoch. Pass 0 otherwise.
    function isAncestor(
        bytes32 ancestorHash,
        uint32 ancestorHeight,
        uint32 ancestorEpochTime,
        bytes32 descendantHash,
        uint32 descendantHeight,
        uint32 descendantEpochTime,
        uint32 prevEpochTime
    ) external view returns (bool linked) {
        (linked, ) = walk(
            ancestorHash,
            ancestorHeight,
            ancestorEpochTime,
            descendantHash,
            descendantHeight,
            descendantEpochTime,
            prevEpochTime
        );
    }

    /// @notice The same walk, and the mining work of the blocks on the way:
    /// every block after `ancestor`, up to and with `descendant`. Zero when
    /// the two are not linked.
    function walk(
        bytes32 ancestorHash,
        uint32 ancestorHeight,
        uint32 ancestorEpochTime,
        bytes32 descendantHash,
        uint32 descendantHeight,
        uint32 descendantEpochTime,
        uint32 prevEpochTime
    ) public view returns (bool linked, uint256 work) {
        if (descendantHeight < ancestorHeight) return (false, 0);
        uint256 steps = descendantHeight - ancestorHeight;
        if (steps > MAX_WALK) revert WalkTooLong();

        bytes32 hash = descendantHash;
        uint32 height = descendantHeight;
        uint32 epochTime = descendantEpochTime;
        uint32 childBits;
        bool childIsFirstOfEpoch;

        for (uint256 i = 0; ; i++) {
            Node memory node = _nodes[nodeId(hash, height, epochTime)];
            if (node.storedAt == 0) return (false, 0);
            // The first block of an epoch gives the epoch its time.
            if (height % EPOCH_BLOCKS == 0 && node.time != epochTime) return (false, 0);

            if (i > 0) {
                uint32 expected = childIsFirstOfEpoch
                    ? BitcoinHeaderLib.retarget(node.bits, epochTime, node.time, _powLimit())
                    : node.bits;
                if (expected != childBits) return (false, 0);
            }
            if (i == steps) break;

            work += BitcoinHeaderLib.work(BitcoinHeaderLib.targetFromBits(node.bits));
            childBits = node.bits;
            childIsFirstOfEpoch = height % EPOCH_BLOCKS == 0;
            hash = node.prevHash;
            height -= 1;
            if (childIsFirstOfEpoch) epochTime = prevEpochTime;
        }

        if (hash != ancestorHash || epochTime != ancestorEpochTime) return (false, 0);
        return (true, work);
    }

    /// @notice Whether a transaction is in a stored block.
    /// @param id The entry of the block.
    /// @param rawTx The transaction without witness data, so that its double
    /// SHA-256 is the txid.
    /// @param siblingsLE The Merkle siblings, from the leaf up, in header byte order.
    /// @param index The position of the transaction in the block.
    function txInBlock(
        bytes32 id,
        bytes calldata rawTx,
        bytes32[] calldata siblingsLE,
        uint256 index
    ) external view returns (bool) {
        Node memory node = _nodes[id];
        if (node.storedAt == 0) revert UnknownBlock();
        // A 64-byte "transaction" could be an inner node of the Merkle tree
        // passed off as a leaf.
        if (rawTx.length == 64) revert InvalidTransaction();
        if (siblingsLE.length > 32 || index >> siblingsLE.length != 0) revert InvalidIndex();

        bytes32 txid = sha256(abi.encodePacked(sha256(rawTx)));
        return BitcoinHeaderLib.merkleRootFrom(txid, siblingsLE, index) == node.merkleRoot;
    }

    /// @notice The mining work one block of this difficulty stands for.
    function workOf(uint32 bits) external pure returns (uint256) {
        return BitcoinHeaderLib.work(BitcoinHeaderLib.targetFromBits(bits));
    }

    // ------------------------------------------------------------------
    // Internal
    // ------------------------------------------------------------------

    /// @dev The checks every block passes, however it arrives: enough work for
    /// its own difficulty, the minimum difficulty (D38), and a time that is not
    /// in the future.
    function _checkHeader(bytes calldata header, uint256 nowTime) private view returns (bytes32 blockHash) {
        blockHash = header.hashLE();
        uint256 target = BitcoinHeaderLib.targetFromBits(header.bits());
        if (target > _maxTarget()) revert DifficultyTooLow();
        if (BitcoinHeaderLib.uintFromLE(blockHash) > target) revert InsufficientWork();
        if (uint256(header.time()) > nowTime + MAX_FUTURE_TIME) revert TimeTooNew();
    }

    /// @dev Stores a block unless it is stored already (D48).
    function _store(
        bytes32 blockHash,
        bytes calldata header,
        uint32 height,
        uint32 epochTime,
        uint256 nowTime
    ) private returns (bytes32 id) {
        id = nodeId(blockHash, height, epochTime);
        if (_nodes[id].storedAt != 0) return id;

        bytes32 prevHash = header.prevLE();
        _nodes[id] = Node({
            prevHash: prevHash,
            merkleRoot: header.merkleRootLE(),
            bits: header.bits(),
            time: header.time(),
            height: height,
            epochTime: epochTime,
            storedAt: uint40(nowTime),
            anchoredAt: 0
        });
        emit BlockStored(id, blockHash, height, epochTime, prevHash);
    }

    /// @dev D45: the epoch start counts if its difficulty is at least half of
    /// the highest among the epoch starts of the last 4 weeks before the
    /// anchor, counted in whole days by the time of their first block. A lower
    /// target is a higher difficulty.
    function _counts(EpochStart memory es, uint32 anchorTime) private view returns (bool) {
        uint256 target = BitcoinHeaderLib.targetFromBits(es.bits);
        uint256 lowest = target;

        uint256 lastDay = uint256(anchorTime) / COMPARE_STEP;
        uint256 firstDay = anchorTime > MAX_EPOCH_START_AGE
            ? (uint256(anchorTime) - MAX_EPOCH_START_AGE) / COMPARE_STEP
            : 0;
        for (uint256 day = firstDay; day <= lastDay; day++) {
            uint256 other = _lowestTargetOfDay[day];
            if (other != 0 && other < lowest) lowest = other;
        }

        // Half the difficulty is twice the target.
        return target - lowest <= lowest;
    }

    function _now() internal view virtual returns (uint256) {
        return block.timestamp;
    }

    function _maxTarget() internal view virtual returns (uint256) {
        return MIN_DIFFICULTY_TARGET;
    }

    function _powLimit() internal view virtual returns (uint256) {
        return POW_LIMIT;
    }
}
