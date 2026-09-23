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
    // Robinhood Chain testnet — an Arbitrum-Orbit L2 for tokenized equities/stablecoins.
    // Chain ID 46630. Same operator wallet as every other network
    // in this repo (see the root README for the shared address).
    robinhoodTestnet: {
      type: "http",
      chainType: "l1",
      url: configVariable("ROBINHOOD_TESTNET_RPC_URL"),
      accounts: [configVariable("ROBINHOOD_TESTNET_PRIVATE_KEY")],
      chainId: 46630,
    },
  },
};

export default config;
