# iPoW — EVM contracts (and Ethereum Sepolia)

The Solidity source of the protocol ([`docs/design/ipow-protocol.md`](../docs/design/ipow-protocol.md))
and its applications, for every EVM network iPoW runs on. Each network's
differences are settings in [`deploy/networks.ts`](deploy/networks.ts), not
copies of the contracts; the other networks' READMEs are in
[`../networks/`](../networks/). This README also holds the addresses on
Ethereum Sepolia, network 1.

## New protocol: test network deployment

The new protocol
([`docs/design/ipow-protocol.md`](../docs/design/ipow-protocol.md)),
deployed on Sepolia (chain 11155111) on 2026-10-02 from the one source in
[`evm`](.) with this network's settings
([`deploy/networks.ts`](deploy/networks.ts)), and read back: each contract's
code compared with the build and each setting read (`node
scripts/verify-deployments.ts` in `evm`). Lowest Bitcoin height 965,567;
Conversion's largest swap 100,000 sats. The vaults were redeployed in
genesis on 2026-10-07 from commit `f97885e`
([`docs/specs/ipow-vault-genesis.md`](../docs/specs/ipow-vault-genesis.md));
the vault paired with Solana names the pair account
`Cztcuoj5XAhZ35ky6Ud3WMq9R7ijcvPTnnvSjGQ88sEg`, `["config", 1]` of the vault
program `3TZ1LJ4fVZVKbBUyzVMRjJ9FNPGaqVmGX56JuT5QdXEy`. The vaults they
replaced are in [`deployments/replaced/`](deployments/replaced/).

| Contract | Address |
|---|---|
| Light client (`iPoWLightClient`) | `0x8e09508deF58997f8481FF678cd7f278DFe22C3F` |
| Protocol | `0x21348ef459dDBa5607732Ca1e9992968B4cC5611` |
| Conversion | `0x3896b0B95D853655A62CCb83079509716E77962d` |
| Conversion, the build before (its swaps end there by its own rules) | `0xFB82B968e1529E740588aAE0F6379c956BF6683C` |
| BETA (`BetaBaskets`), named baskets and parts named before bridged (E4, E5), 2026-10-05 | `0x7311B038A8e2c1F1C2b6a266E6cA38ed3804F3BF` |
| BETA, the E4-only build of the same morning (its baskets stay there) | `0x80f99051921754C0b2F7BeC7468b72a0E6e2850b` |
| BETA, the build of 2026-10-02 (its baskets stay there) | `0xb5375fDaF8b6E04942dFc593E7065A606Fe5588d` |
| Vault home factory (genesis, 2026-10-07) | `0xB98d11e0a2a28882aB8375438bdF6Ea8D1b3D5b8` |
| Vault receipts factory (genesis, 2026-10-07) | `0x1568FA92Cd8fB05c93d6890432e19e6911347079` |
| Vault paired with Solana (genesis, 2026-10-07) | `0x26187A8aC987c7d6d7610c64a497595698C9A77F` |
| Its home part | `0x221C94c0B10acb1cA1E032690c418327bB32a119` |
| Its receipts part | `0xb73a2D0a046fF29CaC0eBdcD00BfF6Fd800a7979` |
| Vault paired with Base Sepolia (genesis, 2026-10-07) | `0x907b399028627110F42fa21aF1C9911BA41B7a66`; home `0x9cd1C673e02B5ddAb7915368ac2aAD3C78609560`, receipts `0xc29926Fe5280809174813Dfec09Cf486E86115E8`, its factories `0xA9ad6673BD7fB506b46B04Bf32632C63252bf3a6` and `0xcC291d2b4db35A3f292954d4Dbe0c7b3d79CAC05` |
| Vault paired with Robinhood testnet (genesis, 2026-10-07) | `0xA555077E1497a870C3CefC0354D40c42D48Ce451`; home `0xF1CCc7D7f2Ce35BfF665e1C2740DdaA22EDdA342`, receipts `0x5aA93CA6C612b0CDcE79d8919Bcd618818ee381E`, its factories `0x932FCF9FB82D5CcCAb27577501744520F7eE24E2` and `0x769c19fAc515590C2781f123A999b6049F99072e` |
| Vault paired with Arbitrum Sepolia (genesis, 2026-10-07) | `0x8aF9Fa14907cc385640971a1bA7B941EA7c4D99A`; home `0x8c824700124A6DA2991255A6aE799fDF65f59477`, receipts `0xC6a5FE86e56D0Faa1b8305a03532cED023c2Ad43`, its factories `0x5Ae336613Dbb482b35Ef01FBC946089204050093` and `0x8281608897bEBB650E80883df58739523E57cE93` |
| Vault paired with HyperEVM testnet (genesis, 2026-10-07) | `0x7170F74b56d7888FA6E2CE6C0542816Dff509b3A`; home `0xa419BfDFe315a9331d3c00D7a3D32832425C4D12`, receipts `0xf2f071144fb9AE3c95FEFeF3408b1764B90c68d0`, its factories `0x5Cfae63e2107D11Ae5c2b888f53c0e9e253E044D` and `0x79C5F86f92DA4FfDE8426a502b204d16F15E675F` |

**Genesis.** These vaults' receipts started in genesis for the deployer,
ending between 2026-10-21 16:56 and 2026-10-21 17:50 UTC at the latest
(`genesisEnd` in the record). `sdk/scripts/genesis-check.ts`, run at commit
`2897c06` (then `packages/sdk/scripts/genesis-check.ts`) with the genesis
run's ledger
([`deployments/genesis-testnet.json`](deployments/genesis-testnet.json)),
read every genesis receipt and issue back from the chains: 409 receipts and
470 issues across all the pairs, every one backed by its lock. Of these, 61
of the receipts and 71 of the issues are on this network's receipts parts,
and 95 of the locks issued are here.

## Contracts

- `contracts/protocol/` — the protocol: the light client
  (`iPoWLightClient`), the protocol (`iPoWProtocol`, `iPoWProtocolToken`),
  its data fee, the vault (`iPoWVault`, `iPoWVaultToken`) and the vault's
  parts in `contracts/protocol/vault/`.
- `contracts/applications/` — the applications on it: Conversion
  (`conversion/`) and BETA (`beta/`, `BetaBaskets`).
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
