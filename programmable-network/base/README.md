# iPoW — Base

Same Solidity contracts as [`programmable-network/ethereum`](../ethereum),
deployed to Base testnet. Base Sepolia is Coinbase's standard OP-Stack L2 testnet — no chain-specific contract changes needed, a plain EVM network from Hardhat's side. See the
[root architecture doc](../../docs/design/ipow-implementation.md) for how a conversion
actually flows, and
[design/ipow-implementation.md §8](../../docs/design/ipow-implementation.md) for BETA's composition design.

## Setup

```sh
pnpm install
cp .env.example .env   # fill in BASE_SEPOLIA_RPC_URL and BASE_SEPOLIA_PRIVATE_KEY
```

## Testing

```sh
pnpm test
```

Same test suite as the Ethereum package (the contract source is
identical) — see that package's README for the breakdown.

## Deploying

```sh
npx hardhat ignition deploy ignition/modules/IPoW.ts --network baseSepolia
```

Chain ID 84532. Current deployment: `0xB7054E399E31A2cFE181c4fD59C7235562a6d45d`.

### BetaVault (design/ipow-implementation.md §6/§8)

```sh
npx hardhat ignition deploy ignition/modules/BetaVault.ts --network baseSepolia \
  --parameters '{"BetaVaultModule":{"ipowHeaders":"<iPoW address above>"}}'
```

Current deployment: `0x6354779b4Dbb564c712ea91c179eCF521C15BE73` (plus a
`MockERC20` test token at `0xE8780640839860F9049132606d18c916956A44A5`,
deployed alongside it purely to exercise §8.13's ERC20 local-leg
support — not part of the protocol itself).

### BetaHub (design/ipow-implementation.md §8.18/§8.19)

```sh
npx hardhat ignition deploy ignition/modules/BetaHub.ts --network baseSepolia
```

Current deployment: `0x24765955eCbffAaACB48a8c3747975d6C750C075`
(`HubToken`/"iBETA" alongside it).

### iPoWConversion (design/ipow-implementation.md §1–§2/§9.2)

The permissionless-auction Conversion base primitive — its own dedicated
`iPoW` header source, deliberately separate from Beta's above (same
split every other network uses). No operator automation needed — this
one is permissionless by design.

```sh
npx hardhat ignition deploy ignition/modules/IPoWConversion.ts --network baseSepolia
```

Current deployment: `0xa43f67C41Fc6e8474278d5a7C1540862CC2a16bA`
(its own `iPoW`: `0x5F63BF3638f8E2BcBD56a4E43cEC7D39637A6f80`).
Cross-registered with every other network's own `iPoWConversion` via
`addNetwork` — see design/ipow-implementation.md §9.2.

## Post-deploy configuration

`scripts/configure.ts` follows the standard 18-decimal weibar convention
(see `hedera/README.md` if you need the tinybar-scaling caveat that
applies there instead).

```sh
ACTION=status npx hardhat run scripts/configure.ts --network baseSepolia
```
