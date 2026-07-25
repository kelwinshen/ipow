# Architecture & design

This crate split enforces a strict separation between protocol logic and
per-chain implementation, so the operator can support multiple blockchain
families (EVM and Solana today) with shared business logic instead of
duplicating the tick loop per chain.

## Crate structure

### `crates/core`

Defines the rules every chain implementation has to follow:

- **Traits**: `ChainStack`, `StreamingAdapter`, `ApprovingAdapter`,
  `ConvertingAdapter`, `ChainProviderAdapter`.
- **BTC service** (`btc/btc_service.rs`): all direct Bitcoin RPC/Esplora
  interaction — header/tx lookups, Merkle proof construction, address
  derivation, broadcasting, wallet sweeping.
- **Models**: `Conversion`, `TransactionPhase`, `TransactionType`.
- Shared dependencies: Redis storage, config loading, tracing setup.

### `crates/network-vm`

Concrete trait implementations per chain family:

- `evm-revm` — Ethereum, Hedera, and Polkadot Hub. All three are
  EVM-compatible and share this one implementation; only network-specific
  config (RPC URL, chain ID, contract address) differs between them.
- `svm` — Solana, via an Anchor-generated client.

### `crates/operator`

The orchestration layer. `ChainOperator` (`chain_operator.rs`) runs a
periodic tick per chain it's watching:

1. **Approving** — approve pending commits and cross-chain tunnel requests
   once duty conditions are met.
2. **Streaming** — push just enough Bitcoin headers for whatever a chain's
   active conversions currently need.
3. **Converting** — pay out BTC for approved Native→Bitcoin conversions,
   and detect + settle incoming Bitcoin→Native payments.
4. **Tunneling** — open the second leg of a Native-to-Native conversion on
   its destination chain.
5. **Sweeping** — consolidate the BTC hot wallet's derived-address UTXOs.

`Registry` maps network names to their `ChainStack` instance so the
operator can look up "the Hedera stack" or "the Solana stack" by name at
runtime (used when watching multiple source networks for bridge intents).

## Adding a network

A new EVM-compatible chain needs only config (RPC URL, chain ID, contract
address) added to `evm-revm`'s network list — no new crate. A genuinely new
chain family (not EVM, not Solana) would need a new `network-vm` crate
implementing the same `crates/core` traits.

## Engine decoupling

Setting the `ENGINE` environment variable restricts an operator instance to
one tick (`streamer`, `approver`, `converter`, or `rebalance`) instead of
running all of them — useful for scaling each independently in production.
