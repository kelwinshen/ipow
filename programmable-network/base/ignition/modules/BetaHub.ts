import { buildModule } from "@nomicfoundation/hardhat-ignition/modules";

// BetaHub (DESIGN_V2.md §8.18) on Base Sepolia: an EVM chain acting as a
// mint hub, not just a spoke — same contract as the Ethereum pilot's
// BetaHub. Standard 18-decimal weibar convention.
export default buildModule("BetaHubModule", (m) => {
  const deployer = m.getAccount(0);
  const ipow = m.contractAt("iPoWV1", m.getParameter("ipowHeaders", "0xB7054E399E31A2cFE181c4fD59C7235562a6d45d"));
  const selfNetworkId = m.getParameter("selfNetworkId", 5n); // Base
  const params = {
    tChallengeSecs: 7n * 86400n,
    tSkipSecs: 28800n,
    refundMarginSecs: 600n,
    unbondDelaySecs: 60n,
    minOperatorBond: 3_000_000_000_000_000n, // 0.003 native
    minAuditorBond: 2_000_000_000_000_000n, // 0.002 native
    slashWeiPerUnit: 1_000_000_000_000_000n, // 0.001 native per unit
    vetoSlashWei: 1_000_000_000_000_000n,
    vetoRewardWei: 500_000_000_000_000n,
    bountyBps: 1000n,
  };
  const betaHub = m.contract("BetaHub", [deployer, ipow, selfNetworkId, params, "iBETA", "iBETA"]);
  return { betaHub };
});
