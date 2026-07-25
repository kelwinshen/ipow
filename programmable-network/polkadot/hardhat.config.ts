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
    // Polkadot Hub's REVM backend runs standard, unmodified EVM bytecode via an
    // Ethereum-JSON-RPC-compatible adapter (pallet-revive's eth-rpc), so this is a
    // plain "http" network like any other EVM chain — no special compiler or plugin
    // needed (deliberately NOT using @parity/hardhat-polkadot, which targets PVM/RISC-V
    // via the resolc compiler and only supports Hardhat 2.x, not our Hardhat 3 setup).
    // Chain ID 420420417 = Polkadot Hub TestNet (on Paseo).
    polkadotTestnet: {
      type: "http",
      chainType: "l1",
      url: configVariable("POLKADOT_TESTNET_RPC_URL"),
      accounts: [configVariable("POLKADOT_TESTNET_PRIVATE_KEY")],
      chainId: 420420417,
    },
  },
};

export default config;
