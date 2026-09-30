# iPoW — Interoperable Proof of Work

[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)
[![Security: unaudited](https://img.shields.io/badge/security-unaudited-red.svg)](SECURITY.md)
[![Status: testnet/devnet only](https://img.shields.io/badge/status-testnet%2Fdevnet-yellow.svg)](SECURITY.md)

> [!WARNING]
> This is under active development, deployed to testnets and devnet only.
> It has **not been audited** and is not production-ready. See
> [SECURITY.md](SECURITY.md) for the current trust model.

iPoW moves value between Bitcoin and any of its supported programmable
networks (Ethereum, Hedera, Polkadot, Base, Robinhood Chain, Hyperliquid,
Tempo, Solana) — and, just as importantly, between those networks
themselves, using Bitcoin as the shared settlement layer rather than a
separate bridge for every pair of chains. Move BTC onto Ethereum as a
native asset, move it back off, or go straight from an asset on Ethereum
to one on Solana: all three are the same underlying mechanism, just with
the Bitcoin leg on one, both, or neither end being visible to the user.

Each destination chain runs a contract/program that verifies Bitcoin
transactions itself via SPV (header chain + Merkle proof), rather than
trusting a third-party oracle for "this Bitcoin payment happened." An
off-chain operator service relays Bitcoin headers and coordinates the
approve/settle steps, but never custodies user funds outside of a short,
bounded window.

Two independent layers share that same Bitcoin-SPV foundation:
**Conversion** (`iPoWConversion.sol` / `ipow-conversion`) is a
permissionless, staked-auction native↔Bitcoin swap — the base primitive.
**Beta** (`BetaHub.sol`/`BetaVault.sol` / `beta-factory`) mints a
composed, multi-network token (BETA) backed by real locked value on each
network in its composition, using the same Bitcoin-anchored-statement
pattern, bonded parties, and permissionless judging Conversion
established — but with no direct call/CPI relationship between the two;
see [docs/design/ipow.md](docs/design/ipow.md) for the full design (an
earlier attempt at wiring them together via CPI was tried and abandoned —
see [docs/drafts/abandoned-cpi-funding-attempt.md](docs/drafts/abandoned-cpi-funding-attempt.md)
for why).

See [docs/design/ipow-implementation.md](docs/design/ipow-implementation.md)
for how a conversion actually flows end to end, and
[SECURITY.md](SECURITY.md) for the current
trust model — this is a testnet/devnet deployment under active development,
and the docs are direct about what's centralized today versus what's planned.

## Layout

This is a monorepo — `core/operator` deploys independently of the
on-chain contracts (its own CI job, its own release cadence) but is
tracked in this same repository, not a separate one.

| Path | What it is |
| --- | --- |
| `programmable-network/ethereum` | Solidity contracts, deployed to Sepolia |
| `programmable-network/hedera` | Same contracts, deployed to Hedera Testnet (via the Hashio JSON-RPC relay) |
| `programmable-network/polkadot` | Same contracts, deployed to Polkadot Hub TestNet (via pallet-revive's `eth-rpc`) |
| `programmable-network/base` | Same contracts, deployed to Base Sepolia |
| `programmable-network/robinhood` | Same contracts, deployed to Robinhood Chain testnet |
| `programmable-network/hyperliquid` | Same contracts behind a router+facets split (HyperEVM's tighter block gas limit forced it), deployed to HyperEVM testnet |
| `programmable-network/tempo` | Same contracts, plus PathUSD-denominated variants for the ones Tempo's chain-level native-value rejection breaks (`BetaVaultPathUSD`, `BetaHubPathUSD`, `iPoWConversionPathUSD`), deployed to Tempo (Moderato) testnet |
| `programmable-network/solana` | Anchor programs (`ipow`, `ipow-conversion`, `beta-factory`), deployed to Solana devnet |
| `core/operator` | Rust service that streams Bitcoin headers and drives Beta's claim→relay→exercise lifecycle across all 8 networks above (EVM via one adapter set, Solana via a separate one behind the same trait). Also drives the older, fixed-operator Conversion tunnel fused into `iPoW`/`ipow` itself, on every network except Tempo. The newer permissionless-auction Conversion (`iPoWConversion.sol`/`ipow-conversion`) needs no operator automation at all, by design — claimants act on their own |
| `packages/shared-types` | TypeScript types shared across the EVM/Solana script tooling |

Each package has its own README with setup, testing, and deployment
instructions specific to that chain.

## Why one contract, seven EVM chains

Ethereum, Hedera, Polkadot Hub, Base, and Robinhood Chain all run the same
Solidity source (`iPoW.sol`, `iPoWTypes.sol`, `BitcoinPrimitives.sol`,
`BetaHub.sol`, `BetaVault.sol`) — each exposes a standard Ethereum
JSON-RPC interface (Hedera via its Smart Contract Service, Polkadot Hub
via pallet-revive's `eth-rpc`), so no chain-specific contract logic is
needed. The operator and deploy scripts do account for real differences
between them (e.g. Hedera's `msg.value` being tinybar-scaled rather than the
usual 18-decimal weibar), and those are called out inline where they matter.

Two networks needed real, network-specific contract variants, not just
config differences:

- **Hyperliquid (HyperEVM)** has a tight testnet block gas limit that
  the plain contracts' deployed bytecode exceeds. Fixed with a
  router+facets (Diamond-style) split — the router exposes the identical
  ABI via `delegatecall` dispatch to its facets, so every off-chain
  caller is unaffected.
- **Tempo (Moderato)** rejects any transaction carrying native value
  outright, at the chain level — a real, confirmed constraint, not a gas
  issue. `BetaVaultPathUSD.sol`/`BetaHubPathUSD.sol`/
  `iPoWConversionPathUSD.sol` convert every bond/fee/payout that would
  otherwise use `msg.value` to move Tempo's real settlement ERC20
  (PathUSD) instead — the judging/verification logic itself is
  unchanged from the reference contracts.

Solana runs separate Anchor programs (`programmable-network/solana`) since
it's not EVM-compatible: `ipow` (header relay), `ipow-conversion`
(Conversion), `beta-factory` (Beta's hub) each implement the same
lifecycle and SPV verification logic natively for the SVM.

## Documentation

See [docs/README.md](docs/README.md) for the full documentation index —
the canonical design docs, the current trust model, and where a
work-in-progress design proposal belongs versus something that's already
built and verified.

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md) for setup, the PR process, and code
style. [CLAUDE.md](CLAUDE.md) has the source-of-truth map for anyone
(human or AI) reading this repo for the first time.

## License

[MIT](LICENSE).
