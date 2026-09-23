import type { Signer } from "ethers";

import { IPoWV1__factory } from "../../types/ethers-contracts/index.ts";

// Shared constructor defaults used across the test suite.
//
// iPoW protocol network ID registry (assigned by the protocol, not tied to each
// chain's own chain ID): 1 = Hedera, 2 = Ethereum, 3 = Solana (reserved), 4 = Polkadot.
export const NATIVE_DECIMALS = 18n;
export const SELF_NETWORK_ID = 6n; // Robinhood Chain
export const COMMIT_FEE_BPS = 50n;
export const BPS_DENOM = 10_000n;

export interface DeployOverrides {
  nativeDecimals?: bigint;
  selfNetworkId?: bigint;
  commitFeeBps?: bigint;
}

/**
 * Deploys iPoWV1 via the generated typechain factory rather than
 * `ethers.deployContract("iPoWV1", ...)`. The latter's string-literal overload is
 * keyed to a capitalized `"IPoWV1"` (typechain's PascalCase-ing of a Solidity name
 * that starts with a lowercase letter), which doesn't match the real, case-sensitive
 * artifact name `iPoWV1` — so it type-checks against a loose `BaseContract` and loses
 * all method/property typing. Going through the factory sidesteps that mismatch
 * entirely and returns a fully-typed `IPoWV1` contract instance.
 *
 * `operatorSigner` is used both as the deployer and as the constructor's `_operator`
 * address, matching how every test in this suite deploys.
 */
export async function deployIPoWV1(
  operatorSigner: Signer,
  overrides: DeployOverrides = {}
) {
  const operatorAddress = await operatorSigner.getAddress();
  const contract = await new IPoWV1__factory(operatorSigner).deploy(
    overrides.nativeDecimals ?? NATIVE_DECIMALS,
    overrides.selfNetworkId ?? SELF_NETWORK_ID,
    operatorAddress,
    overrides.commitFeeBps ?? COMMIT_FEE_BPS
  );
  return contract.waitForDeployment();
}
