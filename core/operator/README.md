# iPoW Operator

Rust service that drives the off-chain side of the iPoW protocol: streaming
Bitcoin headers, and running two genuinely independent engines —
**Conversion** (approving/settling native↔Bitcoin swaps) and **Beta**
(claim → relay → exercise for the composed, multi-network BETA token) —
across every supported network: Ethereum, Hedera, Polkadot Hub, Base,
Robinhood Chain, Hyperliquid, Tempo, and Solana. See the
[root architecture doc](../../docs/ARCHITECTURE.md) for how a conversion
flows end to end, [docs/DESIGN_V2.md](../../docs/DESIGN_V2.md) for Beta's
own design, and [architecture.md](./docs/architecture.md) in this package
for how the service itself is structured.

## Layout

Chain-specific logic (`crates/network-vm`) implements a shared set of
traits (`crates/core`) so the same tick logic runs against every network
without per-chain branching.

```text
.
├── crates/
│   ├── core/          # traits, state machine, shared services (BTC RPC, Redis, config)
│   ├── network-vm/    # evm-revm (every EVM network above) and svm (Solana) adapters
│   ├── operator/       # ChainOperator (Conversion's tick loop) and BetaOperator (Beta's, separate)
│   └── api/            # HTTP API (transaction-limit lookups, etc.)
├── docs/                # architecture.md, getting_started.md
└── mise.toml             # task runner definitions
```

Two CLI entry points, not one: `cargo run -- operator --src <network>`
drives Conversion for one network at a time; `cargo run -- beta` drives
Beta across every network with a `beta_networks.*` entry in `config.yml`
in a single process (a composition's remote legs can span several
networks at once, so it isn't per-network like Conversion's own CLI).

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
