import "dotenv/config";

import type { HardhatUserConfig } from "hardhat/config";
import { configVariable } from "hardhat/config";

import hardhatToolboxMochaEthersPlugin from "@nomicfoundation/hardhat-toolbox-mocha-ethers";

const config: HardhatUserConfig = {
  plugins: [hardhatToolboxMochaEthersPlugin],
  solidity: {
    profiles: {
      default: {
        version: "0.8.28",
        settings: {
          optimizer: {
            enabled: true,
            runs: 200,
          },
          viaIR: true,
        },
      },
      production: {
        version: "0.8.28",
        settings: {
          optimizer: {
            enabled: true,
            runs: 200,
          },
          viaIR: true,
        },
      },
    },
  },
  networks: {
    hardhatMainnet: {
      type: "edr-simulated",
      chainType: "l1",
    },
    hardhatOp: {
      type: "edr-simulated",
      chainType: "op",
    },
    // Hedera's Smart Contract Service is reached through a JSON-RPC relay (Hashio),
    // so from Hardhat's perspective this is just another standard EVM "http" network
    // — no special chainType needed. Chain IDs: testnet 296, mainnet 295, previewnet 297.
    hederaTestnet: {
      type: "http",
      chainType: "l1",
      url: configVariable("HEDERA_TESTNET_RPC_URL"),
      accounts: [configVariable("HEDERA_TESTNET_PRIVATE_KEY")],
      chainId: 296,
    },
  },
};

export default config;
