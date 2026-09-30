# iPoW — Polkadot

Same Solidity contracts as [`programmable-network/ethereum`](../ethereum),
deployed to Polkadot Hub TestNet. Polkadot Hub's REVM backend runs standard,
unmodified EVM bytecode behind an Ethereum-JSON-RPC-compatible adapter
(`pallet-revive`'s `eth-rpc`), so this is a plain EVM network from Hardhat's
side — deliberately not using `@parity/hardhat-polkadot`, since that plugin
targets PVM/RISC-V via the `resolc` compiler and only supports Hardhat 2.x,
not the Hardhat 3 setup used here. See the
[root architecture doc](../../docs/design/ipow-implementation.md) for how a conversion
actually flows.

## Setup

```sh
pnpm install
cp .env.example .env   # fill in POLKADOT_TESTNET_RPC_URL and POLKADOT_TESTNET_PRIVATE_KEY
```

## Testing

```sh
pnpm test
```

Same 83-test suite as the Ethereum package (the contract source is
identical) — see that package's README for the breakdown.

## Deploying

```sh
npx hardhat ignition deploy ignition/modules/IPoW.ts --network polkadotTestnet
```

Chain ID 420420417 (Polkadot Hub TestNet, on Paseo). Current deployment:
`0x2dD223DcD7F69539Ea895A29095c69c16b088aDb`.

### BetaVault (design/ipow-implementation.md §6/§8)

```sh
npx hardhat ignition deploy ignition/modules/BetaVault.ts --network polkadotTestnet
```

Points at the iPoW address above by default. Current deployment:
`0x4caE61D96FbF49eC853Ad91853Eb2F4b669D04CC` (plus a `MockERC20` test
token at `0x547B24dA787817a7CbA5fb9c40C771aD2f9675F1`, deployed alongside
it purely to exercise §8.13's ERC20 local-leg support — not part of the
protocol itself). Standard 18-decimal weibar convention, same as
`Params` below.

### BetaHub (design/ipow-implementation.md §8.18/§8.19)

```sh
npx hardhat ignition deploy ignition/modules/BetaHub.ts --network polkadotTestnet
```

Current deployment: `0x754FC037B3d2aBf0b63A57a549F21755442a9a16`
(`HubToken`/"iBETA" alongside it).

### iPoWConversion (design/ipow-implementation.md §1–§2/§9.2)

The permissionless-auction Conversion base primitive — its own dedicated
`iPoW` header source, deliberately separate from Beta's above (same
split every other network uses). No operator automation needed — this
one is permissionless by design.

```sh
npx hardhat ignition deploy ignition/modules/IPoWConversion.ts --network polkadotTestnet
```

Current deployment: `0x02A1A50e251f468cF42f7d0e6Ca0d37C52fcb111`
(its own `iPoW`: `0xEB4CF40eFFbec1758b0Fa6a6CC4b4E5a2091b1cF`).
Cross-registered with every other network's own `iPoWConversion` via
`addNetwork` — see design/ipow-implementation.md §9.2.

## Post-deploy configuration

`scripts/configure.ts` is identical to the Ethereum package's — Polkadot
Hub's `nativeLiquidity`/native-value handling follows the standard
18-decimal weibar convention, unlike Hedera.

```sh
ACTION=status npx hardhat run scripts/configure.ts --network polkadotTestnet
```
