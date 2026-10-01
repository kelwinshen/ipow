// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

import {ERC20} from "@openzeppelin/contracts/token/ERC20/ERC20.sol";
import {IERC20} from "@openzeppelin/contracts/token/ERC20/IERC20.sol";
import {SafeERC20} from "@openzeppelin/contracts/token/ERC20/utils/SafeERC20.sol";
import {ReentrancyGuard} from "@openzeppelin/contracts/utils/ReentrancyGuard.sol";

/// @title BetaBasket
/// @notice One basket's BETA token, which also holds the basket's parts:
/// each basket's backing is its own balance, so one issuer's action on a
/// part falls on that basket only. Only `BetaBaskets`, which made it, mints,
/// burns and pays out.
contract BetaBasket is ERC20 {
    using SafeERC20 for IERC20;

    address public immutable app;

    error NotApp();
    error PaymentFailed();

    constructor() ERC20("BETA", "BETA") {
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
/// rules as `programs/beta-basket` on Solana; design in
/// docs/drafts/ipow-beta-app.md.
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
contract BetaBaskets is ReentrancyGuard {
    using SafeERC20 for IERC20;

    uint256 public constant MAX_PARTS = 8;
    /// @notice Each fee is at most 1% (100 hundredths of a percent).
    uint16 public constant MAX_FEE_BPS = 100;
    uint256 private constant BPS = 10_000;
    /// @notice One whole BETA, in its smallest unit.
    uint256 public constant ONE = 1e9;

    struct Part {
        /// The token; zero for ETH.
        address token;
        /// What one whole BETA holds of it when the basket is new, in its
        /// smallest unit.
        uint256 amount;
        /// Set aside, not yet collected: what burners who deferred this part
        /// are owed, and the fees of its receivers.
        uint256 owed;
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
    }

    mapping(bytes32 => Basket) private _baskets;
    /// @notice What is owed to a holder who deferred part `index` of a basket.
    mapping(bytes32 => mapping(address => mapping(uint8 => uint256))) public owed;
    /// @notice The fees of part `index` of a basket a receiver has earned and
    /// not collected. Fees are set aside in the basket, not sent: a receiver
    /// that refuses a payment (an address a token blocks, a contract that
    /// takes no ETH) can then never stop a mint, a burn or a collection.
    mapping(bytes32 => mapping(address => mapping(uint8 => uint256))) public fees;

    event BasketCreated(bytes32 indexed key, address indexed creator, uint64 id, address beta, uint16 mintFeeBps, uint16 burnFeeBps);
    event Minted(bytes32 indexed key, address indexed user, uint256 amount);
    event Burned(bytes32 indexed key, address indexed user, uint256 amount, uint8 deferred);
    event Collected(bytes32 indexed key, address indexed user, uint8 index, uint256 amount);
    event FeesCollected(bytes32 indexed key, address indexed receiver, uint8 index, uint256 amount);
    event FeeToChanged(bytes32 indexed key, address feeTo);

    error BadParts();
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

    /// @notice Creates basket `id` of the caller, and its BETA token, at an
    /// address fixed by the caller and `id`. Each basket once.
    function createBasket(
        uint64 id,
        address[] calldata tokens,
        uint256[] calldata amounts,
        uint16 mintFeeBps,
        uint16 burnFeeBps
    ) external returns (bytes32 key, address beta) {
        uint256 n = tokens.length;
        if (n == 0 || n > MAX_PARTS || amounts.length != n) revert BadParts();
        if (mintFeeBps > MAX_FEE_BPS || burnFeeBps > MAX_FEE_BPS) revert FeeTooHigh();
        key = basketKey(msg.sender, id);
        Basket storage b = _baskets[key];
        for (uint256 i; i < n; i++) {
            if (amounts[i] == 0) revert BadParts();
            for (uint256 j; j < i; j++) if (tokens[j] == tokens[i]) revert BadParts();
            // A token is a contract: an account with no code would take any
            // transfer as done.
            if (tokens[i] != address(0) && tokens[i].code.length == 0) revert BadParts();
            b.parts.push(Part({token: tokens[i], amount: amounts[i], owed: 0}));
        }
        // CREATE2 by the key: a second basket with the same id fails here.
        BetaBasket token = new BetaBasket{salt: key}();
        b.creator = msg.sender;
        b.id = id;
        b.feeTo = msg.sender;
        b.mintFeeBps = mintFeeBps;
        b.burnFeeBps = burnFeeBps;
        b.beta = token;
        beta = address(token);
        emit BasketCreated(key, msg.sender, id, beta, mintFeeBps, burnFeeBps);
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
            need[i] = _need(b, b.parts[i], amount, supply);
            if (b.parts[i].token == address(0)) eth = need[i] + _fee(need[i], b.mintFeeBps);
        }
        if (msg.value < eth) revert WrongValue();
        for (uint256 i; i < n; i++) {
            Part storage p = b.parts[i];
            uint256 fee = _fee(need[i], b.mintFeeBps);
            uint256 total = need[i] + fee;
            if (p.token == address(0)) {
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
        for (uint256 i; i < n; i++) out[i] = (amount * _held(b, b.parts[i])) / supply;
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

    function _held(Basket storage b, Part storage p) private view returns (uint256) {
        uint256 balance = p.token == address(0) ? address(b.beta).balance : IERC20(p.token).balanceOf(address(b.beta));
        return balance > p.owed ? balance - p.owed : 0;
    }

    /// @dev What minting `amount` takes of a part, rounded up: every BETA
    /// stays fully backed. While the part holds nothing, because its issuer
    /// took it all, nothing can be minted: it would be free for new holders.
    function _need(Basket storage b, Part storage p, uint256 amount, uint256 supply) private view returns (uint256) {
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
