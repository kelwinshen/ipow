# Contributing to iPoW

Thank you for your interest in contributing.

## Getting started

The repo is a monorepo: Solidity/Anchor contracts under
`programmable-network/*`, and a Rust operator service under `core/operator`.
See the root [README](README.md) for the full layout, and
[docs/README.md](docs/README.md) for the design docs.

```sh
pnpm install                 # installs every EVM package (pnpm workspace)
```

Each `programmable-network/<chain>` package and `core/operator` has its own
README with setup, build, and test commands specific to that package —
start there for the chain/service you're touching.

## Rules

- Branch from `dev` (the active development branch); `master` is the stable
  branch.
- Never force-push a shared branch (`git push --force`/`-f`/
  `--force-with-lease`) once others may have pulled it.
- CI (`.github/workflows/check.yml`) must pass before merging: it runs each
  EVM package's local-network test suite, `core/operator`'s `cargo test`,
  and the Solana programs' `cargo test`. None of these need live RPC URLs
  or private keys — they run against local/simulated networks.
- Never commit `.env` or `config.yml` (both gitignored everywhere they
  exist) — see [SECURITY.md](SECURITY.md) for the current trust model and
  what's expected to stay off-chain vs. secret.

## Pull request process

1. Keep PRs focused on a single concern — a new network deployment, a
   contract change, a doc fix, etc., not several unrelated things at once.
2. Write a clear description: what changed, why, and how you tested it
   (which test suite, and whether you verified anything against a live
   deployment rather than just the receipt — see the "never trust a
   receipt" discipline in `docs/design/ipow-implementation.md`'s Tempo
   section for why that matters on some chains).
3. Run the relevant package's own test suite locally before opening the PR.

## Design and code alignment

`docs/design/ipow.md` and `docs/design/ipow-implementation.md` are the
canonical design record (see `.github/CODEOWNERS` — changes to them
need review same as contract changes). If you find the live code diverging
from what a design doc claims, or the design itself looks wrong, **flag it
explicitly in the PR or an issue rather than quietly editing the doc to
match the code or the code to match an assumption about the doc.** Design
changes and code changes are reviewed as what they actually are.

If you're working with Claude Code (or another AI tool that reads
`AGENTS.md`), it has two project skills and an agent set up for this: the
`design-alignment` skill checks a diff against the three canonical docs and
flags every divergence as `⚠️ DESIGN DEVIATION` with the doc/section and
`file:line`; the `reviewer` agent reviews a diff from a clean context
(useful precisely because it starts with no knowledge of why a change was
made). Ask for either explicitly — `.claude/skills/design-alignment/SKILL.md`,
`.claude/skills/review/SKILL.md`, `.claude/agents/reviewer.md`.

## Code style

- Solidity/TypeScript: `pnpm lint` / `pnpm lint:fix` (prettier) in each
  `programmable-network/<chain>` package.
- Rust: `cargo fmt` and `cargo clippy --workspace` in `core/operator` and
  `programmable-network/solana` before committing.

## Licensing

This repository is licensed under the [MIT License](LICENSE). By submitting
a pull request, you agree to license your contribution under the same
license.

## Security

See [SECURITY.md](SECURITY.md) for the current trust model. If you discover
a vulnerability, please report it privately rather than opening a public
issue — see SECURITY.md for how.
