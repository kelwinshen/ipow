import { buildModule } from "@nomicfoundation/hardhat-ignition/modules";

// Hyperliquid-only deploy path (design/ipow-implementation.md §8.17): same reasoning as
// `Router.ts` for `iPoW` — plain `BetaVault` needs ~4.0M gas to deploy
// (deployed bytecode 16,921 bytes; code-deposit cost alone is ~3.38M),
// above HyperEVM testnet's 3,000,000 block gas limit, so this network
// deploys `BetaVaultCoreFacet` + `BetaVaultAnchorFacet` + the immutable
// `BetaVaultRouter` instead of a plain `BetaVault`. `ipowHeaders` should
// point at `Router.ts`'s deployed `iPoWRouter` address (it satisfies
// `IIPoWHeadersView` via its own inherited public getters).
export default buildModule("BetaVaultRouterModule", (m) => {
  const governance = m.getParameter("governance", m.getAccount(0));
  const ipowHeaders = m.getParameter("ipowHeaders");
  const params = {
    ethWeiPerUnit: 1_000_000_000_000_000n, // 0.001 native
    tFinSecs: 14400n,
    tChallengeSecs: 7n * 86400n,
    tSkipSecs: 28800n,
    refundMarginSecs: 600n,
    unbondDelaySecs: 60n,
    minOperatorBond: 3_000_000_000_000_000n,
    minAuditorBond: 2_000_000_000_000_000n,
    vetoSlashWei: 1_000_000_000_000_000n,
    vetoRewardWei: 500_000_000_000_000n,
    bountyBps: 1000n,
  };

  const coreFacet = m.contract("BetaVaultCoreFacet", []);
  const anchorFacet = m.contract("BetaVaultAnchorFacet", []);

  const router = m.contract("BetaVaultRouter", [governance, ipowHeaders, params, coreFacet, anchorFacet]);
  const mockToken = m.contract("MockERC20", ["Mock BETA-leg USD", "mUSD"], { id: "HyperliquidMockToken" });

  return { coreFacet, anchorFacet, router, mockToken };
});
