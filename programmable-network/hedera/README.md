# iPoW — Hedera

Same Solidity contracts as [`programmable-network/ethereum`](../ethereum),
deployed to Hedera Testnet. Hedera's Smart Contract Service is reached
through the Hashio JSON-RPC relay, so from Hardhat's perspective this is
just another standard EVM network — no chain-specific contract changes were
needed. See the [root architecture doc](../../docs/design/ipow-implementation.md) for how
a conversion actually flows.

## New protocol: test network deployment

The new protocol ([`docs/design/ipow-protocol.md`](../../docs/design/ipow-protocol.md)),
deployed on Hedera testnet (chain 296) on 2026-10-02 from the one source in
[`../ethereum`](../ethereum) with this network's settings
([`../ethereum/deploy/networks.ts`](../ethereum/deploy/networks.ts)), and read back:
each contract's code compared with the build and each setting read
(`node scripts/verify-deployments.ts` in `../ethereum`). Lowest Bitcoin height
965,567; Conversion's largest swap 100,000 sats. The vault's
pair on Solana is the account `7Wwn5bjARBmdCjNT7KUBwB7LR3JYSLFAWSSm98pxvE2S`.

| Contract | Address |
|---|---|
| Light client (`iPoWLightClient`) | `0x3f0B074Ae2a695C3D5D9694Bbb52B03e0541f972` |
| Protocol | `0x5fCc0AB68575E3fb84716006bf513e9Fd6569958` |
| Conversion | `0x66785511306769B886746D2f33f8D76F4E267E61` |
| BETA (`BetaBaskets`) | `0x49b893bcA0635A256Ae2a322c3D9A8d6A05194c4` |
| Vault home factory | `0x86D140FF6A9A1FF7a6Aaade0468176B506572DFb` |
| Vault receipts factory | `0xec231FD201dd448feBDFB873B20875FBdFd238F1` |
| Vault paired with Solana | `0x4d1C3FdE9FaD26b9882b4E607120091E2ff285B4` |
| Its home part | `0xb57684c90C266F5d93913fC3C22249dD89858DaB` |
| Its receipts part | `0xC0BF165BCbfF23A83f824C84c818994Ea57078fd` |

## Setup

```sh
pnpm install
cp .env.example .env   # fill in HEDERA_TESTNET_RPC_URL and HEDERA_TESTNET_PRIVATE_KEY
```

## Testing

```sh
pnpm test
```

Same 83-test suite as the Ethereum package (the contract source is
identical) — see that package's README for the breakdown.

## Deploying

```sh
npx hardhat ignition deploy ignition/modules/IPoW.ts --network hederaTestnet
```

Chain ID 296. Current Hedera Testnet deployment:
`0x36D7F82F8B2E800C877592F8DFFF0E8CFAc96CF3`.

### BetaVault (design/ipow-implementation.md §6/§8)

```sh
npx hardhat ignition deploy ignition/modules/BetaVault.ts --network hederaTestnet
```

Points at the iPoW address above by default. Current deployment:
`0xba35203CD4389A66926e2280C66F7DD0646DBF35` (plus a `MockERC20` test token
at `0x37CdDd23F5fa910A7f1D909F0ACB4a5137771432`, deployed alongside it
purely to exercise §8.13's ERC20 local-leg support — not part of the
protocol itself). Its `Params` are tinybar-scaled (8 decimals), not the
usual 18-decimal weibar convention — see the module's own comment and the
tinybar note below.

### BetaHub (design/ipow-implementation.md §8.18/§8.19)

```sh
npx hardhat ignition deploy ignition/modules/BetaHub.ts --network hederaTestnet
```

Current deployment: `0x3cDba300797351664dEE4D5F0D3F7Be28e186bdA`
(`HubToken`/"iBETA" alongside it).

### iPoWConversion (design/ipow-implementation.md §1–§2/§9.2)

The permissionless-auction Conversion base primitive — a genuinely
separate deployment from the `iPoW` above (that one is Beta's own
header source; this one gets its own, matching every other network's
same deliberate split). No operator automation needed for this one —
claimants act on their own.

```sh
npx hardhat ignition deploy ignition/modules/IPoWConversion.ts --network hederaTestnet
```

Current deployment: `0xF5FE37E0bAE715FC61D6FF0f724c4c7E070F8057`
(its own `iPoW` header source: `0x578DD99E07593F72D402E6C7095E0F2C41A0dcc9`).
Cross-registered with every other network's own `iPoWConversion` via
`addNetwork` — see design/ipow-implementation.md §9.2.

## Post-deploy configuration

`scripts/configure.ts` works the same way as the Ethereum package's, with
one real difference worth knowing about: on Hedera, `msg.value` as seen
*inside* executing contract code is scaled to HBAR's native 8-decimal
tinybar unit, not the usual 18-decimal weibar convention that Hashio
presents everywhere else (e.g. `eth_getBalance`). The script accounts for
this when reading/writing `nativeLiquidity`, but it's worth knowing about if
you're calling the contract directly rather than through the script.

```sh
ACTION=status npx hardhat run scripts/configure.ts --network hederaTestnet
```
