# iPoW — Robinhood Chain

Same Solidity contracts as [`programmable-network/ethereum`](../ethereum),
deployed to Robinhood Chain testnet. Robinhood Chain is an Arbitrum-Orbit L2 for tokenized equities/stablecoins, testnet launched February 2026 — fully EVM-compatible, standard tooling works unmodified. See the
[root architecture doc](../../docs/design/ipow-implementation.md) for how a conversion
actually flows, and
[design/ipow-implementation.md §8](../../docs/design/ipow-implementation.md) for BETA's composition design.

## New protocol: test network deployment

The new protocol ([`docs/design/ipow-protocol.md`](../../docs/design/ipow-protocol.md)),
deployed on Robinhood testnet (chain 46630) on 2026-10-02 from the one source in
[`../ethereum`](../ethereum) with this network's settings
([`../ethereum/deploy/networks.ts`](../ethereum/deploy/networks.ts)), and read back:
each contract's code compared with the build and each setting read
(`node scripts/verify-deployments.ts` in `../ethereum`). Lowest Bitcoin height
965,567; Conversion's largest swap 100,000 sats. The vault's
pair on Solana is the account `Dydhoos4EgX15LFLJR5a6DvGtuzmNjegpzzynJSJbF5`.

| Contract | Address |
|---|---|
| Light client (`iPoWLightClient`) | `0xc729b1a6d0325ae559614b6826127e11E51703c1` |
| Protocol | `0xd6b425c7908E171a33dF2a4e6C5687eDF0D2d6c3` |
| Reader of the data price (D135) | `0x6AA1F2dd1a0A28F5FC3a88ec8A219e89987A57aB` |
| Conversion | `0xb856906fEBAFBB21A06DdC35E9BCe476139A86BA` |
| BETA (`BetaBaskets`) | `0xd9a6e550Ea8a3970d08F34b0b1a417338C839C4c` |
| Vault home factory | `0x9d2d501827e84d53addA293E70A8995078c87891` |
| Vault receipts factory | `0x10C9C79C8c46c1f9f07D7A258Bbd42B4ED6f973E` |
| Vault paired with Solana | `0x15390C4901e11D3732489825c2FBa33A2E3e5b37` |
| Its home part | `0xfebE817d06A642D8C66fe5548Df61A5AfaFdc201` |
| Its receipts part | `0xA3E1811D33bBAeA900dB14660fdF09C2EAd87A7a` |

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
npx hardhat ignition deploy ignition/modules/IPoW.ts --network robinhoodTestnet
```

Chain ID 46630. Current deployment: `0x53e1291BdAff473694BbbB8DD257f9844e5f9F3c`.

### BetaVault (design/ipow-implementation.md §6/§8)

```sh
npx hardhat ignition deploy ignition/modules/BetaVault.ts --network robinhoodTestnet \
  --parameters '{"BetaVaultModule":{"ipowHeaders":"<iPoW address above>"}}'
```

Current deployment: `0x35e564d74B90a3A5bfcA8Dec65b1325C83d2e822` (plus a
`MockERC20` test token at `0xF43DF008d31995690C75982937a368545953564A`,
deployed alongside it purely to exercise §8.13's ERC20 local-leg
support — not part of the protocol itself).

### BetaHub (design/ipow-implementation.md §8.18/§8.19)

```sh
npx hardhat ignition deploy ignition/modules/BetaHub.ts --network robinhoodTestnet
```

Current deployment: `0xB7054E399E31A2cFE181c4fD59C7235562a6d45d`
(`HubToken`/"iBETA" alongside it).

### iPoWConversion (design/ipow-implementation.md §1–§2/§9.2)

The permissionless-auction Conversion base primitive — its own dedicated
`iPoW` header source, deliberately separate from Beta's above (same
split every other network uses). No operator automation needed — this
one is permissionless by design.

```sh
npx hardhat ignition deploy ignition/modules/IPoWConversion.ts --network robinhoodTestnet
```

Current deployment: `0xaFBdaC4A4e7428C4bA1Bc7E95B7D4A33B2F564f5`
(its own `iPoW`: `0x4C5769e3213496a0641E139e2F0E94ce7625374C`).
Cross-registered with every other network's own `iPoWConversion` via
`addNetwork` — see design/ipow-implementation.md §9.2.

## Post-deploy configuration

`scripts/configure.ts` follows the standard 18-decimal weibar convention
(see `hedera/README.md` if you need the tinybar-scaling caveat that
applies there instead).

```sh
ACTION=status npx hardhat run scripts/configure.ts --network robinhoodTestnet
```
