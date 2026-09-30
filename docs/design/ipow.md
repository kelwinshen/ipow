# iPoW: design

What iPoW is, why it's built this way, and what guarantees actually hold.
For exact mechanisms, state machines, and per-network detail, see
[`ipow-implementation.md`](ipow-implementation.md). This doc and that one
are the canonical, review-gated source of truth (see
[`.github/CODEOWNERS`](../../.github/CODEOWNERS)) — reason and implement
*from* them.

## The problem in one paragraph

We want value to move between Bitcoin and any of several independent,
mutually-blind chains (Ethereum, Solana, and others), and between those
chains themselves. No chain can read another chain's state — Solana
cannot check that ETH is locked on Ethereum, and Ethereum cannot check
that a token was burned on Solana. Every "clever flow" that tries to get
around this with round trips, derived addresses, escrows, or timers fails
the same test: the honest run and the cheating run produce byte-identical
inputs on the blind chain, so no rule on that chain can tell them apart.

What every chain in this system *can* read is **Bitcoin** — each one runs
its own PoW-validated header relay and verifies Bitcoin transaction
inclusion natively. That is the one shared ground truth every pair of
chains settles against, instead of each pair needing its own bridge.

## Two independent layers, sharing only the header relay

- **Conversion** (`iPoWConversion.sol` / `ipow-conversion`) moves value
  between a chain and Bitcoin by permissionless staked auction. Trustless:
  the party at risk (the user) sees both sides and commits only after the
  counterparty's value is escrowed; each chain's release is a Bitcoin fact
  it verifies itself. Two Conversion legs on two chains, linked by one
  real Bitcoin payment, give chain-A→chain-B transfers — the base
  primitive everything else builds on.
- **Beta** (`BetaHub.sol`/`BetaVault.sol`/`beta-factory`) mints a token
  backed by real locked value across several networks, using a Bitcoin
  **statement bus**: claims travel across chains as Bitcoin transactions;
  the chain that owns a fact judges it by code; a blind chain acts on the
  claim provisionally, inside a challenge window, and punishes the
  claimant if it turns out false.

Conversion cannot carry truth (it moves money, and Bitcoin only proves
payments, not claims about arbitrary state). The statement bus cannot
move money (it carries claims, not value). Beta needs both, decoupled:
a user moves value onto a chain via Conversion, independently and under
their own control, then locks it via Beta's own entry point
(`BetaVault.deposit()`/`lock_sol`) — two separate steps, never one
CPI/cross-program chain. An earlier attempt at wiring them together via
CPI was built and abandoned — see
[`../drafts/abandoned-cpi-funding-attempt.md`](../drafts/abandoned-cpi-funding-attempt.md)
for why.

Both layers, and the older single-fixed-operator Conversion tunnel fused
directly into `iPoW`/`ipow` that predates the permissionless auction
design, share the same Bitcoin header relay/light client as their one
point of contact — nothing else connects them.

## The one rule Beta's statement bus runs on

> **A claim is judged only by the chain that owns the fact it asserts.**

"1 ETH is locked on Ethereum for lock #N" is judged on Ethereum, which can
read its own vault. "A token was burned on Solana under burn #id" is
judged on Solana. The chain that *cannot* see the fact never tries to
judge it — it acts on the claim provisionally (queues it, inside a
challenge window, vetoable), and relies on the owning chain to slash the
claimant if the claim was false. Because every claim is a Bitcoin
transaction, both chains read the same bytes in the same order, and
neither can be shown something the other wasn't.

## Conversion's security model

Bitcoin's proof-of-work can only prove a fact about *itself* — that a
specific payment happened — never about another chain's state. Every
release of value in Conversion is gated on exactly that one fact, checked
independently by whichever chain is paying out, against real value that
already moved (a deposit or a self-escrow) before any proof is possible.
Nothing here trusts a claim about what happened elsewhere; it only ever
checks Bitcoin proving something about Bitcoin. This makes Conversion
**reject-tier**: a lie simply fails, because the paying chain can verify
Bitcoin directly.

## Beta's trust model: punish-tier, optimistic

Beta cannot be reject-tier the way Conversion is, because a blind chain
has to act on a claim about a fact it cannot see — by the time it could
verify, the provisional action would already have to wait, defeating the
point of a fast path. Instead: a lie *executes* on the blind chain and is
*proven false by code* on the chain that owns the fact, within a
challenge window (currently one week). It is safe if **one honest party
submits the lying anchor within that window** — a permissionless,
bounty-paid action — and even if none does, the damage is bounded by the
liar's own escrow, never open-ended.

What must hold for a Beta unit to stay fully backed:
1. Each participating chain's own consensus (as for anything on it).
2. Bitcoin's proof-of-work (as for Conversion).
3. Bond sizing: `bond ≥ cap × unit ÷ (1 − bounty_bps/10_000)` per chain —
   the bond must cover the worst case *plus* the bounty paid to whoever
   catches it, not just the worst case alone.
4. At least one honest, bonded party alive within the challenge window
   (existential, 1-of-n) — for the release direction's safety and for the
   post-slash pause.

What is *not* assumed: that the operator is honest; that governance
judges anything; that any price feed is correct (the SOL/ETH-style ratio
that appears in bond sizing only ever sizes a bond, never moves funds).

**Honest users are never harmed and never wait on a clock** — a bonded
attester fronts the fast path. A cheating operator is punished by code if
anyone submits its statements to the judging chain within the challenge
window; its damage is bounded by its own escrow even if nobody does. When
bonds are undersized, the bill lands on holders as a group — which is
exactly why the sizing rule above is load-bearing, not a suggestion.

An upgrade path exists to remove even the "one honest checker" assumption
for one direction at a time: replace a blind chain's provisional
acceptance with a zk light client of the chain that owns the fact (a
proof of finality + a storage proof of the specific claim). The lie then
fails on the acting chain directly instead of being punished after the
fact — turning "caught" into "rejected," the same guarantee Conversion
already has. Nothing about the statement/account shapes needs to change
for this; it replaces one instruction's predicate.

## Attack table

| Attack | What happens |
|---|---|
| Operator anchors a MINT-equivalent claim for a lock that doesn't exist | The blind chain mints/acts (bounded by its rate/escrow limits); the owning chain slashes the operator's bond → insurance; operator retired there; an auditor dead-vetoes the blind chain; the fake units redeem from insurance |
| Operator anchors a claim with a wrong amount/deadline/user | Same as above |
| Operator colludes with itself as the user | Identical to the above — self-collusion is just a false claim and is slashed the same way; the user-consent signature gains an operator nothing |
| Operator anchors a claim for a user who never consented | The owning chain refuses and slashes the operator's bond there (false predicate on that chain); if the user had already deposited on the other chain, a CANCEL-equivalent lets them refund, or the slash compensates them directly |
| Operator anchors a release/redeem claim with no real burn/redeem behind it | The blind chain queues it; an auditor vetoes; the owning chain judges: no such burn → operator's bond slashed there; the blind chain cancels |
| Operator anchors a real release claim and no auditor is alive for the whole challenge window | The blind chain pays; the owning chain still slashes the operator's bond once the anchor is processed; loss is bounded by the release cap, covered by the bond — the one path whose *safety* (not just punishment) depends on an auditor being alive, hence the delay and the cap |
| Auditor vetoes a true claim | The owning chain judges it true → the vetoing auditor's bond is slashed there; the action proceeds |
| Auditor falsely claims an operator is dead | The chain that owns "is this operator dead" judges it false → the auditor's bond is slashed; the paused side resumes |
| Operator withholds a statement's preimage after anchoring its hash | Cannot be exercised; anyone may skip it after a timeout; it stays auditable later if the preimage ever surfaces |
| A third party tries to forge an anchor | Cannot spend the party's registered Bitcoin UTXO — no signature scheme to break, just the UTXO set |
| Operator stalls the header relay | Operator-first, permissionless fallback (see `ipow-implementation.md`) — anyone may extend it after 30 minutes |

## Worked economic example (bond sizing)

Test composition: 1 unit = 0.01 SOL + 0.001 ETH. Operator bonds 0.2 SOL on
the Solana side and 0.01 ETH on the Ethereum side. Rate cap 100 units per
6-block window.

- **Honest mint**: user locks 0.01 SOL and 0.001 ETH; 1 unit exists; the
  Solana-side reserve holds 0.01 SOL; the Ethereum-side lock is FINAL.
- **Operator lies about 3 units** (no real locks on the Ethereum side):
  the blind (Solana) side mints 3 units, within its rate cap; the
  Ethereum side's judging finds no matching lock, slashes
  `3 × 0.001 = 0.003 ETH` from the 0.01 ETH bond → 0.0027 ETH to
  insurance + 0.0003 ETH bounty to whoever submitted the proof of the
  lie; operator marked dead there. Check solvency: real locks (0) +
  insurance (0.0027) ≥ claims (3 × 0.001 = 0.003)? **No** — 0.0027 < 0.003,
  because the bounty came out of the same slash as the insurance. This is
  exactly why the sizing rule isn't just "bond ≥ worst case": it has to be
  `bond ≥ cap × unit ÷ (1 − bounty_bps/10_000)` — the bond must cover the
  cap *plus* the bounty. At a 10% bounty and this cap/unit, that works out
  to ≥ 0.111 ETH, not 0.01 ETH — the test deployment's smaller bond only
  safely supports a cap of 9 units, not 100.
- **False release of 1 unit**: the Ethereum side queues a 0.001 ETH
  payout for after the challenge window; the Solana side slashes
  `1 × comp_lamports_per_unit = 0.01 SOL` from the SOL bond to insurance;
  an auditor's veto holds the Ethereum payout until the veto lapses (each
  renewal, under the older timer-based design, cost the auditor a fee on
  the acting chain and earned a reward on the owning chain — see
  `ipow-implementation.md` for the current, non-timer-based version of
  this).

## Trust statement, in one paragraph

Bitcoin's proof-of-work is the one fact every chain in this system can
verify on its own. Conversion never releases value except against a
proof of a Bitcoin payment that already, verifiably, happened. Beta never
finalizes a cross-chain claim except against the same kind of proof,
optimistically accepted first and punished second when it's a blind
chain acting on it — bounded by a real bond, backstopped by anyone being
able to submit the proof of the lie for a bounty. Nothing here trusts a
price feed, a single operator's honesty, or governance's judgment of what
happened. It trusts Bitcoin, each chain's own consensus, and the
economics of the bonds.

## Not built yet

- **Relay reorg handling.** Heights are treated as immutable on both
  relays once committed. A stale sibling block committed at the tip (by
  anyone, once the permissionless fallback opens) leaves the relay stuck
  at that height with no replace path. The 30-minute permissionless-
  fallback delay makes this a race against an absent operator rather than
  a free denial-of-service, and a real orphan landing at exactly that
  height is rare, but it is a genuine liveness hole. Closing it needs
  either a bounded tip-replacement rule that also invalidates proofs
  already cached against the replaced header, or a hash-keyed header DAG
  with best-tip-by-work — a change to the base relay itself, not a
  patch.
- **Batching**: several statements anchored under one Merkle root in a
  single `OP_RETURN`, once anchor volume justifies the complexity.
- **The zk audit path** described above under "Beta's trust model."
- **An auditor run by someone who isn't also the operator**, live. Every
  live run so far has used the same wallet for every role — fine for
  proving the mechanics, meaningless for the "one honest checker" security
  assumption until a checker is actually someone else.
- **RELEASE/redeem-side automation and the auditor/watchtower role in
  `core/operator`.** The operator service automates Beta's claim → relay
  → exercise (mint) path on both EVM and Solana; RELEASE/redeem and the
  VETO/ATTEST/CLEAR/ALIVE auditor actions remain manual on both chains.
