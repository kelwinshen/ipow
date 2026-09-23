import type { Signer } from "ethers";

import { IPoWV1__factory, BetaVault__factory, BetaHub__factory } from "../../types/ethers-contracts/index.ts";
import type { BetaVault, BetaHub } from "../../types/ethers-contracts/index.ts";
import { IPoWV1AdminFacet__factory } from "../../types/ethers-contracts/factories/facets/IPoWV1AdminFacet__factory.ts";
import { IPoWV1ConversionEntryFacet__factory } from "../../types/ethers-contracts/factories/facets/IPoWV1ConversionEntryFacet__factory.ts";
import { IPoWV1ConversionSettlementFacet__factory } from "../../types/ethers-contracts/factories/facets/IPoWV1ConversionSettlementFacet__factory.ts";
import { IPoWV1Router__factory } from "../../types/ethers-contracts/factories/IPoWV1Router__factory.ts";
import { BetaVaultCoreFacet__factory } from "../../types/ethers-contracts/factories/facets/BetaVaultCoreFacet__factory.ts";
import { BetaVaultAnchorFacet__factory } from "../../types/ethers-contracts/factories/facets/BetaVaultAnchorFacet__factory.ts";
import { BetaVaultRouter__factory } from "../../types/ethers-contracts/factories/BetaVaultRouter__factory.ts";
import { BetaHubGovernanceFacet__factory } from "../../types/ethers-contracts/factories/facets/BetaHubGovernanceFacet__factory.ts";
import { BetaHubAnchorFacet__factory } from "../../types/ethers-contracts/factories/facets/BetaHubAnchorFacet__factory.ts";
import { BetaHubMintFacet__factory } from "../../types/ethers-contracts/factories/facets/BetaHubMintFacet__factory.ts";
import { BetaHubRouter__factory } from "../../types/ethers-contracts/factories/BetaHubRouter__factory.ts";

// Shared constructor defaults used across the test suite.
//
// iPoW protocol network ID registry (assigned by the protocol, not tied to each
// chain's own chain ID): 1 = Hedera, 2 = Ethereum, 3 = Solana (reserved),
// 4 = Polkadot, 5 = Base, 6 = Robinhood Chain, 7 = Tempo, 8 = Hyperliquid
// (HyperEVM). This package targets Hyperliquid, not Polkadot — SELF_NETWORK_ID
// was left at Polkadot's value (4) when this package was first scaffolded
// from the polkadot/ template; fixed here.
export const NATIVE_DECIMALS = 18n;
export const SELF_NETWORK_ID = 8n; // Hyperliquid (HyperEVM)
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
  if (process.env.IPOW_SPLIT_DEPLOY === "1") {
    return deployIPoWV1Split(operatorSigner, overrides);
  }
  const operatorAddress = await operatorSigner.getAddress();
  const contract = await new IPoWV1__factory(operatorSigner).deploy(
    overrides.nativeDecimals ?? NATIVE_DECIMALS,
    overrides.selfNetworkId ?? SELF_NETWORK_ID,
    operatorAddress,
    overrides.commitFeeBps ?? COMMIT_FEE_BPS
  );
  return contract.waitForDeployment();
}

/**
 * Deploys the Hyperliquid-only router+facets split (DESIGN_V2.md §8.16)
 * instead of the monolithic `iPoWV1`, then returns a handle typed and ABI-
 * decoded exactly like `deployIPoWV1`'s — attached to the *router's*
 * address using the *monolithic* `IPoWV1` interface, since the router
 * responds to the identical set of function selectors via its `fallback`
 * dispatch (the standard way to interact with a proxy/diamond: the logic
 * contracts' ABI, the proxy's address). This is what makes it possible to
 * run this whole existing test suite against the split version completely
 * unchanged, just by setting `IPOW_SPLIT_DEPLOY=1` — the strongest
 * behavioral-equivalence check available without hand-duplicating every
 * test. See `package.json`'s `test:split` script.
 */
export async function deployIPoWV1Split(
  operatorSigner: Signer,
  overrides: DeployOverrides = {}
) {
  const operatorAddress = await operatorSigner.getAddress();
  const adminFacet = await new IPoWV1AdminFacet__factory(operatorSigner).deploy();
  await adminFacet.waitForDeployment();
  const conversionEntryFacet = await new IPoWV1ConversionEntryFacet__factory(operatorSigner).deploy();
  await conversionEntryFacet.waitForDeployment();
  const conversionSettlementFacet = await new IPoWV1ConversionSettlementFacet__factory(operatorSigner).deploy();
  await conversionSettlementFacet.waitForDeployment();

  const router = await new IPoWV1Router__factory(operatorSigner).deploy(
    overrides.nativeDecimals ?? NATIVE_DECIMALS,
    overrides.selfNetworkId ?? SELF_NETWORK_ID,
    operatorAddress,
    overrides.commitFeeBps ?? COMMIT_FEE_BPS,
    await adminFacet.getAddress(),
    await conversionEntryFacet.getAddress(),
    await conversionSettlementFacet.getAddress()
  );
  await router.waitForDeployment();

  return IPoWV1__factory.connect(await router.getAddress(), operatorSigner);
}

/**
 * Deploys `BetaVault` via the generated typechain factory, or, on
 * `BETAVAULT_SPLIT_DEPLOY=1`, the Hyperliquid-only router+facets split
 * (DESIGN_V2.md §8.17) — same toggle pattern as `deployIPoWV1`'s
 * `IPOW_SPLIT_DEPLOY`. `governanceSigner` deploys and is passed as
 * `_governance`, matching how `test/BetaVault.test.ts` (ported from
 * `ethereum/test/BetaVault.test.ts`) deploys.
 */
export async function deployBetaVault(
  governanceSigner: Signer,
  ipowHeaders: string,
  params: BetaVault.ParamsStruct
) {
  if (process.env.BETAVAULT_SPLIT_DEPLOY === "1") {
    return deployBetaVaultSplit(governanceSigner, ipowHeaders, params);
  }
  const governanceAddress = await governanceSigner.getAddress();
  const contract = await new BetaVault__factory(governanceSigner).deploy(governanceAddress, ipowHeaders, params);
  return contract.waitForDeployment();
}

/**
 * Deploys `BetaVaultCoreFacet` + `BetaVaultAnchorFacet` + `BetaVaultRouter`,
 * then returns a handle typed and ABI-decoded exactly like `deployBetaVault`'s
 * — attached to the *router's* address using the *monolithic* `BetaVault`
 * interface, since the router responds to the identical set of function
 * selectors via its `fallback` dispatch. Same proxy-attachment technique as
 * `deployIPoWV1Split` — see its comment.
 */
export async function deployBetaVaultSplit(
  governanceSigner: Signer,
  ipowHeaders: string,
  params: BetaVault.ParamsStruct
) {
  const governanceAddress = await governanceSigner.getAddress();
  const coreFacet = await new BetaVaultCoreFacet__factory(governanceSigner).deploy();
  await coreFacet.waitForDeployment();
  const anchorFacet = await new BetaVaultAnchorFacet__factory(governanceSigner).deploy();
  await anchorFacet.waitForDeployment();

  const router = await new BetaVaultRouter__factory(governanceSigner).deploy(
    governanceAddress,
    ipowHeaders,
    params,
    await coreFacet.getAddress(),
    await anchorFacet.getAddress()
  );
  await router.waitForDeployment();

  return BetaVault__factory.connect(await router.getAddress(), governanceSigner);
}

/**
 * Deploys `BetaHub` via the generated typechain factory, or, on
 * `BETAHUB_SPLIT_DEPLOY=1`, the Hyperliquid-only router+facets split
 * (DESIGN_V2.md §8.19) — same toggle pattern as `deployIPoWV1`'s
 * `IPOW_SPLIT_DEPLOY` and `deployBetaVault`'s `BETAVAULT_SPLIT_DEPLOY`.
 */
export async function deployBetaHub(
  governanceSigner: Signer,
  ipowHeaders: string,
  selfNetworkId: bigint,
  params: BetaHub.ParamsStruct,
  tokenName: string,
  tokenSymbol: string
) {
  if (process.env.BETAHUB_SPLIT_DEPLOY === "1") {
    return deployBetaHubSplit(governanceSigner, ipowHeaders, selfNetworkId, params, tokenName, tokenSymbol);
  }
  const governanceAddress = await governanceSigner.getAddress();
  const contract = await new BetaHub__factory(governanceSigner).deploy(governanceAddress, ipowHeaders, selfNetworkId, params, tokenName, tokenSymbol);
  return contract.waitForDeployment();
}

/**
 * Deploys `BetaHubGovernanceFacet` + `BetaHubAnchorFacet` + `BetaHubMintFacet`
 * + `BetaHubRouter`, then returns a handle typed and ABI-decoded exactly
 * like `deployBetaHub`'s — attached to the *router's* address using the
 * *monolithic* `BetaHub` interface. Same proxy-attachment technique as
 * `deployIPoWV1Split`/`deployBetaVaultSplit` — see their comments.
 */
export async function deployBetaHubSplit(
  governanceSigner: Signer,
  ipowHeaders: string,
  selfNetworkId: bigint,
  params: BetaHub.ParamsStruct,
  tokenName: string,
  tokenSymbol: string
) {
  const governanceAddress = await governanceSigner.getAddress();
  const governanceFacet = await new BetaHubGovernanceFacet__factory(governanceSigner).deploy();
  await governanceFacet.waitForDeployment();
  const anchorFacet = await new BetaHubAnchorFacet__factory(governanceSigner).deploy();
  await anchorFacet.waitForDeployment();
  const mintFacet = await new BetaHubMintFacet__factory(governanceSigner).deploy();
  await mintFacet.waitForDeployment();

  const router = await new BetaHubRouter__factory(governanceSigner).deploy(
    governanceAddress,
    ipowHeaders,
    selfNetworkId,
    params,
    tokenName,
    tokenSymbol,
    await governanceFacet.getAddress(),
    await anchorFacet.getAddress(),
    await mintFacet.getAddress()
  );
  await router.waitForDeployment();

  return BetaHub__factory.connect(await router.getAddress(), governanceSigner);
}
