# iPoW — Robinhood Chain

iPoW's network 4 on Robinhood Chain testnet, an Arbitrum Orbit chain. It
runs the protocol ([`docs/design/ipow-protocol.md`](../../docs/design/ipow-protocol.md)) from the one source in [`evm`](../../evm), with its
settings in [`deploy/networks.ts`](../../evm/deploy/networks.ts): the native coin (ETH), and the `ArbGasInfo` price of
posting data in the commitment fee (D135). Its mainnet chain id is not set
yet.

## New protocol: test network deployment

The new protocol
([`docs/design/ipow-protocol.md`](../../docs/design/ipow-protocol.md)),
deployed on Robinhood testnet (chain 46630) on 2026-10-02 from the one
source in [`../../evm`](../../evm) with this network's settings
([`../../evm/deploy/networks.ts`](../../evm/deploy/networks.ts)), and read
back: each contract's code compared with the build and each setting read
(`node scripts/verify-deployments.ts` in `../../evm`). Lowest Bitcoin height
965,567; Conversion's largest swap 100,000 sats. The vaults were redeployed
in genesis on 2026-10-07 from commit `f97885e`
([`docs/specs/ipow-vault-genesis.md`](../../docs/specs/ipow-vault-genesis.md));
the vault paired with Solana names the pair account
`GCpZ6DFQyeVLNBzKbeV4okqW745kioHvJmGGAjs1W1So`, `["config", 4]` of the vault
program `3TZ1LJ4fVZVKbBUyzVMRjJ9FNPGaqVmGX56JuT5QdXEy`. The vaults they
replaced are in
[`evm/deployments/replaced/`](../../evm/deployments/replaced/).

| Contract | Address |
|---|---|
| Light client (`iPoWLightClient`) | `0xc729b1a6d0325ae559614b6826127e11E51703c1` |
| Protocol | `0xd6b425c7908E171a33dF2a4e6C5687eDF0D2d6c3` |
| Reader of the data price (D135) | `0x6AA1F2dd1a0A28F5FC3a88ec8A219e89987A57aB` |
| Conversion | `0xf35A1C4A9dE7B7FffF62c7cEfda2ac91a71cB6cc` |
| Conversion, the build before (its swaps end there by its own rules) | `0x6c5d75F830C002f4B73b4625F3E2bFaeC50b5D2E` |
| BETA (`BetaBaskets`), named baskets and parts named before bridged (E4, E5), 2026-10-05 | `0x529bbD58E239C515465137B1e23cc1Fd1360Fb4b` |
| BETA, the E4-only build of the same morning (its baskets stay there) | `0x7EbA21758f149b18FDc0438B492D6059E37031b2` |
| BETA, the build of 2026-10-02 (its baskets stay there) | `0xd9a6e550Ea8a3970d08F34b0b1a417338C839C4c` |
| Vault home factory (genesis, 2026-10-07) | `0x1611E0170471D75E7A3e979B0884699b938B1b22` |
| Vault receipts factory (genesis, 2026-10-07) | `0xc3C9129d441504fF719cf813366Dd58F5003d140` |
| Vault paired with Solana (genesis, 2026-10-07) | `0xf8374e052e5b3A8518f1834C527A09A91023F58C` |
| Its home part | `0x3d6f11ab86c9b13a34b6a3F5B329760AF603A7B3` |
| Its receipts part | `0xA88B23932537Ca6bE3CfFF4A66Ec2dE6566bb41f` |
| Vault paired with Sepolia (genesis, 2026-10-07) | `0x576f2e4CdE5CfBe9b2035969B67Ec1C5967A4599`; home `0xB879ff813a1cb1162e9F43C886aBDaa2e8Dbe794`, receipts `0x41f07514258c2a3D2Dfa261d0ae0b040A3C4e383`, its factories `0x1C770b7fb68c28FfAAAE386646ff9c03334EE89c` and `0xbdAFC86221d3f139c6EBD77ED023e5554C7E2297` |
| Vault paired with Base Sepolia (genesis, 2026-10-07) | `0xbe0dCDFC20B7e19Ee893206448dDb865e0b51374`; home `0x6f0Ac3e0a0164690C5F3F4519b2390a286B281f1`, receipts `0x51f1B231Aee11503BD8B998C32C74aA593c931dE`, its factories `0xfD7b1bEFC0086809ea02542A30e85e5c439AF4c0` and `0xEF9173ADcF393846873e05e9d58411b8BA823f4c` |
| Vault paired with Arbitrum Sepolia (genesis, 2026-10-07) | `0x25A155e97EA6e44E9e872b7Aa6866BC10991732e`; home `0xD8e4E19B7F03F6dbB67483e4C0A04720d9082aAd`, receipts `0x352a5d01ff87D29995e3a0eDe348A7c8c77815AF`, its factories `0x5FF4ee23851eDF4445c01460A108187c501eEbFd` and `0xc1C5dBFF7D71d254502a78276875144BcD513f58` |
| Vault paired with HyperEVM testnet (genesis, 2026-10-07) | `0x63C581Be19c699e614D047d70A2db649469312eB`; home `0xD8b5675720255DA4DD989FdD114A7B82d2B639Ad`, receipts `0xAD3CcB6B808A8710C99Ad77c7f14d5209AFa8Ce4`, its factories `0xDe908E13Df6105C9c710B0875f81e1EBD57aBd51` and `0x8D259bc04cC95d29e09A43941823089CFeb85D10` |

**Genesis.** These vaults' receipts started in genesis for the deployer,
ending between 2026-10-21 16:58 and 2026-10-21 18:02 UTC at the latest
(`genesisEnd` in the record). `sdk/scripts/genesis-check.ts`, run at commit
`2897c06` (then `packages/sdk/scripts/genesis-check.ts`) with the genesis
run's ledger
([`evm/deployments/genesis-testnet.json`](../../evm/deployments/genesis-testnet.json)),
read every genesis receipt and issue back from the chains: 409 receipts and
470 issues across all the pairs, every one backed by its lock. Of these, 67
of the receipts and 77 of the issues are on this network's receipts parts,
and 65 of the locks issued are here.

## Setup and deploying

This package holds only this README, `.env.example`, and the `.gitignore`
that keeps the network's `.env` out of git
(`cp .env.example .env`, then fill in `ROBINHOOD_TESTNET_RPC_URL` and `ROBINHOOD_TESTNET_PRIVATE_KEY`). The
contracts, their tests and the deployment scripts are the one source in
[`../../evm`](../../evm); see its README for deploying.

The old contract generation that was deployed here (`iPoW`,
`iPoWConversion`, `BetaHub`, `BetaVault`), with its Hardhat package, was
removed; it stays in git at the tag `legacy-v1`.
