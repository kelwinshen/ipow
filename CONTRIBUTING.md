# Contributing to iPoW

Thank you for your interest in contributing.

## Getting started

The repo is a monorepo: the Solidity contracts for every EVM network in
`evm/`, the Anchor programs in `solana/`, the Rust service that runs the
protocol's roles in `node/`, the TypeScript SDK in `sdk/`, and one folder
per other EVM network in `networks/`. See the root [README](README.md) for
the layout and the build and test commands, and
[docs/README.md](docs/README.md) for the design docs.

Requirements: Node 22.18 or later (the TypeScript scripts run with its
type stripping), pnpm 11, Rust and the Solana CLI; the root README's
Quick start has the details.

```sh
pnpm install                 # the pnpm workspace: evm, solana, sdk, networks/*
```

`evm`, `solana`, `node` and `sdk` each have a README with setup, build,
test and deployment commands specific to that package; start there for
the one you're touching.

## Rules

- Branch from `dev` (the active development branch); `master` is the stable
  branch.
- Never force-push a shared branch (`git push --force`/`-f`/
  `--force-with-lease`) once others may have pulled it.
- CI (`.github/workflows/check.yml`) must pass before merging: it runs
  the EVM contracts' Hardhat tests, builds the Solana programs and runs
  their `cargo test`, and builds and tests `node` and `sdk` against those
  builds. None of these need live RPC URLs or private keys; they run
  against local networks.
- Never edit the source of a deployed contract or program — not even a
  comment, and not by moving the file — without first recording a source
  snapshot of what is deployed. A build embeds its source paths and a hash
  of its source files, comments included, so any such change makes other
  bytes. On EVM, `evm/scripts/verify-deployments.ts` compares each
  deployment with today's build unless its record names a source
  (`source`/`sources`) kept in `evm/deployments/source-<commit>/` with that
  commit's artifacts; without the snapshot it fails. On Solana,
  `solana/deployments/devnet.json` records each program's commit, length
  and sha256, which `solana/scripts/verify-devnet.sh` compares devnet with;
  keep that record's commit the one the deployed bytes were built from.
  For the same reason, comments in deployed sources, and the generated
  IDLs, may cite `docs/drafts/` paths on purpose: those were the docs'
  paths when the code was deployed (most are now in `docs/specs/`).
- Never commit `.env` or `config.yml` (both gitignored everywhere they
  exist) — see [SECURITY.md](SECURITY.md) for the current trust model and
  what's expected to stay off-chain vs. secret.

## Pull request process

1. Keep PRs focused on a single concern — a new network deployment, a
   contract change, a doc fix, etc., not several unrelated things at once.
2. Write a clear description: what changed, why, and how you tested it
   (which test suite, and whether you verified anything against a live
   deployment rather than just the receipt — see the "never trust a
   receipt" rule in [`networks/tempo/README.md`](networks/tempo/README.md)
   for why that matters on some chains).
3. Run the relevant package's own test suite locally before opening the PR.

## Design and code alignment

`docs/design/ipow-protocol.md` is the canonical design, and `docs/specs/`
holds the specs of the built applications and features (see
`.github/CODEOWNERS`: changes to them need review, the same as contract
changes). `docs/archive/` describes the retired generation and is not
authoritative. If you find the live code diverging
from what a design doc claims, or the design itself looks wrong, **flag it
explicitly in the PR or an issue rather than quietly editing the doc to
match the code or the code to match an assumption about the doc.** Design
changes and code changes are reviewed as what they actually are.

If you're working with Claude Code (or another AI tool that reads
`AGENTS.md`), it has two project skills and an agent set up for this: the
`design-alignment` skill checks a diff against the design doc and specs and
flags every divergence as `⚠️ DESIGN DEVIATION` with the doc/section and
`file:line`; the `reviewer` agent reviews a diff from a clean context
(useful precisely because it starts with no knowledge of why a change was
made). Ask for either explicitly — `.claude/skills/design-alignment/SKILL.md`,
`.claude/skills/review/SKILL.md`, `.claude/agents/reviewer.md`.

## Code style

- Solidity/TypeScript: `pnpm lint` / `pnpm lint:fix` (prettier) in the
  `evm` and `solana` packages.
- Rust: `cargo fmt` and `cargo clippy --workspace` in `node` and `solana`
  before committing.

## Licensing

This repository is licensed under the [MIT License](LICENSE). By submitting
a pull request, you agree to license your contribution under the same
license.

## Security

See [SECURITY.md](SECURITY.md) for the current trust model. If you discover
a vulnerability, please report it privately rather than opening a public
issue — see SECURITY.md for how.
