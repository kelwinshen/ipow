# iPoW Operator

Rust service that drives the off-chain side of the iPoW protocol: streaming
Bitcoin headers, approving conversions, and settling them across every
supported network. See the [root architecture doc](../../docs/ARCHITECTURE.md)
for how a conversion flows end to end, and [architecture.md](./docs/architecture.md)
in this package for how the service itself is structured.

## Layout

Chain-specific logic (`crates/network-vm`) implements a shared set of
traits (`crates/core`) so the same tick logic in `crates/operator` runs
against every network without per-chain branching.

```text
.
├── crates/
│   ├── core/         # traits, state machine, shared services (BTC RPC, Redis, config)
│   ├── network-vm/   # evm-revm (Ethereum/Hedera/Polkadot Hub) and svm (Solana) adapters
│   ├── operator/      # ChainOperator: the tick loop that drives everything
│   └── api/            # HTTP API (transaction-limit lookups, etc.)
├── docs/                # architecture.md, getting_started.md
└── mise.toml             # task runner definitions
```

## Setup

```sh
mise install
cp .env.example .env         # RPC endpoints, operator private keys, BTC wallet config
cp config.example.yml config.yml   # per-network limits and non-secret settings
```

See [docs/getting_started.md](./docs/getting_started.md) for running it
locally, and `mise tasks` for the full list of available tasks (per-network
runners, linting, Docker build/push).

## Running

```sh
mise run run-ethereum hedera     # run the Ethereum operator, watching Hedera for bridge intents
ENGINE=streamer mise run run-solana   # run just the header-streaming engine
```

## Docker

```sh
mise run docker-build
```
