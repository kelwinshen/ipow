# iPoW — Hedera

iPoW's network 6 on Hedera testnet, reached through the Hashio JSON-RPC
relay. It runs the protocol ([`docs/design/ipow-protocol.md`](../../docs/design/ipow-protocol.md)) from the one source in
[`evm`](../../evm), with its settings in [`deploy/networks.ts`](../../evm/deploy/networks.ts): Hedera's builds (D139),
which price gas from the transaction's gas price (a contract sees a base
fee of 0) and count HBAR in the 8 decimals a contract sees, where the RPC
shows 18. The node does not yet convert Hedera's values (spec, Build
status row 7).

## New protocol: test network deployment

The new protocol
([`docs/design/ipow-protocol.md`](../../docs/design/ipow-protocol.md)),
deployed on Hedera testnet (chain 296) on 2026-10-02 from the one source in
[`../../evm`](../../evm) with this network's settings
([`../../evm/deploy/networks.ts`](../../evm/deploy/networks.ts)), and read
back: each contract's code compared with the build and each setting read
(`node scripts/verify-deployments.ts` in `../../evm`). Lowest Bitcoin height
965,567; Conversion's largest swap 100,000 sats. The vault's pair on Solana
is the account `7Wwn5bjARBmdCjNT7KUBwB7LR3JYSLFAWSSm98pxvE2S`, `["config",
6]` of the earlier vault program
`2e1ZUB5eqA6bQfH3f9pbfvibfie7WnvEJeKif53VAm7p`, not of the live one
(`3TZ1LJ4f…`): Hedera's vault was left out of the genesis redeploys of
2026-10-07 on purpose and keeps its pair with that program
([`solana/README.md`](../../solana/README.md)).

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

## Setup and deploying

This package holds only this README, `.env.example`, and the `.gitignore`
that keeps the network's `.env` out of git
(`cp .env.example .env`, then fill in `HEDERA_TESTNET_RPC_URL` and `HEDERA_TESTNET_PRIVATE_KEY`). The
contracts, their tests and the deployment scripts are the one source in
[`../../evm`](../../evm); see its README for deploying.

The old contract generation that was deployed here (`iPoW`,
`iPoWConversion`, `BetaHub`, `BetaVault`), with its Hardhat package, was
removed; it stays in git at the tag `legacy-v1`.

## Hedera's units

On Hedera, `msg.value` as seen inside executing contract code is in HBAR's
8-decimal tinybar, not the 18-decimal weibar that the Hashio relay presents
elsewhere (for example `eth_getBalance`). Keep this in mind when calling a
contract directly.
