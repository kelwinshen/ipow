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

1. **Solidity correctness & security** (`evm/contracts/`)
   - Access control: the protocol, light client, vault and applications
     have no owner and no key, and nothing can be changed after deployment
     (D59 in `docs/design/ipow-protocol.md`). A new privileged role, an
     owner, a setter or an upgrade path is a design deviation unless the
     spec names it (the vault's genesis key, `docs/specs/ipow-vault-genesis.md`,
     is the one such key, and it must expire). Check that each `only*`
     check (`onlyCore`, `onlyApp`, `onlyGenesis`) admits exactly the caller
     the spec says.
   - Money is credited and withdrawn, never pushed to an address: a new
     transfer to a caller-chosen address in the middle of a state change is
     a finding. State is updated before external calls; `nonReentrant`
     wherever value moves.
   - Both builds of the protocol and the vault (native coin and token,
     D136 to D138) get the change, and the token build refuses native
     value. Decimals and units: amounts in record units vs native units,
     Tempo's 6-decimal PathUSD, Hedera's 8-decimal HBAR (D139).
   - Bond, escrow, fee and timeout arithmetic: no silent overflow or
     rounding; a rounding direction that favours the user over the bond
     (or the reverse) is stated in a comment.
   - Every job, swap, claim or lock needs a permissionless exit if its
     counterparty (operator, user, guardian) never shows up: an expiry, a
     refund, a give-up. Check that a new state has one.
   - Contract size: the protocol and the vault core sit near the 24,576-byte
     limit (Build status row 7); a change that grows them states the new
     margin.

2. **Rust correctness & security** (`node/`, `solana/programs/`)
   - No unexplained `unwrap()`/`expect()` on data that isn't guaranteed
     (acceptable in tests only).
   - Arithmetic on user-influenced values uses `checked_*`/`saturating_*`,
     never a bare operator that can panic or silently wrap.
   - Anchor account validation: every account a handler trusts is actually
     constrained (`#[account(...)]`, seeds, `has_one`, signer checks), not
     just typed. Only `initialize` may require the upgrade authority.
   - The Solana program and the EVM contract of the same part keep the
     same rules; a difference is one the spec's Build status lists as
     "differences made by Solana", or it is a finding.
   - A changed method of the node's network interfaces (`ProtocolNetwork`,
     `ConversionApp`, `VaultApp` in `node/crates/protocol`) matches its documented contract
     in *every* adapter (`node/crates/networks/evm` and
     `node/crates/networks/svm`), not just the one this change touched.
   - Pure logic (no I/O) extracted into a named function and unit tested,
     not left inline and untested inside an async round of a role.

3. **Cross-network consistency**
   - Every EVM network runs the same source in `evm/contracts`, chosen and
     configured per network in `evm/deploy/networks.ts` (D134). A copy of a
     contract for one network is a finding; a per-network build is allowed
     only where a decision names it (the token builds, D136 to D138;
     Hedera's, D139; Polkadot's, D140).
   - Never trust a transaction receipt's reported address or result on
     Tempo without reading the chain back
     ([`networks/tempo/README.md`](../../../networks/tempo/README.md)) —
     flag any new Tempo deploy/write script that skips this.
   - A vault pair is two-sided: both vaults name each other, and on Solana
     the pair's `["config", peer]` account names the EVM vault. Check both
     directions.
   - A claim about live/deployed state (a new address, "this is live now")
     must be independently verifiable — an on-chain read
     (`evm/scripts/verify-deployments.ts`, `solana program show`), not just
     a trusted receipt or a script's stdout. A redeploy updates the
     network's README, `evm/deployments/`, and the SDK's generated files
     (`pnpm sync` in `sdk`).

4. **Testing**
   - New contract/program behavior has a test for both the happy path and
     its permissionless-cleanup/failure path, on EVM and on Solana when
     both have it.
   - A bug fix has a regression test that would have failed before the
     fix.

5. **Design alignment** — invoke the `design-alignment` skill and follow
   its procedure in full; this point is a summary, not a substitute. Any
   deviation from `docs/design/ipow-protocol.md` or a spec in `docs/specs/`
   must be flagged as `⚠️ DESIGN DEVIATION`, citing the doc/section (and
   decision number) and the code location; conform to the design or update
   the design doc with rationale in the same change.

6. **Documentation & hygiene**
   - A live address, a new deployed contract, or a changed mechanism
     claimed as "done" is independently verifiable, not just described.
   - No `.env`, `.env.operator-btc`, `config.yml` or key file staged
     (`git check-ignore -v` any new file in a package that has real
     secrets before trusting the existing `.gitignore` covers it).
   - Commits carry no AI-attribution trailers (per this repo's
     `CLAUDE.md`/`AGENTS.md`).

## Output

List findings ordered by severity (blocking, should-fix, nit), each citing
`file:line`. End with a short "checked" list (what you opened beyond the
diff, and why) and a "not checked" list (anything deliberately skipped).
