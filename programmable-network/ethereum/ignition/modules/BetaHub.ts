import { buildModule } from "@nomicfoundation/hardhat-ignition/modules";

// BetaHub (design/ipow-implementation.md §8.18): an EVM chain acting as a mint hub, not just
// a spoke. Reuses the same Sepolia iPoW relay every BetaVault version has
// (its public getters satisfy IIPoWHeadersView identically). Deploys its
// own HubToken ("iBETA") internally — no separate token deployment needed.
export default buildModule("BetaHubModule", (m) => {
  const deployer = m.getAccount(0);
  const ipow = m.contractAt("iPoW", m.getParameter("ipowHeaders", "0xB8ab960D1121F33B48b4086aBFCDD8B750081588"));
  const selfNetworkId = m.getParameter("selfNetworkId", 2n); // Ethereum
  const params = {
    tChallengeSecs: 7n * 86400n,
    tSkipSecs: 28800n, // 8h
    refundMarginSecs: 600n,
    unbondDelaySecs: 60n,
    minOperatorBond: 3_000_000_000_000_000n, // 0.003 ETH
    minAuditorBond: 2_000_000_000_000_000n, // 0.002 ETH
    slashWeiPerUnit: 1_000_000_000_000_000n, // 0.001 ETH per unit
    vetoSlashWei: 1_000_000_000_000_000n,
    vetoRewardWei: 500_000_000_000_000n,
    bountyBps: 1000n,
  };
  const betaHub = m.contract("BetaHub", [deployer, ipow, selfNetworkId, params, "iBETA", "iBETA"]);
  return { betaHub };
});
