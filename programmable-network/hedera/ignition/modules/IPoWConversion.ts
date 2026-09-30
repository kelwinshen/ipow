import { buildModule } from "@nomicfoundation/hardhat-ignition/modules";

// A fresh, dedicated iPoW for iPoWConversion — deliberately NOT the
// same instance IPoWModule/BetaVaultModule/BetaHubModule share for Beta
// (this session's own precedent, confirmed on Ethereum: Conversion's own
// iPoWConversion.sol deploy used a fresh IPoWModule rather than
// reusing Beta's, since the two app layers' header-relay liveness needs
// are independent).
export default buildModule("IPoWConversionModule", (m) => {
  const nativeDecimals = m.getParameter("nativeDecimals", 18n);
  const selfNetworkId = m.getParameter("selfNetworkId", 1n); // Hedera
  const governance = m.getParameter("governance", m.getAccount(0));
  const commitFeeBps = m.getParameter("commitFeeBps", 50n);

  const ipowHeaders = m.contract("iPoW", [
    nativeDecimals,
    selfNetworkId,
    governance,
    commitFeeBps,
  ]);

  const iPoWConversion = m.contract("iPoWConversion", [governance, ipowHeaders, commitFeeBps]);

  const mockToken = m.contract("MockERC20", ["Mock Underlying", "MOCKU"]);

  return { ipowHeaders, iPoWConversion, mockToken };
});
