import { buildModule } from "@nomicfoundation/hardhat-ignition/modules";

// BETA v2 on Sepolia (docs/DESIGN_V2.md §6): a fresh iPoWV1 header relay
// carrying the §6.9 operator-first/permissionless-fallback change, and a
// BetaVault reading it. Test-sized composition: 1 unit = 0.001 ETH.
export default buildModule("BetaVaultModule", (m) => {
  const deployer = m.getAccount(0);
  const nativeDecimals = m.getParameter("nativeDecimals", 18n);
  const selfNetworkId = m.getParameter("selfNetworkId", 2n);
  const commitFeeBps = m.getParameter("commitFeeBps", 50n);

  const ipow = m.contract("iPoWV1", [nativeDecimals, selfNetworkId, deployer, commitFeeBps]);

  const params = {
    ethWeiPerUnit: m.getParameter("ethWeiPerUnit", 1_000_000_000_000_000n), // 0.001 ETH
    windowBlocks: m.getParameter("windowBlocks", 6n),
    releaseCapUnitsPerWindow: m.getParameter("releaseCapUnitsPerWindow", 100n),
    tFinSecs: m.getParameter("tFinSecs", 3600n),
    tRelSecs: m.getParameter("tRelSecs", 600n),
    tPauseSecs: m.getParameter("tPauseSecs", 600n),
    tSkipSecs: m.getParameter("tSkipSecs", 7200n),
    refundMarginSecs: m.getParameter("refundMarginSecs", 600n),
    unbondDelaySecs: m.getParameter("unbondDelaySecs", 60n),
    minOperatorBond: m.getParameter("minOperatorBond", 10_000_000_000_000_000n), // 0.01 ETH
    minAuditorBond: m.getParameter("minAuditorBond", 5_000_000_000_000_000n), // 0.005 ETH
    vetoSlashWei: m.getParameter("vetoSlashWei", 2_000_000_000_000_000n),
    vetoRewardWei: m.getParameter("vetoRewardWei", 1_000_000_000_000_000n),
    vetoHoldFeeWei: m.getParameter("vetoHoldFeeWei", 500_000_000_000_000n),
    bountyBps: m.getParameter("bountyBps", 1000n),
  };

  const betaVault = m.contract("BetaVault", [deployer, ipow, params]);

  return { ipow, betaVault };
});
