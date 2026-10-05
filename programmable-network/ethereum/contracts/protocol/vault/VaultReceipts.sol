// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

import {ReentrancyGuard} from "@openzeppelin/contracts/utils/ReentrancyGuard.sol";
import {Strings} from "@openzeppelin/contracts/utils/Strings.sol";

import {VaultRecords as R, IVaultCore} from "./VaultRecords.sol";
import {VaultReceipt} from "./VaultReceipt.sol";

/// @title VaultReceipts
/// @notice The vault's part for the receipts on Ethereum of assets whose
/// home is Solana (section 11.9): it makes each receipt once its ASSET
/// record is accepted, issues them for accepted LOCK records, burns them
/// for the asset on Solana, and runs the fast path of locks on Solana
/// (section 11.7). Made by the core, which alone calls its `onlyCore`
/// functions.
///
/// Receipts are credited, and the address withdraws them.
contract VaultReceipts is ReentrancyGuard {
    /// What became of a lock on Solana here. It exists once something
    /// happened, so each happens once.
    struct Mark {
        /// Its receipt counted: issued after its claim, or delivered by an
        /// attest settled with the true record.
        bool issued;
        bool givenUp;
        /// Attests of the lock (D126), those settled, when the first was
        /// made, and the latest: each names the one before.
        uint32 attests;
        uint32 settled;
        uint64 firstAt;
        uint64 lastAttest;
    }

    /// A burn of receipts for the asset on Solana.
    struct Burn {
        address owner;
        uint32 asset;
        uint64 amount;
        bytes32 to; // on Solana
        uint64 fee;
        uint64 fastFee;
        uint64 at;
        bool feePaid;
    }

    /// An attest of a lock on Solana: its receipt minted at once (D123).
    struct Attest {
        address attester;
        uint64 lockId;
        uint32 asset;
        uint64 amount;
        address recipient;
        bytes32 recordHash;
        uint64 collateral;
        uint64 attestedAt;
        /// A claim of the attester's own chain carrying the same record,
        /// opened in time; zero if none (D125).
        uint256 claim;
        /// The attest of the same lock made before it, settled first (D126).
        uint64 prev;
        bool burned;
        bool closed;
    }

    IVaultCore public immutable core;

    mapping(uint32 => VaultReceipt) public receiptOf;
    mapping(uint64 => Mark) private _marks;
    uint64 public burnCount;
    mapping(uint64 => Burn) private _burns;
    uint64 public attestCount;
    mapping(uint64 => Attest) private _attests;
    /// @notice Receipts each address can withdraw, by asset.
    mapping(address => mapping(uint32 => uint256)) public credit;

    event ReceiptMade(uint32 indexed asset, address receipt, bytes32 token, uint8 decimals);
    event Issued(uint64 indexed lockId, uint256 indexed claimId, address to, uint64 amount);
    event GivenUp(uint64 indexed lockId, address recipient);
    event Burned(uint64 indexed requestId, uint32 indexed asset, address indexed owner, uint64 amount, uint64 fee, uint64 fastFee, bytes32 to);
    event FeeEarned(uint64 indexed requestId, address indexed operator, uint64 fee);
    event Attested(uint64 indexed attestId, uint64 indexed lockId, address indexed attester, uint64 amount);
    event AttestLinked(uint64 indexed attestId, uint256 claimId);
    event AttestBurned(uint64 indexed attestId, address by);
    event AttestSettled(uint64 indexed attestId, uint256 claimId, bool counted);
    event CreditWithdrawn(address indexed to, uint32 indexed asset, uint256 amount);

    error NotCore();
    error UnknownAsset();
    error AssetExists();
    error ZeroAmount();
    error ZeroAddress();
    error TooLarge();
    error NotAccepted();
    error WrongRecord();
    error AlreadyDone();
    error NotOperator();
    error NotAttester();
    error WindowOver();
    error WindowNotOver();
    error NotRefused();
    error NotInOrder();

    modifier onlyCore() {
        if (msg.sender != address(core)) revert NotCore();
        _;
    }

    /// @notice This network's number, and its peer's (D133), set once.
    uint8 public here;
    uint8 public peer;

    constructor(IVaultCore core_, uint8 here_, uint8 peer_) {
        core = core_;
        here = here_;
        peer = peer_;
    }

    // ------------------------------------------------------------------
    // Receipts
    // ------------------------------------------------------------------

    /// @notice Makes the receipt of an asset on Solana, from an accepted claim
    /// carrying its ASSET record. Anyone may call, once per asset.
    function makeReceipt(uint256 claimId, bytes calldata record) external returns (address) {
        if (record.length != R.ASSET_LEN || uint8(record[0]) != R.ASSET || uint8(record[1]) != peer) revert WrongRecord();
        _requireCarried(claimId, record);
        uint32 asset = R.u32(record, 2);
        if (address(receiptOf[asset]) != address(0)) revert AssetExists();
        uint8 decimals = uint8(record[38]);
        if (decimals > R.MAX_DECIMALS) revert WrongRecord();
        string memory n = Strings.toString(asset);
        (string memory net, string memory coin) = _names(peer);
        (string memory name, string memory symbol) = asset == 0
            ? (string.concat("iPoW v", coin), string.concat("v", coin))
            : (string.concat("iPoW ", net, " asset ", n), string.concat("v", coin, "-", n));
        VaultReceipt r = new VaultReceipt{salt: bytes32(uint256(asset))}(name, symbol, decimals);
        receiptOf[asset] = r;
        emit ReceiptMade(asset, address(r), R.b32(record, 6), decimals);
        return address(r);
    }

    /// @dev A network of D133's list (with D141's Arbitrum) by name, and its
    /// coin as a receipt's name shows it: vETH for Ethereum's ETH, vETH.base
    /// for Base's, vETH.arbitrum for Arbitrum's.
    function _names(uint8 net) private pure returns (string memory, string memory) {
        if (net == 1) return ("Ethereum", "ETH");
        if (net == 2) return ("Solana", "SOL");
        if (net == 3) return ("Base", "ETH.base");
        if (net == 4) return ("Robinhood", "ETH.robinhood");
        if (net == 5) return ("Polkadot", "DOT");
        if (net == 6) return ("Hedera", "HBAR");
        if (net == 7) return ("Hyperliquid", "HYPE");
        if (net == 9) return ("Arbitrum", "ETH.arbitrum");
        return ("Tempo", "USD.tempo");
    }

    function _receipt(uint32 asset) private view returns (VaultReceipt r) {
        r = receiptOf[asset];
        if (address(r) == address(0)) revert UnknownAsset();
    }

    /// @notice Issues the receipt of a lock on Solana carried by an accepted
    /// claim, with the fast fee nobody earned, once per lock, never for a lock
    /// its recipient gave up. For a lock that was attested, only once every
    /// attest was settled, none stated the true record, and the lock takes no
    /// more attests (D126). Anyone may call.
    function issue(uint256 claimId, bytes calldata record) external nonReentrant {
        (uint64 lockId, uint32 asset, uint64 value, address to) = _readLock(record);
        _requireCarried(claimId, record);
        Mark storage m = _marks[lockId];
        if (m.issued || m.givenUp) revert AlreadyDone();
        if (m.attests != 0 && (m.settled != m.attests || block.timestamp < m.firstAt + R.FAST_OPEN_WINDOW)) revert NotInOrder();
        m.issued = true;
        emit Issued(lockId, claimId, to, value);
        _receipt(asset).mint(to, value);
    }

    /// @notice The recipient of a lock on Solana gives it up: no receipt will
    /// be issued for it, and a CANCEL record returns it to its owner there.
    /// The lock must be known from a LOCK record in an accepted claim
    /// (section 11.5), and neither issued nor attested.
    function giveUp(uint256 claimId, bytes calldata record) external {
        (uint64 lockId, , , address to) = _readLock(record);
        _requireCarried(claimId, record);
        if (msg.sender != to) revert NotAttester();
        Mark storage m = _marks[lockId];
        if (m.issued || m.givenUp || m.attests != 0) revert AlreadyDone();
        m.givenUp = true;
        emit GivenUp(lockId, to);
    }

    function getMark(uint64 lockId) external view returns (Mark memory) {
        return _marks[lockId];
    }

    /// @notice Whether lock `lockId` on Solana was given up here: what a CANCEL
    /// record from Ethereum states.
    function givenUp(uint64 lockId) external view returns (bool) {
        return _marks[lockId].givenUp;
    }

    // ------------------------------------------------------------------
    // Burns for the asset on Solana
    // ------------------------------------------------------------------

    /// @notice Burns receipts for the asset on Solana: request number
    /// `burnCount + 1`. The fee, in receipts, goes to the first operator whose
    /// message carrying the burn is judged true here (D113); the fast fee is
    /// burned with the amount and paid on Solana to an attester who paid at
    /// once, or to `to` (D122).
    function burn(uint32 asset, bytes32 to, uint64 amount, uint64 fee, uint64 fastFee) external returns (uint64 requestId) {
        VaultReceipt r = _receipt(asset);
        if (amount == 0) revert ZeroAmount();
        // An address on the peer: a REQUEST to anything else is judged false
        // there, and its operator slashed.
        if (!R.addressOn(to, peer)) revert ZeroAddress();
        if (uint256(amount) + fee + fastFee > type(uint64).max) revert TooLarge();
        r.burnFrom(msg.sender, uint256(amount) + fastFee);
        if (fee != 0) {
            r.burnFrom(msg.sender, fee);
            r.mint(address(this), fee);
        }
        requestId = ++burnCount;
        _burns[requestId] = Burn({
            owner: msg.sender,
            asset: asset,
            amount: amount,
            to: to,
            fee: fee,
            fastFee: fastFee,
            at: uint64(block.timestamp),
            feePaid: false
        });
        emit Burned(requestId, asset, msg.sender, amount, fee, fastFee, to);
    }

    function getBurn(uint64 requestId) external view returns (Burn memory) {
        return _burns[requestId];
    }

    /// @notice The hash of the REQUEST record that states burn `requestId`,
    /// zero if it does not exist.
    function burnRecordHash(uint64 requestId) external view returns (bytes32) {
        Burn storage b = _burns[requestId];
        if (b.owner == address(0)) return bytes32(0);
        return keccak256(R.request(here, b.asset, requestId, b.amount, b.to, b.fee, b.fastFee, b.at));
    }

    /// @notice The core pays a burn's fee to the operator of the first true
    /// message carrying it.
    function earnFee(uint64 requestId, address operator) external onlyCore {
        Burn storage b = _burns[requestId];
        if (b.feePaid) return;
        b.feePaid = true;
        emit FeeEarned(requestId, operator, b.fee);
        if (b.fee != 0) credit[operator][b.asset] += b.fee;
    }

    // ------------------------------------------------------------------
    // The fast path of locks on Solana (section 11.7)
    // ------------------------------------------------------------------

    /// @notice An operator issues the receipt of a lock on Solana at once: new
    /// receipts minted to the recipient, backed by the locked asset (D122,
    /// D123). It locks 1.25 times the amount in receipts and states the lock's
    /// record; a claim of its own chain carrying the same record must open
    /// within 7 days. A lock may be attested several times, for 7 days from
    /// its first (D126).
    function attestLock(bytes calldata record) external nonReentrant returns (uint64 attestId) {
        (uint64 lockId, uint32 asset, , address to) = _readLock(record);
        if (R.u64(record, 14) == 0) revert ZeroAmount();
        if (!core.operatorActive(msg.sender)) revert NotOperator();
        VaultReceipt r = _receipt(asset);
        Mark storage m = _marks[lockId];
        if (m.issued || m.givenUp) revert AlreadyDone();
        if (m.attests == 0) m.firstAt = uint64(block.timestamp);
        if (block.timestamp >= m.firstAt + R.FAST_OPEN_WINDOW) revert WindowOver();
        uint64 amount = R.u64(record, 14);
        uint64 collateral = uint64((uint256(amount) * R.FAST_COLLATERAL_BPS + R.BPS - 1) / R.BPS);
        attestId = ++attestCount;
        _attests[attestId] = Attest({
            attester: msg.sender,
            lockId: lockId,
            asset: asset,
            amount: amount,
            recipient: to,
            recordHash: keccak256(record),
            collateral: collateral,
            attestedAt: uint64(block.timestamp),
            claim: 0,
            prev: m.lastAttest,
            burned: false,
            closed: false
        });
        m.attests += 1;
        m.lastAttest = attestId;
        emit Attested(attestId, lockId, msg.sender, amount);
        r.burnFrom(msg.sender, collateral);
        r.mint(address(this), collateral);
        r.mint(to, amount);
    }

    function getAttest(uint64 attestId) external view returns (Attest memory) {
        return _attests[attestId];
    }

    /// @notice The attester links its attest to a claim of its own chain
    /// carrying the same LOCK record, opened within 7 days of the attest
    /// (D123, D125). Another may be linked once the linked one is refused.
    function linkFast(uint64 attestId, uint256 claimId) external {
        Attest storage a = _attests[attestId];
        if (msg.sender != a.attester) revert NotAttester();
        if (a.burned || a.closed) revert AlreadyDone();
        (address operator, uint64 openedAt, bool decided, bool accepted) = core.claimInfo(claimId);
        if (operator != a.attester || !core.carries(claimId, a.recordHash)) revert WrongRecord();
        if (openedAt > a.attestedAt + R.FAST_OPEN_WINDOW) revert WindowOver();
        if (decided && !accepted) revert NotAccepted();
        if (a.claim != 0) {
            (, , bool d, bool acc) = core.claimInfo(a.claim);
            if (!d || acc) revert NotRefused();
        }
        a.claim = claimId;
        emit AttestLinked(attestId, claimId);
    }

    /// @notice Burns an attester's receipts when no claim was linked within 7
    /// days of the attest, or the linked claim was refused: the amount, in
    /// place of the receipt minted, and the rest to the caller. If the true
    /// record arrives later, the receipt goes to the attester. Anyone may
    /// call.
    function burnFast(uint64 attestId) external nonReentrant {
        Attest storage a = _attests[attestId];
        if (a.attester == address(0) || a.burned || a.closed) revert AlreadyDone();
        if (a.claim == 0) {
            if (block.timestamp < a.attestedAt + R.FAST_OPEN_WINDOW) revert WindowNotOver();
        } else {
            (, , bool decided, bool accepted) = core.claimInfo(a.claim);
            if (!decided || accepted) revert NotRefused();
        }
        a.burned = true;
        emit AttestBurned(attestId, msg.sender);
        // The collateral is 1.25 times the amount, rounded up: never less.
        credit[msg.sender][a.asset] += a.collateral - a.amount;
        _receipt(a.asset).burn(a.amount);
    }

    /// @notice Settles an attest with an accepted claim carrying a LOCK record
    /// of its lock, after the attest made before it (D126). Anyone may call.
    ///
    /// | The attest | Not burned | Burned |
    /// |---|---|---|
    /// | Counts | Its receipts back and its share of the fast fee | The receipt and its share, as it was only late |
    /// | Wrong or extra | Its amount burned, the rest to the caller | Nothing |
    ///
    /// It counts when it stated the true record and no attest counted
    /// before. The rest of the fast fee goes to the recipient. With none
    /// counting, the last settled issues the true receipt once the lock takes
    /// no more attests.
    function settleFast(uint64 attestId, uint256 claimId, bytes calldata record) external nonReentrant {
        Attest storage a = _attests[attestId];
        if (a.attester == address(0) || a.closed) revert AlreadyDone();
        (uint64 lockId, uint32 asset, uint64 value, address to) = _readLock(record);
        if (lockId != a.lockId) revert WrongRecord();
        _requireCarried(claimId, record);
        if (a.prev != 0 && !_attests[a.prev].closed) revert NotInOrder();
        Mark storage m = _marks[lockId];
        bool counts = keccak256(record) == a.recordHash && !m.issued;
        m.settled += 1;
        bool last = m.settled == m.attests && block.timestamp >= m.firstAt + R.FAST_OPEN_WINDOW;
        a.closed = true;
        emit AttestSettled(attestId, claimId, counts);

        VaultReceipt r = _receipt(asset);
        uint64 amount = R.u64(record, 14);
        uint64 fastFee = R.u64(record, 62);
        uint64 share = R.fastShare(fastFee, R.u64(record, 70), a.attestedAt);
        if (counts) {
            m.issued = true;
            // Not burned: its collateral back and its share. Burned: the
            // receipt it minted was burned, so it receives one.
            uint256 toAttester = a.burned ? uint256(amount) + share : uint256(a.collateral) + share;
            uint256 minted = a.burned ? uint256(amount) + share : share;
            if (minted != 0) r.mint(address(this), minted);
            credit[a.attester][asset] += toAttester;
            if (fastFee > share) r.mint(to, fastFee - share);
            return;
        }
        // A wrong attest pays in its own asset, which may not be the lock's.
        if (!a.burned) {
            _receipt(a.asset).burn(a.amount);
            credit[msg.sender][a.asset] += a.collateral - a.amount;
        }
        // The true receipt, in the lock's asset; when the attest named
        // another, its recipient issues it.
        if (!m.issued && last && a.asset == asset) {
            m.issued = true;
            r.mint(to, value);
        }
    }

    /// @notice The core takes receipts from an operator for its bond.
    function take(uint32 asset, address from, uint256 amount) external onlyCore {
        VaultReceipt r = _receipt(asset);
        r.burnFrom(from, amount);
        r.mint(msg.sender, amount);
    }

    function withdrawCredit(uint32 asset) external nonReentrant {
        uint256 amount = credit[msg.sender][asset];
        if (amount == 0) revert ZeroAmount();
        credit[msg.sender][asset] = 0;
        emit CreditWithdrawn(msg.sender, asset, amount);
        _receipt(asset).transfer(msg.sender, amount);
    }

    /// @dev A LOCK record of a lock on Solana for a real Ethereum address,
    /// whose receipt exists: its number, asset, value (amount and fast fee)
    /// and recipient.
    function _readLock(bytes calldata record) private view returns (uint64 lockId, uint32 asset, uint64 value, address to) {
        if (record.length != R.LOCK_LEN || uint8(record[0]) != R.LOCK || uint8(record[1]) != peer) revert WrongRecord();
        asset = R.u32(record, 2);
        lockId = R.u64(record, 6);
        value = R.u64(record, 14) + R.u64(record, 62);
        to = R.addressOf(R.b32(record, 22));
        if (to == address(0)) revert ZeroAddress();
    }

    function _requireCarried(uint256 claimId, bytes calldata record) private view {
        if (!core.claimAccepted(claimId) || !core.carries(claimId, keccak256(record))) revert NotAccepted();
    }
}
