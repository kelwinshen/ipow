# iPoW — Polkadot

Same Solidity contracts as [`programmable-network/ethereum`](../ethereum),
deployed to Polkadot Hub TestNet. Polkadot Hub's REVM backend runs standard,
unmodified EVM bytecode behind an Ethereum-JSON-RPC-compatible adapter
(`pallet-revive`'s `eth-rpc`), so this is a plain EVM network from Hardhat's
side — deliberately not using `@parity/hardhat-polkadot`, since that plugin
targets PVM/RISC-V via the `resolc` compiler and only supports Hardhat 2.x,
not the Hardhat 3 setup used here. See the
[root architecture doc](../../docs/ARCHITECTURE.md) for how a conversion
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
npx hardhat ignition deploy ignition/modules/IPoWV1.ts --network polkadotTestnet
```

Chain ID 420420417 (Polkadot Hub TestNet, on Paseo). Current deployment:
`0x2dD223DcD7F69539Ea895A29095c69c16b088aDb`.

## Post-deploy configuration

`scripts/configure.ts` is identical to the Ethereum package's — Polkadot
Hub's `nativeLiquidity`/native-value handling follows the standard
18-decimal weibar convention, unlike Hedera.

```sh
ACTION=status npx hardhat run scripts/configure.ts --network polkadotTestnet
```
