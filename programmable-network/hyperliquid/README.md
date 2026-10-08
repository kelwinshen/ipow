# iPoW — Hyperliquid (HyperEVM)

Same protocol as [`programmable-network/ethereum`](../ethereum), deployed to
Hyperliquid (HyperEVM) testnet — but not the same bytecode. Hyperliquid has
two layers: HyperCore (its own non-EVM trading engine — not a deploy target)
and HyperEVM (a separate, EVM-compatible execution layer sharing the same
chain). This package targets HyperEVM, whose native gas token is HYPE, not
ETH. See the [root architecture doc](../../docs/design/ipow-implementation.md) for how a
conversion actually flows, and [design/ipow-implementation.md §8](../../docs/design/ipow-implementation.md) /
[§9](../../docs/design/ipow-implementation.md) for BETA's composition design and Conversion's.

## New protocol: test network deployment

The new protocol ([`docs/design/ipow-protocol.md`](../../docs/design/ipow-protocol.md)),
deployed on HyperEVM testnet (chain 998) on 2026-10-02 from the one source in
[`../ethereum`](../ethereum) with this network's settings
([`../ethereum/deploy/networks.ts`](../ethereum/deploy/networks.ts)), and read back:
each contract's code compared with the build and each setting read
(`node scripts/verify-deployments.ts` in `../ethereum`). Lowest Bitcoin height
965,567; Conversion's largest swap 100,000 sats. The vault's
pair on Solana is the account `D73AnEY7aViFo3r6P8iPxMBxEqh6P5DiR4B6dMj7SixA`.

| Contract | Address |
|---|---|
| Light client (`iPoWLightClient`) | `0x15390C4901e11D3732489825c2FBa33A2E3e5b37` |
| Protocol | `0x2Dd456fCe7B3574AbD76b2899d3106CaB6ff5b9B` |
| Conversion | `0x6c5d75F830C002f4B73b4625F3E2bFaeC50b5D2E` |
| BETA (`BetaBaskets`), named baskets and parts named before bridged (E4, E5), 2026-10-05 | `0x51d0bc4CE62Fa394242415d0DAa167A5AF6Bb45B` |
| BETA, the E4-only build of the same morning (its baskets stay there) | `0x36BA462fF711C7261C4894Af40E5f0c49C98b280` |
| BETA, the build of 2026-10-02 (its baskets stay there) | `0xf35A1C4A9dE7B7FffF62c7cEfda2ac91a71cB6cc` |
| Vault home factory | `0x01BE560A22e91201C9414b634f0a61085F84dD08` |
| Vault receipts factory | `0x4265A27605cDd45b69d36f17635Bad03351387e6` |
| Vault paired with Solana | `0x4cFD981522F31A4dc12Aeb433907ebc3CBD37A55` |
| Its home part | `0xA1C38ee95A97d8F8355313c3De4D1C914B04693c` |
| Its receipts part | `0xD0bE0340C8cB132D8AF1765Af0d7358Af434984c` |
| Vault paired with Robinhood testnet (2026-10-05) | `0x62EDd87078a43B9e915aF91FF0ea137b34e9d7CB`; home `0x36E1d9CFb7CBf13eC1ea6c80E4B5405f5327CfED`, receipts `0xe4aC5A816a24A0c7Ad56DCCdbcfB36F329e76F3a`, its factories `0xCF87B06B09754ed3c9855b64Ae6FD295a7a1da10` and `0xCe6Cda46Ac4d02c6BbB3613de07821e0fd80b313` |
| Vault paired with Base Sepolia (2026-10-05) | `0xAfC5e11a1e2A0443248844757C9a2256Da5A742A`; home `0x70Cceb80c265168D1837030daD812e5e06f00321`, receipts `0x115C9E3c93F384E9f8fb177aA49538D09061B0cD`, its factories `0x6B70Af8a21a16F0c36dF37Ff1BAC581Cd3894Cb7` and `0xB6C7e5Db33F34D86102C8Df0AcACAd7052699162` |
| Vault paired with Sepolia (2026-10-05) | `0xc949e35812679fF3Be6aF0B025e2ddFff16bC2b9`; home `0x03397024aC9191D9a13014d6A9aEAaFdAE7f3774`, receipts `0xE346843867B4756859Ea1a176cBC636d2074fFFA`, its factories `0x7F0211E1BEA9A940f4E2F4A21B52f2f69C6e8dD5` and `0x405FaC50eB5d0134cf971310EBd6cdcDF2721b86` |

## Setup and deploying

This package holds only this README, `.env.example`, and the `.gitignore`
that keeps the network's `.env` out of git
(`cp .env.example .env`, then fill in `HYPEREVM_TESTNET_RPC_URL` and
`HYPEREVM_TESTNET_PRIVATE_KEY`). The contracts, their tests and the
deployment scripts are the one source in [`../ethereum`](../ethereum); see
its README for deploying. Deploying here needs HyperEVM's big blocks for the
deployer (`bigBlocks` in [`../ethereum/deploy/networks.ts`](../ethereum/deploy/networks.ts),
and `scripts/hyperliquid-big-blocks.ts` there).

The old contract generation that was deployed here, split into routers and
facets to fit HyperEVM's small blocks (`iPoWRouter`, `BetaVaultRouter`,
`BetaHubRouter`), with its Hardhat package, was removed; it stays in git at
the tag `legacy-v1`.
