# iPoW — Hyperliquid (HyperEVM)

Same protocol as [`programmable-network/ethereum`](../ethereum), deployed to
Hyperliquid (HyperEVM) testnet — but not the same bytecode. Hyperliquid has
two layers: HyperCore (its own non-EVM trading engine — not a deploy target)
and HyperEVM (a separate, EVM-compatible execution layer sharing the same
chain). This package targets HyperEVM, whose native gas token is HYPE, not
ETH. See the [root architecture doc](../../docs/ARCHITECTURE.md) for how a
conversion actually flows, and [DESIGN_V2.md §8](../../docs/DESIGN_V2.md) /
[§9](../../docs/DESIGN_V2.md) for BETA's composition design and Conversion's.

## Router + facets, not plain contracts

HyperEVM testnet's block gas limit (3,000,000) is well under what the plain
`iPoWV1Conversion`/`BetaHub`/`BetaVault` contracts cost to deploy — e.g.
plain `BetaHub`'s deployed bytecode alone needs ~5.04M gas just for the
code-deposit cost. Every contract here is instead split Diamond-style into
a thin, immutable **router** plus separate **facet** contracts holding the
actual logic, reached via `delegatecall` dispatch — the router exposes the
identical external ABI, so every off-chain caller (including
`core/operator`) is unaffected and unaware of the split. See
`contracts/facets/` for the facet sources and
`ignition/modules/*Router.ts` for how each router wires them together
(each module's own header comment explains the specific gas math for that
contract).

Three independent router deployments exist here:

- **`iPoWV1Router`** — serves *both* `iPoWV1` (the Bitcoin header relay)
  and `iPoWV1Conversion` (the permissionless-auction Conversion base
  primitive) behind one router, via `iPoWV1AdminFacet` +
  `iPoWV1ConversionEntryFacet` + `iPoWV1ConversionSettlementFacet`.
- **`BetaVaultRouter`** — Beta's per-network spoke, via
  `BetaVaultCoreFacet` + `BetaVaultAnchorFacet`.
- **`BetaHubRouter`** — Beta's mint-chain hub, via
  `BetaHubGovernanceFacet` + `BetaHubAnchorFacet` + `BetaHubMintFacet`.

## Setup

```sh
pnpm install
cp .env.example .env   # fill in HYPEREVM_TESTNET_RPC_URL and HYPEREVM_TESTNET_PRIVATE_KEY
```

## Testing

```sh
pnpm test
```

123 tests: the same suites as the Ethereum package, run against the
router+facets deployment instead of the plain contracts, so the dispatch
split itself is exercised on every test, not just the underlying logic.

## Deploying

```sh
npx hardhat ignition deploy ignition/modules/Router.ts --network hyperevmTestnet
npx hardhat ignition deploy ignition/modules/BetaVaultRouter.ts --network hyperevmTestnet \
  --parameters '{"BetaVaultRouterModule":{"ipowHeaders":"<iPoWV1Router address above>"}}'
npx hardhat ignition deploy ignition/modules/BetaHubRouter.ts --network hyperevmTestnet \
  --parameters '{"BetaHubRouterModule":{"ipowHeaders":"<iPoWV1Router address above>"}}'
```

Chain ID 998.

## Currently live (HyperEVM testnet)

| Router | Address | Facets behind it |
| --- | --- | --- |
| `iPoWV1Router` (iPoWV1 + iPoWV1Conversion) | `0xB7054E399E31A2cFE181c4fD59C7235562a6d45d` | AdminFacet `0x53e1291BdAff473694BbbB8DD257f9844e5f9F3c`, ConversionEntryFacet `0xF43DF008d31995690C75982937a368545953564A`, ConversionSettlementFacet `0x35e564d74B90a3A5bfcA8Dec65b1325C83d2e822` |
| `BetaVaultRouter` | `0x554Fe13e4a5d0931e7c8F7d3E74Dea8Ca04C244a` | CoreFacet `0x24765955eCbffAaACB48a8c3747975d6C750C075`, AnchorFacet `0x6354779b4Dbb564c712ea91c179eCF521C15BE73` (plus a `HyperliquidMockToken` test token at `0x806Ae4940f1a68e5371D27f9C8cb9528872aeBd1`) |
| `BetaHubRouter` | `0x4C5769e3213496a0641E139e2F0E94ce7625374C` | GovernanceFacet `0x900D54050f9Fe47ca56cC67A281947Da2EeE2cD1`, AnchorFacet `0x8b9efF66C7D93816Eadf702cC6cE273e8B2b03e7`, MintFacet `0xC40003975B6ff70E46999Bac8f3b06a9908Fb333` |

Always call the router address — the facet addresses are implementation
detail, not separate entry points.

## Post-deploy configuration

`scripts/configure.ts` follows the standard 18-decimal weibar convention
(see `hedera/README.md` if you need the tinybar-scaling caveat that
applies there instead), and targets the router addresses the same way it
would a plain contract.

```sh
ACTION=status npx hardhat run scripts/configure.ts --network hyperevmTestnet
```
