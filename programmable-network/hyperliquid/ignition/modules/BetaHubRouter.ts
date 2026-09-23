import { buildModule } from "@nomicfoundation/hardhat-ignition/modules";

// Hyperliquid-only deploy path (DESIGN_V2.md §8.19): plain `BetaHub` needs
// ~5.04M gas to deploy (deployed bytecode 19,118 bytes; code-deposit cost
// alone — 200 gas/byte — is ~3.82M), above HyperEVM testnet's 3,000,000
// block gas limit, so this network deploys `BetaHubGovernanceFacet` +
// `BetaHubAnchorFacet` + `BetaHubMintFacet` + the immutable `BetaHubRouter`
// instead of a plain `BetaHub`.
export default buildModule("BetaHubRouterModule", (m) => {
  const governance = m.getParameter("governance", m.getAccount(0));
  const ipowHeaders = m.getParameter("ipowHeaders");
  const selfNetworkId = m.getParameter("selfNetworkId", 8n); // Hyperliquid (HyperEVM)
  const params = {
    tChallengeSecs: 7n * 86400n,
    tSkipSecs: 28800n,
    refundMarginSecs: 600n,
    unbondDelaySecs: 60n,
    minOperatorBond: 3_000_000_000_000_000n,
    minAuditorBond: 2_000_000_000_000_000n,
    slashWeiPerUnit: 1_000_000_000_000_000n,
    vetoSlashWei: 1_000_000_000_000_000n,
    vetoRewardWei: 500_000_000_000_000n,
    bountyBps: 1000n,
  };

  const governanceFacet = m.contract("BetaHubGovernanceFacet", []);
  const anchorFacet = m.contract("BetaHubAnchorFacet", []);
  const mintFacet = m.contract("BetaHubMintFacet", []);

  const router = m.contract("BetaHubRouter", [
    governance,
    ipowHeaders,
    selfNetworkId,
    params,
    "iBETA",
    "iBETA",
    governanceFacet,
    anchorFacet,
    mintFacet,
  ]);

  return { governanceFacet, anchorFacet, mintFacet, router };
});
