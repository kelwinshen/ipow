# iPoW — Robinhood Chain

Same Solidity contracts as [`programmable-network/ethereum`](../ethereum),
deployed to Robinhood Chain testnet. Robinhood Chain is an Arbitrum-Orbit L2 for tokenized equities/stablecoins, testnet launched February 2026 — fully EVM-compatible, standard tooling works unmodified. See the
[root architecture doc](../../docs/design/ipow-implementation.md) for how a conversion
actually flows, and
[design/ipow-implementation.md §8](../../docs/design/ipow-implementation.md) for BETA's composition design.

## New protocol: test network deployment

The new protocol ([`docs/design/ipow-protocol.md`](../../docs/design/ipow-protocol.md)),
deployed on Robinhood testnet (chain 46630) on 2026-10-02 from the one source in
[`../ethereum`](../ethereum) with this network's settings
([`../ethereum/deploy/networks.ts`](../ethereum/deploy/networks.ts)), and read back:
each contract's code compared with the build and each setting read
(`node scripts/verify-deployments.ts` in `../ethereum`). Lowest Bitcoin height
965,567; Conversion's largest swap 100,000 sats. The vault's
pair on Solana is the account `Dydhoos4EgX15LFLJR5a6DvGtuzmNjegpzzynJSJbF5`.

| Contract | Address |
|---|---|
| Light client (`iPoWLightClient`) | `0xc729b1a6d0325ae559614b6826127e11E51703c1` |
| Protocol | `0xd6b425c7908E171a33dF2a4e6C5687eDF0D2d6c3` |
| Reader of the data price (D135) | `0x6AA1F2dd1a0A28F5FC3a88ec8A219e89987A57aB` |
| Conversion | `0xb856906fEBAFBB21A06DdC35E9BCe476139A86BA` |
| BETA (`BetaBaskets`), named baskets and parts named before bridged (E4, E5), 2026-10-05 | `0x529bbD58E239C515465137B1e23cc1Fd1360Fb4b` |
| BETA, the E4-only build of the same morning (its baskets stay there) | `0x7EbA21758f149b18FDc0438B492D6059E37031b2` |
| BETA, the build of 2026-10-02 (its baskets stay there) | `0xd9a6e550Ea8a3970d08F34b0b1a417338C839C4c` |
| Vault home factory | `0x9d2d501827e84d53addA293E70A8995078c87891` |
| Vault receipts factory | `0x10C9C79C8c46c1f9f07D7A258Bbd42B4ED6f973E` |
| Vault paired with Solana | `0x15390C4901e11D3732489825c2FBa33A2E3e5b37` |
| Its home part | `0xfebE817d06A642D8C66fe5548Df61A5AfaFdc201` |
| Its receipts part | `0xA3E1811D33bBAeA900dB14660fdF09C2EAd87A7a` |
| Vault paired with HyperEVM testnet (2026-10-05) | `0x51cD31AC42a838d225f3B42F830Af72d99Eb003a`; home `0xD06ea940f00De09851c7acA6f0f6a66b71FaE16E`, receipts `0xC69EBc691174a55E240552674896bBE651ebBad2`, its factories `0xB001c1Eb3B2D6dF4380682E119dDF1e3a9F18D89` and `0x2ab2c9487cC8f83816d556Da149132c705B82e48` |
| Vault paired with Base Sepolia (2026-10-05) | `0x3221102aC4159048960e061FC0ADb81De3133496`; home `0xa6629b7F085114535f1F64deE27BE880e67D876C`, receipts `0x00c360219F11Cc6f4D0472353d411dd0D7891CB8`, its factories `0x85A86D77357898D79b49c464ccaB03411109e722` and `0x96AC11A1Cf37C5c9704ecc080b539Eb7F81d9b81` |
| Vault paired with Sepolia (2026-10-05) | `0x170685feEe5ac2bCddAE20DAe66747658E6fA7c0`; home `0x27824021Cd136F59C357C902b24b35CbE64e0C6A`, receipts `0x4994B8161A265CF219385Bd02Af23C033CC000F2`, its factories `0x592659De4a7D5F31cfE95C1c6c2A0456343b82E0` and `0xC0fE6c22b0034E559CeBB4c064098aB887818E32` |

## Setup and deploying

This package holds only this README, `.env.example`, and the `.gitignore`
that keeps the network's `.env` out of git
(`cp .env.example .env`, then fill in `ROBINHOOD_TESTNET_RPC_URL` and `ROBINHOOD_TESTNET_PRIVATE_KEY`). The
contracts, their tests and the deployment scripts are the one source in
[`../ethereum`](../ethereum); see its README for deploying.

The old contract generation that was deployed here (`iPoW`,
`iPoWConversion`, `BetaHub`, `BetaVault`), with its Hardhat package, was
removed; it stays in git at the tag `legacy-v1`.
