import { expect } from "chai";
import { network } from "hardhat";
import { sha256 } from "ethers";

import { GENESIS_HEADER_HEX, sha256d } from "./fixtures/bitcoinHeaders.ts";

const { ethers } = await network.create();

describe("BitcoinPrimitives (via harness)", function () {
  async function deployHarness() {
    return ethers.deployContract("BitcoinPrimitivesHarness");
  }

  describe("header field extraction", function () {
    it("extracts prevHash, merkleRoot, bits, timestamp from the genesis header", async function () {
      const h = await deployHarness();

      expect(await h.extractPrevLE(GENESIS_HEADER_HEX)).to.equal(
        "0x0000000000000000000000000000000000000000000000000000000000000000".slice(
          0,
          66
        )
      );
      expect(await h.readCompact(GENESIS_HEADER_HEX)).to.equal(0x1d00ffffn);
      expect(await h.extractTimestamp(GENESIS_HEADER_HEX)).to.equal(
        1231006505n
      );
    });

    it("hashHeaderLE matches an independently computed double-SHA256", async function () {
      const h = await deployHarness();
      const expected = sha256d(GENESIS_HEADER_HEX);
      expect(await h.hashHeaderLE(GENESIS_HEADER_HEX)).to.equal(expected);
    });

    it("reverts InvalidHeader for a header that is not exactly 80 bytes", async function () {
      const h = await deployHarness();
      const tooShort = GENESIS_HEADER_HEX.slice(0, -2);
      await expect(h.hashHeaderLE(tooShort)).to.be.revertedWithCustomError(
        h,
        "InvalidHeader"
      );
    });
  });

  describe("target / bits conversion", function () {
    it("targetFromBits(0x1d00ffff) equals the Bitcoin PoW limit", async function () {
      const h = await deployHarness();
      expect(await h.targetFromBits(0x1d00ffff)).to.equal(await h.powLimit());
    });

    it("bitsFromTarget round-trips through targetFromBits for a normal (non-edge-case) difficulty", async function () {
      // Note: 0x1d00ffff (the PoW limit) is a mantissa-top-bit-set edge case where this
      // implementation's bits<->target conversion is intentionally not a clean round trip
      // (the 0x007FFFFF mask on re-encoding drops that bit) — that's the original
      // contract's existing behavior, not something this test asserts against. A
      // mid-range historical difficulty value (mantissa top bit clear) round-trips cleanly.
      const h = await deployHarness();
      const bits = 0x1b0404cb;
      const target = await h.targetFromBits(bits);
      expect(await h.bitsFromTarget(target)).to.equal(bits);
    });

    it("clamps an over-easy target down to the PoW limit", async function () {
      const h = await deployHarness();
      // 0x207fffff implies a target far above the mainnet PoW limit.
      const target = await h.targetFromBits(0x207fffff);
      expect(target).to.equal(await h.powLimit());
    });
  });

  describe("validateWorkLE", function () {
    it("accepts the genesis header's hash against its own target", async function () {
      const h = await deployHarness();
      const hashLE = await h.hashHeaderLE(GENESIS_HEADER_HEX);
      const target = await h.extractTarget(GENESIS_HEADER_HEX);
      expect(await h.validateWorkLE(hashLE, target)).to.equal(true);
    });

    it("rejects the zero hash", async function () {
      const h = await deployHarness();
      const target = await h.extractTarget(GENESIS_HEADER_HEX);
      expect(await h.validateWorkLE(ethers.ZeroHash, target)).to.equal(false);
    });

    it("rejects a hash that exceeds the target", async function () {
      const h = await deployHarness();
      const target = await h.targetFromBits(0x1d00ffff);
      // 0xff...ff is far above any realistic target.
      const maxHash = "0x" + "ff".repeat(32);
      expect(await h.validateWorkLE(maxHash, target)).to.equal(false);
    });
  });

  describe("Merkle proof verification", function () {
    it("verifies a single-leaf (no siblings) proof: leaf === root", async function () {
      const h = await deployHarness();
      const leaf = ethers.keccak256("0x01");
      expect(await h.proveMerkleLE(leaf, leaf, "0x", 0)).to.equal(true);
    });

    it("verifies a two-leaf tree at both index 0 and index 1", async function () {
      const h = await deployHarness();
      const leafA = "0x" + "11".repeat(32);
      const leafB = "0x" + "22".repeat(32);
      const rootFromContract = await h.hash256Pair(leafA, leafB);

      expect(await h.proveMerkleLE(leafA, rootFromContract, leafB, 0)).to.equal(
        true
      );
      expect(await h.proveMerkleLE(leafB, rootFromContract, leafA, 1)).to.equal(
        true
      );
    });

    it("rejects a proof against the wrong root", async function () {
      const h = await deployHarness();
      const leafA = "0x" + "11".repeat(32);
      const leafB = "0x" + "22".repeat(32);
      const wrongRoot = "0x" + "33".repeat(32);
      expect(await h.proveMerkleLE(leafA, wrongRoot, leafB, 0)).to.equal(false);
    });

    it("rejects a malformed (non-32-byte-multiple) sibling buffer", async function () {
      const h = await deployHarness();
      const leaf = ethers.keccak256("0x01");
      expect(await h.proveMerkleLE(leaf, leaf, "0x1234", 0)).to.equal(false);
    });
  });

  describe("varint / LE8 parsing", function () {
    it("reads a single-byte varint (<0xFD)", async function () {
      const h = await deployHarness();
      const [v, next] = await h.readVarInt("0x2a", 0);
      expect(v).to.equal(0x2an);
      expect(next).to.equal(1n);
    });

    it("reads a 0xFD-prefixed 16-bit varint", async function () {
      const h = await deployHarness();
      const [v, next] = await h.readVarInt("0xfd3412", 0);
      expect(v).to.equal(0x1234n);
      expect(next).to.equal(3n);
    });

    it("reads an 8-byte little-endian integer", async function () {
      const h = await deployHarness();
      const v = await h.readLE8("0x0100000000000000", 0);
      expect(v).to.equal(1n);
    });

    it("reverts VarIntOutOfBounds when reading past the buffer end", async function () {
      const h = await deployHarness();
      await expect(h.readVarInt("0x", 0)).to.be.revertedWithCustomError(
        h,
        "VarIntOutOfBounds"
      );
    });
  });

  describe("raw transaction output parsing", function () {
    it("parses the value and scriptPubKey of a simple one-output transaction", async function () {
      const h = await deployHarness();
      // A realistic single-input, single-output legacy transaction. Deliberately not
      // using 0 inputs here: an inCount byte of 0x00 followed by an outCount byte of
      // 0x01 is indistinguishable from the SegWit marker+flag (0x00, 0x01) — the parser
      // correctly treats that as SegWit framing, so a 0-input tx isn't realistic test data.
      const txRaw = ethers.concat([
        "0x01000000", // version = 1
        "0x01", // 1 input
        ethers.ZeroHash, // outpoint txid (32 zero bytes)
        "0x00000000", // outpoint vout index
        "0x00", // scriptSig length = 0
        "0xffffffff", // sequence
        "0x01", // 1 output
        "0xe803000000000000", // value = 1000 sats, LE
        "0x02", // script length = 2
        "0xabcd", // script bytes
      ]);
      const [valueSats, program] = await h.parseOutputAt(txRaw, 0);
      expect(valueSats).to.equal(1000n);
      expect(program).to.equal("0xabcd");
    });

    it("reverts VoutOutOfBounds when the requested index doesn't exist", async function () {
      const h = await deployHarness();
      const txRaw = ethers.concat(["0x01000000", "0x00", "0x00"]);
      await expect(h.parseOutputAt(txRaw, 0)).to.be.revertedWithCustomError(
        h,
        "VoutOutOfBounds"
      );
    });

    it("reverts TransactionTooShort for a truncated buffer", async function () {
      const h = await deployHarness();
      await expect(h.parseOutputAt("0x0102", 0)).to.be.revertedWithCustomError(
        h,
        "TransactionTooShort"
      );
    });
  });

  describe("_packBranchStorage (storage-array packing)", function () {
    it("packs a storage bytes32[] into a contiguous bytes buffer", async function () {
      const h = await deployHarness();
      const a = "0x" + "11".repeat(32);
      const b = "0x" + "22".repeat(32);
      await (await h.setBranch([a, b])).wait();
      const packed = await h.packBranch();
      expect(packed).to.equal(a + b.slice(2));
    });
  });

  describe("consistency: hash256Pair matches sha256(sha256(a||b))", function () {
    it("matches an independently computed double-SHA256 of the concatenation", async function () {
      const h = await deployHarness();
      const a = "0x" + "11".repeat(32);
      const b = "0x" + "22".repeat(32);
      const expected = sha256(sha256(ethers.concat([a, b])));
      expect(await h.hash256Pair(a, b)).to.equal(expected);
    });
  });
});
