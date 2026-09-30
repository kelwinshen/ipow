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
