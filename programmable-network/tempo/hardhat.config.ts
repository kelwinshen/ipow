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
    // Tempo's "Moderato" public testnet — a Stripe/Paradigm payments-focused L1.
    // Chain ID 42431. Same operator wallet as every other network
    // in this repo (see the root README for the shared address).
    tempoTestnet: {
      type: "http",
      chainType: "l1",
      url: configVariable("TEMPO_TESTNET_RPC_URL"),
      accounts: [configVariable("TEMPO_TESTNET_PRIVATE_KEY")],
      chainId: 42431,
    },
  },
};

export default config;
