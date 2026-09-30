---
name: design-alignment
description: Verify changes stay aligned with the canonical design docs (docs/design/ipow.md, docs/design/ipow-implementation.md, and docs/design/ipow-protocol.md for the new protocol contracts); any deviation from the documented design must be explicitly flagged
---

`docs/design/ipow.md` and `docs/design/ipow-implementation.md` are the
**canonical design** of this project (see
[`.github/CODEOWNERS`](../../../.github/CODEOWNERS)). They must always be
true to what is in the code: code must conform to the design, and the
design docs must accurately describe actual system behavior. Neither side
drifts from the other silently — the design is never changed implicitly by
code, and the code is never changed on an assumption about what the design
"must" mean.

## What to do

Given a set of changes (local diff against `dev` unless stated otherwise,
or a PR number via `gh pr view`/`gh pr diff`):

1. Identify which design areas the change touches:
   - **`docs/design/ipow.md`** — the problem, the two-layer architecture
     (Conversion + Beta) and why they're decoupled, the core adjudication
     principle, Conversion's and Beta's trust models, the attack table,
     bond-sizing economics, and what's explicitly not built yet.
   - **`docs/design/ipow-implementation.md`** — exact mechanisms: the
     header relay's operator-first/permissionless-fallback access
     control, Conversion's two generations (the fixed-operator tunnel and
     the permissionless auction) and their known open gaps, Beta's
     statement-bus lifecycle (mint/redeem, statement kinds, composition),
     the `addNetwork` cross-network registry, and exactly why each
     per-network contract variant (Hyperliquid's router+facets, Tempo's
     PathUSD) exists.
   - **`docs/design/ipow-protocol.md`** — the approved design of the new
     protocol, being built: the light client, operators, jobs, fees,
     punishment, claims and attest. It is the spec for the new contracts
     (`contracts/protocol/` and what follows them), and for nothing that
     is live. Its rules are numbered decisions (D1, D2, ...); cite the
     decision. Its "Build status" section says what exists. A change to
     the old contracts is checked against the two documents above, a
     change to the new contracts against this one.
   Check each doc's own headings — sections may have been added since this
   skill was written. Live deployed addresses are **not** in either
   canonical doc — each network's own README has those; a change that
   only updates a live address doesn't need this skill.

2. Optionally consult `docs/drafts/` for background — an unratified,
   work-in-progress proposal (context, not spec), a superseded/removed
   design, or the dated live-run log.
   Never treat anything there as authoritative.

3. Read the relevant sections and compare against the changed behavior:
   state machines (commit → approve/claim → proof → payout/exercise),
   access control (`onlyOperator`, fully permissionless, or
   operator-first-with-permissionless-fallback — ipow deliberately uses
   all three in different places, so the *specific* one matters), bond/
   fee/timeout parameters, cross-network address-format assumptions, and
   which light-client instance (`iPoW`/`ipow`) a contract or program
   actually points at.

4. Check consistency in both directions — code that contradicts the
   design, and design text the change makes false. Classify every
   divergence:
   - **⚠️ DESIGN DEVIATION** — the change makes the code behave
     differently from the documented design. Cite the design doc +
     section it contradicts and the code location (`file:line`). Never
     let it pass silently; never treat the code as the new source of
     truth by default.
   - **Aligned** — implements or refines what the design already
     specifies.

5. A deviation is not automatically wrong, but it requires an explicit,
   deliberate decision from the user: revert the code to match the
   documented design, or update the design doc in the same change with the
   rationale for the new behavior.

## Output

- A short verdict: aligned, or N deviations flagged.
- Each deviation as a `⚠️ DESIGN DEVIATION` item: what the design says (doc
  + section), what the code now does (`file:line`), and the required
  action (conform, or update the design doc with rationale).
