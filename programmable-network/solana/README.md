# iPoW — Solana

The new protocol's Anchor programs for the SVM
([`docs/design/ipow-protocol.md`](../../docs/design/ipow-protocol.md)), deployed
to devnet. Unlike the EVM networks (which all run the same Solidity source),
Solana has its own implementation of the light client, the protocol, the
vault and the applications.

## New protocol: test network deployment

The new protocol's programs ([`docs/design/ipow-protocol.md`](../../docs/design/ipow-protocol.md)),
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
| Vault (`ipow-vault`) | `2e1ZUB5eqA6bQfH3f9pbfvibfie7WnvEJeKif53VAm7p` |
| Conversion (`conversion`) | `9Argk3M83p8t5pWG92PhwEhYzysWt5n82siEWyb29w7D` |
| BETA (`beta-basket`) | `3DuG4iNPptEyCAA7FM1YQja43G6HkTAwrswT3Ds2d94R` |

| Vault pair | Pair account | The vault there |
|---|---|---|
| Sepolia (1) | `5Ks1r25VGX5S7dUMf8c5oP6qzXfFvckQ67wgtqywnDAN` | `0xD18b7d290f8c94fe561710A78F17494ff2520302` |
| Base Sepolia (3) | `Gyhrq1CzHyU9BfECK5G8RW4ho75oaRPqMngXYYvLuduL` | `0x4cFD981522F31A4dc12Aeb433907ebc3CBD37A55` |
| Robinhood testnet (4) | `Dydhoos4EgX15LFLJR5a6DvGtuzmNjegpzzynJSJbF5` | `0x15390C4901e11D3732489825c2FBa33A2E3e5b37` |
| Polkadot testnet (5) | `BcTMgkGcknJu9GW6XQ7HoRdXdP3Jo6iRt8ZjKe6fm7r7` | `0x77ef03207173b08A82d6511a35f970033E185d6c` |
| Hedera testnet (6) | `7Wwn5bjARBmdCjNT7KUBwB7LR3JYSLFAWSSm98pxvE2S` | `0x4d1C3FdE9FaD26b9882b4E607120091E2ff285B4` |
| HyperEVM testnet (7) | `D73AnEY7aViFo3r6P8iPxMBxEqh6P5DiR4B6dMj7SixA` | `0x4cFD981522F31A4dc12Aeb433907ebc3CBD37A55` |
| Tempo testnet (8) | `7wvWJdeTW9WynaP17Le7dnWcBkV7qpVz78sR9TrA977Q` | `0x10C9C79C8c46c1f9f07D7A258Bbd42B4ED6f973E` |

## Programs

Each in `programs/<name>/`, with its id declared in `src/lib.rs` and listed
under `[programs.devnet]` in `Anchor.toml`:

- `ipow-light-client` — the Bitcoin light client.
- `ipow-protocol` — the protocol (operators, jobs, claims).
- `ipow-vault` — the vault, paired with each EVM network's vault.
- `conversion` — Conversion, an application on the protocol.
- `beta-basket` — BETA.

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
cargo build-sbf --manifest-path programs/ipow-light-client/Cargo.toml --features test-limits --sbf-out-dir target/deploy-test
cargo build-sbf --manifest-path programs/ipow-vault/Cargo.toml --features test-limits --sbf-out-dir target/deploy-test
```

## Post-deploy configuration

`scripts/init-testnet.ts` sets the programs up on devnet after they are
deployed and reads every pair back; see the comment at its top.
