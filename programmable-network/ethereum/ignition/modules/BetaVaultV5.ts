import { buildModule } from "@nomicfoundation/hardhat-ignition/modules";

// BETA v4 (design/ipow-implementation.md §8.13): ERC20 local-leg support, plus the MINT
// statement now carrying compositionId/componentIndex (§8.4/§8.10) —
// the deployed v4 vault would otherwise misparse a MINT anchored by the
// current Solana hub. Reuses the same Sepolia iPoW relay every prior
// BetaVault version has.
export default buildModule("BetaVaultV5Module", (m) => {
  const deployer = m.getAccount(0);
  const ipow = m.contractAt("iPoW", m.getParameter("ipowHeaders", "0xB8ab960D1121F33B48b4086aBFCDD8B750081588"));
  const params = {
    ethWeiPerUnit: 1_000_000_000_000_000n, // 0.001 ETH
    tFinSecs: 14400n, // 4h
    tChallengeSecs: 7n * 86400n,
    tSkipSecs: 28800n, // 8h
    refundMarginSecs: 600n,
    unbondDelaySecs: 60n,
    minOperatorBond: 3_000_000_000_000_000n, // 0.003 ETH
    minAuditorBond: 2_000_000_000_000_000n, // 0.002 ETH
    vetoSlashWei: 1_000_000_000_000_000n,
    vetoRewardWei: 500_000_000_000_000n,
    bountyBps: 1000n,
  };
  const betaVault = m.contract("BetaVault", [deployer, ipow, params]);
  const mockToken = m.contract("MockERC20", ["Mock BETA-leg USD", "mUSD"], { id: "V5MockToken" });
  return { betaVault, mockToken };
});
