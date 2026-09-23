# iPoW — Robinhood Chain

Same Solidity contracts as [`programmable-network/ethereum`](../ethereum),
deployed to Robinhood Chain testnet. Robinhood Chain is an Arbitrum-Orbit L2 for tokenized equities/stablecoins, testnet launched February 2026 — fully EVM-compatible, standard tooling works unmodified. See the
[root architecture doc](../../docs/ARCHITECTURE.md) for how a conversion
actually flows, and
[DESIGN_V2.md §8](../../docs/DESIGN_V2.md) for BETA's composition design.

## Setup

```sh
pnpm install
cp .env.example .env   # fill in ROBINHOOD_TESTNET_RPC_URL and ROBINHOOD_TESTNET_PRIVATE_KEY
```

## Testing

```sh
pnpm test
```

Same test suite as the Ethereum package (the contract source is
identical) — see that package's README for the breakdown.

## Deploying

```sh
npx hardhat ignition deploy ignition/modules/IPoWV1.ts --network robinhoodTestnet
```

Chain ID 46630. Current deployment: `0x53e1291BdAff473694BbbB8DD257f9844e5f9F3c`.

### BetaVault (DESIGN_V2.md §6/§8)

```sh
npx hardhat ignition deploy ignition/modules/BetaVault.ts --network robinhoodTestnet \
  --parameters '{"BetaVaultModule":{"ipowHeaders":"<iPoWV1 address above>"}}'
```

Current deployment: `0x35e564d74B90a3A5bfcA8Dec65b1325C83d2e822` (plus a
`MockERC20` test token at `0xF43DF008d31995690C75982937a368545953564A`,
deployed alongside it purely to exercise §8.13's ERC20 local-leg
support — not part of the protocol itself).

### BetaHub (DESIGN_V2.md §8.18/§8.19)

```sh
npx hardhat ignition deploy ignition/modules/BetaHub.ts --network robinhoodTestnet
```

Current deployment: `0xB7054E399E31A2cFE181c4fD59C7235562a6d45d`
(`HubToken`/"iBETA" alongside it).

### iPoWV1Conversion (DESIGN_V2.md §1–§2/§9.2)

The permissionless-auction Conversion base primitive — its own dedicated
`iPoWV1` header source, deliberately separate from Beta's above (same
split every other network uses). No operator automation needed — this
one is permissionless by design.

```sh
npx hardhat ignition deploy ignition/modules/IPoWV1Conversion.ts --network robinhoodTestnet
```

Current deployment: `0xaFBdaC4A4e7428C4bA1Bc7E95B7D4A33B2F564f5`
(its own `iPoWV1`: `0x4C5769e3213496a0641E139e2F0E94ce7625374C`).
Cross-registered with every other network's own `iPoWV1Conversion` via
`addNetwork` — see DESIGN_V2.md §9.2.

## Post-deploy configuration

`scripts/configure.ts` follows the standard 18-decimal weibar convention
(see `hedera/README.md` if you need the tinybar-scaling caveat that
applies there instead).

```sh
ACTION=status npx hardhat run scripts/configure.ts --network robinhoodTestnet
```
