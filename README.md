# iPoW

[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)
[![Security: unaudited](https://img.shields.io/badge/security-unaudited-red.svg)](SECURITY.md)
[![Status: test networks only](https://img.shields.io/badge/status-test%20networks-yellow.svg)](SECURITY.md)

> [!WARNING]
> Deployed on test networks only. Not audited. Not for real funds. See
> [SECURITY.md](SECURITY.md) for what is trustless today and what the
> deployer still controls.

iPoW uses Bitcoin's chain as the shared source of truth between
programmable networks (Ethereum, Solana, and other EVM networks). Instead
of a bridge per pair of networks, each network verifies Bitcoin itself,
and anything one network needs another to know is written to Bitcoin.

- **Every network runs a Bitcoin light client.** A contract (on Solana, a
  program) that stores Bitcoin block headers and checks their
  proof-of-work, difficulty and Merkle proofs on chain. Anyone can add
  blocks; there is no oracle and no owner.
- **Operators bond stake and take jobs.** Anyone can lock a bond and
  become an operator. An application opens a job, operators bid for it,
  and the winner must publish a tagged Bitcoin transaction for it and
  prove it to the light client before a deadline. A missed deadline, or a
  proof on blocks that are not Bitcoin's real chain, gets its escrow
  slashed. Guardians, anyone watching, challenge false proofs and are paid
  from the slash.
- **Vaults move assets between networks.** A user locks an asset in the
  vault on one network and receives a receipt token for it on another
  (lock ETH on Ethereum, receive vETH on Solana; burn the vETH, get the ETH
  back). The operator carries the record of the lock as a claim, backed by
  its vault bond and anchored by one hash on Bitcoin. Anyone can object to
  a false claim during its 7-day window; a proven lie is slashed. A claim
  is safe while at least one honest guardian watches.

The full design, with every decision numbered (D1, D2, ...), is
[`docs/design/ipow-protocol.md`](docs/design/ipow-protocol.md).

## Protocol and applications

The protocol is the shared layer: light client, operators and jobs,
claims, and the vault. Applications are contracts that register with the
protocol (no approval needed) and open jobs on it; they hold no operator
logic of their own.

```
            ┌─────────────────────── applications ───────────────────────┐
            │  Conversion: a network's coin or token  ⇄  real BTC        │
            │              (and network ⇄ network, via one BTC payment)  │
            │  BETA:       a token backed by a basket of vault receipts  │
            └──────────────┬──────────────────────────────┬──────────────┘
                     opens jobs                     holds receipts
            ┌──────────────┴──────────── protocol ────────┴──────────────┐
            │  light client   operators, bonds, jobs, claims    vault    │
            └──────────────┬─────────────────────────────────────────────┘
                 headers, proofs, tagged transactions, batch hashes
            ┌──────────────┴─────────────────────────────────────────────┐
            │                         Bitcoin                            │
            └────────────────────────────────────────────────────────────┘
```

- **Conversion** ([spec](docs/specs/ipow-conversion-app.md)): a user sells
  a network's coin or a token for real BTC, or buys it with BTC. The
  operator that wins the job is the other side of the swap; its Bitcoin
  payment is proven to the light client before it is paid. Two Conversion
  legs linked by one Bitcoin payment convert between two programmable
  networks ([tunnel spec](docs/specs/ipow-conversion-tunnel.md)).
- **BETA baskets** ([spec](docs/specs/ipow-beta-app.md)): anyone creates a
  token backed by a fixed basket of up to 8 parts, for example 1 SOL +
  1 vETH. Minting deposits the parts; burning returns them.

## Networks

Deployed on test networks, all from the same source in this repository.
Each network's README describes the deployment there and has its
addresses: an EVM network's are those of its deployment record (below),
with the vaults of the genesis redeploy of 2026-10-07.

| Network | Number | Addresses |
|---|---|---|
| Ethereum Sepolia | 1 | [`evm/README.md`](evm/README.md) |
| Solana devnet | 2 | [`solana/README.md`](solana/README.md) |
| Base Sepolia | 3 | [`networks/base`](networks/base/README.md) |
| Robinhood Chain testnet | 4 | [`networks/robinhood`](networks/robinhood/README.md) |
| Polkadot Hub TestNet | 5 | [`networks/polkadot`](networks/polkadot/README.md) |
| Hedera testnet | 6 | [`networks/hedera`](networks/hedera/README.md) |
| HyperEVM testnet | 7 | [`networks/hyperliquid`](networks/hyperliquid/README.md) |
| Tempo testnet (Moderato) | 8 | [`networks/tempo`](networks/tempo/README.md) |
| Arbitrum Sepolia | 9 | [`networks/arbitrum`](networks/arbitrum/README.md) |

The machine-readable records are `evm/deployments/<network>-testnet.json`,
and `sdk/src/generated/` holds what the SDK reads. Every EVM deployment is
read back from its network by `evm/scripts/verify-deployments.ts`. The
light clients take Bitcoin mainnet blocks: there is no Bitcoin testnet
mode, and operators spend real (small) amounts of BTC.

## Repository layout

| Path | What it is |
|---|---|
| [`evm/`](evm/README.md) | Hardhat project: the Solidity contracts for every EVM network. `contracts/protocol` (light client, protocol, vault), `contracts/applications/{conversion,beta}`, `contracts/testnet` (mock RWA tokens), `deploy/` (per-network settings), `scripts/`, `test/`, `deployments/` |
| [`solana/`](solana/README.md) | Anchor workspace: `programs/protocol/{ipow-light-client,ipow-protocol,ipow-vault}` and `programs/applications/{conversion,beta-basket}` |
| [`node/`](node/README.md) | The Rust service that runs the operator, guardian and attester roles, and the vault's roles, on any number of networks |
| [`sdk/`](sdk/README.md) | TypeScript SDK (`@ipow/sdk`) for apps built on the protocol |
| [`networks/`](networks/) | One folder per EVM network other than Ethereum: its README (addresses, what is particular to it) and `.env.example`; Tempo also has its own deploy scripts |
| [`docs/`](docs/README.md) | `design/` (the canonical design), `specs/` (built applications and features), `drafts/` (build plan, live-run records), `archive/` (the retired generation) |

## Quick start

Requirements: Node 22.18 or later (it runs the TypeScript scripts with
its type stripping) and pnpm 11; Rust; the Solana CLI (CI uses
v3.1.10) to build the Solana programs and for the SDK's tests (a local
validator). The node's tests need the built programs, not the CLI.
None of the tests need RPC endpoints or keys: they run on local networks.
These are the commands CI runs ([`.github/workflows/check.yml`](.github/workflows/check.yml)).

```sh
pnpm install --frozen-lockfile

# EVM contracts (Hardhat, local network)
pnpm --dir evm test

# Solana programs: build one at a time, plus the test-limits builds, then test
cd solana
for p in protocol/ipow-light-client protocol/ipow-protocol protocol/ipow-vault applications/conversion applications/beta-basket; do
  cargo build-sbf --manifest-path programs/$p/Cargo.toml
done
cargo build-sbf --manifest-path programs/protocol/ipow-light-client/Cargo.toml --features test-limits --sbf-out-dir target/deploy-test
cargo build-sbf --manifest-path programs/protocol/ipow-vault/Cargo.toml --features test-limits --sbf-out-dir target/deploy-test
cargo test --workspace
cd ..

# The node (needs the Solana programs built above; starts Hardhat itself)
(cd node && cargo build --workspace && cargo test --workspace)

# The SDK (its Solana tests run a local validator with the built programs)
pnpm --dir sdk build && pnpm --dir sdk test
```

Each package's README covers running and deploying.

## Used by Greatwall

[Greatwall.finance](https://github.com/Renrensan/greatwall), in its own
repository, is the user-facing app built on the SDK: it converts between
networks and BTC with Conversion, and creates and mints BETA baskets.

## Security

Unaudited, test networks only. The EVM contracts have no owner and no
key, with one exception: the vaults' genesis key, until genesis ends. The
deployer holds that key and the Solana programs' upgrade authority, and
runs the only operator today.
[SECURITY.md](SECURITY.md) states each of these with where to check it,
and how to report a vulnerability.

## Contributing and license

See [CONTRIBUTING.md](CONTRIBUTING.md). [CLAUDE.md](CLAUDE.md) has the
rules and the source-of-truth map for anyone, human or AI, working in this
repository. Licensed under the [MIT License](LICENSE).
