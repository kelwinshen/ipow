import { buildModule } from "@nomicfoundation/hardhat-ignition/modules";

// BETA v3 (docs/DESIGN_V2.md §7), redeployed with the mintAttester
// overwrite fix (only the first attester of an unresolved MINT is
// recorded — §7.4a-bis, 2026-09-22). Reuses the Sepolia iPoWV1 relay
// already deployed by BetaVaultModule.
export default buildModule("BetaVaultV4Module", (m) => {
  const deployer = m.getAccount(0);
  const ipow = m.contractAt("iPoWV1", m.getParameter("ipowHeaders", "0xB8ab960D1121F33B48b4086aBFCDD8B750081588"));
  const params = {
    ethWeiPerUnit: 1_000_000_000_000_000n, // 0.001 ETH
    tFinSecs: 14400n, // 4h — raised 2026-09-22 after a near-miss during an explorer outage
    tChallengeSecs: 7n * 86400n,
    tSkipSecs: 28800n, // 8h
    refundMarginSecs: 600n,
    unbondDelaySecs: 60n,
    minOperatorBond: 3_000_000_000_000_000n, // 0.003 ETH
    minAuditorBond: 2_000_000_000_000_000n, // 0.002 ETH
    vetoSlashWei: 1_000_000_000_000_000n,
    vetoRewardWei: 500_000_000_000_000n,
    bountyBps: 1000n,
  };
  const betaVault = m.contract("BetaVault", [deployer, ipow, params]);
  return { betaVault };
});
