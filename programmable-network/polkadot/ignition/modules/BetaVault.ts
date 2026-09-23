import { buildModule } from "@nomicfoundation/hardhat-ignition/modules";

// BETA (DESIGN_V2.md §6/§8) on Polkadot Hub TestNet: same contract as the
// Ethereum package's BetaVaultV5 (§8.13's ERC20 local-leg support, current
// MINT statement format), pointed at the iPoWV1 relay already deployed
// here. Polkadot Hub's REVM backend follows the standard 18-decimal
// weibar convention (unlike Hedera — see hedera/ignition/modules/
// BetaVault.ts), so these params match the Ethereum deployment's exactly.
export default buildModule("BetaVaultModule", (m) => {
  const deployer = m.getAccount(0);
  const ipow = m.contractAt("iPoWV1", m.getParameter("ipowHeaders", "0x2dD223DcD7F69539Ea895A29095c69c16b088aDb"));
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
  const betaVault = m.contract("BetaVault", [deployer, ipow, params]);
  const mockToken = m.contract("MockERC20", ["Mock BETA-leg USD", "mUSD"], { id: "PolkadotMockToken" });
  return { betaVault, mockToken };
});
