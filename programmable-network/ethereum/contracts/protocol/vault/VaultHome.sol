// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

import {ReentrancyGuard} from "@openzeppelin/contracts/utils/ReentrancyGuard.sol";
import {IERC20} from "@openzeppelin/contracts/token/ERC20/IERC20.sol";
import {IERC20Metadata} from "@openzeppelin/contracts/token/ERC20/extensions/IERC20Metadata.sol";
import {SafeERC20} from "@openzeppelin/contracts/token/ERC20/utils/SafeERC20.sol";

import {VaultRecords as R, IVaultCore} from "./VaultRecords.sol";

/// @title VaultHome
/// @notice The vault's part for the assets whose home is Ethereum (section
/// 11.9): ETH, asset 0, and any ERC-20 token anyone registers (D128). It
/// holds what is locked for receipts on Solana, the backing of those
/// receipts, and pays the burns of them that the core's claims accept. Made
/// by the core, which alone calls its `onlyCore` functions.
///
/// Money is credited, and the address withdraws it.
contract VaultHome is ReentrancyGuard {
    using SafeERC20 for IERC20;

    struct Asset {
        address token; // zero for ETH
        uint8 decimals;
        /// The receipt's decimals, at most 9: records count in its unit.
        uint8 recordDecimals;
        /// 10 ** (decimals - recordDecimals): native units per record unit.
        uint256 unit;
    }

    struct Lock {
        address owner;
        uint32 asset;
        uint64 amount;
        uint64 fee;
        /// For the attester of the fast path, or the recipient (D122).
        uint64 fastFee;
        /// Its block's time, in the LOCK record (D124).
        uint64 lockedAt;
        bytes32 recipient; // on Solana
        bool feePaid;
        bool returned;
    }

    /// A burn on Solana paid at once by an attester, under the record it
    /// stated (section 11.7).
    struct FastPay {
        address attester;
        uint64 paidAt;
    }

    IVaultCore public immutable core;

    Asset[] private _assets;
    mapping(address => uint32) public assetOfToken; // asset number + 1
    uint64 public lockCount;
    mapping(uint64 => Lock) private _locks;
    /// @notice What backs each asset's receipts, in record units: the locks
    /// not returned, less what was paid out, plus the backing share of
    /// slashes.
    mapping(uint32 => uint256) public reserve;
    /// @notice Burn numbers on Solana paid here.
    mapping(uint64 => bool) public requestPaid;
    mapping(uint64 => mapping(bytes32 => FastPay)) private _fastPays;
    /// @notice Native units each address can withdraw, by asset.
    mapping(address => mapping(uint32 => uint256)) public credit;

    event AssetRegistered(uint32 indexed asset, address indexed token, uint8 decimals, uint8 recordDecimals);
    event Locked(
        uint64 indexed lockId,
        uint32 indexed asset,
        address indexed owner,
        uint64 amount,
        uint64 fee,
        uint64 fastFee,
        uint64 lockedAt,
        bytes32 recipient
    );
    event FeeEarned(uint64 indexed lockId, address indexed operator, uint64 fee);
    event FastPaid(uint64 indexed requestId, address indexed attester, address to, uint32 asset, uint64 amount);
    event RequestPaid(uint64 indexed requestId, uint256 indexed claimId, address to, uint64 amount);
    event LockReturned(uint64 indexed lockId, address owner, uint64 amount);
    event CreditWithdrawn(address indexed to, uint32 indexed asset, uint256 amount);

    error NotCore();
    error UnknownAsset();
    error AssetExists();
    error ZeroAmount();
    error ZeroAddress();
    error TooLarge();
    error WrongValue();
    error NotAccepted();
    error WrongRecord();
    error AlreadyDone();
    error AlreadyPaid();
    error Underfunded();
    error TransferFailed();

    modifier onlyCore() {
        if (msg.sender != address(core)) revert NotCore();
        _;
    }

    /// @notice This network's number, and its peer's (D133), set once.
    uint8 public here;
    uint8 public peer;

    /// @param coin_ The network's coin (D136): zero for a native coin, which
    /// is asset 0 as ETH is; else the token that is asset 0.
    /// @param nativeDecimals_ The native coin's decimals as a contract sees
    /// them: 18, or 8 on Hedera (D139). Unused when the coin is a token.
    constructor(IVaultCore core_, uint8 here_, uint8 peer_, address coin_, uint8 nativeDecimals_) {
        core = core_;
        here = here_;
        peer = peer_;
        if (coin_ == address(0)) {
            uint8 rd = nativeDecimals_ > R.MAX_DECIMALS ? R.MAX_DECIMALS : nativeDecimals_;
            _assets.push(Asset({token: address(0), decimals: nativeDecimals_, recordDecimals: rd, unit: 10 ** (nativeDecimals_ - rd)}));
        } else {
            uint8 d = IERC20Metadata(coin_).decimals();
            uint8 rd = d > R.MAX_DECIMALS ? R.MAX_DECIMALS : d;
            _assets.push(Asset({token: coin_, decimals: d, recordDecimals: rd, unit: 10 ** (d - rd)}));
            assetOfToken[coin_] = 1;
        }
        Asset storage c = _assets[0];
        emit AssetRegistered(0, c.token, c.decimals, c.recordDecimals);
    }

    // ------------------------------------------------------------------
    // Assets (D128)
    // ------------------------------------------------------------------

    /// @notice Registers an ERC-20 token: it gets the next number. Anyone may
    /// call, once per token. Its receipt on Solana is made by an accepted
    /// claim carrying its ASSET record.
    function registerAsset(address token) external returns (uint32 asset) {
        if (token == address(0)) revert ZeroAddress();
        if (assetOfToken[token] != 0) revert AssetExists();
        uint8 decimals = IERC20Metadata(token).decimals();
        uint8 recordDecimals = decimals > R.MAX_DECIMALS ? R.MAX_DECIMALS : decimals;
        asset = uint32(_assets.length);
        _assets.push(Asset({token: token, decimals: decimals, recordDecimals: recordDecimals, unit: 10 ** (decimals - recordDecimals)}));
        assetOfToken[token] = asset + 1;
        emit AssetRegistered(asset, token, decimals, recordDecimals);
    }

    function assetCount() external view returns (uint32) {
        return uint32(_assets.length);
    }

    function getAsset(uint32 asset) public view returns (Asset memory) {
        if (asset >= _assets.length) revert UnknownAsset();
        return _assets[asset];
    }

    /// @notice The hash of the ASSET record that states `asset`, zero if it
    /// does not exist.
    function assetRecordHash(uint32 asset) external view returns (bytes32) {
        if (asset >= _assets.length) return bytes32(0);
        Asset storage a = _assets[asset];
        return keccak256(R.assetRecord(here, asset, bytes32(uint256(uint160(a.token))), a.recordDecimals));
    }

    // ------------------------------------------------------------------
    // Locks (section 11.5, 11.9)
    // ------------------------------------------------------------------

    /// @notice Locks an asset for its receipt on Solana. `amount`, `fee` and
    /// `fastFee` are in record units; ETH is sent with the call, a token
    /// taken from the sender. The fee goes to the first operator whose
    /// message carrying the lock is judged true (D113); the fast fee is
    /// minted on Solana to the attester, or the recipient (D122). A token
    /// that takes a fee on transfer locks what arrived (D128).
    function lock(uint32 asset, bytes32 recipient, uint64 amount, uint64 fee, uint64 fastFee)
        external
        payable
        nonReentrant
        returns (uint64 lockId)
    {
        Asset memory a = getAsset(asset);
        // An address on the peer, or its receipt could never be issued.
        if (!R.addressOn(recipient, peer)) revert ZeroAddress();
        uint256 total = uint256(amount) + fee + fastFee;
        if (amount == 0) revert ZeroAmount();
        if (total > type(uint64).max) revert TooLarge();
        if (a.token == address(0)) {
            if (msg.value != total * a.unit) revert WrongValue();
        } else {
            if (msg.value != 0) revert WrongValue();
            IERC20 t = IERC20(a.token);
            uint256 before = t.balanceOf(address(this));
            t.safeTransferFrom(msg.sender, address(this), total * a.unit);
            uint256 arrived = (t.balanceOf(address(this)) - before) / a.unit;
            if (arrived < uint256(fee) + fastFee + 1) revert ZeroAmount();
            amount = uint64(arrived - fee - fastFee);
        }
        lockId = ++lockCount;
        _locks[lockId] = Lock({
            owner: msg.sender,
            asset: asset,
            amount: amount,
            fee: fee,
            fastFee: fastFee,
            lockedAt: uint64(block.timestamp),
            recipient: recipient,
            feePaid: false,
            returned: false
        });
        reserve[asset] += uint256(amount) + fastFee;
        emit Locked(lockId, asset, msg.sender, amount, fee, fastFee, uint64(block.timestamp), recipient);
    }

    function getLock(uint64 lockId) external view returns (Lock memory) {
        return _locks[lockId];
    }

    /// @notice The hash of the LOCK record that states lock `lockId`, zero if
    /// it does not exist. Returned or not: Solana issues nothing for a lock it
    /// marked never usable (section 11.5).
    function lockRecordHash(uint64 lockId) external view returns (bytes32) {
        Lock storage l = _locks[lockId];
        if (l.owner == address(0)) return bytes32(0);
        return keccak256(R.lock(here, l.asset, lockId, l.amount, l.recipient, l.fee, l.fastFee, l.lockedAt));
    }

    /// @notice The asset and what lock `lockId` backs: its amount and fast
    /// fee. Zero amount when it does not exist.
    function lockValue(uint64 lockId) external view returns (uint32 asset, uint64 value) {
        Lock storage l = _locks[lockId];
        if (l.owner == address(0)) return (0, 0);
        return (l.asset, l.amount + l.fastFee);
    }

    /// @notice The core pays a lock's fee to the operator of the first true
    /// message carrying it.
    function earnFee(uint64 lockId, address operator) external onlyCore {
        Lock storage l = _locks[lockId];
        if (l.feePaid) return;
        l.feePaid = true;
        emit FeeEarned(lockId, operator, l.fee);
        if (l.fee != 0) credit[operator][l.asset] += uint256(l.fee) * _assets[l.asset].unit;
    }

    /// @notice Returns a lock whose CANCEL an accepted claim carries, with its
    /// fee if no message earned it. Anyone may call.
    function returnLock(uint256 claimId, bytes calldata record) external nonReentrant {
        if (record.length != R.CANCEL_LEN || uint8(record[0]) != R.CANCEL || uint8(record[1]) != peer) revert WrongRecord();
        _requireCarried(claimId, record);
        uint64 lockId = R.u64(record, 2);
        Lock storage l = _locks[lockId];
        if (l.owner == address(0)) revert WrongRecord();
        if (l.returned) revert AlreadyDone();
        uint256 value = uint256(l.amount) + l.fastFee;
        if (value > reserve[l.asset]) revert Underfunded();
        l.returned = true;
        reserve[l.asset] -= value;
        if (!l.feePaid) {
            // The fee is settled: no later message can earn it.
            l.feePaid = true;
            value += l.fee;
        }
        emit LockReturned(lockId, l.owner, uint64(value));
        credit[l.owner][l.asset] += value * _assets[l.asset].unit;
    }

    // ------------------------------------------------------------------
    // Burns on Solana of receipts of these assets
    // ------------------------------------------------------------------

    /// @notice Pays at once a burn on Solana of a receipt of an asset here,
    /// from the sender's own money, to the address the burn names (section
    /// 11.7). The sender states the burn's REQUEST record. When a claim
    /// carrying the same record is accepted, the vault pays it the amount and
    /// its share of the fast fee (D124). Stated wrongly, the sender's money is
    /// lost: it checks the burn on Solana first. Anyone may call; each record
    /// stated once, so nobody blocks the true one (D126).
    function fastPay(bytes calldata record) external payable nonReentrant {
        (uint64 requestId, uint32 asset, uint64 amount, address to) = _readRequest(record);
        FastPay storage f = _fastPays[requestId][keccak256(record)];
        if (requestPaid[requestId] || f.attester != address(0)) revert AlreadyPaid();
        f.attester = msg.sender;
        f.paidAt = uint64(block.timestamp);
        Asset memory a = _assets[asset];
        uint256 native = uint256(amount) * a.unit;
        emit FastPaid(requestId, msg.sender, to, asset, amount);
        if (a.token == address(0)) {
            if (msg.value != native) revert WrongValue();
            (bool ok, ) = to.call{value: native}("");
            if (!ok) revert TransferFailed();
        } else {
            if (msg.value != 0) revert WrongValue();
            IERC20(a.token).safeTransferFrom(msg.sender, to, native);
        }
    }

    /// @notice Who paid burn `requestId` at once under `record`, and when.
    function getFastPay(uint64 requestId, bytes calldata record) external view returns (FastPay memory) {
        return _fastPays[requestId][keccak256(record)];
    }

    /// @notice Pays a burn carried by an accepted claim, once, with its fast
    /// fee: to the attester that paid it at once with the same record, its
    /// amount and share of the fast fee, the rest to the burn's address; or
    /// else all to the address. Anyone may call.
    function payRequest(uint256 claimId, bytes calldata record) external nonReentrant {
        (uint64 requestId, uint32 asset, uint64 amount, address to) = _readRequest(record);
        _requireCarried(claimId, record);
        if (requestPaid[requestId]) revert AlreadyDone();
        uint64 fastFee = R.u64(record, 62);
        uint256 value = uint256(amount) + fastFee;
        if (value > reserve[asset]) revert Underfunded();
        requestPaid[requestId] = true;
        reserve[asset] -= value;
        uint256 unit = _assets[asset].unit;
        FastPay storage f = _fastPays[requestId][keccak256(record)];
        if (f.attester == address(0)) {
            emit RequestPaid(requestId, claimId, to, uint64(value));
            credit[to][asset] += value * unit;
        } else {
            uint64 share = R.fastShare(fastFee, R.u64(record, 70), f.paidAt);
            emit RequestPaid(requestId, claimId, f.attester, amount + share);
            credit[f.attester][asset] += (uint256(amount) + share) * unit;
            if (fastFee > share) credit[to][asset] += uint256(fastFee - share) * unit;
        }
    }

    // ------------------------------------------------------------------
    // Backing and credit
    // ------------------------------------------------------------------

    /// @notice The core adds a slashed bond's backing share, already sent
    /// here (D109).
    function addBacking(uint32 asset, uint256 amount) external onlyCore {
        reserve[asset] += amount;
    }

    function withdrawCredit(uint32 asset) external nonReentrant {
        uint256 amount = credit[msg.sender][asset];
        if (amount == 0) revert ZeroAmount();
        credit[msg.sender][asset] = 0;
        emit CreditWithdrawn(msg.sender, asset, amount);
        address token = _assets[asset].token;
        if (token == address(0)) {
            (bool ok, ) = msg.sender.call{value: amount}("");
            if (!ok) revert TransferFailed();
        } else {
            IERC20(token).safeTransfer(msg.sender, amount);
        }
    }

    /// @dev A REQUEST record of a burn on Solana of a receipt of an asset
    /// here, paying a real Ethereum address: the core judges any other
    /// false.
    function _readRequest(bytes calldata record)
        private
        view
        returns (uint64 requestId, uint32 asset, uint64 amount, address to)
    {
        if (record.length != R.REQUEST_LEN || uint8(record[0]) != R.REQUEST || uint8(record[1]) != peer) revert WrongRecord();
        asset = R.u32(record, 2);
        if (asset >= _assets.length) revert UnknownAsset();
        requestId = R.u64(record, 6);
        amount = R.u64(record, 14);
        to = R.addressOf(R.b32(record, 22));
        if (to == address(0)) revert ZeroAddress();
    }

    function _requireCarried(uint256 claimId, bytes calldata record) private view {
        if (!core.claimAccepted(claimId) || !core.carries(claimId, keccak256(record))) revert NotAccepted();
    }

    /// @dev ETH comes only from the core: a slashed ETH bond's backing.
    receive() external payable {
        if (msg.sender != address(core)) revert NotCore();
    }
}
