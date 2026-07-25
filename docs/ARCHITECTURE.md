# Architecture

This describes how a conversion moves through the protocol, and what each
piece (contract, operator service) is actually responsible for. It applies
to all four networks — Ethereum, Hedera, and Polkadot Hub run the same
Solidity contract; Solana runs an Anchor program implementing the same
lifecycle.

The three conversion types below (Native→Bitcoin, Bitcoin→Native,
Native→Native) aren't three separate features — Native→Native is the other
two chained back to back, anchored to the same Bitcoin header. That's what
makes this an interoperability layer between the programmable networks
themselves, not just a Bitcoin bridge for each one individually: Bitcoin's
header chain is the one shared trust root every pair of networks settles
against, instead of each pair needing its own bridge.

## The trust problem this solves

A destination chain (say, Ethereum) has no native way to know "this Bitcoin
payment happened." The usual fix is a trusted oracle or custodian attesting
to it. iPoW instead has the contract verify Bitcoin directly: it keeps its
own copy of the Bitcoin header chain and checks proof-of-work, difficulty
retargeting, and chain continuity itself, then accepts Merkle proofs against
that self-verified chain. An operator still has to *deliver* headers and
proofs — the contract can't fetch them on its own — but it can't be lied to
about their contents. That's the core SPV (Simplified Payment Verification)
property this protocol relies on, and it's why the operator's role is
mostly about liveness (showing up on time), not correctness (the contract
checks correctness itself).

## Conversion types

- **Native → Bitcoin** (`commitNativeToBitcoin`): user locks native tokens
  and a destination Bitcoin script. The operator pays BTC out of its own hot
  wallet, then proves that payment on-chain to release the locked native
  tokens (plus fee) back to itself.
- **Bitcoin → Native** (`commitBitcoinToNative`): user sends BTC to an
  address the operator monitors. Once the operator proves that payment
  on-chain, the contract releases native tokens to the user from its
  liquidity reserve.
- **Native → Native (tunnel)**: the same `commitBitcoinToNative` entry
  point, called directly by the operator, opens a second leg on a
  destination chain anchored to the same Bitcoin header height as the
  first leg's proof — using Bitcoin's header chain as the shared trust
  root between two non-Bitcoin chains, rather than a separate bridge
  protocol between them.

## Header relay

The contract stores its own running chain of Bitcoin headers
(`GlobalHeaderMeta`): hash, previous hash, Merkle root, difficulty bits, and
timestamp, Little-Endian. The operator pushes raw 80-byte headers via
`commitGlobalBitcoinHeader80`, and the contract independently validates:

- proof-of-work against the claimed difficulty bits,
- difficulty retargeting every `DIFF_PERIOD` (2016) blocks, against the
  previous epoch's actual timespan,
- and continuity — a new header must build on the current tip, and can't
  rewrite an already-recorded height.

Because of this, the operator can choose *when* to relay headers, but not
*what* they say.

## Approval, duty windows, and permissionless cleanup

After a user commits, the operator has `APPROVAL_WINDOW_SEC` (15 minutes) to
approve the conversion and anchor it to a specific header height. Approval
starts a bounded "duty window" — the operator's deadline to finish its side
(pay out BTC, or relay/confirm enough headers for a proof). If the operator
doesn't show up, the conversion doesn't get stuck waiting on it: functions
like `refundIfNotApproved`, `refundAfterNoProof_NativeToBitcoin`, and
`claimNative_AfterOperatorExpired` are permissionless — anyone, including
the user themselves, can call them once the relevant deadline has passed, to
unwind the conversion. `timeoutNoDeposit_NativetoBitcoin` and
`closeNoBitcoin_BitcoinToNative` are the operator-side equivalents for when
the *user's* side times out instead.

Once a proof is submitted, it isn't necessarily final immediately — Bitcoin
confirmation and destination-chain block time don't line up, so proofs are
cached (`ProofCache`) and only finalized once enough further headers
(`PROOF_BLOCKS_WINDOW`, 40 blocks) have been relayed on top of the block the
proof references.

## The operator service (`core/operator`)

The Rust operator (a separate git repository) runs a periodic tick per chain
it's watching, via `ChainOperator` (`crates/operator/src/chain_operator.rs`):

- **approving** — approves pending commits and cross-chain tunnel requests
  once duty conditions are met.
- **streaming** — pushes just enough Bitcoin headers to satisfy whatever a
  chain's active conversions currently need (a proof confirmation, or an
  anchor height for a pending tunnel).
- **converting** — pays out BTC for approved Native→Bitcoin conversions,
  and detects + settles incoming Bitcoin→Native payments.
- **tunneling** — opens the second leg of a Native-to-Native conversion on
  the destination chain.
- **sweeping** — consolidates the BTC hot wallet's derived-address UTXOs
  back to the main wallet.

Each network gets its own `ChainStack` implementation
(`crates/network-vm/evm-revm` for Ethereum/Hedera/Polkadot Hub,
`crates/network-vm/svm` for Solana) behind a shared set of traits defined in
`crates/core`, so the tick logic above is written once and reused across
every chain.

## Current limitations

This is a testnet/devnet deployment with a single operator key per chain —
see [SECURITY.md](../SECURITY.md) for what that means in practice and where
this is headed.
