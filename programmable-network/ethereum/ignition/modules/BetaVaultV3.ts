import { buildModule } from "@nomicfoundation/hardhat-ignition/modules";

// BETA v3 (docs/design/ipow-implementation.md §7): optimistic settlement — challenge window,
// attest/clear/alive, escrow-paid releases. Reuses the Sepolia iPoW relay
// deployed by BetaVaultModule. Testnet-sized bonds.
export default buildModule("BetaVaultV3Module", (m) => {
  const deployer = m.getAccount(0);
  const ipow = m.contractAt("iPoW", m.getParameter("ipowHeaders", "0xB8ab960D1121F33B48b4086aBFCDD8B750081588"));
  const params = {
    ethWeiPerUnit: 1_000_000_000_000_000n, // 0.001 ETH
    tFinSecs: 3600n,
    tChallengeSecs: 7n * 86400n,
    tSkipSecs: 7200n,
    refundMarginSecs: 600n,
    unbondDelaySecs: 60n,
    minOperatorBond: 3_000_000_000_000_000n, // 0.003 ETH (must cover one unit's escrow + slack)
    minAuditorBond: 2_000_000_000_000_000n, // 0.002 ETH
    vetoSlashWei: 1_000_000_000_000_000n,
    vetoRewardWei: 500_000_000_000_000n,
    bountyBps: 1000n,
  };
  const betaVault = m.contract("BetaVault", [deployer, ipow, params]);
  return { betaVault };
});
