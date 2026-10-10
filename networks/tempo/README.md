# iPoW — Tempo

iPoW's network 8 on Tempo's Moderato testnet, a payments chain with no
native coin. It runs the protocol ([`docs/design/ipow-protocol.md`](../../docs/design/ipow-protocol.md)) from the one source in
[`evm`](../../evm), with its settings in [`deploy/networks.ts`](../../evm/deploy/networks.ts): the token builds
(`iPoWProtocolToken`, `iPoWVaultToken`, D136 to D138), with PathUSD
(`0x20C0000000000000000000000000000000000000`, 6 decimals) as the coin, since
Tempo refuses any transaction carrying native value. The gas price is in
attodollars, so the token build divides the commitment fee by 10^12.

Two things particular to Tempo:

- **Never trust a Tempo deploy or write receipt.** A contract lands at
  `CREATE(sender, ordinary nonce)` whatever nonce lane sent it, and a
  deploy transaction's receipt can report the wrong `contractAddress`, or
  the address of a contract never created. Every deploy here works the
  address out from the ordinary nonce and reads the code and state back.
  The measurements behind this are in the archived
  [`docs/archive/ipow-implementation.md`](../../docs/archive/ipow-implementation.md),
  Tempo section.
- **Ethers cannot send Tempo's transactions** (fee-sponsored, with
  two-dimensional nonces), so every deploy and write here goes through
  `viem`'s Tempo support (`scripts/`).

## New protocol: test network deployment

The new protocol
([`docs/design/ipow-protocol.md`](../../docs/design/ipow-protocol.md)),
deployed on Tempo's Moderato testnet (chain 42431) on 2026-10-03 from the
one source in [`../../evm`](../../evm), commit `bc09e28`, with Tempo's
settings ([`../../evm/deploy/networks.ts`](../../evm/deploy/networks.ts)):
the token builds, with PathUSD
(`0x20C0000000000000000000000000000000000000`) as the coin. Sent with viem's
Tempo support by
[`scripts/deploy-new-protocol.ts`](scripts/deploy-new-protocol.ts) in the
deployer's ordinary nonce lane, each address worked out from the nonce, and
read back: each contract's code compared with the build and each setting
read (`node scripts/verify-deployments.ts` in `../../evm`). Lowest Bitcoin
height 965,567; Conversion's largest swap 100,000 sats. The vault was
redeployed in genesis on 2026-10-07 from commit `6b8617d`
([`docs/specs/ipow-vault-genesis.md`](../../docs/specs/ipow-vault-genesis.md));
it names the pair account `8USDsC8MpsnKZmt8KAkVDr18R6s5g71Fro8Jr2JMJmmr`,
`["config", 8]` of the vault program
`3TZ1LJ4fVZVKbBUyzVMRjJ9FNPGaqVmGX56JuT5QdXEy`. The vault it replaced is in
[`evm/deployments/replaced/`](../../evm/deployments/replaced/).

BETA was redeployed on 2026-10-09 from commit `642891c` by
[`scripts/redeploy-beta.ts`](scripts/redeploy-beta.ts): the build of
2026-10-03 predates baskets whose parts are the vault's receipts (E5,
[`docs/specs/ipow-beta-app.md`](../../docs/specs/ipow-beta-app.md)), which
the SDK calls. The new contract's code was compared with the build and its
limits read back, and `verify-deployments.ts` matches it. No basket was made
on the old one (no event from it, read on 2026-10-09). Tempo has no coin of
its own and refuses a transaction carrying value: a basket whose part is the
zero address (the coin, elsewhere) can be made here but never minted.
Greatwall offers only tokens as parts on Tempo, and a mint of those carries
no value.

| Contract | Address |
|---|---|
| Light client (`iPoWLightClient`) | `0x2dD223DcD7F69539Ea895A29095c69c16b088aDb` |
| Protocol (token build, `iPoWProtocolToken`) | `0x0496e48C51E3783F5a059AC82B70F5D398448D3A` |
| Conversion | `0xc729b1a6d0325ae559614b6826127e11E51703c1` |
| BETA (`BetaBaskets`), named baskets and parts named before bridged (E4, E5), 2026-10-09, from commit `642891c` | `0xE101001bD62451c6f3337f44EF2cC4F166dd891D` |
| BETA, the build of 2026-10-03 (no baskets were made there) | `0x6AA1F2dd1a0A28F5FC3a88ec8A219e89987A57aB` |
| Vault home factory (genesis, 2026-10-07) | `0x8A5EB387b8b1CBd5cBAFBE2088aCE6d589eb4F59` |
| Vault receipts factory (genesis, 2026-10-07) | `0x07eFF65A853f36cBA8FEFbC495eb6A0D98d26a75` |
| Vault paired with Solana (`iPoWVaultToken`, genesis, 2026-10-07) | `0x6c692BEdCa89292D0FEfc6e86b82f451E371f11D` |
| Its home part | `0xb811b6DFd8855EA19d3aCd6ea06FEB762DE66278` |
| Its receipts part | `0x713D05559eD598a077D42818aABfB1a7873f579F` |

**Genesis.** This vault's receipts started in genesis for the deployer,
ending at 2026-10-21 17:17 UTC at the latest (`genesisEnd` in the record).
`sdk/scripts/genesis-check.ts`, run at commit `479d93e` (then
`packages/sdk/scripts/genesis-check.ts`) with the genesis run's ledger
([`evm/deployments/genesis-testnet.json`](../../evm/deployments/genesis-testnet.json)),
read every genesis receipt and issue back from the chains: 409 receipts and
470 issues across all the pairs, every one backed by its lock. Of these, no
receipt is made here; 3 of the locks issued are here, issued on Solana.

## Setup and deploying

The contracts and their tests are the one source in
[`../../evm`](../../evm). This package holds this README, the network's
git-ignored `.env` (`cp .env.example .env`, then fill in
`TEMPO_TESTNET_RPC_URL` and `TEMPO_TESTNET_PRIVATE_KEY`), and Tempo's own
deployment scripts, which send Tempo's transaction type with `viem`
(`pnpm install` here for `viem` and `ethers`; compile in `../../evm`
first). Each script's header comment says how to run it:

- [`scripts/deploy-new-protocol.ts`](scripts/deploy-new-protocol.ts) — the protocol, and vaults paired with Solana.
- [`scripts/deploy-mock-rwa.ts`](scripts/deploy-mock-rwa.ts) — Greatwall's mock RWA tokens.
- [`scripts/genesis-tempo.ts`](scripts/genesis-tempo.ts) — Tempo's part of the vaults' genesis.
- [`scripts/redeploy-beta.ts`](scripts/redeploy-beta.ts) — BETA (`BetaBaskets`) alone.

Chain ID 42431. Never trust a Tempo deploy receipt's address (above); the
scripts work it out from the nonce and read the code back.

The old contract generation that was deployed here (`iPoW`, `BetaVault`,
`BetaHub` and their PathUSD variants, `iPoWConversionPathUSD`), with its
Hardhat package and `scripts/deploy_*.mjs`, was removed; it stays in git at
the tag `legacy-v1`.
