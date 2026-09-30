---
name: review
description: Review local changes or a pull request (authoritative review criteria for this repo)
---

Single source of truth for code review criteria in this repo.

If no arguments are passed, review the diff between the base branch
(`dev` by default) and the current branch. If a PR number is passed,
review it via `gh pr view`/`gh pr diff`.

If this session wrote the code under review, do not review it in place:
delegate to the `reviewer` agent (`.claude/agents/reviewer.md`), which runs
this skill from a clean context, and relay its findings.

## Procedure

1. Read the change's stated scope (PR/commit description, or the user's
   request) first; flag work outside it.
2. Read the diff hunk by hunk. Decide a finding from the diff alone
   whenever it's enough: a wrong comparison, a swallowed error, a missing
   test, a convention violation. Open surrounding code only for a specific
   doubt, and only as far as that doubt requires — a hunk calls or changes
   the contract of something outside the diff, deletes/moves behavior with
   unclear callers, or a finding would be blocking and the diff alone can't
   confirm it. Note what you opened and why.
3. Apply the criteria below, then write findings.

## Criteria

1. **Solidity correctness & security**
   - Access control: is a function's actual modifier (`onlyOperator`,
     nothing, or a custom check) what the design doc says it should be?
     This repo deliberately mixes fully-permissionless (Conversion
     claims), operator-first-with-permissionless-fallback (header relay,
     §6.9), and fully-operator-gated (liquidity, network admin) access —
     the wrong one on the wrong function is a real bug, not a style nit.
   - Reentrancy: state updated before external calls/value transfers;
     `nonReentrant` present wherever value moves.
   - Bond/fee/timeout arithmetic: no silent overflow/rounding assumptions;
     `BPS_DENOM`-style scaling applied consistently; watch for `as u128`/
     float-to-int casts that truncate rather than round (a real,
     pre-existing characteristic in `chain_operator.rs`'s payout math —
     don't introduce a new one without a comment explaining it, the way
     that one is documented).
   - Every conversion/composition state machine needs a permissionless
     exit if the counterparty (operator or claimant) never shows up —
     check whether a new state has one.

2. **Rust (`core/operator`, Anchor programs) correctness & security**
   - No unexplained `unwrap()`/`expect()` on data that isn't guaranteed
     (acceptable in tests only).
   - Arithmetic on user-influenced values uses `checked_*`/`saturating_*`,
     never a bare operator that can panic or silently wrap.
   - Anchor account validation: every account a handler trusts is actually
     constrained (`#[account(...)]`, `has_one`, signer checks), not just
     typed.
   - A changed adapter method (`ChainStack`, `BetaHubAdapter`,
     `ConvertingAdapter`, etc.) matches the trait's documented contract for
     *every* implementor (EVM and SVM), not just the one this change
     touched.
   - Genuinely pure logic (no I/O, no `Arc<dyn Trait>`) extracted into a
     named function and unit tested, rather than left inline and untested
     inside an async tick function — see `chain_operator.rs`'s
     `evaluate_rbf`/`compute_safe_native_amount`/etc. for the pattern.

3. **Cross-network consistency**
   - A change to one `programmable-network/<chain>` package's contracts
     that isn't mirrored across the others when it should be — check
     whether the difference is chain-specific for a stated reason
     (Hyperliquid's gas limit, Tempo's native-value rejection) or an
     accidental divergence.
   - Never trust a transaction receipt's reported address/result on Tempo
     without independent on-chain verification
     (`docs/design/ipow-implementation.md`'s Tempo section) — flag any new
     Tempo deploy/write script that skips this.
   - A new cross-network registration (`addNetwork`) should be reciprocal
     — check both directions got registered, not just one.
   - A claim about live/deployed state (a new address, "this is live now")
     must be independently verifiable — an on-chain read, not just a
     trusted receipt or a script's stdout.

4. **Testing**
   - New contract/program behavior has a test for both the happy path and
     its permissionless-cleanup/failure path.
   - A bug fix has a regression test that would have failed before the
     fix.

5. **Design alignment** — invoke the `design-alignment` skill and follow
   its procedure in full; this point is a summary, not a substitute. Any
   deviation from `docs/design/ipow.md` or `docs/design/ipow-implementation.md`
   must be flagged as `⚠️ DESIGN DEVIATION`, citing the doc/section and the
   code location; conform to the design or update the design doc with
   rationale in the same change.

6. **Documentation & hygiene**
   - A live address, a new deployed contract, or a changed mechanism
     claimed as "done" is independently verifiable, not just described.
   - No `.env` or `config.yml` staged (`git check-ignore -v` any new file
     in a package that has real secrets before trusting the existing
     `.gitignore` covers it).
   - Commits carry no AI-attribution trailers (per this repo's
     `CLAUDE.md`/`AGENTS.md`).

## Output

List findings ordered by severity (blocking, should-fix, nit), each citing
`file:line`. End with a short "checked" list (what you opened beyond the
diff, and why) and a "not checked" list (anything deliberately skipped).
