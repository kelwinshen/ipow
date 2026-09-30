# iPoW

Rules for agents (and anyone else's AI) working in this repo.

**This file contains rules and pointers only — never facts about the
system.** Facts (contract addresses, mechanisms, parameters, flows) live in
`docs/` and the code itself, per the source-of-truth map below. Do not
restate them here: this file is loaded into every session, so a design
fact copied here becomes an unreviewed, drift-prone shadow spec the moment
the real thing changes. Link, don't copy.

## Design & spec discipline (read this first)

- [`docs/design/ipow.md`](docs/design/ipow.md) and
  [`docs/design/ipow-implementation.md`](docs/design/ipow-implementation.md)
  are the **canonical design docs** (see [`.github/CODEOWNERS`](.github/CODEOWNERS)
  — changes to them are reviewed the same as contract changes). Reason and
  implement *from* them; treat them as the spec.
- [`docs/design/ipow-protocol.md`](docs/design/ipow-protocol.md) is the
  **canonical design of the new protocol**, approved and being built. It
  is the spec for the new contracts. It does not describe what is live;
  its "Build status" section says what exists.
- **Before changing contract, program, or operator behavior, read the
  relevant section of those docs first** — don't rely on a summary of them
  found elsewhere in the repo or in a prior conversation.
- **Validate code against the design. On any divergence — the code doesn't
  match what a doc claims, or the design itself looks wrong or unsafe —
  stop and flag it to the user.** Never quietly edit the design doc to
  match the code, and never quietly change the code because you assumed
  what the design "must" mean. State what you actually found; let the user
  decide which one is wrong.
- For non-trivial changes, run the `design-alignment` skill before
  reporting the change as done.
- Before reporting a change as done, run the `reviewer` agent on the local
  diff and relay its findings — fix the ones the user wants fixed. Design
  deviations it surfaces are flagged per the rule above, not silently
  fixed in either direction.
- Real deployed addresses are **not** in the design docs — each network's
  own README (`programmable-network/<chain>/README.md`) has its current
  live addresses. Don't add an address table to a design doc; that's
  deployment data, not design.
- `docs/drafts/` (see [`docs/README.md`](docs/README.md)) is where an
  unratified, work-in-progress design proposal goes before it's promoted
  into a canonical doc, folded into an existing one, or dropped.
  [`docs/drafts/README.md`](docs/drafts/README.md) lists what is there.
  If you write a design doc for something that isn't built and verified
  yet, it goes in `docs/drafts/`, not into a canonical doc as if it were
  already true.

## Source-of-truth map

| To know about… | Read… |
|---|---|
| What the system is, the problem, the two-layer architecture, trust model | root [`README.md`](README.md), then [`docs/design/ipow.md`](docs/design/ipow.md) |
| Exact mechanisms: the header relay, both Conversion generations, Beta's statement-bus lifecycle, composition, per-network contract variants and why | [`docs/design/ipow-implementation.md`](docs/design/ipow-implementation.md) |
| The new protocol being built (light client, operators, jobs, claims), its decisions, and what is built so far | [`docs/design/ipow-protocol.md`](docs/design/ipow-protocol.md); build order in [`docs/drafts/ipow-build-plan.md`](docs/drafts/ipow-build-plan.md) |
| Superseded/removed designs, and a dated log of specific live runs/incidents | [`docs/drafts/`](docs/drafts/) — **not authoritative** for current behavior |
| Real deployed addresses, per network | each network's own README, e.g. [`programmable-network/ethereum/README.md`](programmable-network/ethereum/README.md) — **not** the design docs |
| Exact contract behavior (bond amounts, timeouts, auction mechanics) | the Solidity source itself — `programmable-network/<chain>/contracts/*.sol`; every EVM network runs the same source except Hyperliquid (router+facets split) and Tempo (PathUSD variants) — see each package's own README for why |
| Solana program behavior | `programmable-network/solana/programs/{ipow,ipow-conversion,beta-factory}/src/`; see [`programmable-network/solana/README.md`](programmable-network/solana/README.md) |
| The operator service's tick logic (approving, converting, streaming, tunneling, Beta claim/relay/exercise) | `core/operator/crates/operator/src/{chain_operator,beta_operator,beta_registry}.rs`; [`core/operator/README.md`](core/operator/README.md) for the CLI entry points |
| Current trust model / what's centralized today | [`SECURITY.md`](SECURITY.md) — kept as a direct statement of present state, not an aspirational one |

## Agent rules

**Git commit rules:**
- Do not add `Co-Authored-By` lines to commits (the user has explicitly
  asked for these to be left out).
- Never force-push (`git push --force`/`-f`/`--force-with-lease`) once a
  branch may have been pulled by someone else, without the user's explicit
  go-ahead for that specific push.
- Only commit or push when the user asks for it in the current message —
  don't commit proactively as a side effect of some other task.

**Work flow rules:**
- Never commit `.env` or `config.yml` — verify with `git check-ignore -v`
  before staging anything new that touches a package with real secrets in
  it, not just by assuming the existing `.gitignore` covers a new file.
- Before reporting deployment/config work as done, verify live state by
  reading it back on-chain — don't trust a transaction receipt alone (see
  the Tempo CREATE-address bug documented in
  `docs/design/ipow-implementation.md`'s Tempo section for why this
  matters concretely, not just as caution).

## Documentation

- Keep docs accurate to current code, not aspirational. If a doc describes
  something as done, it should be independently verifiable (a live
  address, a passing test, a real transaction) — not just described.
- Plain, direct language. Say what changed and why; skip filler.
