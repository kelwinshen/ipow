# iPoW: implementation

> **Archived.** This document describes the old contract generation (`iPoW`, `iPoWConversion`, `BetaHub`/`BetaVault`, the `ipow`/`ipow-conversion`/`beta-factory` Solana programs and `core/operator`), retired and removed from the repository; its code is in git at the tag `legacy-v1`. It is not authoritative for anything in the repository today. The current design is [`../design/ipow-protocol.md`](../design/ipow-protocol.md). Its Tempo section's account of the `CREATE`-address bug is chain behaviour, not contract design, and still applies (see [`../../networks/tempo/README.md`](../../networks/tempo/README.md)).

Exact mechanisms, state machines, and per-network detail. See
[`ipow.md`](ipow.md) first for what the system is and why. This doc and
that one are the canonical, review-gated source of truth (see
[`.github/CODEOWNERS`](../../.github/CODEOWNERS)).

Live deployed addresses are **not** tracked here — the deployment is the
value; this doc has the rationale. Each network's own README
(`networks/<chain>/README.md`, `evm/README.md`, `solana/README.md`) has its current live
addresses. For a dated record of specific live runs, redeploys, and
incidents, see [`../drafts/live-run-log.md`](../drafts/live-run-log.md).

## Header relay: operator-first, permissionless fallback

Every network keeps its own copy of the Bitcoin header chain on-chain
(`globalHeaders` in `iPoW.sol`, `GlobalHeader` in the `ipow` Anchor
program). For every header pushed in, regardless of who submits it, the
contract independently checks:

- proof-of-work against the claimed difficulty bits;
- inside a difficulty epoch, that the bits match the current tip's; at an
  epoch boundary, the retarget rule against the real prior epoch's
  timespan;
- that a new header extending the tip actually links to it
  (`prev_hash == tip.hash`) — a header can't rewrite history or skip a
  gap.

Because of this, whoever submits headers can choose *when* to relay them,
but never *what* they say.

**Access control**: extending the tip by one header, and the fresh-relay
anchoring/jump-ahead operations, are normally operator-only
(`onlyOperator` on EVM). But the extension path specifically opens to
**anyone** once the tip has sat unextended for `PERMISSIONLESS_HEADER_DELAY`
(30 minutes) — the operator can no longer stall an audit or a proof
indefinitely, only delay it by up to that long. Anchoring a fresh relay
and jumping ahead to a specific height stay operator-only with no
fallback; see [`ipow.md`](ipow.md)'s "not built yet" for the reorg-
handling gap this leaves open.

This is a real behavior change from an earlier version: before it, the
relay validated a header's proof-of-work against its own claimed bits
with no linkage/epoch check at all, so *any* real Bitcoin header from any
height — or one mined at the network's absolute minimum difficulty —
would have passed. Harmless with a trusted operator; fatal for a
permissionless submitter. Both relays (EVM and Solana) now enforce the
full set of checks above for every submitter, operator included.

## Conversion, generation 1: the fixed-operator tunnel

The original design, fused directly into `iPoW.sol`/`ipow`'s own entry
points. Still live today on every network except Tempo (which only has
generation 2 — see below).

### Conversion types

- **Native → Bitcoin** (`commitNativeToBitcoin`): user locks native
  tokens and names a destination Bitcoin script. The operator pays BTC
  out of its own hot wallet, then proves that payment on-chain to release
  the locked native tokens (plus fee) back to itself.
- **Bitcoin → Native** (`commitBitcoinToNative`): user sends BTC to an
  address the operator monitors. Once the operator proves that payment
  on-chain, the contract releases native tokens to the user from its own
  liquidity reserve.
- **Native → Native (tunnel)**: the same `commitBitcoinToNative` entry
  point, called directly by the operator, opens a second leg on a
  destination chain anchored to the same Bitcoin header height as the
  first leg's proof — using Bitcoin's header chain as the shared trust
  root between two non-Bitcoin chains, rather than a separate bridge
  protocol between them. The three conversion types aren't three separate
  features: Native→Native is the other two chained back to back, anchored
  to the same Bitcoin header.

### Approval, duty windows, and permissionless cleanup

After a user commits, the operator has `APPROVAL_WINDOW_SEC` (15 minutes)
to approve the conversion and anchor it to a specific header height.
Approval starts a bounded "duty window" — the operator's deadline to
finish its side (pay out BTC, or relay/confirm enough headers for a
proof). If the operator doesn't show up, the conversion doesn't get
stuck waiting on it: `refundIfNotApproved`, `refundAfterNoProof_
NativeToBitcoin`, and `claimNative_AfterOperatorExpired` are
permissionless — anyone, including the user themselves, can call them
once the relevant deadline has passed, to unwind the conversion.
`timeoutNoDeposit_NativetoBitcoin` and `closeNoBitcoin_BitcoinToNative`
are the operator-side equivalents for when the *user's* side times out
instead.

Once a proof is submitted, it isn't necessarily final immediately —
Bitcoin confirmation and destination-chain block time don't line up, so
proofs are cached (`ProofCache`) and only finalized once enough further
headers (`PROOF_BLOCKS_WINDOW`, 40 blocks) have been relayed on top of
the block the proof references.

### The operator service's role (`core/operator`)

The Rust operator runs a periodic tick per chain it's watching, via
`ChainOperator` (`crates/operator/src/chain_operator.rs`):

- **approving** — approves pending commits and cross-chain tunnel
  requests once duty conditions are met.
- **streaming** — pushes just enough Bitcoin headers to satisfy whatever
  a chain's active conversions currently need.
- **converting** — pays out BTC for approved Native→Bitcoin conversions,
  and detects + settles incoming Bitcoin→Native payments.
- **tunneling** — opens the second leg of a Native-to-Native conversion
  on the destination chain.
- **sweeping** — consolidates the BTC hot wallet's derived-address UTXOs
  back to the main wallet.

Each network gets its own `ChainStack` implementation
(`crates/network-vm/evm-revm` for EVM networks, `crates/network-vm/svm`
for Solana) behind a shared set of traits defined in `crates/core`, so
this tick logic is written once and reused across every chain.

## Conversion, generation 2: the permissionless auction (`iPoWConversion.sol` / `ipow-conversion`)

A separate, standalone contract/program — its own dedicated header-relay
instance on every EVM network (deliberately not sharing Beta's or
generation 1's), needing no fixed operator at all. Two directions,
mirrored on both EVM and Solana with the same field names/semantics:

- **`token→bitcoin`** (`commitTokenToBitcoin`/`commit_token_to_bitcoin`):
  a user locks value (native or any supported token, via `tokenAddr`/
  `token_mint`), naming a Bitcoin receive-script they want paid. Released
  to whoever wins the claim auction and then proves, via a real Bitcoin
  header + Merkle proof, that they paid it. Optionally carries a
  **bundle**: up to 3 extra `{mint, amount}` pairs beyond the primary
  slot, all released or refunded together, settled by the same single
  Bitcoin-amount proof.
- **`bitcoin→token`** (`commitBitcoinToToken`/`commit_bitcoin_to_token`):
  the winning claimant self-escrows the payout amount themselves at claim
  time, released to the committer once *they* prove a real Bitcoin
  payment landed at the claimant's registered receive-script. No bundle
  support on EVM yet (Solana only).

### Auction lifecycle

- **`proposeClaimConversion`/`propose_claim_conversion`** — permissionless,
  competes on **rate**, not stake. The committer's own commit-time value
  is the ask: meeting it exactly wins a first claim; every later
  out-bidding claim must strictly beat the current best. What wins is
  `proposedRateAmount`: for token→bitcoin, how much real BTC the claimant
  will pay the user (higher wins); for bitcoin→token, how much value
  they'll front for the fixed required Bitcoin proof (higher wins). The
  eventual proof-submission step enforces whatever rate won, so a
  claimant can never renege after winning.

  For **bitcoin→token**, the claimant also self-escrows their own
  proposed payout amount right here — there is no governance-funded
  liquidity pool; whoever wins the auction personally fronts the value
  they're promising, symmetric with how token→bitcoin has the *user*
  front real value later (see `depositApprovedConversion` below). This
  only happens pre-finalize; a round reopened after a claimant defaults
  inherits the already-real escrowed amount as-is.

  A fixed anti-griefing bond (`requiredBond`, set by whoever committed)
  is also required. On the current Solana implementation this bond is
  required only for token→bitcoin (that claimant never escrows anything
  on-chain, only a later-proven promise, so the bond is their only real
  skin in the game — bitcoin→token's self-escrow already creates that
  exposure, so a separate bond there would be redundant).
  **The EVM contract has not had this change ported** — `iPoWConversion.sol`
  still requires and posts a stake for both directions.

- **`finalizeClaimConversion`/`finalize_claim_conversion`** — permission-
  less, quiet-period gated (`CLAIM_QUIET_PERIOD_SEC`). Locks in the
  current best claimant, starts the duty window, and snapshots
  `windowStartHeight` from the current Bitcoin header-relay tip — captured
  once, never reset on a later reopen, so a claimant handoff can't push
  out a user's downstream refund/force-claim deadline.

- **`reclaimExpiredConversion`/`reclaim_expired_conversion`** —
  permissionless. If no real commitment exists yet (a defaulting
  token→bitcoin claimant who never deposited, or an equivalent case),
  refunds the claimant's stake and reopens cleanly. If a real commitment
  already exists, the stake is forfeited. On Solana, the forfeited stake
  pays the user directly, immediately, as compensation for the delay. **The
  EVM contract has not had this change ported** — `iPoWConversion.sol`
  still forfeits into a `bounty` field that pays out to whichever claimant
  eventually completes the duty, the same way this design used to work on
  Solana too.

- **`submitBitcoinMerkleProofWithTx`/`submit_proof_cache`** — checks a
  real Bitcoin header (via the header relay) and Merkle proof, checks the
  payment amount/script match what was committed/proposed, and pays out.
  **Restricted**: only `responsibleOperator` (token→bitcoin) or `user`
  (bitcoin→token) may call this — not a permissionless third party, even
  though the proof itself is independently verifiable by anyone.

### Guaranteed resolution

- **`token→bitcoin`**: if the claimant never proves,
  `refundNoProofNativeToBitcoin`/`refund_no_proof_native_to_bitcoin` is
  permissionless and returns the deposit once the proof window passes —
  a **block-height** check against the header relay's tip, independent of
  any wall-clock deadline.
- **`bitcoin→token`**: if the claimant never proves,
  `claimNativeOperatorExpired`/`claim_native_operator_expired`
  ("force-claim") is permissionless and pays the committer the already-
  self-escrowed amount unconditionally. This check is purely
  **time-based** (`operatorDutyExpiresAt` vs. wall-clock), with no
  dependency on any Bitcoin header being relayed.

Every path terminates with real value correctly resolved: a real
proof-gated payout, or a real forfeiture/refund. Conversion never
releases more than what was actually deposited or self-escrowed.

**Two known, open gaps, not yet fixed on either chain**:

1. `reclaimExpiredConversion`/`reclaim_expired_conversion` only handles
   "claimed, then the claimant went dark." There's no permissionless
   fallback for "nobody ever claimed within `CLAIM_WINDOW_SEC` (15 min)
   at all" — a committer's conversion can get stuck in `Committed`
   forever, with no refund path for their commit fee.
2. `submitBitcoinMerkleProofWithTx`/`submit_proof_cache` hard-restricts
   the caller to `user` or `responsibleOperator` — a real person (or a
   contract willing to be driven permissionlessly) must personally act
   within the proof window; there's no "anyone may submit on their
   behalf" fallback even though the proof is independently verifiable.

**A third gap, found during review, not yet in any prior write-up**: the
auction winner picks `dutyWindowSeconds` freely in `proposeClaimConversion`/
`propose_claim_conversion`, with no minimum or maximum enforced anywhere.
For **bitcoin→token specifically**, the only guaranteed-resolution path
(`claimNativeOperatorExpired`) is gated purely on that self-chosen
deadline passing — a claimant who sets an absurdly long window and never
proves locks the conversion (and their own self-escrowed funds)
indefinitely, with no block-height-based alternative the way
token→bitcoin has. Token→bitcoin's own fund-safety exit
(`refundNoProofNativeToBitcoin`) is block-height-gated and unaffected,
but its permissionless-reopen path (`reclaimExpiredConversion`) is still
delayed by the same unbounded field.

### No liquidity pool — self-escrow only

There is no governance-funded pool. `bitcoin→token`'s payout is entirely
backed by whichever claimant wins the auction personally self-escrowing
their own proposed amount — symmetric with how `token→bitcoin` has the
*user* front real value via `depositApprovedConversion`/
`deposit_conversion`. The contract's own aggregate counters
(`totalReservedAmount`/`Pool.total_reserved`, etc.) are bookkeeping only,
never read in a decision anywhere — verified directly against the
Solidity source (every reference is a write, never a conditional read).

### Multi-token

`tokenAddr`/`token_mint` (default = native) selects the asset a given
conversion moves; every value-moving instruction branches once on it.
The auction stake and commit fee are always native regardless of what
the conversion itself moves — a separate pot of money from the escrowed
amount, which follows `tokenAddr`/`token_mint` and can be any supported
token. Deposit-direction transfers measure the actual amount received via
a before/after balance delta rather than trusting the requested amount,
so a fee-on-transfer token cannot silently under-collateralize an
already-fixed promise.

### Tunnels without an auction: `openBundleTunnel`/`open_bundle_tunnel`

A permissionless, atomic "open + fully fund" path for `bitcoin→token`
conversions carrying a multi-token bundle, restricted to cross-network
destinations (`networkId != 0`). Collapses commit + propose + finalize
into one call: the opener self-funds the entire fixed bundle right there,
becomes the responsible party, and the conversion is immediately
approved with its window already started. Deliberately has no auction —
a bundle can't be rate-competed without a price oracle (there's no
natural ordering between "100 USDC + 5 SOL" and "90 USDC + 6 SOL"). This
is purely additive: a direct-Bitcoin, single-token `bitcoin→token`
conversion already has a perfectly good auctioned path
(`commitBitcoinToToken`); this isn't a backdoor around it.

### Cross-chain linking: two Conversion legs, one real Bitcoin payment

The general principle behind every Native-to-Native tunnel, both
generations: two independently-deployed Conversion instances (one per
chain) link with no new trust mechanism beyond Conversion's own —
chain A's `token→bitcoin` leg locks value there, released once someone
proves (via chain A's own header relay) they paid chain A's registered
script; chain B's `bitcoin→token` leg has a claimant self-escrow value,
released once someone proves — via chain B's own header relay,
independently — that the *same* real Bitcoin transaction paid chain B's
registered script. A claimant free to register any script on the
`bitcoin→token` leg can make it match whatever the `token→bitcoin` leg
expects, so one real Bitcoin payment, submitted separately to each
chain's own relay, independently satisfies both. Neither chain reads or
trusts the other's state; each only ever checks a fact about Bitcoin.

**What this does not do**: verify that whoever registered matching
scripts on both legs is honest, or even intends to link them. The
protocol accepts whatever script bytes a caller supplies — getting the
scripts to actually match is the job of whoever orchestrates both legs
(generation 1's operator, or generation 2's `openBundleTunnel`
caller/orchestrating app). Get it wrong and both legs still resolve on
their own (proof or recovery); they just don't link as intended. A
correctness concern for the orchestrator, not a fund-safety risk.

### Cross-network address registry (`addNetwork`)

`iPoWConversion`'s `addNetwork(networkId, minAddrLen, maxAddrLen)` /
`networkConfigs` registry exists solely for `openBundleTunnel`'s
`networkId`/`networkAddress` destination metadata — the base auction
flow (commit/claim/proof) never needs it. Every EVM deployment needs
`addNetwork` called for every other network it should be able to tunnel
to; a network missing from another's registry means `openBundleTunnel`
reverts `IncorrectNetwork` for that destination.

## Beta: statement-bus mechanics

### Statement chains

Every registered party (operator, auditors) has a **statement chain**: a
linear sequence of Bitcoin transactions where each spends output 0 of the
previous one.

```
input[0]  = the party's registered head (only its key can spend it)
output[0] = a small amount back to its own address → the next head
output[1] = OP_RETURN  ver=0x01 | kind | sha256(statement)
```

Properties, with no signature verification anywhere:
- **Unforgeable authorship** — nobody else can spend the party's UTXO.
- **Unhideable** — the chain is public and linear; a party cannot show one
  chain an anchor and withhold it from the other, because *anyone* can
  submit any anchor to either chain for judging (earning a bounty if it
  was a lie).
- **Ordered** — each chain advances a party's pointer only by processing
  anchors in sequence, so "the party's history up to here" is a
  well-defined, identical object on both chains.
- **Cheap** — one dust output plus a fee (~300 sats in the live runs).

A party that anchors a statement's hash and never reveals the preimage
has stalled its own chain; after a timeout, anyone may skip it (the
mechanism keeps skipped anchors auditable later if the preimage
eventually surfaces — a party can't escape by withholding it forever,
since every *exercised* statement has its preimage on-chain by
construction).

### Statement kinds (current: challenge-window/attestation design)

```
MINT    = 0x01 | composition_id u64 | component_index u8 | lock_id u64 | sol_user 32 | nonce u64 | units u64 | deadline i64
RELEASE = 0x02 | lock_id u64 | burn_id u64 | to 20 | units u64
VETO    = 0x03 | target_party_id 32 | target_txid_le 32   (all-zero txid = "operator is dead")
CANCEL  = 0x04 | lock_id u64
ATTEST  = 0x05 | target_txid_le 32     "that MINT/RELEASE is true; I escrow for it"
CLEAR   = 0x06 | target_txid_le 32     "that MINT/RELEASE is true; lift the veto"
ALIVE   = 0x07 | target_party_id 32    "that operator is not dead; un-pause"
```

| statement about | judged on | if false | if true | acting (blind) chain |
|---|---|---|---|---|
| MINT | the chain holding the named lock | operator slashed, **and its attester** | FINAL | mint chain: queue; exercise on ATTEST or at window end |
| RELEASE | the mint chain (`burn` record) | operator slashed, **and its attester** | burn claimed | reserve chain: queue; pay on ATTEST (from escrow) or at window end (from vault) |
| ATTEST of X | wherever X is judged | attester slashed | — | acting chain: reserve escrow from attester's bond; act now |
| VETO of X | wherever X is judged | vetoer slashed | vetoer rewarded (from a governance-funded pool, never from insurance) | acting chain: hold, no expiry |
| CLEAR of X | wherever X is judged | clearer slashed | — | acting chain: lift the hold |
| dead-VETO | the reserve chain's own "is this operator dead" state | vetoer slashed | rewarded | mint chain: operator paused, no expiry |
| ALIVE | the reserve chain's own "is this operator dead" state | claimer slashed | — | mint chain: un-pause |

`MAX(65)` bytes is the largest statement kind (MINT); the format is
compact by design since every byte ships inside a Bitcoin `OP_RETURN`.

### Mint, end to end

1. User locks value on the mint chain (`lock_sol`-equivalent) →
   `pending[user, nonce]`.
2. User deposits the matching value on the reserve chain (`deposit`) →
   a `lock[N]` in `PENDING` state, refundable after its deadline unless
   finalized.
3. User consents (`approve_pending`) once they've seen both of their own
   locks land — the mint chain will not act on a MINT statement whose
   nonce hasn't been consented to.
4. Operator (or any bonded party) anchors `MINT{composition_id,
   component_index, lock_id, user, nonce, units, deadline}` on Bitcoin.
5. **Fast path**: any bonded party anchors `ATTEST` for the same target —
   reserves `units × amount_per_unit` from the attester's own bond as
   escrow, and the mint exercises **immediately**, not after the
   challenge window.
6. **Slow path**: nobody attests — the mint exercises once the challenge
   window (currently one week) passes with no held veto. Free, just slow.
7. **Challenge**: a `VETO` (judged on the reserve chain, against the real
   `lock[N]`) holds the pending mint with no expiry; a `CLEAR` lifts it.
8. **Settle**, after the challenge window: if not held, any escrow
   returns to the attester's bond and the mint is final. If held and
   already exercised, the escrow is forfeited to insurance (it backs the
   unit that should not exist). If held and not yet exercised, the anchor
   is cancelled and the pending slot freed so the user can retry or
   expire out.

On the reserve chain side, a MINT anchor's own processing still finalizes
or slashes at once when submitted — a false MINT also slashes any party
whose ATTEST named it, whether that ATTEST was processed before or after
the MINT itself.

### Redeem, end to end

1. Holder burns on the mint chain (`burn_redeem`) → pays back that
   chain's own local component instantly, records `burn[burn_id]`.
2. Operator anchors `RELEASE{lock_id, burn_id, to, units}` on Bitcoin.
3. Reserve chain processes it: `lock[N]` FINAL and unreleased → queued
   with the same challenge-window mechanics as mint. **Fast path**: an
   `ATTEST` pays the holder immediately, from the attester's own bond —
   the vault itself has paid nothing yet. **Slow path**: after the
   window, the vault pays from its own reserve/insurance.
4. Mint chain processes the same RELEASE anchor: `burn[burn_id]` must
   exist and be unclaimed → mark claimed. If it doesn't exist, that's a
   lie about the mint chain: slash the operator's bond there.
5. **Settle**, after the window: if not held, the vault reimburses
   whichever attester fronted the payout (from the released lock or
   insurance); final. If held, the reimbursement is cancelled — **the
   vault never pays for a false release**; an attester who fronted a lie
   paid it with their own money and is slashed on the mint chain besides.

### Adjudication and slashing, general rules

- Slashed bond on the reserve chain → that chain's own insurance pool.
  Solvency there is `real locks + insurance ≥ outstanding claims`; a
  fake unit's eventual redemption is paid from the slash — same asset as
  the lie, no price conversion needed.
- Slashed bond on the mint chain → that chain's own insurance, used for
  any mint-chain-side compensation and shortfalls.
- A submitter bounty (a fixed fraction of any slash) rewards whoever
  proves a lie — auditing is permissionless and paid, so a watchtower can
  be an unattended bot.
- A "dead" operator's anchors are still processed for audit purposes (to
  catch further lies) but no longer exercised (no new mints, no new
  releases).
- A skipped anchor (preimage withheld past its timeout) remains
  auditable forever: revealing the real statement later still slashes it
  if it was false.

### Composition: multi-network, multi-token

Generalizes a fixed "one asset on the mint chain + one asset on one
reserve chain" pair to an arbitrary, governance-registered basket: any
number of components, each on any reserve chain, any number of
components per chain.

**The one realization that keeps this simple**: a component behaves
exactly like a single reserve-chain leg always did. It has its own lock
on its own chain, its own MINT-equivalent statement, its own
ATTEST/VETO/CLEAR/challenge-window, judged entirely by the chain it lives
on — nothing about the mint/redeem mechanism above changes per
component. The only two genuinely new things are a **composition
registry** (what components exist, and how much of each backs one unit)
and **exercising a mint gates on every component being ready, not just
one**:

```
allowed = NOT held (on every component)
      AND for every registered component i:
            component_final[i]
        AND (attested_by[i] != empty OR now >= challenge_until[i])
```

Each component keeps its own independent escrow, challenge window, and
settlement — a slow component doesn't block a fast one from being
attested; it only blocks the *overall* mint from exercising until it too
is ready. Multi-token-on-one-network falls out for free: two components
with the same network id. Redeem generalizes the same way — one RELEASE
per component, each reserve chain paying its own leg from its own vault,
independently.

A composition needs at least one local (mint-chain) leg; a composition
with **zero remote legs** needs no operator at all — every leg was
already verified directly by the local lock's own transfers, so it can
mint as soon as it's approved, with nothing left to relay across
Bitcoin's statement bus.

The trust statement is unchanged per component — still optimistic, still
bounded by that component's own escrow, still judged where the fact
lives. A basket spanning N chains is N independent instances of the same
mechanism, gated together by the hub. No new trust is introduced by
adding components; the composition registry is the only new governance
surface.

### `BetaHub`: an EVM chain acting as a hub, not just a spoke

Every EVM chain, via `BetaVault.sol`, can act as a **spoke**: it locks
value and judges its own leg's claim, but never mints anything itself.
`BetaHub.sol` lets an EVM chain also act as a **hub**: mint its own
token, backed by a governance-registered composition of legs on that
chain (local, permissionless) and on other EVM chains (remote, judged the
same way a spoke already judges its own leg). Each hub mints its own
independent token (deployed per-pilot as `"iBETA"`) rather than one
global fungible supply — a future bridging layer could unify per-hub
tokens later, but that's not built.

**No new cross-chain trust primitive is introduced.** A remote leg's
finality is never proven cross-chain — each chain independently
re-verifies Bitcoin-inclusion of the same operator-posted, Bitcoin-
anchored statement via its own local header relay, and judges only its
own local bookkeeping. Trust is bonding + a challenge window, not a
cross-chain message. This is why an existing `BetaVault` deployment works
unmodified as a remote leg for a `BetaHub` on a different chain — the
opaque 32-byte user identifier in a lock is never compared except
byte-for-byte, so it doesn't matter which chain originated it.

Solana as a remote leg is explicitly out of scope (no Solana spoke
program exists that mirrors `BetaVault.sol`'s judging logic); a
`BetaHub`'s composition registry fails closed on Solana's network id.

## Per-network contract variants, and exactly why each exists

Every EVM network runs the same Solidity source (`iPoW.sol`,
`iPoWTypes.sol`, `BitcoinPrimitives.sol`, `iPoWConversion.sol`,
`BetaHub.sol`, `BetaVault.sol`) except two, each forced by a real,
confirmed chain-level constraint — not a design preference:

### Hyperliquid (HyperEVM): router + facets

HyperEVM testnet's block gas limit (3,000,000) is well under what the
plain contracts cost to deploy — e.g. plain `BetaHub`'s deployed
bytecode alone needs roughly 3.82M gas of code-deposit cost, confirmed
directly against the RPC's `eth_estimateGas`. This is a per-transaction
ceiling, not a balance problem — funding more doesn't help, and
optimizer tuning only shrinks bytecode marginally. The fix, applied
identically to `iPoW`, `BetaVault`, and `BetaHub` in turn: a
Diamond-style (EIP-2535-inspired) router+facets split.

- A shared storage base contract that every facet and the router inherit
  identically, so `delegatecall`-executed facets read/write the router's
  own storage correctly. Anything that was `immutable` in the monolithic
  contract has to become regular storage — immutables are baked into a
  contract's own bytecode at construction and don't propagate through
  `delegatecall`, so each facet would otherwise read a meaningless zero.
- Facets split along genuinely non-overlapping helper-function dependency
  graphs (verified by listing every function's internal-helper call graph
  before splitting), not an arbitrary line count.
- **The router is deliberately immutable**: every facet address is fixed
  once in the constructor, with no admin function to swap one later. This
  closes off the standard security criticism of general-purpose (mutable)
  Diamond patterns — a compromised or malicious upgrade swapping in bad
  logic — at the cost of never being able to fix a facet bug in place. A
  deliberate, accepted tradeoff, not an oversight.
- Dispatch is explicit `bytes4` selector comparison, not a
  mapping/storage-backed table — cheaper, and every dispatch entry is
  individually visible in the router's source.

**Verification discipline**: each split's full existing test suite (the
one written against the plain, monolithic contract) is run unchanged
against the router's address instead, using the monolithic contract's
own ABI attached to the router (the standard proxy/diamond testing
technique). All pre-existing tests pass identically in both plain and
split modes; a handful of new tests target only the genuinely new
dispatch logic itself.

Every other network keeps the plain, unmodified contracts — this split
exists only in `networks/hyperliquid/`.

### Tempo (Moderato): PathUSD-denominated variants

Tempo rejects any transaction carrying native value outright, at the
chain level — a real, confirmed constraint (`"Revm error: value transfer
in Tempo Transaction not allowed"`), not a gas issue. Every real value
movement on Tempo goes through its actual fee/settlement ERC20,
**PathUSD** (6 decimals), instead.

- `BetaVaultPathUSD.sol` and `BetaHubPathUSD.sol` convert every bond/fee/
  payout that would otherwise use `msg.value` to move PathUSD instead —
  the judging/verification logic itself is unchanged from the reference
  contracts (native-value code paths are left present but permanently
  unreachable, a dead path rather than a live footgun). `BetaHubPathUSD`
  achieves this via inheritance (`HubPartyRegistry._pay` marked
  `virtual`, overridden to redirect through one PathUSD choke point)
  rather than full duplication, since `BetaHub` was already factored into
  reusable base contracts; `BetaVaultPathUSD` had to duplicate
  `BetaVault.sol` wholesale, since that contract predates the base-
  contract split.
- `iPoWConversionPathUSD.sol` is narrower: `iPoWConversion` already
  has a fully flexible token choice for the *conversion value itself*
  (native is simply a provably-dead branch here, same precedent) — only
  the **commit fee**, the **auction stake**, and the **bounty** are
  hard-coded to native value regardless of the chosen token, so those
  move to PathUSD. `proposeClaimConversion` gains an explicit
  `stakeAmount` parameter, since it can no longer be inferred from
  `msg.value` (which now only ever covers the still-native
  bitcoin→native self-escrow case).

**A confirmed, unfixed chain-level bug, unrelated to the above**:
contract-creation addresses on Tempo ignore the sender's nonce *lane*
entirely, deriving only from the raw nonce number — proven with a
completely fresh, never-used lane that still reported the address (and
bytecode) of an unrelated *earlier* contract created by the same sender
in a different lane. A deploy transaction's own receipt can report
`status: success` with the wrong `contractAddress`, or the address of a
contract that was never actually created. **Rule: never trust a Tempo
deploy or write receipt.** Always independently derive candidate `CREATE`
addresses for nearby raw nonces and confirm by reading real contract
state (not just a bytecode-length match) before treating any Tempo
deployment as real. This is a genuine Tempo chain-level issue, not
something this repo controls, and remains unfixed on Tempo's side.

Measured further on 2026-10-02 and 2026-10-03, for the new protocol's
deployment: the deployer's ordinary nonce was 12 and exactly the `CREATE`
addresses of raw nonces 0 to 11 held code, though its nonce lane 1, which
the earlier deploys used, had reached 27. So a contract appears to land at
`CREATE(sender, ordinary nonce)` whatever lane sent it, and a deploy in
another lane can collide with an address already used. The new protocol was
then deployed in the ordinary lane (nonce key 0), nine transactions from
nonce 18, and every contract landed at the address its ordinary nonce
gives, each confirmed by its code and state
(`networks/tempo/scripts/deploy-new-protocol.ts`). The rule
above still holds; in the ordinary lane the receipt and the nonce agreed.

Plain Hardhat/ethers can't speak Tempo's fee-sponsored, two-dimensional-
nonce transaction model at all; every real deploy/write against Tempo
goes through `viem`'s native Tempo support instead.

## Not built yet (implementation-specific)

- **Redeem-side generalization for compositions** — RELEASE/CANCEL
  statements already carry the field additions composition needs, but
  the actual per-component redeem flow is design-only; only mint-side
  composition is built and tested.
- **ERC20 support on more than one reserve chain's vault, and Token-2022
  mint support on the mint chain** — only classic SPL Token local legs
  and the pilot EVM network's ERC20 support are built.
- **Solana as a `BetaHub` remote leg** — no Solana spoke program exists;
  explicitly out of scope for now, enforced by a registration-time guard.
- **No EVM equivalent of Solana's `audit_skipped_release`** — confirmed
  by grepping `BetaVault.sol`/`BetaHub.sol` (no such function anywhere).
  Nothing on the EVM side lets a skipped anchor be revisited if its
  preimage later surfaces; Solana has this, EVM doesn't. A real,
  currently-open gap, not just a historical note.
- **`core/operator` automation for RELEASE/redeem and the auditor/
  watchtower role (VETO/ATTEST/CLEAR/ALIVE)** — the operator service
  automates claim → relay → exercise (mint) on both EVM and Solana;
  everything else on this list remains a manual, script-driven action.
- **Runtime parameter values for a real deployment** (challenge windows,
  bonds, fees, rate/exposure caps) and the zk audit path from
  [`ipow.md`](ipow.md) — both open design questions, not just missing
  code.
