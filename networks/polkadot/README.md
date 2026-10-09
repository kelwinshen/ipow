# iPoW — Polkadot

iPoW's network 5 on Polkadot Hub TestNet, reached through `pallet-revive`'s
Ethereum JSON-RPC adapter (`eth-rpc`). It runs the protocol ([`docs/design/ipow-protocol.md`](../../docs/design/ipow-protocol.md)) from the
one source in [`evm`](../../evm), with its settings in [`deploy/networks.ts`](../../evm/deploy/networks.ts): the native coin
in 18 decimals, and Polkadot's build of the protocol (`iPoWProtocolPolkadot`,
D140), which scales the light client's work, since its gas is not
Ethereum's (measured 0.08 to 0.124 of it).

## New protocol: test network deployment

The new protocol
([`docs/design/ipow-protocol.md`](../../docs/design/ipow-protocol.md)),
deployed on Polkadot testnet (chain 420420417) on 2026-10-02 from the one
source in [`../../evm`](../../evm) with this network's settings
([`../../evm/deploy/networks.ts`](../../evm/deploy/networks.ts)), and read
back: each contract's code compared with the build and each setting read
(`node scripts/verify-deployments.ts` in `../../evm`). Lowest Bitcoin height
965,567; Conversion's largest swap 100,000 sats. The vault was redeployed in
genesis on 2026-10-07 from commit `f97885e`
([`docs/specs/ipow-vault-genesis.md`](../../docs/specs/ipow-vault-genesis.md));
it names the pair account `36fbxW7aLxS5VFBo4HUcNFweBo4WRQTm2SorKuMzqSfX`,
`["config", 5]` of the vault program
`3TZ1LJ4fVZVKbBUyzVMRjJ9FNPGaqVmGX56JuT5QdXEy`. The vault it replaced is in
[`evm/deployments/replaced/`](../../evm/deployments/replaced/).

| Contract | Address |
|---|---|
| Light client (`iPoWLightClient`) | `0x2E2b4eFf60659AFc08ae85E8A124a515008AEb9f` |
| Protocol | `0x5D38c6Bec531A3290f121C39C1f88231fa4B49b9` |
| Conversion | `0x3221102aC4159048960e061FC0ADb81De3133496` |
| BETA (`BetaBaskets`) | `0xB001c1Eb3B2D6dF4380682E119dDF1e3a9F18D89` |
| Vault home factory (genesis, 2026-10-07) | `0x51cD31AC42a838d225f3B42F830Af72d99Eb003a` |
| Vault receipts factory (genesis, 2026-10-07) | `0x48aA89dcB9355016c2010426f99d4c44D65281e9` |
| Vault paired with Solana (genesis, 2026-10-07) | `0xf0768cD3fDab897d6913De24fba571f103E3565F` |
| Its home part | `0x1aaB5EcCA56064b4bDa2507841717295677849FB` |
| Its receipts part | `0xD7881C39F422d01d4989930C3645f4983bCa9D4b` |

**Genesis.** This vault's receipts started in genesis for the deployer,
ending at 2026-10-21 17:00 UTC at the latest (`genesisEnd` in the record).
`sdk/scripts/genesis-check.ts`, run at commit `2897c06` (then
`packages/sdk/scripts/genesis-check.ts`) with the genesis run's ledger
([`evm/deployments/genesis-testnet.json`](../../evm/deployments/genesis-testnet.json)),
read every genesis receipt and issue back from the chains: 409 receipts and
470 issues across all the pairs, every one backed by its lock. Of these, 15
of the receipts and 16 of the issues are on this network's receipts parts,
and 1 of the locks issued are here.

## Setup and deploying

This package holds only this README, `.env.example`, and the `.gitignore`
that keeps the network's `.env` out of git
(`cp .env.example .env`, then fill in `POLKADOT_TESTNET_RPC_URL` and `POLKADOT_TESTNET_PRIVATE_KEY`). The
contracts, their tests and the deployment scripts are the one source in
[`../../evm`](../../evm); see its README for deploying.

The old contract generation that was deployed here (`iPoW`,
`iPoWConversion`, `BetaHub`, `BetaVault`), with its Hardhat package, was
removed; it stays in git at the tag `legacy-v1`.
