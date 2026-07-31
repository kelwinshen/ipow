# iPoW — Solana

Anchor program implementing the iPoW protocol natively for the SVM, deployed
to devnet. Unlike Ethereum/Hedera/Polkadot Hub (which all run the same
Solidity contract, since they're EVM-compatible), Solana needs its own
implementation of the same conversion lifecycle and Bitcoin SPV verification
logic. See the [root architecture doc](../../docs/ARCHITECTURE.md) for how a
conversion actually flows.

## Program layout

`programs/ipow/src/`:

- `lib.rs` — `#[program]` module with thin instruction entry points.
- `instructions/*.rs` — one file per instruction (commit, approve, deposit,
  refund, header relay, proof submission, liquidity, network admin, etc.),
  each holding its handler and `#[derive(Accounts)]` context together.
- `state/*.rs` — on-chain account structs (`GlobalState`, `Conversion`,
  `ProofCache`, `GlobalHeader`, `SupportedNetwork`, and a few small tracker
  accounts), shared across instructions.
- `bitcoin/*.rs` — the Bitcoin-specific pure functions: transaction/output
  parsing (`parsing.rs`) and proof-of-work/difficulty validation (`pow.rs`).
- `constants.rs`, `errors.rs`, `utils.rs` — protocol constants, the full
  error enum, and shared helpers (e.g. the escrow-transfer CPI).

`programs/ipow/tests/` has real Anchor integration tests (via `litesvm`),
not just a stub — currently 37 tests across admin/access-control, commit
validation, header relay (including ingesting a real Bitcoin genesis header
and rejecting a tampered one), initialization, and one full settlement-flow
integration test.

Program ID (devnet): `EsmGbkui9ZFC6Fch9J6xoyNRZSp6TwfvcjS88beP1Vem`.

## Setup

```sh
yarn install
```

Requires Rust, the Solana CLI, and Anchor `1.0.0` (see `Anchor.toml`).

## Build & test

```sh
anchor build
anchor test        # or: cargo test --manifest-path programs/ipow/Cargo.toml
```

## Deploying

```sh
anchor keys sync
solana airdrop 2    # devnet only
anchor deploy
```

## Post-deploy configuration

`scripts/init.ts` is the admin CLI for a deployed program — initializing
`global_state`, registering the other iPoW networks, and managing
liquidity. It mirrors the EVM packages' `configure.ts` scripts and is
env-var driven; see the comment block at the top of the file for the full
command list.

```sh
ACTION=status npx ts-node scripts/init.ts
ACTION=add-liquidity AMOUNT=0.5 npx ts-node scripts/init.ts
```
