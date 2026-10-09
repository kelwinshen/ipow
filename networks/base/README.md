# iPoW — Base

iPoW's network 3 on Base Sepolia, an OP Stack rollup. It runs the protocol
([`docs/design/ipow-protocol.md`](../../docs/design/ipow-protocol.md)) from the one source in [`evm`](../../evm), with its settings in
[`deploy/networks.ts`](../../evm/deploy/networks.ts): the native coin (ETH), and the price of posting data to Ethereum,
read from the OP Stack `GasPriceOracle`, in the commitment fee (D135).

## New protocol: test network deployment

The new protocol
([`docs/design/ipow-protocol.md`](../../docs/design/ipow-protocol.md)),
deployed on Base Sepolia (chain 84532) on 2026-10-02 from the one source in
[`../../evm`](../../evm) with this network's settings
([`../../evm/deploy/networks.ts`](../../evm/deploy/networks.ts)), and read
back: each contract's code compared with the build and each setting read
(`node scripts/verify-deployments.ts` in `../../evm`). Lowest Bitcoin height
965,567; Conversion's largest swap 100,000 sats. The vaults were redeployed
in genesis on 2026-10-07 from commit `6b8617d`
([`docs/specs/ipow-vault-genesis.md`](../../docs/specs/ipow-vault-genesis.md));
the vault paired with Solana names the pair account
`EEyy1MZhpDXurack8QhkeoAVTDmsHkn2zZ1H9QynppMN`, `["config", 3]` of the vault
program `3TZ1LJ4fVZVKbBUyzVMRjJ9FNPGaqVmGX56JuT5QdXEy`. The vaults they
replaced are in
[`evm/deployments/replaced/`](../../evm/deployments/replaced/).

| Contract | Address |
|---|---|
| Light client (`iPoWLightClient`) | `0x10C9C79C8c46c1f9f07D7A258Bbd42B4ED6f973E` |
| Protocol | `0x2Dd456fCe7B3574AbD76b2899d3106CaB6ff5b9B` |
| Reader of the data price (D135) | `0x15390C4901e11D3732489825c2FBa33A2E3e5b37` |
| Conversion | `0x07eFF65A853f36cBA8FEFbC495eb6A0D98d26a75` |
| Conversion, the build before (its swaps end there by its own rules) | `0x8A5EB387b8b1CBd5cBAFBE2088aCE6d589eb4F59` |
| BETA (`BetaBaskets`), named baskets and parts named before bridged (E4, E5), 2026-10-05 | `0xB6C7e5Db33F34D86102C8Df0AcACAd7052699162` |
| BETA, the E4-only build of the same morning (its baskets stay there) | `0x6B70Af8a21a16F0c36dF37Ff1BAC581Cd3894Cb7` |
| BETA, the build of 2026-10-02 (its baskets stay there) | `0xf35A1C4A9dE7B7FffF62c7cEfda2ac91a71cB6cc` |
| Vault home factory (genesis, 2026-10-07) | `0x556d1CA6460714bC4D4d23Fe9230Ca77780EE22F` |
| Vault receipts factory (genesis, 2026-10-07) | `0x7966F9ba8966718d40516f4C4758e4f19e6Db292` |
| Vault paired with Solana (genesis, 2026-10-07) | `0x82Fd247e3dBA26E5De0023043b262C5B1DBEe03c` |
| Its home part | `0xD20E666A0e88BBAD5C5250Cbb36C56054F3D0B8F` |
| Its receipts part | `0x380c431675886c883692370b4d1B39f0E3e3f571` |
| Vault paired with Sepolia (genesis, 2026-10-07) | `0x3CC9b0836d26d15153ee82b217bC77dA4738c8d5`; home `0x74e65CD29B584e424aA529f1Bf6D302dd4eaEA40`, receipts `0x387aE4Ed0E077058a1C3635C7a3054b43ffE6E5e`, its factories `0xee504a7Ff86B9B2e2DCB87953B443c7244d6cE45` and `0x17B38C9eE9BE2210C3ec8D0fB3Fe4D9785cBB51c` |
| Vault paired with Robinhood testnet (genesis, 2026-10-07) | `0x9e5660845231AAb5572D26788119825DEa6eF102`; home `0xDb043d5acEa1487ac2880B52A5c25292baACAF41`, receipts `0x3243bf2CD3170e56d966af891a7b158d706DFCa4`, its factories `0x806782a1A113f8fb242F543c275DBC939A1956b9` and `0xBbaaFB3b1A2362309f6A82478C1ddc6ec1d68d5F` |
| Vault paired with Arbitrum Sepolia (genesis, 2026-10-07) | `0x6D206Eb0F742cF6ADbbF42Ae2724F3e0417a0d91`; home `0xc87Cb251671C6292a6cE65a80dCAa5Fb904709fc`, receipts `0x7F399C9989988f9F5DC7BFE05fF28670930E6515`, its factories `0x2110dEb3A2a01834A2d356470c4451Db9A5ba809` and `0x865C6C7EC2C28C0096ef932Bff277B781bFF793C` |
| Vault paired with HyperEVM testnet (genesis, 2026-10-07) | `0xf591f519fe87C851A7686cD4eD9b5DC5B62ce22e`; home `0xC1A589619f2954C24E5f2EB536cD9b668Bb68Caf`, receipts `0xC852658A4D3470595b7Bbe3d8f0FdD86FDE02F6d`, its factories `0x75Ab3B99752f0DE0a2dB8AFa09f6503aACFb4576` and `0x5F0696a1c3ef6821F143FB78a4CEb939D85bA0c4` |

**Genesis.** These vaults' receipts started in genesis for the deployer,
ending between 2026-10-21 16:58 and 2026-10-21 17:57 UTC at the latest
(`genesisEnd` in the record). `sdk/scripts/genesis-check.ts`, run at commit
`479d93e` (then `packages/sdk/scripts/genesis-check.ts`) with the genesis
run's ledger
([`evm/deployments/genesis-testnet.json`](../../evm/deployments/genesis-testnet.json)),
read every genesis receipt and issue back from the chains: 409 receipts and
470 issues across all the pairs, every one backed by its lock. Of these, 66
of the receipts and 76 of the issues are on this network's receipts parts,
and 70 of the locks issued are here.

## Setup and deploying

This package holds only this README, `.env.example`, and the `.gitignore`
that keeps the network's `.env` out of git
(`cp .env.example .env`, then fill in `BASE_SEPOLIA_RPC_URL` and `BASE_SEPOLIA_PRIVATE_KEY`). The
contracts, their tests and the deployment scripts are the one source in
[`../../evm`](../../evm); see its README for deploying.

The old contract generation that was deployed here (`iPoW`,
`iPoWConversion`, `BetaHub`, `BetaVault`), with its Hardhat package, was
removed; it stays in git at the tag `legacy-v1`.
