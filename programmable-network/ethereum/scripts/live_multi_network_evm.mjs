// Real EVM-side setup for scripts/live_multi_network_mint.ts's composition
// id=2: register the SAME operator party (id 0x05 repeated 32x) used on
// Solana, pointed at the SAME real Bitcoin chain head, then deposit 1 unit
// of native currency on each of the four remote BetaVault deployments to
// get a real lockId. Solana pending nonce/deadline must match exactly
// (fetched from the live pending account, not recomputed here).
//
//   node scripts/live_multi_network_evm.mjs <network>
// network: ethereum | base | robinhood | hyperliquid

import { ethers } from "ethers";
import fs from "node:fs";

const PARTY_ID = "0x" + "05".repeat(32);
const CHAIN_HEAD_TXID_LE = "0x90ac6e14edc401dfb799fd166d310733dbb6b5785ce69ca1217f1d9cfae47149";
const CHAIN_HEAD_VOUT = 0;
const SOL_USER = "0xe0e72437e8caeddcb5368336c372742770ee2e3bb3b8e426a6d961e57a14f74a";
const NONCE = 2n;
const UNITS = 1n;
const DEADLINE = 1790724106n;
const BOND = ethers.parseEther("0.01"); // above every network's minOperatorBond (0.003 or 0.004 native)

const NETWORKS = {
  ethereum: {
    envDir: "../../ethereum",
    rpcKey: "SEPOLIA_RPC_URL",
    pkKey: "SEPOLIA_PRIVATE_KEY",
    betaVault: "0x30A386AEc5aD0afAcC6C8624b47ABbB719596a6b",
    artifact: "../../ethereum/artifacts/contracts/BetaVault.sol/BetaVault.json",
  },
  base: {
    envDir: "../../base",
    rpcKey: "BASE_SEPOLIA_RPC_URL",
    pkKey: "BASE_SEPOLIA_PRIVATE_KEY",
    betaVault: "0x6354779b4Dbb564c712ea91c179eCF521C15BE73",
    artifact: "../../base/artifacts/contracts/BetaVault.sol/BetaVault.json",
  },
  robinhood: {
    envDir: "../../robinhood",
    rpcKey: "ROBINHOOD_TESTNET_RPC_URL",
    pkKey: "ROBINHOOD_TESTNET_PRIVATE_KEY",
    betaVault: "0x35e564d74B90a3A5bfcA8Dec65b1325C83d2e822",
    artifact: "../../robinhood/artifacts/contracts/BetaVault.sol/BetaVault.json",
  },
  hyperliquid: {
    envDir: "../../hyperliquid",
    rpcKey: "HYPEREVM_TESTNET_RPC_URL",
    pkKey: "HYPEREVM_TESTNET_PRIVATE_KEY",
    betaVault: "0x554Fe13e4a5d0931e7c8F7d3E74Dea8Ca04C244a", // BetaVaultRouter — plain BetaVault ABI works via delegatecall dispatch
    artifact: "../../hyperliquid/artifacts/contracts/BetaVault.sol/BetaVault.json",
  },
};

function loadEnv(dir) {
  const text = fs.readFileSync(new URL(dir + "/.env", import.meta.url), "utf8");
  return Object.fromEntries(
    text.split("\n").filter((l) => l.includes("=") && !l.startsWith("#")).map((l) => {
      const i = l.indexOf("=");
      return [l.slice(0, i).trim(), l.slice(i + 1).trim().replace(/^"|"$/g, "")];
    })
  );
}

async function main() {
  const name = process.argv[2];
  const cfg = NETWORKS[name];
  if (!cfg) throw new Error("usage: node live_multi_network_evm.mjs <ethereum|base|robinhood|hyperliquid>");
  const env = loadEnv(cfg.envDir);
  const provider = new ethers.JsonRpcProvider(env[cfg.rpcKey]);
  const wallet = new ethers.Wallet(env[cfg.pkKey], provider);
  const artifact = JSON.parse(fs.readFileSync(new URL(cfg.artifact, import.meta.url), "utf8"));
  const vault = new ethers.Contract(cfg.betaVault, artifact.abi, wallet);

  console.log(name, "operator wallet:", wallet.address, "balance:", ethers.formatEther(await provider.getBalance(wallet.address)));

  const existingParty = await vault.parties(PARTY_ID);
  if (!existingParty.exists) {
    const govAddr = await vault.governance();
    console.log("governance:", govAddr, "(should equal operator wallet — approving+registering as governance+operator, same key)");
    const approveTx = await vault.approveOperator(PARTY_ID, wallet.address);
    await approveTx.wait();
    console.log("approveOperator sig", approveTx.hash);
    const registerTx = await vault.registerParty(PARTY_ID, 0 /* Operator */, CHAIN_HEAD_TXID_LE, CHAIN_HEAD_VOUT, { value: BOND });
    await registerTx.wait();
    console.log("registerParty sig", registerTx.hash);
  } else {
    console.log("party already registered on this vault, skipping");
  }

  const params = await vault.params();
  const unitCost = params.ethWeiPerUnit * UNITS;
  console.log("depositing", ethers.formatEther(unitCost), "native for", UNITS, "unit(s)");
  const nextLockId = await vault.nextLockId();
  const depositTx = await vault.deposit(ethers.ZeroAddress, SOL_USER, NONCE, UNITS, DEADLINE, { value: unitCost });
  const receipt = await depositTx.wait();
  console.log("deposit sig", depositTx.hash, "assigned lockId (from nextLockId before call):", nextLockId.toString());

  const lock = await vault.locks(nextLockId);
  console.log("verify lock:", { solUser: lock.solUser, nonce: lock.nonce.toString(), units: lock.units.toString(), deadline: lock.deadline.toString(), state: lock.state.toString() });
  console.log(`\n${name} lockId = ${nextLockId.toString()}`);
}

main().catch((e) => {
  console.error(e);
  process.exit(1);
});
