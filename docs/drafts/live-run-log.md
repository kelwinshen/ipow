# Live-run log

**Status: historical record, not spec.** A dated account of specific
deploys, live transactions, incidents, and bugs found while building and
running this system for real. Not authoritative for current behavior —
see [`../design/ipow.md`](../design/ipow.md) and
[`../design/ipow-implementation.md`](../design/ipow-implementation.md)
for that. Kept because the *lessons* here (a caught bug, a chain-level
quirk, a timing constant raised after a near-miss) explain why the
current design has specific numbers and rules, even after the narrative
detail itself stops mattering day to day.

## 2026-09-21: first BETA statement-bus runs (v2, timer-based)

Solana `beta-factory` and Ethereum `BetaVault.sol` deployed for the first
time (not the current design — the original rate-cap/timer-window
version, superseded the next day by the attestation/challenge-window
design below). Test composition: 1 unit = 0.01 SOL + 0.001 ETH.

Live record (Bitcoin mainnet):

| Block | Statement | Result |
|---|---|---|
| 967886 | MINT lock 1 | Minted 1 BETA; lock 1 FINAL |
| 967894 | RELEASE lock 1 | Burn claimed; paid 0.001 ETH after 10 min |
| 967913 | MINT lock 2 | Minted; FINAL |
| 967913 | MINT lock 99 (a deliberate lie) | Minted on the blind chain anyway; **slashed 0.001 ETH** on Ethereum; operator retired |
| 967913 | auditor dead-veto | Operator paused on Solana; judged true → auditor rewarded |
| 967913 | RELEASE burn 7 (a deliberate lie) | **Slashed 0.01 SOL** on Solana; operator retired; Ethereum's queued payout refused |
| 967913 | auditor VETO on the lie | Judged true → rewarded; payout held |

Total Bitcoin spent across every live run in this whole log: under 6,000
sats, including 1,244 sats lost to a daemon bug (below).

**Stranded queued mint, found by asking "does it always end normally for
the user?"**: a MINT judged true and queued on Solana whose operator was
then retired could neither exercise (dead party) nor expire (queued) nor
be re-anchored (queued ⇒ predicate false). Fixed same day: a queued
slot now records which party queued it; a live operator's MINT for the
same slot can pass the retired party to re-queue it under the new
anchor; an `expire_pending` path cancels a queued slot once its queuing
party is dead.

**Lesson learned, became a rule**: veto rewards on both chains now come
from a governance-funded pool, never from insurance — otherwise the
slash wouldn't fully back the units the lie created.

## 2026-09-22: v3 (attestation/challenge-window) replaces v2's timers

Solana `beta-factory` upgraded in place (slot 502011808); Ethereum fresh
`BetaVault` v3 deployed (Sepolia can't upgrade in place). This is the
design documented as current in `ipow.md`/`ipow-implementation.md`.

**First live v3 mint (block 968030)**: operator anchored MINT and, right
behind it on its own chain, self-ATTEST (both fee-bumped after ~2 hours
unconfirmed — the wallet couldn't initially fund the "fastest" fee rate
for both). Relays needed to jump to 968030, since Sepolia required the
epoch-start header recorded first. Devnet: MINT queued → ATTEST reserved
escrow from the operator's bond → exercised **immediately**. Sepolia:
MINT processed 38 minutes after its block → lock FINAL.

**Lessons that became rules**: (1) Esplora's `/tx/:id/status` endpoint
returns `{"confirmed":false}` for *unknown* txids too, not just
unconfirmed ones — use `/tx/:id` (404) or `/outspend` to distinguish "in
mempool" from "never broadcast." (2) The MINT-processing window (`tFin`)
was 1 hour, which is tight if an anchor confirms during an explorer
outage — raised to 4 hours. (3) Header relay must be able to jump, not
just extend one at a time, when far behind, and the EVM side needs its
epoch-start header pre-relayed before a jump — both built the same day.

**First daemon-driven mint, and first independent attester (blocks
968040/968046)**: the operator's own daemon anchored a MINT by itself;
its self-ATTEST couldn't be funded, so the mint waited. An independent
auditor's loop saw "verified on Ethereum, unattested" and anchored its
own ATTEST — then, every cycle until that anchor confirmed, anchored
three more *identical* ATTESTs (1,244 sats wasted, since a statement
whose target was already attested was still being re-anchored). **Fixed
same day**: the daemon keeps a statement-hash ledger and never re-anchors
the same statement twice; the on-chain program treats a redundant ATTEST
as a harmless no-op instead of erroring. With those in place, the mint
succeeded on the independent attester's own escrow. Spend caps (max fee,
max anchors/hour, max sats/hour) were added to the daemon after this
incident.

**First slow-path redeem (block 968053)**: a RELEASE anchored
deliberately unattested, to exercise the window-close path for real —
paid from the vault after the challenge window with no attester
involved. First live use of the relay's jump path for headers, and the
first real test of settlement with nothing fast-tracked.

**Acceleration fee added (MINT side only)**: an ATTEST buys the user
*speed*, not correctness — the operator's bond already covers correctness
on every claim, attested or not. Speed gets its own, separate incentive:
the user who wants speed pays for it directly (an optional `attest_fee`
on `lock_sol`, paid to whoever accelerates them immediately, refunded if
nobody does). Deliberately scoped to MINT only; the same hook on RELEASE
was identified as a natural follow-up, not built same-day.

**EVM/SVM parity check, done by reading both, not assuming**: found and
fixed a real bug where a second attester of an already-attested MINT
silently overwrote the first in storage — only the *last* attester would
have borne a fan-out slash if the mint proved false, letting the real
first attester walk away. Fixed to lock in only the first attester,
matching the Solana side's existing semantics. Also confirmed one
genuine, still-open gap: Ethereum has no equivalent of Solana's
`audit_skipped_release` — nothing lets a skipped anchor be revisited if
its preimage later surfaces, on the Ethereum side specifically.

## 2026-09-23: composition (multi-network, multi-token)

Solana hub generalized to support an arbitrary composition of components
across networks and tokens (built and tested incrementally over the
day — a flat per-composition budget of 8 components, up to 4 of them
local, replaced an earlier, more rigid 4-networks-×-4-tokens grid after
review found the grid and the mint-time account limit had silently
drifted apart).

**Tempo and Hyperliquid deploy blockers, found and resolved the same
day**: Tempo's contract-creation addresses turned out to ignore the
sender's nonce lane (a confirmed chain bug — see
`ipow-implementation.md`'s Tempo section for the rule this produced:
never trust a Tempo deploy receipt). Hyperliquid's HyperEVM testnet block
gas limit turned out to be too small for the plain contracts to even
deploy — resolved with the router+facets split, applied in turn to
`iPoW`, `BetaVault`, and `BetaHub` as each was rolled out to that
network.

**`BetaHub` built and deployed to every EVM network** — an EVM chain
acting as a mint hub for the first time, not just a spoke. Two real bugs
caught during implementation, before deployment: (1) an early draft let
a caller supply the anchor-skip timeout directly as an argument, meaning
anyone could pass zero and skip instantly — fixed by moving it into
governance-set parameters like every other timing constant. (2) The
MINT-targeting ATTEST/CLEAR branch was checked against the wrong
assumption about fast-payout behavior — corrected after re-reading the
reference contract's actual behavior rather than assuming symmetry with
its RELEASE branch.

## 2026-09-23: first real multi-network BETA mints, using genuine Bitcoin

**First 5-network mint**: Solana's `beta-factory` as hub, with legs on
Solana (local) plus Ethereum, Base, Robinhood Chain, and Hyperliquid
(remote) — every anchor a real, mined Bitcoin mainnet transaction. A real
operational lesson caught before any harm: statements were originally
chained into one linear sequence for speed, which works for the hub
(which processes every link in order) but not for each spoke's own
`Party.anchorTxidLE` pointer, which can only advance one link at a time
and has no way to skip links meant for other chains — the first attempt
had three of four remote legs revert with a "not on this chain's
statement chain" error. A revert here is atomic (no partial state, no
funds moved) and was caught immediately; fixed by registering a second
party per affected chain, pointed at the correct intermediate outpoint.
Result: 1 BETA minted, backed by 1 real locked local unit plus 4 real
finalized remote locks, independently verified against each chain's own
state.

**First EVM-hub, Tempo-spoke mint**: Ethereum's `BetaHub` as hub, Tempo's
`BetaVault` as the sole remote leg. Surfaced the confirmed, previously-
undocumented fact that Tempo rejects any transaction carrying native
value outright — not fixable by changing how a script calls the existing
contract; required the PathUSD-denominated `BetaVaultPathUSD.sol`
variant documented in `ipow-implementation.md`. Because the MINT anchor's
operator had already self-attested, the mint exercised immediately rather
than waiting for the full challenge window.

## Housekeeping

**2026-09-24**: a stale, expired, never-claimed test `Pending` account on
Solana devnet (from an early composition-support validation run) was
cleared via a real, permissionless `expire_pending` call, refunding its
locked SOL. `beta-factory` devnet had zero live `Pending` accounts
afterward — a clean slate, not evidence anything was ever broken.
