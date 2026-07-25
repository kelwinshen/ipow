# Getting started

## Prerequisites

- **Rust** (latest stable) — for building the operator.
- **mise** — task runner; all commands below go through it.
- **Docker & Docker Compose** — for local Redis.

## Installation

```sh
git clone <repo-url>
cd operator

cp .env.example .env               # RPC endpoints, operator private keys, BTC wallet config
cp config.example.yml config.yml   # per-network limits and non-secret settings

mise install
```

## Running locally

```sh
mise tasks   # list every available task
```

The general form for a per-network task is:
`[ENGINE=<type>] [WATCH=<networks>] mise run <task-name>`

### Per-network runners

Each supported network has its own task (`run-ethereum`, `run-hedera`,
`run-solana`, `run-polkadot`), which also ensures the local Redis container
is running:

```sh
# Run Hedera, watching Ethereum for bridge intents
mise run run-hedera eth

# Run Solana with only the streaming engine
ENGINE=streamer mise run run-solana
```

`run-custom <src_network> [watch_sources...]` runs any network supported by
the codebase that doesn't have a dedicated shortcut task yet.

### Engines

The `ENGINE` variable restricts a run to one part of the tick loop instead
of running all of them:

| Engine | What it does |
| --- | --- |
| `all` (default) | Runs approving, streaming, and converting. |
| `approver` | Approves pending commits and tunnel requests. |
| `streamer` | Streams Bitcoin headers to the contract/program. |
| `converter` | Executes the final settlement transactions. |
| `rebalance` | Runs liquidity rebalancing only. |

There are also dedicated shortcuts for a couple of the more commonly
isolated combinations: `stream-solana`, `convert-solana`,
`rebalance-<network>`.

### API server

```sh
mise run run-api   # HTTP API (transaction-limit lookups, etc.), default port 8888
```

### Linting

```sh
mise run lint   # cargo fmt --check + clippy
```

### Docker

```sh
mise run docker-build
mise run docker-push
```

---

## Troubleshooting

**Unexpected argument error:** if you see
`error: unexpected argument '...' found`, run the command through the
`mise` task rather than `cargo run` directly — the tasks handle the
`--watch-sources` flag for you.

**Shell syntax error:** if you see `sh: Syntax error`, make sure your
environment has `bash` available; the tasks run through `bash -c` so
arguments reach the Rust binary correctly.
