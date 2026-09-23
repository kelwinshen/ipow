import { buildModule } from "@nomicfoundation/hardhat-ignition/modules";

// BetaHub (DESIGN_V2.md §8.18) on Hedera: an EVM chain acting as a mint
// hub, not just a spoke — same contract as the Ethereum pilot's BetaHub.
//
// All native-currency params below are tinybar-scaled (8 decimals), NOT
// the usual 18-decimal weibar convention — see hedera/README.md and
// hedera/ignition/modules/BetaVault.ts's own note: `msg.value` as seen
// *inside* executing contract code is tinybar-scaled on Hedera's HSCS.
// Getting this wrong means every `registerParty()`/`lockLocal()` call
// with a native leg reverts.
export default buildModule("BetaHubModule", (m) => {
  const deployer = m.getAccount(0);
  const ipow = m.contractAt("iPoWV1", m.getParameter("ipowHeaders", "0x36D7F82F8B2E800C877592F8DFFF0E8CFAc96CF3"));
  const selfNetworkId = m.getParameter("selfNetworkId", 1n); // Hedera
  const params = {
    tChallengeSecs: 7n * 86400n,
    tSkipSecs: 28800n,
    refundMarginSecs: 600n,
    unbondDelaySecs: 60n,
    minOperatorBond: 300_000n, // 0.003 HBAR (tinybar-scaled)
    minAuditorBond: 200_000n, // 0.002 HBAR
    slashWeiPerUnit: 100_000n, // 0.001 HBAR per unit
    vetoSlashWei: 100_000n, // 0.001 HBAR
    vetoRewardWei: 50_000n, // 0.0005 HBAR
    bountyBps: 1000n,
  };
  const betaHub = m.contract("BetaHub", [deployer, ipow, selfNetworkId, params, "iBETA", "iBETA"]);
  return { betaHub };
});
