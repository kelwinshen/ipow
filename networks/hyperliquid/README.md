# iPoW — Hyperliquid (HyperEVM)

iPoW's network 7 on HyperEVM testnet, the EVM layer of Hyperliquid (its
coin is HYPE). It runs the protocol ([`docs/design/ipow-protocol.md`](../../docs/design/ipow-protocol.md)) from the one source in
[`evm`](../../evm), with its settings in [`deploy/networks.ts`](../../evm/deploy/networks.ts). HyperEVM's small blocks hold
3M gas and a 30M big block comes every 60 seconds; the protocol takes about
5.4M gas to deploy and a vault about 10.5M, so the deployer's address must
use big blocks (`bigBlocks` in [`deploy/networks.ts`](../../evm/deploy/networks.ts), `scripts/hyperliquid-big-blocks.ts` in
`evm`). The node streams at most 30 headers per call to stay under 3M.

## New protocol: test network deployment

The new protocol
([`docs/design/ipow-protocol.md`](../../docs/design/ipow-protocol.md)),
deployed on HyperEVM testnet (chain 998) on 2026-10-02 from the one source
in [`../../evm`](../../evm) with this network's settings
([`../../evm/deploy/networks.ts`](../../evm/deploy/networks.ts)), and read
back: each contract's code compared with the build and each setting read
(`node scripts/verify-deployments.ts` in `../../evm`). Lowest Bitcoin height
965,567; Conversion's largest swap 100,000 sats. The vaults were redeployed
in genesis on 2026-10-07 from commit `f97885e`
([`docs/specs/ipow-vault-genesis.md`](../../docs/specs/ipow-vault-genesis.md));
the vault paired with Solana names the pair account
`8nJB7bCtN6DpeZGHNtLxaE11rdKnRkFhGR4LJUTsCffo`, `["config", 7]` of the vault
program `3TZ1LJ4fVZVKbBUyzVMRjJ9FNPGaqVmGX56JuT5QdXEy`. The vaults they
replaced are in
[`evm/deployments/replaced/`](../../evm/deployments/replaced/).

| Contract | Address |
|---|---|
| Light client (`iPoWLightClient`) | `0x15390C4901e11D3732489825c2FBa33A2E3e5b37` |
| Protocol | `0x2Dd456fCe7B3574AbD76b2899d3106CaB6ff5b9B` |
| Conversion | `0x07eFF65A853f36cBA8FEFbC495eb6A0D98d26a75` |
| Conversion, the build before (its swaps end there by its own rules) | `0x8A5EB387b8b1CBd5cBAFBE2088aCE6d589eb4F59` |
| BETA (`BetaBaskets`), named baskets and parts named before bridged (E4, E5), 2026-10-05 | `0x51d0bc4CE62Fa394242415d0DAa167A5AF6Bb45B` |
| BETA, the E4-only build of the same morning (its baskets stay there) | `0x36BA462fF711C7261C4894Af40E5f0c49C98b280` |
| BETA, the build of 2026-10-02 (its baskets stay there) | `0xf35A1C4A9dE7B7FffF62c7cEfda2ac91a71cB6cc` |
| Vault home factory (genesis, 2026-10-07) | `0xFBa6A7d933E6f98B51f7c5e03A177E261e966Db1` |
| Vault receipts factory (genesis, 2026-10-07) | `0x556d1CA6460714bC4D4d23Fe9230Ca77780EE22F` |
| Vault paired with Solana (genesis, 2026-10-07) | `0x05EAD8b6dac6f78781cF4c582fF4311358F38118` |
| Its home part | `0x5CCd286125ADD2B35F244Ff60A1458B7E13525fB` |
| Its receipts part | `0xD20E666A0e88BBAD5C5250Cbb36C56054F3D0B8F` |
| Vault paired with Sepolia (genesis, 2026-10-07) | `0x806782a1A113f8fb242F543c275DBC939A1956b9`; home `0x387aE4Ed0E077058a1C3635C7a3054b43ffE6E5e`, receipts `0x7eB196425f6c906904DC9BF93C1c10357A674382`, its factories `0x17B38C9eE9BE2210C3ec8D0fB3Fe4D9785cBB51c` and `0xebae6b411D3C360aD9bB74DDCd036ab2A7D6cBde` |
| Vault paired with Base Sepolia (genesis, 2026-10-07) | `0x2110dEb3A2a01834A2d356470c4451Db9A5ba809`; home `0x3243bf2CD3170e56d966af891a7b158d706DFCa4`, receipts `0x709fF26b32A714e15815533b04F85fa8c2D85A8f`, its factories `0xBbaaFB3b1A2362309f6A82478C1ddc6ec1d68d5F` and `0x12E1016D8Ca5702E67AC47D95a3490B00F9d9408` |
| Vault paired with Robinhood testnet (genesis, 2026-10-07) | `0x75Ab3B99752f0DE0a2dB8AFa09f6503aACFb4576`; home `0x7F399C9989988f9F5DC7BFE05fF28670930E6515`, receipts `0xcA73F9cc6F0dF820A9d325ed1D57FCC121b1e591`, its factories `0x865C6C7EC2C28C0096ef932Bff277B781bFF793C` and `0x8a691DCf09fAD46383cED06C27B0424e790728B1` |
| Vault paired with Arbitrum Sepolia (genesis, 2026-10-07) | `0x1d63844887FDd4C8a4fB21E50Ed75493ef817ef9`; home `0xC852658A4D3470595b7Bbe3d8f0FdD86FDE02F6d`, receipts `0xA6FD8dC895cd8Af71A5f3B4149884D262637Ee95`, its factories `0x5F0696a1c3ef6821F143FB78a4CEb939D85bA0c4` and `0xa6419d52845D8FD27aA60493076470eA4A9d33a0` |

**Genesis.** These vaults' receipts started in genesis for the deployer,
ending between 2026-10-21 17:22 and 2026-10-21 18:07 UTC at the latest
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
(`cp .env.example .env`, then fill in `HYPEREVM_TESTNET_RPC_URL` and
`HYPEREVM_TESTNET_PRIVATE_KEY`). The contracts, their tests and the
deployment scripts are the one source in [`../../evm`](../../evm); see
its README for deploying. Deploying here needs HyperEVM's big blocks for the
deployer (`bigBlocks` in [`../../evm/deploy/networks.ts`](../../evm/deploy/networks.ts),
and `scripts/hyperliquid-big-blocks.ts` there).

The old contract generation that was deployed here, split into routers and
facets to fit HyperEVM's small blocks (`iPoWRouter`, `BetaVaultRouter`,
`BetaHubRouter`), with its Hardhat package, was removed; it stays in git at
the tag `legacy-v1`.
