import { buildModule } from "@nomicfoundation/hardhat-ignition/modules";

// Redeploys BetaVault (insurance releases, reward pool, relaxed auditor
// retirement — docs/design/ipow-implementation.md §6.12) against the iPoW relay already
// deployed by BetaVaultModule on Sepolia.
export default buildModule("BetaVaultV2Module", (m) => {
  const deployer = m.getAccount(0);
  const ipow = m.contractAt("iPoW", m.getParameter("ipowHeaders", "0xB8ab960D1121F33B48b4086aBFCDD8B750081588"));
  const params = {
    ethWeiPerUnit: 1_000_000_000_000_000n,
    windowBlocks: 6n,
    releaseCapUnitsPerWindow: 100n,
    tFinSecs: 3600n,
    tRelSecs: 600n,
    tPauseSecs: 600n,
    tSkipSecs: 7200n,
    refundMarginSecs: 600n,
    unbondDelaySecs: 60n,
    minOperatorBond: 10_000_000_000_000_000n,
    minAuditorBond: 5_000_000_000_000_000n,
    vetoSlashWei: 2_000_000_000_000_000n,
    vetoRewardWei: 1_000_000_000_000_000n,
    vetoHoldFeeWei: 500_000_000_000_000n,
    bountyBps: 1000n,
  };
  const betaVault = m.contract("BetaVault", [deployer, ipow, params]);
  return { betaVault };
});
