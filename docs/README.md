# Documentation

```
docs/
├── design/
│   ├── ipow.md                 — canonical: what the system is, why, trust model
│   ├── ipow-implementation.md  — canonical: exact mechanisms, state machines, per-network detail
│   └── ipow-protocol.md        — canonical: the approved protocol design, being built, not live
└── drafts/                     — unratified proposals, superseded designs, and a dated live-run log
```

- **`design/`** is the source of truth — the three docs above. Changes
  require review — see [`.github/CODEOWNERS`](../.github/CODEOWNERS).
  Reason and implement *from* them.
- **`drafts/`** holds two different things, neither authoritative: an
  unratified/WIP proposal not yet promoted into `design/`, and
  superseded/removed designs plus a dated live-run log, kept
  for context on why the current design looks the way it does — see
  [`drafts/README.md`](drafts/README.md).

## Where to start

- **[Root README](../README.md)** — what the system is, package layout.
- **[`design/ipow.md`](design/ipow.md)** — the problem, the two-layer
  architecture (Conversion + Beta), trust model, security model.
- **[`design/ipow-implementation.md`](design/ipow-implementation.md)** —
  exact mechanisms: Conversion's two generations, Beta's statement-bus
  lifecycle, composition, per-network contract variants and why each
  exists.
- **[`design/ipow-protocol.md`](design/ipow-protocol.md)** — the approved
  design of the new protocol (light client, operators, jobs, claims).
  Being built, not live. Its "Build status" says what exists.
- **[`SECURITY.md`](../SECURITY.md)** — current trust model: what's
  trustless today vs. still centralized.
- **[`CLAUDE.md`](../CLAUDE.md)** (repo root) — agent/contributor rules
  and the full source-of-truth map (which file to read for which
  question).
- **[Contributing](../CONTRIBUTING.md)** — setup, PR process, code style.

## Per-network / per-service docs

Each `programmable-network/<chain>` package and `core/operator` has its
own README with setup, build, test, and real deployed-address details
specific to that package. Live addresses are tracked there, not in the
design docs.
