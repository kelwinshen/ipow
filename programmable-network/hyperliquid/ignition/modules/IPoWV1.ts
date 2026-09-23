import { buildModule } from "@nomicfoundation/hardhat-ignition/modules";

export default buildModule("IPoWV1Module", (m) => {
  // iPoW protocol network ID registry: 1 = Hedera, 2 = Ethereum, 3 = Solana
  // (reserved), 4 = Polkadot, 5 = Base, 6 = Robinhood Chain, 7 = Tempo,
  // 8 = Hyperliquid (HyperEVM).
  const nativeDecimals = m.getParameter("nativeDecimals", 18n);
  const selfNetworkId = m.getParameter("selfNetworkId", 8n); // Hyperliquid (HyperEVM)
  const operator = m.getParameter("operator", m.getAccount(0));
  const commitFeeBps = m.getParameter("commitFeeBps", 50n);

  const iPoWV1 = m.contract("iPoWV1", [
    nativeDecimals,
    selfNetworkId,
    operator,
    commitFeeBps,
  ]);

  return { iPoWV1 };
});
