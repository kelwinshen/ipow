// A network whose coin is a token (D136), as Tempo: the protocol's token
// build on a test light client, a 6-decimal coin standing in for PathUSD,
// and an operator with a bond and a chain head, so a job can be won and
// slashed in the coin.

import { COIN_SCRIPT, buildTx, concat, headerHashLE, merkle, mine, targetFromBits, txidLE } from "./bitcoin.ts";

export const USD = 10n ** 6n;
export const SCALE = 10n ** 12n; // attodollars per microdollar
export const TEMPO_BASE_FEE = 2n * 10n ** 10n; // attodollars per gas
const EASY = 0x207fffff;
const ZERO_HASH = "0x" + "00".repeat(32);

export async function tokenNetwork(ethers: any) {
  const [, application, user, operator, guardian, stranger] = await ethers.getSigners();
  const lightClient = await ethers.deployContract("iPoWLightClientHarness", [0]);
  await lightClient.setLimits(targetFromBits(EASY), targetFromBits(EASY));
  const coin = await ethers.deployContract("MockToken", ["PathUSD", "pathUSD", 6]);
  const protocol = await ethers.deployContract("iPoWProtocolToken", [await lightClient.getAddress(), await coin.getAddress(), SCALE]);
  for (const who of [application, user, operator, guardian, stranger]) {
    await coin.mint(who.address, 1_000n * USD);
    await coin.connect(who).approve(await protocol.getAddress(), ethers.MaxUint256);
  }

  const latestTime = async () => (await ethers.provider.getBlock("latest"))!.timestamp;
  const mineAt = async (t: number) => {
    await ethers.provider.send("evm_setNextBlockTimestamp", [t]);
    await ethers.provider.send("evm_mine", []);
  };
  const atTempoPrice = () => ethers.provider.send("hardhat_setNextBlockBaseFeePerGas", ["0x" + TEMPO_BASE_FEE.toString(16)]);

  // Six blocks start an epoch; a seventh carries the operator's chain head.
  const epochTime = (await latestTime()) - 3600;
  const headers: string[] = [];
  let prevLE = ZERO_HASH;
  for (let i = 0; i < 6; i++) {
    const h = mine({ prevLE, time: epochTime + i * 600, bits: EASY });
    headers.push(h);
    prevLE = headerHashLE(h);
  }
  await lightClient.addEpochStart(concat(headers), 0);
  await protocol.connect(operator).lockBond(100n * USD);
  const raw = buildTx({
    inputs: [{ txidLE: ethers.id("funding"), vout: 0 }],
    outputs: [
      { value: 546n, script: COIN_SCRIPT },
      { value: 0n, script: "0x6a20" + (await protocol.chainHeadCommitment(operator.address)).slice(2) },
    ],
  });
  const coinbase = buildTx({ inputs: [{ txidLE: ZERO_HASH, vout: 0xffffffff }], outputs: [{ value: 1n, script: COIN_SCRIPT }] });
  const txids = [txidLE(coinbase), txidLE(raw)];
  const header = mine({ prevLE, time: (await latestTime()) + 1, bits: EASY, merkleRootLE: merkle(txids, 0).rootLE });
  await lightClient.extend(header, 5, epochTime);
  const block = { hash: headerHashLE(header), height: 6, epochTime };
  await protocol.connect(operator).registerChainHead(block, raw, merkle(txids, 1).siblings, 1, 0, 1);

  /** The operator wins the job, misses its deadline, and a guardian has it slashed. */
  async function slash(jobId: bigint) {
    await protocol.connect(operator).bid(jobId, await protocol.minimumBidOf(jobId));
    await mineAt((await latestTime()) + 61);
    await mineAt(Number(await protocol.deadlineOf(jobId)) + 1);
    const salt = ethers.id("salt");
    await protocol.connect(guardian).sealNote(await protocol.noteFor(guardian.address, jobId, ZERO_HASH, salt));
    await protocol.connect(guardian).reportMissedDuty(jobId, salt);
    return (await protocol.getJob(jobId)).escrow as bigint;
  }

  return { lightClient, coin, protocol, application, user, operator, guardian, stranger, latestTime, mineAt, atTempoPrice, slash };
}
