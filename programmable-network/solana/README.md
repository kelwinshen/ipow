# iPoW — Solana

Three Anchor programs implementing the iPoW protocol natively for the SVM,
deployed to devnet. Unlike the EVM networks (which all run the same
Solidity source), Solana needs its own implementation of the same
conversion lifecycle and Bitcoin SPV verification logic. See the
[root architecture doc](../../docs/design/ipow-implementation.md) for how a conversion
actually flows, and [design/ipow-implementation.md](../../docs/design/ipow-implementation.md) (top-level
summary and §9) for Beta's composition design and how Conversion and Beta
relate — two independent layers sharing only the Bitcoin header relay, no
CPI between them.

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

## Programs

- **`ipow`** (`programs/ipow/`) — the Bitcoin header relay: ingests and
  validates the Bitcoin header chain (proof-of-work/difficulty checks) and
  caches SPV proofs other programs verify against. Program ID (devnet):
  `EsmGbkui9ZFC6Fch9J6xoyNRZSp6TwfvcjS88beP1Vem`.
- **`ipow-conversion`** (`programs/ipow-conversion/`) — the permissionless,
  staked-auction native↔Bitcoin swap (SVM counterpart of
  `iPoWConversion.sol`): commit, propose/finalize claim, submit proof,
  `add_network`/`remove_network` cross-registration, `open_bundle_tunnel`.
  Program ID (devnet): `FbwXLABMqUeRPw85C7MpMS2LQi4mveXR9a9fA79DfS3V`. Has
  its own dedicated `ipow` header source, separate from Beta's below.
- **`beta-factory`** (`programs/beta-factory/`) — Beta's hub (SVM
  counterpart of `BetaHub.sol`): bonded-party registry, composition
  registration, anchor judging, `lock_sol`/`exercise_mint`/`burn_redeem`.
  Program ID (devnet): `BkNb9JfNbfbn3rMiA3j3z2pnFdNibxiYWWsKZ9VdzoD1` (a
  fresh ID as of 2026-09-23 for composition support — a breaking
  account-layout change, not an in-place upgrade; see the comment in
  `Anchor.toml`). Points at its own `ipow` deployment,
  `EsmGbkui9ZFC6Fch9J6xoyNRZSp6TwfvcjS88beP1Vem` — the same header-relay
  instance the `ipow` program above already deploys, reused rather than
  duplicated.

An earlier `beta-mint` program (basket-mint/redeem via CPI into
`ipow-conversion`) has been removed — see design/ipow-implementation.md §9.1 for why
Conversion and Beta were decoupled instead of composed via CPI.

Each program's own `programs/<name>/src/` follows the same shape:
`lib.rs` (thin `#[program]` entry points), one file per instruction,
`state/*.rs` (account structs), `errors.rs`/`constants.rs`/`utils.rs`.
`ipow/` additionally has `bitcoin/*.rs` for the Bitcoin-specific pure
functions (transaction/output parsing, proof-of-work validation) that
`ipow-conversion` and `beta-factory` both verify SPV proofs against.

Real Anchor integration tests (via `litesvm`) exist for all three:
`ipow` (23 across admin/access-control, commit validation, header relay —
including ingesting a real Bitcoin genesis header and rejecting a
tampered one — initialization, and a full settlement-flow test),
`ipow-conversion` (21, covering the full commit → auction →
proof-submission → payout lifecycle plus reclaim/refund paths), and
`beta-factory` (30, covering party registration, composition
registration, `lock_sol`, anchor judging, `exercise_mint`/`burn_redeem`,
bond top-up/unbond, and `expire_pending`).

## Setup

```sh
yarn install
```

Requires Rust, the Solana CLI, and Anchor `1.0.0` (see `Anchor.toml`).

## Build & test

```sh
anchor build
anchor test        # or: cargo test --manifest-path programs/<name>/Cargo.toml, per program
```

## Deploying

```sh
anchor keys sync
solana airdrop 2    # devnet only
anchor deploy
```

`anchor deploy` deploys every program listed under `[programs.devnet]` in
`Anchor.toml`.

## Post-deploy configuration

`scripts/init.ts` is the admin CLI for the deployed `ipow` program —
initializing `global_state`, registering the other iPoW networks, and
managing liquidity. It mirrors the EVM packages' `configure.ts` scripts
and is env-var driven; see the comment block at the top of the file for
the full command list.

```sh
ACTION=status npx ts-node scripts/init.ts
ACTION=add-liquidity AMOUNT=0.5 npx ts-node scripts/init.ts
```

`scripts/beta_factory_e2e.ts` exercises `beta-factory`'s live devnet
deployment end to end (initialize, register a party, register a
composition, lock, exercise) — useful as a reference for the account
shapes each instruction expects, not something to run casually against
the real deployment.
