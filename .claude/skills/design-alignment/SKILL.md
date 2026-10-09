---
name: design-alignment
description: Verify changes stay aligned with the canonical design (docs/design/ipow-protocol.md and the specs in docs/specs/); any deviation from the documented design must be explicitly flagged
---

`docs/design/ipow-protocol.md` is the **canonical design** of this project,
and `docs/specs/` holds the specs of the applications and features built on
it (see [`.github/CODEOWNERS`](../../../.github/CODEOWNERS)). They must
always be true to what is in the code: code must conform to the design, and
the design must accurately describe actual system behavior. Neither side
drifts from the other silently — the design is never changed implicitly by
code, and the code is never changed on an assumption about what the design
"must" mean.

## What to do

Given a set of changes (local diff against `dev` unless stated otherwise,
or a PR number via `gh pr view`/`gh pr diff`):

1. Identify which design areas the change touches:
   - **`docs/design/ipow-protocol.md`** — the protocol: the light client
     (section 2), operators (3), jobs, the auction and the duty (4), fees
     (5), punishment and challenges (6), claims and attest (7), the vault
     and its receipts (11), and the applications on top (10). Its rules are
     numbered decisions (D1, D2, ...); cite the decision. Its "Build status"
     section says what is built, tested and deployed, and the differences
     each network makes. It is the spec for `evm/contracts/protocol`,
     `solana/programs/protocol` and the node's roles (`node/`).
   - **`docs/specs/`** — the built applications and features:
     Conversion (`ipow-conversion-app.md`) and its tunnel
     (`ipow-conversion-tunnel.md`), BETA (`ipow-beta-app.md`), the vault's
     discussion (`ipow-vault-claims.md`; section 11 of the design is
     authoritative), vault genesis (`ipow-vault-genesis.md`), the other
     networks (`ipow-stage7-networks.md`) and the SDK (`ipow-sdk.md`). The
     code cites the spec it implements in its header comments.
   Check each doc's own headings — sections may have been added since this
   skill was written. Live deployed addresses are **not** in the design or
   specs — each network's own README has those; a change that only
   updates a live address doesn't need this skill.

2. Optionally consult `docs/drafts/` for background: the build plan and the
   dated live-run records. `docs/archive/` describes the generation retired
   at the tag `legacy-v1`. Never treat either as authoritative.

3. Read the relevant sections and compare against the changed behavior:
   the life of a job (open → auction → anchor → tagged transaction → proof
   → lock → settle, or slash), of a claim (open → objections → decided),
   of a vault lock or burn (slow path and fast path); access control (no
   owner and no key, D59; anyone may operate, D12, and register an
   application, D63); bond, escrow, fee, window and size parameters; the
   network numbers and address formats of a vault pair; and the
   differences between the EVM and Solana builds.

4. Check consistency in both directions — code that contradicts the
   design, and design text the change makes false (including a Build
   status row the change makes out of date). Classify every divergence:
   - **⚠️ DESIGN DEVIATION** — the change makes the code behave
     differently from the documented design. Cite the design doc +
     section (and decision) it contradicts and the code location
     (`file:line`). Never let it pass silently; never treat the code as
     the new source of truth by default.
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
