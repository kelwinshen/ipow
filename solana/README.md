# iPoW — Solana

The new protocol's Anchor programs for the SVM
([`docs/design/ipow-protocol.md`](../docs/design/ipow-protocol.md)), deployed
to devnet. Unlike the EVM networks (which all run the same Solidity source),
Solana has its own implementation of the light client, the protocol, the
vault and the applications.

## New protocol: test network deployment

The new protocol's programs ([`docs/design/ipow-protocol.md`](../docs/design/ipow-protocol.md)),
deployed on devnet on 2026-10-02 at their declared ids and set up with
[`scripts/init-testnet.ts`](scripts/init-testnet.ts), which reads every pair back:
the light client's lowest Bitcoin height 965,567, and the vault's pair with each
EVM network, with a deposit of 0.001 SOL and a least certifying escrow of 0.01
SOL. The upgrade authority is kept on devnet, so the programs can be fixed
while testing; on mainnet it is removed (D59).

| Program | Id |
|---|---|
| Light client (`ipow-light-client`) | `Ctotq1SSaDJ2EUSe4sMdau92qBesyutH7GPaTRiYp4vJ` |
| Protocol (`ipow-protocol`) | `ChYhovM8vm2tuMRaRFn4m6etG979bjixVa71fXBDwjPL` |
| Vault (`ipow-vault`), the genesis build | `3TZ1LJ4fVZVKbBUyzVMRjJ9FNPGaqVmGX56JuT5QdXEy` |
| Conversion (`conversion`) | `9Argk3M83p8t5pWG92PhwEhYzysWt5n82siEWyb29w7D` |
| BETA (`beta-basket`) | `3DuG4iNPptEyCAA7FM1YQja43G6HkTAwrswT3Ds2d94R` |

The vault is the program deployed for the vaults' genesis
([`docs/specs/ipow-vault-genesis.md`](../docs/specs/ipow-vault-genesis.md)),
its id declared in `programs/protocol/ipow-vault/src/lib.rs` and in
`Anchor.toml`. Its pairs, each `["config", peer]` of the program, each in
genesis until 2026-10-21 17:29:45 UTC (read from devnet on 2026-10-09):

| Vault pair | Pair account | The vault there |
|---|---|---|
| Sepolia (1) | `Cztcuoj5XAhZ35ky6Ud3WMq9R7ijcvPTnnvSjGQ88sEg` | `0x26187A8aC987c7d6d7610c64a497595698C9A77F` |
| Base Sepolia (3) | `EEyy1MZhpDXurack8QhkeoAVTDmsHkn2zZ1H9QynppMN` | `0x82Fd247e3dBA26E5De0023043b262C5B1DBEe03c` |
| Robinhood testnet (4) | `GCpZ6DFQyeVLNBzKbeV4okqW745kioHvJmGGAjs1W1So` | `0xf8374e052e5b3A8518f1834C527A09A91023F58C` |
| Polkadot testnet (5) | `36fbxW7aLxS5VFBo4HUcNFweBo4WRQTm2SorKuMzqSfX` | `0xf0768cD3fDab897d6913De24fba571f103E3565F` |
| HyperEVM testnet (7) | `8nJB7bCtN6DpeZGHNtLxaE11rdKnRkFhGR4LJUTsCffo` | `0x05EAD8b6dac6f78781cF4c582fF4311358F38118` |
| Tempo testnet (8) | `8USDsC8MpsnKZmt8KAkVDr18R6s5g71Fro8Jr2JMJmmr` | `0x6c692BEdCa89292D0FEfc6e86b82f451E371f11D` |
| Arbitrum Sepolia (9) | `AizTkFPMioKpZEMxQ1fsBBbjTi9U8KHmwXX32Z6pBwsh` | `0x170685feEe5ac2bCddAE20DAe66747658E6fA7c0` |

Hedera was not redeployed in genesis: its vault,
`0x4d1C3FdE9FaD26b9882b4E607120091E2ff285B4`, stays paired with the earlier
vault program, `2e1ZUB5eqA6bQfH3f9pbfvibfie7WnvEJeKif53VAm7p`, through its
pair account `7Wwn5bjARBmdCjNT7KUBwB7LR3JYSLFAWSSm98pxvE2S`. That program
is still deployed, with its earlier pairs with the other networks, which the
genesis vaults replaced.

**Genesis.** `sdk/scripts/genesis-check.ts`, run at commit `479d93e` (then
`packages/sdk/scripts/genesis-check.ts`) with the genesis run's ledger
([`evm/deployments/genesis-testnet.json`](../evm/deployments/genesis-testnet.json)),
read every genesis receipt and issue back from the chains: 409 receipts and
470 issues across all the pairs, every one backed by its lock. Of these, 67
of the receipts and 77 of the issues are on this program's pairs, and 101 of
the locks issued are on Solana.

## Programs

Each with its id declared in `src/lib.rs` and listed under
`[programs.devnet]` in `Anchor.toml`. The protocol, in `programs/protocol/`:

- `ipow-light-client` — the Bitcoin light client.
- `ipow-protocol` — the protocol (operators, jobs, claims).
- `ipow-vault` — the vault, paired with each EVM network's vault.

The applications, in `programs/applications/`:

- `conversion` — Conversion ([spec](../docs/specs/ipow-conversion-app.md)).
- `beta-basket` — BETA ([spec](../docs/specs/ipow-beta-app.md)).

The old program generation (`ipow`, `ipow-conversion`, `beta-factory`) and
its scripts (`scripts/init.ts`, `scripts/beta_factory_e2e.ts`) were removed;
they stay in git at the tag `legacy-v1`. Their devnet deployments are
untouched.

## Setup

```sh
yarn install
```

Requires Rust, the Solana CLI, and Anchor `1.0.0` (see `Anchor.toml`).

## Build & test

The program keypairs are not in this repository, so build without syncing
keys (never `anchor keys sync`: it would rewrite the declared ids):

```sh
anchor build -p <name> --ignore-keys     # each program, into target/deploy
cargo test --workspace
```

Build one program at a time (a workspace build unifies features and
empties the programs others call). The tests load the built programs:
`target/deploy/*.so`, and for `ipow-light-client` and `ipow-vault` also a
build with the `test-limits` feature in `target/deploy-test/`:

```sh
cargo build-sbf --manifest-path programs/protocol/ipow-light-client/Cargo.toml --features test-limits --sbf-out-dir target/deploy-test
cargo build-sbf --manifest-path programs/protocol/ipow-vault/Cargo.toml --features test-limits --sbf-out-dir target/deploy-test
```

## Post-deploy configuration

`scripts/init-testnet.ts` sets the programs up on devnet after they are
deployed and reads every pair back; see the comment at its top.

## What is on devnet

[`deployments/devnet.json`](deployments/devnet.json) records each devnet
program as deployed: its id, the commit its program code was built from,
and the length and sha256 of the deployed program. A build embeds its
source paths, so a fresh build (another tree, or since the move to
`solana/programs/{protocol,applications}/`) is other bytes even from the
same code: compare devnet with the record, not with `target/deploy`.
`scripts/verify-devnet.sh [<name> ...]` reads the programs back
(`solana program dump`, read-only) and checks them against it. Update the
record whenever a program is deployed or upgraded.
