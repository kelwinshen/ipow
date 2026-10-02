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

## New protocol: test network deployment

The new protocol ([`docs/design/ipow-protocol.md`](../../docs/design/ipow-protocol.md)),
deployed on Polkadot testnet (chain 420420417) on 2026-10-02 from the one source in
[`../ethereum`](../ethereum) with this network's settings
([`../ethereum/deploy/networks.ts`](../ethereum/deploy/networks.ts)), and read back:
each contract's code compared with the build and each setting read
(`node scripts/verify-deployments.ts` in `../ethereum`). Lowest Bitcoin height
965,567; Conversion's largest swap 100,000 sats. The vault's
pair on Solana is the account `BcTMgkGcknJu9GW6XQ7HoRdXdP3Jo6iRt8ZjKe6fm7r7`.

| Contract | Address |
|---|---|
| Light client (`iPoWLightClient`) | `0x2E2b4eFf60659AFc08ae85E8A124a515008AEb9f` |
| Protocol | `0x5D38c6Bec531A3290f121C39C1f88231fa4B49b9` |
| Conversion | `0x3221102aC4159048960e061FC0ADb81De3133496` |
| BETA (`BetaBaskets`) | `0xB001c1Eb3B2D6dF4380682E119dDF1e3a9F18D89` |
| Vault home factory | `0x2ab2c9487cC8f83816d556Da149132c705B82e48` |
| Vault receipts factory | `0x895584abec0b2971A97c5De927BBE46cFAb9A618` |
| Vault paired with Solana | `0x77ef03207173b08A82d6511a35f970033E185d6c` |
| Its home part | `0xC69EBc691174a55E240552674896bBE651ebBad2` |
| Its receipts part | `0x6708f6882d9a0F0335aDAbe944c8436936679435` |

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
