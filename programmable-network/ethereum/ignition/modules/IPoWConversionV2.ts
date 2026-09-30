import { buildModule } from "@nomicfoundation/hardhat-ignition/modules";

// Redeploy of iPoWConversion on Sepolia — the original
// IPoWConversionModule's instance predates addNetwork/networkConfigs/
// openBundleTunnel entirely (10,641 bytes deployed vs. 13,918 bytes for
// the current source), confirmed via on-chain bytecode comparison. That
// old instance had 5 real committed conversions (4 already Completed, 1
// negligible stuck Committed one — already the known "never claimed"
// gap, not new risk) and is abandoned here, not migrated. Reuses the same
// already-deployed iPoW header relay the original module pointed at,
// for the same reason that module gave: iPoWConversion only ever reads
// it, never writes to it.
export default buildModule("IPoWConversionV2Module", (m) => {
  const governance = m.getParameter("governance", m.getAccount(0));
  const ipowHeaders = m.getParameter("ipowHeaders", "0x3a6b4B540BAc87056618696dc3fE6929c28e9b6C");
  const commitFeeBps = m.getParameter("commitFeeBps", 50n);

  const iPoWConversion = m.contract("iPoWConversion", [governance, ipowHeaders, commitFeeBps]);

  return { iPoWConversion };
});
