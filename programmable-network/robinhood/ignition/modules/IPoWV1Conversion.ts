import { buildModule } from "@nomicfoundation/hardhat-ignition/modules";

// A fresh, dedicated iPoWV1 for iPoWV1Conversion — deliberately NOT the
// same instance IPoWV1Module/BetaVaultModule/BetaHubModule share for Beta
// (this session's own precedent, confirmed on Ethereum: Conversion's own
// iPoWV1Conversion.sol deploy used a fresh IPoWV1Module rather than
// reusing Beta's, since the two app layers' header-relay liveness needs
// are independent).
export default buildModule("IPoWV1ConversionModule", (m) => {
  const nativeDecimals = m.getParameter("nativeDecimals", 18n);
  const selfNetworkId = m.getParameter("selfNetworkId", 6n); // Robinhood Chain
  const governance = m.getParameter("governance", m.getAccount(0));
  const commitFeeBps = m.getParameter("commitFeeBps", 50n);

  const ipowHeaders = m.contract("iPoWV1", [
    nativeDecimals,
    selfNetworkId,
    governance,
    commitFeeBps,
  ]);

  const iPoWV1Conversion = m.contract("iPoWV1Conversion", [governance, ipowHeaders, commitFeeBps]);

  const mockToken = m.contract("MockERC20", ["Mock Underlying", "MOCKU"]);

  return { ipowHeaders, iPoWV1Conversion, mockToken };
});
