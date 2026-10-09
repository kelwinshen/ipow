// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

import {ERC20} from "@openzeppelin/contracts/token/ERC20/ERC20.sol";
import {IERC20} from "@openzeppelin/contracts/token/ERC20/IERC20.sol";
import {SafeERC20} from "@openzeppelin/contracts/token/ERC20/utils/SafeERC20.sol";
import {ReentrancyGuard} from "@openzeppelin/contracts/utils/ReentrancyGuard.sol";

/// @dev What `BetaBaskets` tells the token it is creating: its name and
/// symbol, read by the token's constructor. They are not constructor
/// arguments, so that every token's init code is the same and `CREATE2` by
/// the basket's key keeps each basket at an address fixed by its creator and
/// number, and refuses a second one with the same id (E4).
interface IBasketNaming {
    function creatingName() external view returns (string memory);
    function creatingSymbol() external view returns (string memory);
}

/// @dev The vault's receipts on this network (protocol/vault/VaultReceipts.sol):
/// the receipt of the peer's asset `n`, or zero while it is not made yet.
interface IVaultReceipts {
    function receiptOf(uint32 asset) external view returns (address);
}

/// @title BetaBasket
/// @notice One basket's BETA token, which also holds the basket's parts:
/// each basket's backing is its own balance, so one issuer's action on a
/// part falls on that basket only. Only `BetaBaskets`, which made it, mints,
/// burns and pays out. Named by its creator (E4).
contract BetaBasket is ERC20 {
    using SafeERC20 for IERC20;

    address public immutable app;

    error NotApp();
    error PaymentFailed();

    constructor() ERC20(IBasketNaming(msg.sender).creatingName(), IBasketNaming(msg.sender).creatingSymbol()) {
        app = msg.sender;
    }

    modifier onlyApp() {
        if (msg.sender != app) revert NotApp();
        _;
    }

    /// @notice 9, as on Solana: a part's amount is what one whole BETA holds.
    function decimals() public pure override returns (uint8) {
        return 9;
    }

    function mint(address to, uint256 amount) external onlyApp {
        _mint(to, amount);
    }

    function burnFrom(address from, uint256 amount) external onlyApp {
        _burn(from, amount);
    }

    /// @notice Pays `amount` of a part: ETH when `token` is zero. Paying the
    /// basket itself moves nothing.
    function pay(address token, address to, uint256 amount) external onlyApp {
        if (amount == 0 || to == address(this)) return;
        if (token == address(0)) {
            (bool ok, ) = to.call{value: amount}("");
            if (!ok) revert PaymentFailed();
        } else {
            IERC20(token).safeTransfer(to, amount);
        }
    }

    /// @notice ETH a part holds. Anyone sending ETH adds to the backing.
    receive() external payable {}
}

/// @title BetaBaskets
/// @notice BETA on Ethereum: a token backed by a fixed basket of up to 8
/// parts, one per network, all held on this network, for example 1 ETH and 1
/// vSOL, the vault's receipt for SOL on Solana (spec section 11.9). The same
/// rules as `programs/applications/beta-basket` on Solana; design in
/// docs/specs/ipow-beta-app.md.
///
/// Anyone creates a basket, with no approval, and sets a creator fee on mint
/// and on burn, each at most 1%, fixed at creation and paid as a share of
/// each part. The creator may hand the fee to another address, never change
/// the basket or the fee.
///
/// A part may be a token whose issuer controls it (E3). A burn pays each
/// part on its own, and a share of what the basket actually holds, so one
/// issuer's action falls only on its own part, and equally on every holder.
/// Ethereum cannot read an issuer's powers from a token, so none are
/// recorded here. No key can change the contract (D59).
///
/// The creator names the basket's token and gives it a metadata URI (E4):
/// the name and symbol are the token's own, read by wallets; the URI points
/// at JSON with a description and an image, kept here for apps. Fixed at
/// creation, like the parts.
///
/// A part may be another network's asset, held here as the vault's receipt
/// of it, named before the vault has made that receipt (E5): the part is
/// then the receipt of the peer's asset `asset` in the `receipts` contract,
/// resolved when it is first used. Nothing can be minted while a part's
/// receipt is not made: the asset must be bridged here first.
contract BetaBaskets is ReentrancyGuard, IBasketNaming {
    using SafeERC20 for IERC20;

    uint256 public constant MAX_PARTS = 8;
    /// @notice The name, symbol and metadata URI of a basket's token: at
    /// most these many bytes, and a name and symbol of at least one.
    uint256 public constant MAX_NAME = 64;
    uint256 public constant MAX_SYMBOL = 16;
    uint256 public constant MAX_URI = 2048;
    /// @notice Each fee is at most 1% (100 hundredths of a percent).
    uint16 public constant MAX_FEE_BPS = 100;
    uint256 private constant BPS = 10_000;
    /// @notice One whole BETA, in its smallest unit.
    uint256 public constant ONE = 1e9;

    struct Part {
        /// The token; zero for ETH, or for a receipt not resolved yet (E5).
        address token;
        /// The vault's receipts contract and the peer's asset number, for a
        /// part that is a receipt (E5); zero otherwise.
        address receipts;
        uint32 asset;
        /// What one whole BETA holds of it when the basket is new, in its
        /// smallest unit.
        uint256 amount;
        /// Set aside, not yet collected: what burners who deferred this part
        /// are owed, and the fees of its receivers.
        uint256 owed;
    }

    /// @notice A part as the creator names it: a token here (zero for ETH),
    /// or the vault's receipt of the peer's asset `asset` in `receipts`.
    struct PartSpec {
        address token;
        address receipts;
        uint32 asset;
        uint256 amount;
    }

    struct Basket {
        address creator;
        uint64 id;
        /// Receives the fees; the only thing that can change, by itself.
        address feeTo;
        uint16 mintFeeBps;
        uint16 burnFeeBps;
        BetaBasket beta;
        Part[] parts;
        /// The token's metadata (E4): JSON with a description and an image.
        string uri;
    }

    mapping(bytes32 => Basket) private _baskets;
    /// @dev The name and symbol of the token being created, for its
    /// constructor; empty between creations.
    string private _creatingName;
    string private _creatingSymbol;
    /// @notice What is owed to a holder who deferred part `index` of a basket.
    mapping(bytes32 => mapping(address => mapping(uint8 => uint256))) public owed;
    /// @notice The fees of part `index` of a basket a receiver has earned and
    /// not collected. Fees are set aside in the basket, not sent: a receiver
    /// that refuses a payment (an address a token blocks, a contract that
    /// takes no ETH) can then never stop a mint, a burn or a collection.
    mapping(bytes32 => mapping(address => mapping(uint8 => uint256))) public fees;

    event BasketCreated(bytes32 indexed key, address indexed creator, uint64 id, address beta, string name, string symbol, string uri, uint16 mintFeeBps, uint16 burnFeeBps);
    event Minted(bytes32 indexed key, address indexed user, uint256 amount);
    event Burned(bytes32 indexed key, address indexed user, uint256 amount, uint8 deferred);
    event Collected(bytes32 indexed key, address indexed user, uint8 index, uint256 amount);
    event FeesCollected(bytes32 indexed key, address indexed receiver, uint8 index, uint256 amount);
    event FeeToChanged(bytes32 indexed key, address feeTo);

    error BadParts();
    error BadMetadata();
    /// @notice A part is a receipt the vault has not made yet: bridge the
    /// asset here first.
    error PartNotMadeYet();
    error FeeTooHigh();
    error UnknownBasket();
    error ZeroAmount();
    error FirstMintNotWhole();
    error PartEmpty();
    error NothingOwed();
    error NotFeeReceiver();
    error WrongValue();
    error WrongDefer();
    error TransferFeeToken();
    error PaymentFailed();
    error ZeroAddress();

    // ------------------------------------------------------------------
    // Reading
    // ------------------------------------------------------------------

    function basketKey(address creator, uint64 id) public pure returns (bytes32) {
        return keccak256(abi.encode(creator, id));
    }

    /// @inheritdoc IBasketNaming
    function creatingName() external view returns (string memory) {
        return _creatingName;
    }

    /// @inheritdoc IBasketNaming
    function creatingSymbol() external view returns (string memory) {
        return _creatingSymbol;
    }

    function getBasket(bytes32 key) external view returns (Basket memory) {
        return _baskets[key];
    }

    /// @notice What the basket holds of part `i` for its holders: its
    /// balance, less what is set aside. While a burn pays out, a payee called
    /// back sees the supply already lowered and the parts not all paid: this
    /// and `mintCost` are not a price to trust from inside such a call.
    function held(bytes32 key, uint256 i) public view returns (uint256) {
        Basket storage b = _basket(key);
        if (i >= b.parts.length) revert BadParts();
        return _held(b, b.parts[i]);
    }

    /// @notice Part `i`'s token now: zero for ETH, and for a receipt the
    /// vault has not made yet (E5).
    function partToken(bytes32 key, uint256 i) external view returns (address) {
        Basket storage b = _basket(key);
        if (i >= b.parts.length) revert BadParts();
        Part storage p = b.parts[i];
        return p.receipts == address(0) ? p.token : _receiptOf(p);
    }

    /// @notice What minting `amount` BETA takes of each part, before the
    /// creator's fee: for a client to approve and send.
    function mintCost(bytes32 key, uint256 amount) external view returns (uint256[] memory need, uint256[] memory fee) {
        Basket storage b = _basket(key);
        uint256 supply = b.beta.totalSupply();
        need = new uint256[](b.parts.length);
        fee = new uint256[](b.parts.length);
        for (uint256 i; i < b.parts.length; i++) {
            need[i] = _need(b, b.parts[i], amount, supply);
            fee[i] = _fee(need[i], b.mintFeeBps);
        }
    }

    // ------------------------------------------------------------------
    // Baskets
    // ------------------------------------------------------------------

    /// @notice Creates basket `id` of the caller, and its BETA token named
    /// `name` (`symbol`) with metadata at `uri`, at an address fixed by the
    /// caller and `id`. Each basket once. A part may be a receipt not made
    /// yet (E5): its `receipts` contract and `asset` are kept, its token
    /// resolved when first used.
    function createBasket(
        uint64 id,
        string calldata name,
        string calldata symbol,
        string calldata uri,
        PartSpec[] calldata specs,
        uint16 mintFeeBps,
        uint16 burnFeeBps
    ) external returns (bytes32 key, address beta) {
        uint256 n = specs.length;
        if (n == 0 || n > MAX_PARTS) revert BadParts();
        if (bytes(name).length == 0 || bytes(name).length > MAX_NAME || bytes(symbol).length == 0 || bytes(symbol).length > MAX_SYMBOL || bytes(uri).length > MAX_URI)
            revert BadMetadata();
        if (mintFeeBps > MAX_FEE_BPS || burnFeeBps > MAX_FEE_BPS) revert FeeTooHigh();
        key = basketKey(msg.sender, id);
        Basket storage b = _baskets[key];
        for (uint256 i; i < n; i++) {
            PartSpec calldata sp = specs[i];
            if (sp.amount == 0) revert BadParts();
            address tok = sp.token;
            if (sp.receipts != address(0)) {
                // A receipt, named by its asset; the contract making it must
                // exist. Resolved now when made already.
                if (tok != address(0) || sp.receipts.code.length == 0) revert BadParts();
                tok = IVaultReceipts(sp.receipts).receiptOf(sp.asset);
                if (tok != address(0) && tok.code.length == 0) revert BadParts();
            } else if (tok != address(0) && tok.code.length == 0) {
                // A token is a contract: an account with no code would take
                // any transfer as done.
                revert BadParts();
            }
            for (uint256 j; j < i; j++) {
                Part storage q = b.parts[j];
                bool sameReceipt = sp.receipts != address(0) && q.receipts == sp.receipts && q.asset == sp.asset;
                bool sameToken = tok != address(0) ? q.token == tok : (sp.receipts == address(0) && q.receipts == address(0) && q.token == address(0));
                if (sameReceipt || sameToken) revert BadParts();
            }
            b.parts.push(Part({token: tok, receipts: sp.receipts, asset: sp.asset, amount: sp.amount, owed: 0}));
        }
        // CREATE2 by the key, with the same init code for every token: a
        // second basket with the same id fails here, whatever its name.
        _creatingName = name;
        _creatingSymbol = symbol;
        BetaBasket token = new BetaBasket{salt: key}();
        delete _creatingName;
        delete _creatingSymbol;
        b.creator = msg.sender;
        b.id = id;
        b.feeTo = msg.sender;
        b.mintFeeBps = mintFeeBps;
        b.burnFeeBps = burnFeeBps;
        b.beta = token;
        b.uri = uri;
        beta = address(token);
        emit BasketCreated(key, msg.sender, id, beta, name, symbol, uri, mintFeeBps, burnFeeBps);
    }

    /// @notice Mints `amount` BETA (smallest units) to the caller. The first
    /// mint deposits each part's amount per BETA; after that, each part in
    /// proportion to what the basket holds per BETA, rounded up, so new
    /// holders never pay for a past loss. The creator's fee, a share of each
    /// part, is paid on top. Tokens are taken with the caller's approval; ETH
    /// is sent with the call, and any more than needed is sent back.
    function mint(bytes32 key, uint256 amount) external payable nonReentrant {
        if (amount == 0) revert ZeroAmount();
        Basket storage b = _basket(key);
        uint256 supply = b.beta.totalSupply();
        // A whole number of BETA first: a dust first mint would set the
        // basket's mix for every later minter.
        if (supply == 0 && amount % ONE != 0) revert FirstMintNotWhole();
        uint256 n = b.parts.length;
        uint256[] memory need = new uint256[](n);
        // Every amount from the holdings before anything moves.
        uint256 eth;
        for (uint256 i; i < n; i++) {
            _resolve(b.parts[i]);
            need[i] = _need(b, b.parts[i], amount, supply);
            if (_isEth(b.parts[i])) eth = need[i] + _fee(need[i], b.mintFeeBps);
        }
        if (msg.value < eth) revert WrongValue();
        for (uint256 i; i < n; i++) {
            Part storage p = b.parts[i];
            uint256 fee = _fee(need[i], b.mintFeeBps);
            uint256 total = need[i] + fee;
            if (_isEth(p)) {
                _sendEth(address(b.beta), total);
            } else {
                IERC20 t = IERC20(p.token);
                // What arrived: a token that takes a fee on transfer would
                // leave the basket short, so it is refused.
                uint256 before = t.balanceOf(address(b.beta));
                t.safeTransferFrom(msg.sender, address(b.beta), total);
                if (t.balanceOf(address(b.beta)) - before != total) revert TransferFeeToken();
            }
            _setFeeAside(key, b, i, fee);
        }
        if (msg.value > eth) _sendEth(msg.sender, msg.value - eth);
        b.beta.mint(msg.sender, amount);
        emit Minted(key, msg.sender, amount);
    }

    /// @notice Burns `amount` BETA of the caller, who receives of each part
    /// its share of what the basket holds, rounded down, less the creator's
    /// fee. A part whose bit is set in `defer` is not moved now: it is owed to
    /// the caller, and collected later, the fee taken then. A part that
    /// cannot move (frozen, paused, an address not allowed) fails the burn
    /// unless deferred. What is paid now goes to `to`.
    function burn(bytes32 key, uint256 amount, uint8 defer, address to) external nonReentrant {
        if (amount == 0) revert ZeroAmount();
        if (to == address(0)) revert ZeroAddress();
        Basket storage b = _basket(key);
        uint256 n = b.parts.length;
        // Only parts the basket has may be deferred (8 parts use every bit).
        if (n < 8 && defer >> n != 0) revert WrongDefer();
        uint256 supply = b.beta.totalSupply();
        uint256[] memory out = new uint256[](n);
        for (uint256 i; i < n; i++) {
            _resolve(b.parts[i]);
            out[i] = (amount * _held(b, b.parts[i])) / supply;
        }
        b.beta.burnFrom(msg.sender, amount);
        for (uint256 i; i < n; i++) {
            Part storage p = b.parts[i];
            if (defer & (1 << i) != 0) {
                owed[key][msg.sender][uint8(i)] += out[i];
                p.owed += out[i];
            } else {
                uint256 fee = _fee(out[i], b.burnFeeBps);
                _setFeeAside(key, b, i, fee);
                b.beta.pay(p.token, to, out[i] - fee);
            }
        }
        emit Burned(key, msg.sender, amount, defer);
    }

    /// @notice Pays what the caller is owed of part `index`, less the
    /// creator's burn fee, taken here rather than when the part was deferred,
    /// to `to`: a holder whose own address a token blocks collects elsewhere.
    function collectOwed(bytes32 key, uint8 index, address to) external nonReentrant {
        if (to == address(0)) revert ZeroAddress();
        Basket storage b = _basket(key);
        if (index >= b.parts.length) revert BadParts();
        uint256 amount = owed[key][msg.sender][index];
        if (amount == 0) revert NothingOwed();
        owed[key][msg.sender][index] = 0;
        Part storage p = b.parts[index];
        _resolve(p);
        p.owed -= amount;
        uint256 fee = _fee(amount, b.burnFeeBps);
        _setFeeAside(key, b, index, fee);
        b.beta.pay(p.token, to, amount - fee);
        emit Collected(key, msg.sender, index, amount);
    }

    /// @notice Pays the caller its fees of part `index`, to `to`.
    function collectFees(bytes32 key, uint8 index, address to) external nonReentrant {
        if (to == address(0)) revert ZeroAddress();
        Basket storage b = _basket(key);
        if (index >= b.parts.length) revert BadParts();
        uint256 amount = fees[key][msg.sender][index];
        if (amount == 0) revert NothingOwed();
        fees[key][msg.sender][index] = 0;
        Part storage p = b.parts[index];
        _resolve(p);
        p.owed -= amount;
        b.beta.pay(p.token, to, amount);
        emit FeesCollected(key, msg.sender, index, amount);
    }

    /// @notice Hands the fee to another address. Only the one receiving it
    /// may. Handing it to the basket's own BETA token sends every later fee
    /// into the backing, for good: the token can never call this.
    function setFeeTo(bytes32 key, address feeTo) external {
        Basket storage b = _basket(key);
        if (msg.sender != b.feeTo) revert NotFeeReceiver();
        // Fees for address zero or for this contract could never be collected.
        if (feeTo == address(0) || feeTo == address(this)) revert ZeroAddress();
        b.feeTo = feeTo;
        emit FeeToChanged(key, feeTo);
    }

    // ------------------------------------------------------------------
    // Inside
    // ------------------------------------------------------------------

    function _basket(bytes32 key) private view returns (Basket storage b) {
        b = _baskets[key];
        if (address(b.beta) == address(0)) revert UnknownBasket();
    }

    /// @dev Sets a fee aside in the basket for its receiver. Handed to the
    /// basket's own token, it is not set aside: it backs BETA.
    function _setFeeAside(bytes32 key, Basket storage b, uint256 i, uint256 fee) private {
        if (fee == 0 || b.feeTo == address(b.beta)) return;
        fees[key][b.feeTo][uint8(i)] += fee;
        b.parts[i].owed += fee;
    }

    /// @dev Whether a part is ETH: neither a token nor a receipt.
    function _isEth(Part storage p) private view returns (bool) {
        return p.token == address(0) && p.receipts == address(0);
    }

    /// @dev A receipt part's token as the vault has it now, or zero (E5).
    function _receiptOf(Part storage p) private view returns (address) {
        return p.token != address(0) ? p.token : IVaultReceipts(p.receipts).receiptOf(p.asset);
    }

    /// @dev Keeps a receipt part's token once the vault has made it; refuses
    /// to go on before (E5).
    function _resolve(Part storage p) private {
        if (p.receipts == address(0) || p.token != address(0)) return;
        address token = IVaultReceipts(p.receipts).receiptOf(p.asset);
        if (token == address(0)) revert PartNotMadeYet();
        // The vault's receipts are contracts; a receipts contract of the
        // creator's own choosing naming an account with no code is refused
        // rather than kept for good.
        if (token.code.length == 0) revert BadParts();
        p.token = token;
    }

    function _held(Basket storage b, Part storage p) private view returns (uint256) {
        uint256 balance;
        if (_isEth(p)) {
            balance = address(b.beta).balance;
        } else {
            address token = p.receipts == address(0) ? p.token : _receiptOf(p);
            if (token == address(0)) revert PartNotMadeYet();
            balance = IERC20(token).balanceOf(address(b.beta));
        }
        return balance > p.owed ? balance - p.owed : 0;
    }

    /// @dev What minting `amount` takes of a part, rounded up: every BETA
    /// stays fully backed. While the part holds nothing, because its issuer
    /// took it all, nothing can be minted: it would be free for new holders.
    function _need(Basket storage b, Part storage p, uint256 amount, uint256 supply) private view returns (uint256) {
        // A receipt not made yet cannot be minted, so it is not priced (E5).
        if (p.receipts != address(0) && _receiptOf(p) == address(0)) revert PartNotMadeYet();
        if (supply == 0) return _ceilDiv(amount * p.amount, ONE);
        uint256 h = _held(b, p);
        if (h == 0) revert PartEmpty();
        return _ceilDiv(amount * h, supply);
    }

    /// @dev The creator's fee on `value`, rounded up: splitting a mint or a
    /// burn into small ones never avoids it. At most `value`.
    function _fee(uint256 value, uint16 bps) private pure returns (uint256) {
        return _ceilDiv(value * bps, BPS);
    }

    function _ceilDiv(uint256 a, uint256 b) private pure returns (uint256) {
        return a == 0 ? 0 : (a - 1) / b + 1;
    }

    function _sendEth(address to, uint256 amount) private {
        if (amount == 0) return;
        (bool ok, ) = to.call{value: amount}("");
        if (!ok) revert PaymentFailed();
    }
}
