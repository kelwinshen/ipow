import { expect } from "chai";
import { network } from "hardhat";

const { ethers } = await network.create();

// BETA on Ethereum: a token backed by a fixed basket of up to 8 parts, the
// same rules as `programs/beta-basket` on Solana. Design:
// docs/drafts/ipow-beta-app.md.

const ONE = 10n ** 9n; // one whole BETA
const ETH = 10n ** 18n;
const ZERO = ethers.ZeroAddress;

async function setup() {
  const [creator, user, other] = await ethers.getSigners();
  const app = await ethers.deployContract("BetaBaskets");
  // A receipt of SOL on Ethereum, with 9 decimals, as the vault makes.
  const vsol = await ethers.deployContract("MockToken", ["vSOL", "vSOL", 9]);
  await vsol.mint(user.address, 100n * ONE);
  await vsol.connect(user).approve(await app.getAddress(), ethers.MaxUint256);
  return { app, vsol, creator, user, other };
}

type Ctx = Awaited<ReturnType<typeof setup>>;

/** A basket's naming for the tests: the creator's name and symbol, and a URI. */
const NAMED = ["Test Basket", "TBSK", "data:application/json,{}"] as const;

/** Parts as the contract takes them: tokens here (E5's receipts are named in their own tests). */
const specs = (tokens: string[], amounts: bigint[]) => tokens.map((token, i) => ({ token, receipts: ZERO, asset: 0, amount: amounts[i] }));

async function create(ctx: Ctx, id: number, tokens: string[], amounts: bigint[], mintFee: number, burnFee: number) {
  await ctx.app.connect(ctx.creator).createBasket(id, ...NAMED, specs(tokens, amounts), mintFee, burnFee);
  const key = await ctx.app.basketKey(ctx.creator.address, id);
  const b = await ctx.app.getBasket(key);
  const beta = await ethers.getContractAt("BetaBasket", b.beta);
  return { key, beta };
}

/** ETH spent on gas by a transaction. */
async function gasOf(tx: { wait: () => Promise<{ gasUsed: bigint; gasPrice: bigint } | null> }) {
  const r = await tx.wait();
  return r!.gasUsed * r!.gasPrice;
}

/** A token the user holds `amount` of, approved to the app. */
async function token(app: string, amount: bigint) {
  const [, user] = await ethers.getSigners();
  const t = await ethers.deployContract("MockToken", ["Part", "PART", 9]);
  await t.mint(user.address, amount);
  await t.connect(user).approve(app, ethers.MaxUint256);
  return t;
}

/** A token whose issuer (the deployer) can freeze and seize, the user holding `amount`. */
async function issuerToken(app: string, amount: bigint) {
  const [, user] = await ethers.getSigners();
  const t = await ethers.deployContract("IssuerToken");
  await t.mint(user.address, amount);
  await t.connect(user).approve(app, ethers.MaxUint256);
  return t;
}

const up = (v: bigint, bps: bigint) => (v * bps + 9_999n) / 10_000n;

describe("BetaBaskets", function () {
  it("mints and burns 1 ETH + 0.5 vSOL per BETA, setting the creator's fee aside as a share of each part", async function () {
    const ctx = await setup();
    const { app, vsol, creator, user } = ctx;
    // 0.3% on mint, 0.2% on burn.
    const { key, beta } = await create(ctx, 1, [ZERO, await vsol.getAddress()], [ETH, ONE / 2n], 30, 20);
    const basket = await beta.getAddress();
    expect(await beta.decimals()).to.equal(9);

    const [need, fee] = await app.mintCost(key, 2n * ONE);
    expect(need).to.deep.equal([2n * ETH, ONE]);
    expect(fee).to.deep.equal([up(2n * ETH, 30n), up(ONE, 30n)]);
    // More ETH than needed is sent back.
    const userEth = await ethers.provider.getBalance(user.address);
    const gas = await gasOf(await app.connect(user).mint(key, 2n * ONE, { value: 3n * ETH }));
    expect(await beta.balanceOf(user.address)).to.equal(2n * ONE);
    expect(await ethers.provider.getBalance(user.address)).to.equal(userEth - gas - need[0] - fee[0]);
    // The basket holds the parts and the fees, set aside for the creator.
    expect(await ethers.provider.getBalance(basket)).to.equal(need[0] + fee[0]);
    expect(await vsol.balanceOf(basket)).to.equal(need[1] + fee[1]);
    expect(await app.held(key, 0)).to.equal(2n * ETH);
    expect(await app.held(key, 1)).to.equal(ONE);
    expect(await app.fees(key, creator.address, 0)).to.equal(fee[0]);

    // Burning 1 BETA gives half of what the holders have, less 0.2%.
    const before = await ethers.provider.getBalance(user.address);
    const g = await gasOf(await app.connect(user).burn(key, ONE, 0, user.address));
    expect(await ethers.provider.getBalance(user.address)).to.equal(before - g + ETH - up(ETH, 20n));
    expect(await app.held(key, 0)).to.equal(ETH);
    expect(await app.held(key, 1)).to.equal(ONE / 2n);
    expect(await app.fees(key, creator.address, 1)).to.equal(fee[1] + up(ONE / 2n, 20n));

    // The creator collects its fees, to any address.
    const vfees = await app.fees(key, creator.address, 1);
    await app.connect(creator).collectFees(key, 1, ctx.other.address);
    expect(await vsol.balanceOf(ctx.other.address)).to.equal(vfees);
    await expect(app.connect(creator).collectFees(key, 1, creator.address)).to.be.revertedWithCustomError(app, "NothingOwed");
    expect(await app.held(key, 1)).to.equal(ONE / 2n);
    expect(await beta.totalSupply()).to.equal(ONE);
  });

  it("asks enough ETH, sends back ETH a basket without ETH does not take, and only BetaBaskets moves anything", async function () {
    const ctx = await setup();
    const { app, vsol, user } = ctx;
    const { key, beta } = await create(ctx, 1, [ZERO], [ETH], 0, 0);
    await expect(app.connect(user).mint(key, ONE, { value: ETH - 1n })).to.be.revertedWithCustomError(app, "WrongValue");
    await app.connect(user).mint(key, ONE, { value: ETH });
    await expect(beta.connect(user).mint(user.address, 1)).to.be.revertedWithCustomError(beta, "NotApp");
    await expect(beta.connect(user).burnFrom(user.address, 1)).to.be.revertedWithCustomError(beta, "NotApp");
    await expect(beta.connect(user).pay(ZERO, user.address, 1)).to.be.revertedWithCustomError(beta, "NotApp");
    await expect(app.connect(user).mint(ethers.ZeroHash, ONE)).to.be.revertedWithCustomError(app, "UnknownBasket");
    await expect(app.held(key, 1)).to.be.revertedWithCustomError(app, "BadParts");
    await expect(app.connect(user).burn(key, ONE, 0, ZERO)).to.be.revertedWithCustomError(app, "ZeroAddress");

    const tokenOnly = await create(ctx, 2, [await vsol.getAddress()], [ONE], 0, 0);
    const before = await ethers.provider.getBalance(user.address);
    const gas = await gasOf(await app.connect(user).mint(tokenOnly.key, ONE, { value: ETH }));
    expect(await ethers.provider.getBalance(user.address)).to.equal(before - gas);
  });

  it("puts a basket at an address fixed by its creator and number, each once", async function () {
    const ctx = await setup();
    const { app, vsol, creator, other } = ctx;
    const v = await vsol.getAddress();
    const key = await app.basketKey(creator.address, 7);
    const code = (await ethers.getContractFactory("BetaBasket")).bytecode;
    const predicted = ethers.getCreate2Address(await app.getAddress(), key, ethers.keccak256(code));
    await app.connect(creator).createBasket(7, ...NAMED, specs([v], [ONE]), 0, 0);
    expect((await app.getBasket(key)).beta).to.equal(predicted);
    await expect(app.connect(creator).createBasket(7, ...NAMED, specs([ZERO], [ETH]), 0, 0)).to.be.revert(ethers);
    // Another creator's number 7 is another basket.
    await app.connect(other).createBasket(7, ...NAMED, specs([ZERO], [ETH]), 0, 0);
  });

  it("names the token as its creator does, keeps the metadata URI, and refuses bad names (E4)", async function () {
    const { app, vsol, creator } = await setup();
    const v = await vsol.getAddress();
    const c = app.connect(creator);
    await expect(c.createBasket(3, "Gold & Treasuries", "GTB", "data:application/json,{\"description\":\"x\"}", specs([v], [ONE]), 25, 50))
      .to.emit(app, "BasketCreated")
      .withArgs(await app.basketKey(creator.address, 3), creator.address, 3, (a: string) => a !== ZERO, "Gold & Treasuries", "GTB", "data:application/json,{\"description\":\"x\"}", 25, 50);
    const key = await app.basketKey(creator.address, 3);
    const b = await app.getBasket(key);
    const beta = await ethers.getContractAt("BetaBasket", b.beta);
    expect(await beta.name()).to.equal("Gold & Treasuries");
    expect(await beta.symbol()).to.equal("GTB");
    expect(await beta.decimals()).to.equal(9);
    expect(b.uri).to.equal("data:application/json,{\"description\":\"x\"}");
    // Nothing is left for the next creation to read.
    expect(await app.creatingName()).to.equal("");
    expect(await app.creatingSymbol()).to.equal("");
    // Limits: a name and a symbol of 1 to 64 and 16 bytes, a URI of at most 2,048.
    await expect(c.createBasket(4, "", "X", "", specs([v], [ONE]), 0, 0)).to.be.revertedWithCustomError(app, "BadMetadata");
    await expect(c.createBasket(4, "N", "", "", specs([v], [ONE]), 0, 0)).to.be.revertedWithCustomError(app, "BadMetadata");
    await expect(c.createBasket(4, "n".repeat(65), "X", "", specs([v], [ONE]), 0, 0)).to.be.revertedWithCustomError(app, "BadMetadata");
    await expect(c.createBasket(4, "N", "s".repeat(17), "", specs([v], [ONE]), 0, 0)).to.be.revertedWithCustomError(app, "BadMetadata");
    await expect(c.createBasket(4, "N", "X", "u".repeat(2049), specs([v], [ONE]), 0, 0)).to.be.revertedWithCustomError(app, "BadMetadata");
    await c.createBasket(4, "n".repeat(64), "s".repeat(16), "u".repeat(2048), specs([v], [ONE]), 0, 0);
    // The same id with another name is still the same address: refused.
    await expect(c.createBasket(3, "Another", "AN", "", specs([ZERO], [ETH]), 0, 0)).to.be.revert(ethers);
  });

  it("takes a part that is a receipt not made yet, and mints once the vault has made it (E5)", async function () {
    const { app, vsol, creator, user } = await setup();
    const v = await vsol.getAddress();
    const mock = await ethers.deployContract("MockReceipts");
    const receipts = await mock.getAddress();
    const c = app.connect(creator);
    // ETH, plus the peer's asset 1 as a receipt, named before it exists.
    await c.createBasket(5, ...NAMED, [{ token: ZERO, receipts: ZERO, asset: 0, amount: ETH }, { token: ZERO, receipts, asset: 1, amount: ONE }], 0, 0);
    const key = await app.basketKey(creator.address, 5);
    let b = await app.getBasket(key);
    expect(b.parts[1].token).to.equal(ZERO);
    expect(b.parts[1].receipts).to.equal(receipts);
    expect(b.parts[1].asset).to.equal(1);
    expect(await app.partToken(key, 0)).to.equal(ZERO);
    expect(await app.partToken(key, 1)).to.equal(ZERO);
    // Nothing can be priced or minted while the receipt is not made.
    await expect(app.held(key, 1)).to.be.revertedWithCustomError(app, "PartNotMadeYet");
    await expect(app.mintCost(key, ONE)).to.be.revertedWithCustomError(app, "PartNotMadeYet");
    await expect(app.connect(user).mint(key, ONE, { value: ETH })).to.be.revertedWithCustomError(app, "PartNotMadeYet");
    // The vault makes it (here, the mock): vSOL. The part resolves on use.
    await mock.make(1, v);
    expect(await app.partToken(key, 1)).to.equal(v);
    const [need] = await app.mintCost(key, ONE);
    expect(need[1]).to.equal(ONE);
    await app.connect(user).mint(key, ONE, { value: ETH });
    b = await app.getBasket(key);
    expect(b.parts[1].token).to.equal(v);
    expect(await app.held(key, 1)).to.equal(ONE);
    const beta = await ethers.getContractAt("BetaBasket", b.beta);
    expect(await beta.balanceOf(user.address)).to.equal(ONE);
    await app.connect(user).burn(key, ONE, 0, user.address);
    expect(await vsol.balanceOf(user.address)).to.equal(100n * ONE);
    // A receipt made already resolves at creation.
    await c.createBasket(6, ...NAMED, [{ token: ZERO, receipts, asset: 1, amount: ONE }], 0, 0);
    expect((await app.getBasket(await app.basketKey(creator.address, 6))).parts[0].token).to.equal(v);
  });

  it("refuses bad receipt parts (E5)", async function () {
    const { app, vsol, creator } = await setup();
    const v = await vsol.getAddress();
    const mock = await ethers.deployContract("MockReceipts");
    const receipts = await mock.getAddress();
    const c = app.connect(creator);
    // A receipts contract with no code; a receipt part naming a token too.
    await expect(c.createBasket(1, ...NAMED, [{ token: ZERO, receipts: creator.address, asset: 1, amount: ONE }], 0, 0)).to.be.revertedWithCustomError(app, "BadParts");
    await expect(c.createBasket(1, ...NAMED, [{ token: v, receipts, asset: 1, amount: ONE }], 0, 0)).to.be.revertedWithCustomError(app, "BadParts");
    // The same asset twice, and a made receipt also named as a token.
    await expect(c.createBasket(1, ...NAMED, [{ token: ZERO, receipts, asset: 1, amount: ONE }, { token: ZERO, receipts, asset: 1, amount: ONE }], 0, 0)).to.be.revertedWithCustomError(app, "BadParts");
    await mock.make(2, v);
    await expect(c.createBasket(1, ...NAMED, [{ token: v, receipts: ZERO, asset: 0, amount: ONE }, { token: ZERO, receipts, asset: 2, amount: ONE }], 0, 0)).to.be.revertedWithCustomError(app, "BadParts");
    // ETH twice.
    await expect(c.createBasket(1, ...NAMED, [{ token: ZERO, receipts: ZERO, asset: 0, amount: ETH }, { token: ZERO, receipts: ZERO, asset: 0, amount: ETH }], 0, 0)).to.be.revertedWithCustomError(app, "BadParts");
  });

  it("rounds in the basket's favour", async function () {
    const ctx = await setup();
    const { app, vsol, user } = ctx;
    // 1 BETA holds 3 of the smallest vSOL unit.
    const { key, beta } = await create(ctx, 1, [await vsol.getAddress()], [3n], 0, 0);
    const basket = await beta.getAddress();
    // The first mint is whole BETA: 1 BETA holds exactly 3.
    await expect(app.connect(user).mint(key, 1)).to.be.revertedWithCustomError(app, "FirstMintNotWhole");
    await app.connect(user).mint(key, ONE);
    expect(await vsol.balanceOf(basket)).to.equal(3n);
    // One smallest unit more holds 3e-9 of a unit: paid in as 1.
    await app.connect(user).mint(key, 1);
    expect(await vsol.balanceOf(basket)).to.equal(4n);
    // Paid out as 0: every BETA stays fully backed.
    await app.connect(user).burn(key, 1, 0, user.address);
    expect(await vsol.balanceOf(basket)).to.equal(4n);
  });

  it("starts again from whole BETA once every BETA is burned", async function () {
    const ctx = await setup();
    const { app, vsol, user } = ctx;
    const { key, beta } = await create(ctx, 1, [await vsol.getAddress()], [3n], 0, 0);
    await app.connect(user).mint(key, ONE);
    await app.connect(user).mint(key, 1);
    await app.connect(user).burn(key, ONE + 1n, 0, user.address);
    expect(await beta.totalSupply()).to.equal(0n);
    // The dust left is a gift to the next holders.
    expect(await app.held(key, 0)).to.equal(0n);
    await expect(app.connect(user).mint(key, 1)).to.be.revertedWithCustomError(app, "FirstMintNotWhole");
    await app.connect(user).mint(key, ONE);
    expect(await app.held(key, 0)).to.equal(3n);
    await expect(app.connect(ctx.other).burn(key, 1, 0, user.address)).to.be.revertedWithCustomError(beta, "ERC20InsufficientBalance");
  });

  it("counts ETH sent straight to a basket as backing", async function () {
    const ctx = await setup();
    const { app, user, other } = ctx;
    const { key, beta } = await create(ctx, 1, [ZERO], [ETH], 0, 0);
    await app.connect(user).mint(key, ONE, { value: ETH });
    await other.sendTransaction({ to: await beta.getAddress(), value: ETH });
    expect(await app.held(key, 0)).to.equal(2n * ETH);
    // A new BETA takes what the basket now holds per BETA.
    const [need] = await app.mintCost(key, ONE);
    expect(need[0]).to.equal(2n * ETH);
  });

  it("refuses bad baskets", async function () {
    const ctx = await setup();
    const { app, vsol, creator } = ctx;
    const v = await vsol.getAddress();
    const c = app.connect(creator);
    await expect(c.createBasket(1, ...NAMED, specs([], []), 0, 0)).to.be.revertedWithCustomError(app, "BadParts");
    await expect(c.createBasket(1, ...NAMED, specs([v, v], [ONE, ONE]), 0, 0)).to.be.revertedWithCustomError(app, "BadParts");
    await expect(c.createBasket(1, ...NAMED, specs([v], [0]), 0, 0)).to.be.revertedWithCustomError(app, "BadParts");
    await expect(c.createBasket(1, ...NAMED, [{ token: v, receipts: ZERO, asset: 0, amount: ONE }, { token: ZERO, receipts: ZERO, asset: 0, amount: 0n }], 0, 0)).to.be.revertedWithCustomError(app, "BadParts");
    const nine = await Promise.all(Array.from({ length: 9 }, () => ethers.deployContract("MockToken", ["T", "T", 9])));
    await expect(c.createBasket(1, ...NAMED, specs(await Promise.all(nine.map((t) => t.getAddress())), nine.map(() => 1n)), 0, 0)).to.be.revertedWithCustomError(app, "BadParts");
    await expect(c.createBasket(1, ...NAMED, specs([v], [ONE]), 101, 0)).to.be.revertedWithCustomError(app, "FeeTooHigh");
    await expect(c.createBasket(1, ...NAMED, specs([v], [ONE]), 0, 101)).to.be.revertedWithCustomError(app, "FeeTooHigh");
    // Not a token at all: an account with no code.
    await expect(c.createBasket(1, ...NAMED, specs([creator.address], [ONE]), 0, 0)).to.be.revertedWithCustomError(app, "BadParts");
    await c.createBasket(1, ...NAMED, specs([v], [ONE]), 100, 100);
  });

  it("refuses a token that takes a fee on transfer when minting", async function () {
    const ctx = await setup();
    const { app, user } = ctx;
    const fee = await ethers.deployContract("FeeOnTransferToken");
    await fee.mint(user.address, 100n * ONE);
    await fee.connect(user).approve(await app.getAddress(), ethers.MaxUint256);
    const { key } = await create(ctx, 1, [await fee.getAddress()], [ONE], 0, 0);
    await expect(app.connect(user).mint(key, ONE)).to.be.revertedWithCustomError(app, "TransferFeeToken");
  });

  it("defers a frozen part and pays the others; the part is collected once it moves, to any address", async function () {
    const ctx = await setup();
    const { app, creator, user, other } = ctx;
    const frz = await issuerToken(await app.getAddress(), 10_000_000n);
    // 0.1% on burn.
    const { key, beta } = await create(ctx, 1, [ZERO, await frz.getAddress()], [ETH, 1_000_000n], 0, 10);
    const basket = await beta.getAddress();
    await app.connect(user).mint(key, 2n * ONE, { value: 2n * ETH });

    await frz.setFrozen(basket, true);
    // Moving the frozen part fails the whole burn.
    await expect(app.connect(user).burn(key, ONE, 0, user.address)).to.be.revertedWithCustomError(frz, "Frozen");
    // Only parts the basket has may be deferred.
    await expect(app.connect(user).burn(key, ONE, 0b100, user.address)).to.be.revertedWithCustomError(app, "WrongDefer");
    // Deferring it: ETH now, less 0.1%; the frozen part owed, with no fee.
    const before = await ethers.provider.getBalance(user.address);
    const gas = await gasOf(await app.connect(user).burn(key, ONE, 0b10, user.address));
    expect(await ethers.provider.getBalance(user.address)).to.equal(before - gas + ETH - ETH / 1000n);
    expect(await app.owed(key, user.address, 1)).to.equal(1_000_000n);
    expect((await app.getBasket(key)).parts[1].owed).to.equal(1_000_000n);
    // Not collected while the basket's account is frozen.
    await expect(app.connect(user).collectOwed(key, 1, user.address)).to.be.revertedWithCustomError(frz, "Frozen");
    await frz.setFrozen(basket, false);
    // The user's own address is frozen now: it collects to another.
    await frz.setFrozen(user.address, true);
    await app.connect(user).collectOwed(key, 1, other.address);
    // The burn fee is taken when collected: deferring never avoids it.
    expect(await frz.balanceOf(other.address)).to.equal(1_000_000n - 1_000n);
    expect(await app.fees(key, creator.address, 1)).to.equal(1_000n);
    expect(await app.owed(key, user.address, 1)).to.equal(0n);
    await expect(app.connect(user).collectOwed(key, 1, other.address)).to.be.revertedWithCustomError(app, "NothingOwed");
    await frz.setFrozen(user.address, false);
    // What was set aside stays aside: the last BETA still has its full share.
    const left = await frz.balanceOf(user.address);
    await app.connect(user).burn(key, ONE, 0, user.address);
    expect((await frz.balanceOf(user.address)) - left).to.equal(1_000_000n - 1_000n);
    expect(await app.held(key, 1)).to.equal(0n);
    expect(await app.fees(key, creator.address, 1)).to.equal(2_000n);
  });

  it("keeps what is owed aside from later burns and mints", async function () {
    const ctx = await setup();
    const { app, user } = ctx;
    const frz = await issuerToken(await app.getAddress(), 10_000_000n);
    const { key, beta } = await create(ctx, 1, [await frz.getAddress()], [1_000_000n], 0, 0);
    await app.connect(user).mint(key, 2n * ONE);
    await app.connect(user).burn(key, ONE, 0b1, user.address);
    // The basket holds 2, of which 1 is owed: 1 BETA holds 1.
    expect(await frz.balanceOf(await beta.getAddress())).to.equal(2_000_000n);
    expect(await app.held(key, 0)).to.equal(1_000_000n);
    const [need] = await app.mintCost(key, ONE);
    expect(need[0]).to.equal(1_000_000n);
  });

  it("lets nobody stop holders by refusing the fee: a receiver that takes no ETH, or a blocked one", async function () {
    const ctx = await setup();
    const { app, creator, user } = ctx;
    const frz = await issuerToken(await app.getAddress(), 10n * ONE);
    const { key } = await create(ctx, 1, [ZERO, await frz.getAddress()], [ETH, ONE], 100, 100);
    // The fee goes to a contract that refuses ETH, and the token blocks the
    // creator.
    const refuser = await ethers.deployContract("BetaBaskets");
    await expect(app.connect(creator).setFeeTo(key, ZERO)).to.be.revertedWithCustomError(app, "ZeroAddress");
    await expect(app.connect(creator).setFeeTo(key, await app.getAddress())).to.be.revertedWithCustomError(app, "ZeroAddress");
    await app.connect(creator).setFeeTo(key, await refuser.getAddress());
    await frz.setFrozen(creator.address, true);
    await frz.setFrozen(await refuser.getAddress(), true);
    const [need, fee] = await app.mintCost(key, ONE);
    await app.connect(user).mint(key, ONE, { value: need[0] + fee[0] });
    await app.connect(user).burn(key, ONE / 2n, 0, user.address);
    await app.connect(user).burn(key, ONE / 2n, 0b10, user.address);
    await app.connect(user).collectOwed(key, 1, user.address);
    // The fees wait for their receiver.
    expect(await app.fees(key, await refuser.getAddress(), 0)).to.be.greaterThan(0n);
  });

  it("shares a seized part equally, and new mints follow the ratio", async function () {
    const ctx = await setup();
    const { app, vsol, user } = ctx;
    const stock = await issuerToken(await app.getAddress(), 10n * ONE);
    const { key, beta } = await create(ctx, 1, [await vsol.getAddress(), await stock.getAddress()], [ONE, ONE], 0, 0);
    const basket = await beta.getAddress();
    await app.connect(user).mint(key, 2n * ONE);
    // The issuer takes 1 of the basket's 2.
    await stock.seize(basket, ONE);
    // A new BETA deposits 0.5 of the part, as the basket holds per BETA.
    let before = await stock.balanceOf(user.address);
    await app.connect(user).mint(key, ONE);
    expect(before - (await stock.balanceOf(user.address))).to.equal(ONE / 2n);
    // Each BETA now pays 1 vSOL and 0.5 of the part.
    before = await stock.balanceOf(user.address);
    await app.connect(user).burn(key, ONE, 0, user.address);
    expect((await stock.balanceOf(user.address)) - before).to.equal(ONE / 2n);
    expect(await vsol.balanceOf(basket)).to.equal(2n * ONE);
  });

  it("pays what is set aside first come when a seizure is larger than the holders' share", async function () {
    const ctx = await setup();
    const { app, user } = ctx;
    const stock = await issuerToken(await app.getAddress(), 10n * ONE);
    const { key, beta } = await create(ctx, 1, [await stock.getAddress()], [ONE], 0, 0);
    await app.connect(user).mint(key, 2n * ONE);
    // Half deferred: 1 owed, 1 for the last BETA.
    await app.connect(user).burn(key, ONE, 0b1, user.address);
    // The issuer takes 1.5 of the 2.
    await stock.seize(await beta.getAddress(), (3n * ONE) / 2n);
    // The holders had 1: their part is gone, and nothing can be minted.
    expect(await app.held(key, 0)).to.equal(0n);
    await expect(app.connect(user).mint(key, ONE)).to.be.revertedWithCustomError(app, "PartEmpty");
    // The owed 1 cannot be paid in full: 0.5 is left.
    await expect(app.connect(user).collectOwed(key, 0, user.address)).to.be.revertedWithCustomError(stock, "ERC20InsufficientBalance");
  });

  it("stops minting while a part holds nothing", async function () {
    const ctx = await setup();
    const { app, user } = ctx;
    const stock = await issuerToken(await app.getAddress(), 10n * ONE);
    const { key, beta } = await create(ctx, 1, [ZERO, await stock.getAddress()], [ETH, ONE], 0, 0);
    await app.connect(user).mint(key, ONE, { value: ETH });
    await stock.seize(await beta.getAddress(), ONE);
    // The part would be free for new holders: no mint.
    await expect(app.connect(user).mint(key, ONE, { value: ETH })).to.be.revertedWithCustomError(app, "PartEmpty");
    // Burns go on: the ETH comes back.
    await app.connect(user).burn(key, ONE, 0, user.address);
    expect(await ethers.provider.getBalance(await beta.getAddress())).to.equal(0n);
  });

  it("lets only the fee receiver hand the fee on; handed to the basket, it backs BETA", async function () {
    const ctx = await setup();
    const { app, vsol, creator, user, other } = ctx;
    const { key, beta } = await create(ctx, 1, [await vsol.getAddress()], [ONE], 10, 10);
    await expect(app.connect(other).setFeeTo(key, other.address)).to.be.revertedWithCustomError(app, "NotFeeReceiver");
    await app.connect(creator).setFeeTo(key, other.address);
    await app.connect(user).mint(key, ONE);
    expect(await app.fees(key, other.address, 0)).to.equal(ONE / 1000n);
    // Handed to the basket's own token: every later fee stays in the backing,
    // and nobody can hand it on again.
    await app.connect(other).setFeeTo(key, await beta.getAddress());
    await app.connect(user).mint(key, ONE);
    expect(await app.held(key, 0)).to.equal(2n * ONE + ONE / 1000n);
    // Burning all 2 BETA pays out the holders' whole share, that fee
    // included, less the burn fee, which stays.
    const before = await vsol.balanceOf(user.address);
    await app.connect(user).burn(key, 2n * ONE, 0, user.address);
    const out = 2n * ONE + ONE / 1000n;
    expect((await vsol.balanceOf(user.address)) - before).to.equal(out - up(out, 10n));
    expect(await app.held(key, 0)).to.equal(up(out, 10n));
    // The first receiver still collects what it earned before.
    await app.connect(other).collectFees(key, 0, other.address);
    expect(await vsol.balanceOf(other.address)).to.equal(ONE / 1000n);
  });

  it("mints and burns a basket of eight parts, and defers the eighth", async function () {
    const ctx = await setup();
    const { app, user } = ctx;
    const a = await app.getAddress();
    const parts = [];
    for (let i = 0n; i < 8n; i++) parts.push({ t: await token(a, 10n * ONE), amount: ((i + 1n) * ONE) / 10n });
    const { key } = await create(ctx, 8, await Promise.all(parts.map((p) => p.t.getAddress())), parts.map((p) => p.amount), 10, 10);
    await app.connect(user).mint(key, 2n * ONE);
    await app.connect(user).burn(key, ONE, 0, user.address);
    await app.connect(user).burn(key, ONE, 0x80, user.address);
    expect(await app.owed(key, user.address, 7)).to.equal(parts[7].amount);
    await app.connect(user).collectOwed(key, 7, user.address);
    for (const p of parts) {
      // All paid back but the two fees on each BETA, each rounded up.
      expect(await p.t.balanceOf(user.address)).to.equal(10n * ONE - 4n * up(p.amount, 10n));
    }
  });

  it("rounds the fee up, and sets none aside without a fee", async function () {
    const ctx = await setup();
    const { app, vsol, creator, user } = ctx;
    // 1 BETA holds 50 units; 1% of 50 is 0.5, paid as 1.
    const { key } = await create(ctx, 1, [await vsol.getAddress()], [50n], 100, 100);
    await app.connect(user).mint(key, ONE);
    expect(await app.fees(key, creator.address, 0)).to.equal(1n);
    const free = await create(ctx, 2, [await vsol.getAddress()], [ONE], 0, 0);
    await app.connect(user).mint(free.key, ONE);
    await app.connect(user).burn(free.key, ONE, 0, user.address);
    expect(await app.fees(free.key, creator.address, 0)).to.equal(0n);
  });

  it("lets no payee reenter while a burn pays", async function () {
    const ctx = await setup();
    const { app, user } = ctx;
    const { key, beta } = await create(ctx, 1, [ZERO], [ETH], 0, 0);
    const attacker = await ethers.deployContract("BetaReenterer", [await app.getAddress(), key]);
    await app.connect(user).mint(key, 2n * ONE, { value: 2n * ETH });
    await beta.connect(user).transfer(await attacker.getAddress(), ONE);
    // Its payout calls back into the app: the whole burn fails.
    await expect(attacker.burn(ONE)).to.be.revertedWithCustomError(app, "PaymentFailed");
    expect(await beta.balanceOf(await attacker.getAddress())).to.equal(ONE);
    expect(await app.held(key, 0)).to.equal(2n * ETH);
  });
});
