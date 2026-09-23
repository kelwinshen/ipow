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
    // HyperEVM testnet — Hyperliquid's EVM-compatible execution layer (separate from HyperCore, its non-EVM trading engine).
    // Chain ID 998. Same operator wallet as every other network
    // in this repo (see the root README for the shared address).
    hyperevmTestnet: {
      type: "http",
      chainType: "l1",
      url: configVariable("HYPEREVM_TESTNET_RPC_URL"),
      accounts: [configVariable("HYPEREVM_TESTNET_PRIVATE_KEY")],
      chainId: 998,
    },
  },
};

export default config;
