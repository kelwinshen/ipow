# iPoW

Rules for agents (and anyone else's AI) working in this repo.

**This file contains rules and pointers only — never facts about the
system.** Facts (contract addresses, mechanisms, parameters, flows) live in
`docs/` and the code itself, per the source-of-truth map below. Do not
restate them here: this file is loaded into every session, so a design
fact copied here becomes an unreviewed, drift-prone shadow spec the moment
the real thing changes. Link, don't copy.

## Design & spec discipline (read this first)

- [`docs/design/ipow-protocol.md`](docs/design/ipow-protocol.md) is the
  **canonical design doc**, and the specs in [`docs/specs/`](docs/specs/)
  are the canonical specs of the applications and features built on it
  (see [`.github/CODEOWNERS`](.github/CODEOWNERS) — changes to them are
  reviewed the same as contract changes). Reason and implement *from*
  them; treat them as the spec. The design doc's "Build status" section
  says what exists.
- [`docs/archive/`](docs/archive/) holds the design docs of the retired
  generation (git tag `legacy-v1`) and other history. **Not
  authoritative** for anything in the repository today.
- **Before changing contract, program, or node behavior, read the
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
- Real deployed addresses are **not** in the design docs or specs — each
  network's own README has its current live addresses (see the map
  below). Don't add an address table to a design doc; that's deployment
  data, not design.
- `docs/drafts/` (see [`docs/README.md`](docs/README.md)) is where an
  unratified, work-in-progress design proposal goes before it's promoted
  into a canonical doc, folded into an existing one, or dropped.
  [`docs/drafts/README.md`](docs/drafts/README.md) lists what is there.
  If you write a design doc for something that isn't built and verified
  yet, it goes in `docs/drafts/`, not into `docs/design/` or
  `docs/specs/` as if it were already true.

## Source-of-truth map

| To know about… | Read… |
|---|---|
| What the system is, for a first-time reader | root [`README.md`](README.md) |
| The protocol (light client, operators, jobs, fees, punishment, claims, vault), its numbered decisions, and what is built and deployed | [`docs/design/ipow-protocol.md`](docs/design/ipow-protocol.md) |
| A built application or feature (Conversion, the tunnel, BETA, vault genesis, the other networks, the SDK) | its spec in [`docs/specs/`](docs/specs/); index in [`docs/README.md`](docs/README.md) |
| Build order, dated live runs and incidents | [`docs/drafts/`](docs/drafts/) (build plan, live-run log) — **not authoritative** for current behavior |
| The retired generation and superseded designs | [`docs/archive/`](docs/archive/) — **not authoritative** for current behavior |
| Real deployed addresses, per network | Sepolia: [`evm/README.md`](evm/README.md); Solana devnet: [`solana/README.md`](solana/README.md); every other network: `networks/<chain>/README.md`. The machine-readable records they follow: `evm/deployments/<network>-testnet.json` (read back by `evm/scripts/verify-deployments.ts`) and `solana/deployments/devnet.json` (by `solana/scripts/verify-devnet.sh`) — **not** the design docs |
| Exact contract behavior (bond amounts, timeouts, auction mechanics) | the Solidity source itself — `evm/contracts/protocol/` (light client, protocol, vault) and `evm/contracts/applications/` (Conversion, BETA); every EVM network runs this one source, its builds and settings chosen per network in `evm/deploy/networks.ts` |
| Solana program behavior | `solana/programs/protocol/*/src/` and `solana/programs/applications/*/src/`; see [`solana/README.md`](solana/README.md) |
| The node: the operator, guardian and attester roles, and the vault's roles | `node/crates/`; [`node/README.md`](node/README.md) for running it |
| The SDK apps build on | `sdk/src/`; [`sdk/README.md`](sdk/README.md) |
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
  the Tempo CREATE-address bug in
  [`networks/tempo/README.md`](networks/tempo/README.md) for why this
  matters concretely, not just as caution).

## Documentation

- Keep docs accurate to current code, not aspirational. If a doc describes
  something as done, it should be independently verifiable (a live
  address, a passing test, a real transaction) — not just described.
- Plain, direct language. Say what changed and why; skip filler.
