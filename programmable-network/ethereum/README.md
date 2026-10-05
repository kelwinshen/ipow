# iPoW — Ethereum

Solidity implementation of the iPoW protocol, deployed to Sepolia. See the
[root architecture doc](../../docs/design/ipow-implementation.md) for how a conversion
actually flows; this covers building, testing, and deploying this package.

## New protocol: test network deployment

The new protocol ([`docs/design/ipow-protocol.md`](../../docs/design/ipow-protocol.md)),
deployed on Sepolia (chain 11155111) on 2026-10-02 from the one source in
[`../ethereum`](../ethereum) with this network's settings
([`../ethereum/deploy/networks.ts`](../ethereum/deploy/networks.ts)), and read back:
each contract's code compared with the build and each setting read
(`node scripts/verify-deployments.ts` in `../ethereum`). Lowest Bitcoin height
965,567; Conversion's largest swap 100,000 sats. The vault's
pair on Solana is the account `5Ks1r25VGX5S7dUMf8c5oP6qzXfFvckQ67wgtqywnDAN`.

| Contract | Address |
|---|---|
| Light client (`iPoWLightClient`) | `0x8e09508deF58997f8481FF678cd7f278DFe22C3F` |
| Protocol | `0x21348ef459dDBa5607732Ca1e9992968B4cC5611` |
| Conversion | `0x9dF80412e01720F904f5059559eD6f66c7558803` |
| BETA (`BetaBaskets`), named baskets and parts named before bridged (E4, E5), 2026-10-05 | `0x7311B038A8e2c1F1C2b6a266E6cA38ed3804F3BF` |
| BETA, the E4-only build of the same morning (its baskets stay there) | `0x80f99051921754C0b2F7BeC7468b72a0E6e2850b` |
| BETA, the build of 2026-10-02 (its baskets stay there) | `0xb5375fDaF8b6E04942dFc593E7065A606Fe5588d` |
| Vault home factory | `0xfeD6F6DbA0860C9E8cECDf33d4BE60B0Da6980b3` |
| Vault receipts factory | `0x3Dda5326A3380E8b24D6a482CC1c3205Ae535502` |
| Vault paired with Solana | `0xD18b7d290f8c94fe561710A78F17494ff2520302` |
| Its home part | `0x89b83188c77172dB53873e4E67181a627753eD6d` |
| Its receipts part | `0xd72b80270BB25AD1Cb826e4C43ee1359dBB2DC79` |
| Vault paired with HyperEVM testnet (2026-10-05) | `0x181e3C5D35E4ecaC682FEFC375dEc9CC5D1d3A91`; home `0x122f0C67aa722861E68d03682707AA7C061cDE48`, receipts `0x947BcdA4F8F6787B38484A8389077c901e9e8A39`, its factories `0xCC6B502c4bb87ffD2fBA3412F8EcC7c27659B12A` and `0x769d3CB24E20C77b45Bb0FCec35c7750f7E02CfC` |
| Vault paired with Robinhood testnet (2026-10-05) | `0x995D56414b04Fe2699978C7E0B945FF623c1326b`; home `0x0e81c951966FFE6E6067266a2Ff237AfE35E6729`, receipts `0x44DeBbBB5f4d91a1de3228C7796C7193a648c6Be`, its factories `0xe5aA792d82281Bd86fD297290ac7eEe72C79b1C1` and `0x5B9148d4f4f91A83CD9642d3cd6E08F01bbd0826` |
| Vault paired with Base Sepolia (2026-10-05) | `0x7901fd0a77124EBA55cB4599F67b0405F43E00c3`; home `0x71b935346d2A8F7709abeE700fb7a2470A6F19A1`, receipts `0x50Cb86cdB69b08a89C8a1e02C5cAA5177B1cf2B0`, its factories `0xF4475a11a71940CF09468e6397565321AE49d333` and `0xD630Fe315F8F1b78e0aE2095dc9F93Dc64D43988` |

## Contracts

- `contracts/iPoW.sol` — main contract: commit/approve/settle logic,
  header relay, liquidity management.
- `contracts/base/iPoWTypes.sol` — shared errors, events, structs,
  enums, and constants. `iPoW` inherits this, so it all compiles into the
  same deployed contract.
- `contracts/libraries/BitcoinPrimitives.sol` — Bitcoin transaction/header
  parsing and proof-of-work verification, as an internal library.

## Setup

```sh
pnpm install
cp .env.example .env   # fill in SEPOLIA_RPC_URL and SEPOLIA_PRIVATE_KEY
```

## Testing

```sh
pnpm test
```

83 tests across five suites (`test/iPoW.Admin.test.ts`,
`test/iPoW.CommitValidation.test.ts`, `test/iPoW.HeaderRelay.test.ts`,
`test/iPoW.Liquidity.test.ts`, `test/BitcoinPrimitives.test.ts`) — the
last one runs against `contracts/test/BitcoinPrimitivesHarness.sol`, a
thin harness that exposes the library's `internal` functions for testing,
since Solidity libraries can't be called directly from outside a contract.

## Deploying

Deployment is managed by Hardhat Ignition (`ignition/modules/IPoW.ts`):

```sh
npx hardhat ignition deploy ignition/modules/IPoW.ts --network sepolia
```

Ignition tracks deployments by network under
`ignition/deployments/chain-<id>/`; re-running the same module against a
network that already has a tracked deployment reuses it rather than
redeploying. Pass `--reset` to force a fresh deployment instead.

Current Sepolia deployment: `0x3a6b4B540BAc87056618696dc3fE6929c28e9b6C`.

## Post-deploy configuration

`scripts/configure.ts` is the admin CLI for a deployed contract — it's the
one place to register other iPoW networks, manage liquidity, adjust fees,
or hand off the operator role. It's driven entirely by environment
variables (see the comment block at the top of the file for the full
command list), for example:

```sh
ACTION=status npx hardhat run scripts/configure.ts --network sepolia
ACTION=add-liquidity AMOUNT=0.5 npx hardhat run scripts/configure.ts --network sepolia
```
