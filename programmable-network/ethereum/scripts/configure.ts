// Post-deploy admin/config CLI for the deployed iPoWV1 contract on this chain.
//
// Every action below maps 1:1 to an `onlyOperator` contract function, so the
// signer resolved from the connected network's configured account (see
// hardhat.config.ts) must be the contract's current `operator`.
//
// Usage (all parameters are env vars, since Hardhat 3's `run` task doesn't
// forward extra positional CLI args to the script):
//
//   ACTION=status                npx hardhat run scripts/configure.ts --network sepolia
//   ACTION=add-network      NETWORK_ID=1 [MIN_ADDR_LEN=20] [MAX_ADDR_LEN=20] npx hardhat run scripts/configure.ts --network sepolia
//   ACTION=remove-network    NETWORK_ID=1 npx hardhat run scripts/configure.ts --network sepolia
//   ACTION=add-liquidity        AMOUNT=0.5 npx hardhat run scripts/configure.ts --network sepolia
//   ACTION=remove-liquidity     AMOUNT=0.5 npx hardhat run scripts/configure.ts --network sepolia
//   ACTION=set-fees   COMMIT_FEE_BPS=50 npx hardhat run scripts/configure.ts --network sepolia
//   ACTION=set-operator  NEW_OPERATOR=0x... npx hardhat run scripts/configure.ts --network sepolia
//   ACTION=all         [AMOUNT=0.5]        npx hardhat run scripts/configure.ts --network sepolia
//
// `all` (the default) is the original "post-deploy setup" convenience path:
// it idempotently registers every *other* iPoW-registry network (skipping
// ones already enabled) and, if AMOUNT is set, adds that much liquidity.

import fs from "node:fs";
import path from "node:path";

import { network } from "hardhat";

import { IPoWV1__factory } from "../types/ethers-contracts/index.ts";

// iPoW protocol network ID registry (assigned by the protocol, not tied to
// each chain's own chain ID): 1 = Hedera, 2 = Ethereum, 3 = Solana, 4 = Polkadot.
// addrBytes is the on-the-wire byte length of that chain's native address
// encoding (20-byte EVM addresses for Hedera/Ethereum/Polkadot, since all
// three are EVM-compatible deployments; 32-byte pubkeys for Solana).
const NETWORK_REGISTRY = [
  { id: 1n, name: "Hedera", addrBytes: 20 },
  { id: 2n, name: "Ethereum", addrBytes: 20 },
  { id: 3n, name: "Solana", addrBytes: 32 },
  { id: 4n, name: "Polkadot", addrBytes: 20 },
];

async function getContract() {
  const { ethers } = await network.create();
  const [operator] = await ethers.getSigners();
  const net = await ethers.provider.getNetwork();

  // CONTRACT_ADDRESS lets you target a deployment other than the one Ignition
  // currently tracks for this chain (e.g. a superseded/abandoned contract).
  let address = process.env.CONTRACT_ADDRESS;
  if (!address) {
    const deployedAddressesPath = path.join(
      import.meta.dirname,
      "..",
      "ignition",
      "deployments",
      `chain-${net.chainId}`,
      "deployed_addresses.json"
    );
    if (!fs.existsSync(deployedAddressesPath)) {
      throw new Error(
        `No Ignition deployment found for chain ${net.chainId} at ${deployedAddressesPath}. Deploy first, or set CONTRACT_ADDRESS.`
      );
    }
    const deployedAddresses = JSON.parse(
      fs.readFileSync(deployedAddressesPath, "utf8")
    );
    address = Object.values(deployedAddresses)[0] as string;
  }

  const contract = IPoWV1__factory.connect(address, operator);
  console.log(
    `Connected to iPoWV1 at ${address} on chain ${net.chainId} as ${operator.address}`
  );
  return { contract, ethers, operator };
}

function requireEnv(name: string): string {
  const value = process.env[name];
  if (!value) throw new Error(`Missing required env var ${name}`);
  return value;
}

async function actionStatus() {
  const { contract, ethers } = await getContract();
  const [selfNetworkId, operator, commitFeeBps, nativeLiquidity, balance] =
    await Promise.all([
      contract.SELF_NETWORK_ID(),
      contract.operator(),
      contract.commitFeeBps(),
      contract.nativeLiquidity(),
      ethers.provider.getBalance(await contract.getAddress()),
    ]);

  console.log(`SELF_NETWORK_ID: ${selfNetworkId}`);
  console.log(`operator:        ${operator}`);
  console.log(`commitFeeBps:    ${commitFeeBps}`);
  console.log(`nativeLiquidity: ${ethers.formatEther(nativeLiquidity)}`);
  console.log(`contract balance:${ethers.formatEther(balance)}`);
  console.log(`registered networks:`);
  for (const net of NETWORK_REGISTRY) {
    if (net.id === selfNetworkId) continue;
    const cfg = await contract.networkConfigs(net.id);
    console.log(
      `  ${net.id} (${net.name}): enabled=${cfg.enabled} minAddrLen=${cfg.minAddrLen} maxAddrLen=${cfg.maxAddrLen}`
    );
  }
}

async function actionAddNetwork() {
  const { contract } = await getContract();
  const networkId = BigInt(requireEnv("NETWORK_ID"));
  const known = NETWORK_REGISTRY.find((n) => n.id === networkId);
  const minAddrLen = Number(process.env.MIN_ADDR_LEN ?? known?.addrBytes);
  const maxAddrLen = Number(process.env.MAX_ADDR_LEN ?? known?.addrBytes);
  if (!minAddrLen || !maxAddrLen) {
    throw new Error(
      `Unknown network ${networkId}: pass MIN_ADDR_LEN/MAX_ADDR_LEN explicitly`
    );
  }

  console.log(
    `Registering network ${networkId}${known ? ` (${known.name})` : ""} (minAddrLen=${minAddrLen}, maxAddrLen=${maxAddrLen})...`
  );
  const tx = await contract.addNetwork(networkId, minAddrLen, maxAddrLen);
  await tx.wait();
  console.log(`Done: ${tx.hash}`);
}

async function actionRemoveNetwork() {
  const { contract } = await getContract();
  const networkId = BigInt(requireEnv("NETWORK_ID"));
  console.log(`Removing network ${networkId}...`);
  const tx = await contract.removeNetwork(networkId);
  await tx.wait();
  console.log(`Done: ${tx.hash}`);
}

async function actionAddLiquidity() {
  const { contract, ethers } = await getContract();
  const amount = ethers.parseEther(requireEnv("AMOUNT"));
  console.log(`Adding ${ethers.formatEther(amount)} native liquidity...`);
  const tx = await contract.addNativeLiquidity({ value: amount });
  await tx.wait();
  console.log(`Done: ${tx.hash}`);
}

async function actionRemoveLiquidity() {
  const { contract, ethers } = await getContract();
  const amount = ethers.parseEther(requireEnv("AMOUNT"));
  console.log(`Removing ${ethers.formatEther(amount)} native liquidity...`);
  const tx = await contract.removeNativeLiquidity(amount);
  await tx.wait();
  console.log(`Done: ${tx.hash}`);
}

async function actionSetFees() {
  const { contract } = await getContract();
  const bps = BigInt(requireEnv("COMMIT_FEE_BPS"));
  console.log(`Setting commitFeeBps to ${bps}...`);
  const tx = await contract.setFees(bps);
  await tx.wait();
  console.log(`Done: ${tx.hash}`);
}

async function actionSetOperator() {
  const { contract } = await getContract();
  const newOperator = requireEnv("NEW_OPERATOR");
  console.log(`Transferring operator role to ${newOperator}...`);
  const tx = await contract.setOperator(newOperator);
  await tx.wait();
  console.log(`Done: ${tx.hash}`);
}

async function actionAll() {
  const { contract, ethers } = await getContract();
  const selfNetworkId = await contract.SELF_NETWORK_ID();

  for (const net of NETWORK_REGISTRY) {
    if (net.id === selfNetworkId) continue;
    const cfg = await contract.networkConfigs(net.id);
    if (cfg.enabled) {
      console.log(`  network ${net.id} (${net.name}) already registered, skipping`);
      continue;
    }
    console.log(
      `  registering network ${net.id} (${net.name}), addrLen=${net.addrBytes}`
    );
    const tx = await contract.addNetwork(net.id, net.addrBytes, net.addrBytes);
    await tx.wait();
  }

  const liquidityEnv = process.env.AMOUNT;
  if (liquidityEnv && Number(liquidityEnv) > 0) {
    const amount = ethers.parseEther(liquidityEnv);
    console.log(`  adding ${liquidityEnv} native liquidity`);
    const tx = await contract.addNativeLiquidity({ value: amount });
    await tx.wait();
  } else {
    console.log("  AMOUNT not set (or 0) — skipping addNativeLiquidity");
  }

  console.log("Done.");
}

const ACTIONS: Record<string, () => Promise<void>> = {
  status: actionStatus,
  "add-network": actionAddNetwork,
  "remove-network": actionRemoveNetwork,
  "add-liquidity": actionAddLiquidity,
  "remove-liquidity": actionRemoveLiquidity,
  "set-fees": actionSetFees,
  "set-operator": actionSetOperator,
  all: actionAll,
};

async function main() {
  const action = process.env.ACTION ?? "all";
  const fn = ACTIONS[action];
  if (!fn) {
    throw new Error(
      `Unknown ACTION "${action}". Valid: ${Object.keys(ACTIONS).join(", ")}`
    );
  }
  await fn();
}

main().catch((err) => {
  console.error(err);
  process.exitCode = 1;
});
