# iPoW — Interoperable Proof of Work

iPoW moves value between Bitcoin and any of its supported programmable
networks (Ethereum, Hedera, Polkadot, Solana) — and, just as importantly,
between those networks themselves, using Bitcoin as the shared settlement
layer rather than a separate bridge for every pair of chains. Move BTC onto
Ethereum as a native asset, move it back off, or go straight from an asset
on Ethereum to one on Solana: all three are the same underlying mechanism,
just with the Bitcoin leg on one, both, or neither end being visible to the
user.

Each destination chain runs a contract/program that verifies Bitcoin
transactions itself via SPV (header chain + Merkle proof), rather than
trusting a third-party oracle for "this Bitcoin payment happened." An
off-chain operator service relays Bitcoin headers and coordinates the
approve/settle steps, but never custodies user funds outside of a short,
bounded window.

See [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md) for how a conversion
actually flows end to end, and [SECURITY.md](SECURITY.md) for the current
trust model — this is a testnet/devnet deployment under active development,
and the docs are direct about what's centralized today versus what's planned.

## Layout

This is a monorepo. `core/operator` is tracked as its own git repository
(separate remote) since it deploys independently of the on-chain contracts.

| Path | What it is |
| --- | --- |
| `programmable-network/ethereum` | Solidity contracts, deployed to Sepolia |
| `programmable-network/hedera` | Same contracts, deployed to Hedera Testnet (via the Hashio JSON-RPC relay) |
| `programmable-network/polkadot` | Same contracts, deployed to Polkadot Hub TestNet (via pallet-revive's `eth-rpc`) |
| `programmable-network/solana` | Anchor program, deployed to Solana devnet |
| `core/operator` | Rust service that streams Bitcoin headers, approves conversions, and settles them across all four networks |
| `packages/shared-types` | TypeScript types shared across the EVM/Solana script tooling |

Each package has its own README with setup, testing, and deployment
instructions specific to that chain.

## Why one contract, three EVM chains

Ethereum, Hedera, and Polkadot Hub all run the same Solidity source
(`iPoWV1.sol`, `iPoWV1Types.sol`, `BitcoinPrimitives.sol`) — Hedera's
Smart Contract Service and Polkadot Hub's REVM backend both expose a
standard Ethereum JSON-RPC interface, so no chain-specific contract logic is
needed. The operator and deploy scripts do account for real differences
between them (e.g. Hedera's `msg.value` being tinybar-scaled rather than the
usual 18-decimal weibar), and those are called out inline where they matter.

Solana runs a separate Anchor program (`programmable-network/solana`) since
it's not EVM-compatible, implementing the same conversion lifecycle and SPV
verification logic natively for the SVM.
