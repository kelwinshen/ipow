import { buildModule } from "@nomicfoundation/hardhat-ignition/modules";

// BETA (DESIGN_V2.md §6/§8) on Hyperliquid testnet (HyperEVM): same contract as the Ethereum
// package's BetaVaultV5 (§8.13's ERC20 local-leg support, current MINT
// statement format). No iPoWV1 relay existed on this network before this
// deploy, unlike Hedera/Polkadot — so `ipowHeaders` has no default here;
// run `ignition/modules/IPoWV1.ts` first and pass its address explicitly
// via --parameters (see README.md). Standard 18-decimal weibar
// convention (no Hedera-style tinybar rescaling needed here).
export default buildModule("BetaVaultModule", (m) => {
  const deployer = m.getAccount(0);
  const ipow = m.contractAt("iPoWV1", m.getParameter("ipowHeaders"));
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
  const mockToken = m.contract("MockERC20", ["Mock BETA-leg USD", "mUSD"], { id: "HyperliquidMockToken" });
  return { betaVault, mockToken };
});
