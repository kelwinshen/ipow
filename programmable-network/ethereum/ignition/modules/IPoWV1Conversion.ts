import { buildModule } from "@nomicfoundation/hardhat-ignition/modules";

// Points at the already-deployed iPoWV1 on Sepolia
// (0x3a6b4B540BAc87056618696dc3fE6929c28e9b6C) rather than deploying a new
// one — iPoWV1Conversion only ever reads its header relay, never writes to
// it.
export default buildModule("IPoWV1ConversionModule", (m) => {
  const governance = m.getParameter("governance", m.getAccount(0));
  const ipowHeaders = m.getParameter("ipowHeaders", "0x3a6b4B540BAc87056618696dc3fE6929c28e9b6C");
  const commitFeeBps = m.getParameter("commitFeeBps", 50n);

  const iPoWV1Conversion = m.contract("iPoWV1Conversion", [governance, ipowHeaders, commitFeeBps]);

  const mockToken = m.contract("MockERC20", ["Mock Underlying", "MOCKU"]);

  return { iPoWV1Conversion, mockToken };
});
