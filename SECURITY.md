# Security & trust model

This is under active development, deployed to testnets and devnet only
(Sepolia, Hedera Testnet, Polkadot Hub TestNet, Solana devnet) — no mainnet
deployment yet. This document is meant to be a direct, current statement of
what's centralized today, not an aspirational description of the end state.

## What the contracts already protect against

Bitcoin verification itself is trustless by design (see
[docs/ARCHITECTURE.md](docs/ARCHITECTURE.md)): the contract validates header
proof-of-work, difficulty retargeting, and Merkle proofs itself, so the
operator can't forge what a header or proof says. Users also aren't at the
operator's mercy for liveness — if the operator doesn't approve or complete
a conversion within its deadline, permissionless refund/claim functions let
the user (or anyone) unwind it without needing the operator's cooperation.

## What's still centralized

- **One operator key per chain.** Each deployed contract gates approvals,
  header relay, and liquidity management behind a single `onlyOperator`
  address (`global_state.operator` on Solana). Whoever holds that key can
  approve or withhold approvals, and controls the pace of header relay.
- **One Bitcoin hot wallet.** The operator's BTC-side actions (paying out
  Native→Bitcoin conversions, sweeping funds) are signed by a single HD
  wallet (mnemonic/xpub). Bitcoin itself has no way to enforce the
  duty-window/permissionless-cleanup protections that the destination-chain
  contracts provide — a compromised or dishonest operator can misdirect BTC
  that's already left the destination chain's custody.
- **Liquidity reserves are held directly by each contract**, funded and
  withdrawable by the operator address.

In short: the destination-chain side of a conversion has real trustless
guarantees today; the Bitcoin-custody side does not yet.

## Where this is headed

The plan is to move from a single operator key to multiple operators with
threshold-signed custody (so no single key can move funds alone) and a
bonded/slashable stake backing operator duty, rather than unilateral trust
in one key. This isn't implemented yet — it's a deliberate next phase, not
a promise of a specific timeline.

## Reporting a vulnerability

If you find a security issue, please don't open a public GitHub issue for
it. Use GitHub's private security advisory feature on this repository
instead, so it can be addressed before details are public.
