import { buildModule } from "@nomicfoundation/hardhat-ignition/modules";

// BETA (design/ipow-implementation.md §6/§8) on Hedera: same contract as the Ethereum
// package's BetaVaultV5 (§8.13's ERC20 local-leg support, current MINT
// statement format), pointed at the iPoW relay already deployed here.
//
// All native-currency params below are tinybar-scaled (8 decimals), NOT
// the usual 18-decimal weibar convention — see hedera/README.md and
// scripts/configure.ts's nativeLiquidity note: `msg.value` as seen
// *inside* executing contract code is tinybar-scaled on Hedera's HSCS,
// even though Hashio's outer RPC layer accepts standard `parseEther`-
// style 18-decimal values and converts them before execution. Getting
// this wrong means every `deposit()`/`registerParty()` call reverts,
// since `msg.value` would never equal a weibar-scaled `ethWeiPerUnit`.
// The nominal amounts are the same 0.001/0.003/0.002/0.001/0.0005 HBAR-
// equivalent used on every other network, just correctly re-scaled.
export default buildModule("BetaVaultModule", (m) => {
  const deployer = m.getAccount(0);
  const ipow = m.contractAt("iPoW", m.getParameter("ipowHeaders", "0x36D7F82F8B2E800C877592F8DFFF0E8CFAc96CF3"));
  const params = {
    ethWeiPerUnit: 100_000n, // 0.001 HBAR (tinybar-scaled)
    tFinSecs: 14400n,
    tChallengeSecs: 7n * 86400n,
    tSkipSecs: 28800n,
    refundMarginSecs: 600n,
    unbondDelaySecs: 60n,
    minOperatorBond: 300_000n, // 0.003 HBAR
    minAuditorBond: 200_000n, // 0.002 HBAR
    vetoSlashWei: 100_000n, // 0.001 HBAR
    vetoRewardWei: 50_000n, // 0.0005 HBAR
    bountyBps: 1000n,
  };
  const betaVault = m.contract("BetaVault", [deployer, ipow, params]);
  const mockToken = m.contract("MockERC20", ["Mock BETA-leg USD", "mUSD"], { id: "HederaMockToken" });
  return { betaVault, mockToken };
});
