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

- `contracts/protocol/` — the protocol: the light client
  (`iPoWLightClient`), the protocol (`iPoWProtocol`, `iPoWProtocolToken`),
  its data fee, the vault (`iPoWVault`, `iPoWVaultToken`) and the vault's
  parts in `contracts/protocol/vault/`.
- `contracts/apps/` — the applications on it: Conversion and BETA
  (`BetaBaskets`).
- `contracts/testnet/` — the test networks' mock RWA token.
- `contracts/test/` — harnesses and mocks used only by the tests.

The old contract generation (`iPoW`, `iPoWConversion`, `BetaHub`,
`BetaVault`) was removed; its source and Ignition deployment records stay
in git at the tag `legacy-v1`.

## Setup

```sh
pnpm install
cp .env.example .env   # fill in SEPOLIA_RPC_URL and SEPOLIA_PRIVATE_KEY
```

## Testing

```sh
pnpm test
```

## Deploying

Deployments are scripts over the shared deploy function
(`deploy/deploy.ts`) and each network's settings (`deploy/networks.ts`);
each writes the addresses it reads back to `deployments/`. See the comment
at the top of each: `scripts/deploy-network.ts` (one network),
`scripts/deploy-testnets.sh` (the test networks in order), and
`scripts/verify-deployments.ts` (reads every deployment back).
