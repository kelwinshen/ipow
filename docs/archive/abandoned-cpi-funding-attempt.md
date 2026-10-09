# Abandoned: CPI from Beta into Conversion, as the funding mechanism for a lock

> **Archived.** About the old contract generation, retired at the tag `legacy-v1`. History only. The current design is [`../design/ipow-protocol.md`](../design/ipow-protocol.md).

**Status: superseded/removed. Not authoritative. Kept for why the current
decoupled design (see [`ipow.md`](ipow.md)) looks the
way it does.**

Two separate eras of the same idea — fund a `BetaHub`/`BetaVault` lock by
having a contract drive `iPoWConversion`/`ipow-conversion` via CPI, so a
user only ever touches the mint chain — were built and then removed:

- **The original same-chain/basket design** — `BetaMint.sol` (mint funded
  directly through a bitcoin→native `iPoWConversion` auction) and
  `BasketBridge.sol` (a two-hop Solana↔Ethereum basket using the same
  CPI). See [`superseded-beta-designs.md`](superseded-beta-designs.md)
  for the full design. Already superseded by the statement-chain design
  (`BetaVault`/`BetaHub`/`beta-factory`) before this attempt started;
  their Solana-side counterpart (`beta-mint`, a genuinely separate
  program from `beta-factory`) was left running but unused, and has since
  been removed entirely.
- **A later attempt**: `BetaCompositionFundingIntent.sol` (the mint-chain
  leg) + `BetaVaultConversionConnector.sol` (a same-chain connector
  funding one remote leg via real Bitcoin) + a Rust "fan-out relayer"
  (`funding_relay.rs`) meant to settle every leg of a composition from
  one Bitcoin transaction, permissionlessly.

Stress-testing the second design surfaced two real fund-safety gaps in
`iPoWConversion.sol` itself, not just the new connector contracts (both
are documented as still-open in
[`ipow-implementation.md`](ipow-implementation.md)):

1. `reclaimExpiredConversion` only handles "claimed, then the claimant
   went dark" — there's no permissionless fallback for "nobody ever
   claimed within `CLAIM_WINDOW_SEC` (15 min) at all." A delegate
   contract's committed conversion can get stuck exactly like a real
   user's would.
2. `submitBitcoinMerkleProofWithTx` hard-restricts the caller to
   `c.user` (bitcoin→token) or `c.responsibleOperator` (native→bitcoin)
   — a real person, or a contract willing to be driven permissionlessly,
   must personally act within `PROOF_BLOCKS_WINDOW`/`DEPOSIT_BLOCKS_WINDOW`
   (real Bitcoin block-time windows, not generous ones).

The delegation pattern (a contract, not a person, as `c.user`) already
solves gap 2. It does not solve gap 1 — and combining both into "pay
once on the mint chain, a relayer settles every leg" means trusting a
third party's *willingness* to show up, with no guaranteed unstick if it
doesn't. Rather than fix gap 1 and keep the relayer, the conclusion was
to **decouple the two concerns entirely**:

- **Cross-chain value movement** (getting an asset onto a remote chain)
  stays the user's own responsibility, using `iPoWConversion`'s
  existing permissionless auction directly — the user is `c.user`,
  controls their own claim/proof, and nothing depends on a relayer's
  cooperation.
- **Locking** (once the user already holds the asset on that chain) is
  `BetaVault.deposit()` — already permissionless, already safe, no
  auction, no claim window, no proof-submission deadline. This was never
  built *for* the connector design; it's the vault's own original,
  unmodified entry point, always available.

Removed entirely: `BetaMint.sol`, `BetaToken.sol`, `BasketBridge.sol`,
`BetaCompositionFundingIntent.sol`, `BetaVaultConversionConnector.sol`,
their tests, ~14 one-off scratchpad scripts that drove them by hand, and
the Rust side's `funding_relay.rs`/`funding_relay_adapter.rs`
(trait + EVM impl)/`multi_output_payment.rs` plus their bindings. Kept
untouched: `BetaHub`/`BetaVault`/`HubToken` (the live statement-chain
system — never used Conversion CPI at all) and `iPoWConversion.sol`
itself (a standalone, independently-useful base primitive, not something
built for this experiment).
