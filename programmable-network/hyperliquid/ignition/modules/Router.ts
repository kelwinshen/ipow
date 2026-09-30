import { buildModule } from "@nomicfoundation/hardhat-ignition/modules";

// Hyperliquid-only deploy path (design/ipow-implementation.md §8.16): HyperEVM testnet's block
// gas limit (3,000,000) is below what the monolithic `iPoW` needs to deploy
// (~4.79M), so this network deploys the three facets + the immutable router
// instead of `IPoW.ts`'s single `iPoW` contract. The router satisfies the
// exact same external interface (including BetaVault's `IIPoWHeadersView`
// dependency, via its own inherited public getters) — `BetaVault.ts`'s
// `ipowHeaders` parameter should point at this module's `router` address.
export default buildModule("RouterModule", (m) => {
  // iPoW protocol network ID registry: 1 = Hedera, 2 = Ethereum, 3 = Solana
  // (reserved), 4 = Polkadot, 5 = Base, 6 = Robinhood Chain, 7 = Tempo,
  // 8 = Hyperliquid (HyperEVM).
  const nativeDecimals = m.getParameter("nativeDecimals", 18n);
  const selfNetworkId = m.getParameter("selfNetworkId", 8n); // Hyperliquid (HyperEVM)
  const operator = m.getParameter("operator", m.getAccount(0));
  const commitFeeBps = m.getParameter("commitFeeBps", 50n);

  const adminFacet = m.contract("iPoWAdminFacet", []);
  const conversionEntryFacet = m.contract("iPoWConversionEntryFacet", []);
  const conversionSettlementFacet = m.contract("iPoWConversionSettlementFacet", []);

  const router = m.contract("iPoWRouter", [
    nativeDecimals,
    selfNetworkId,
    operator,
    commitFeeBps,
    adminFacet,
    conversionEntryFacet,
    conversionSettlementFacet,
  ]);

  return { adminFacet, conversionEntryFacet, conversionSettlementFacet, router };
});
