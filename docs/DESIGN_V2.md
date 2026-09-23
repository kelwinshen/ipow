# Design v2: Conversion, and BETA built on top of it

Current-state reference only — describes what the code does, not how it
got there, **except §§2–5 and §6.1–6.12, which are historical**: earlier
design phases, superseded by §7/§8's statement-chain BETA and formally
abandoned as a *funding mechanism* in §9.1 (2026-09-24) — read them for
context on why the current design looks the way it does, not as
descriptions of what's deployed today.

**The current architecture, in one paragraph**: two independent app
layers, sharing only `iPoWV1`/`ipow`'s Bitcoin header relay, read-only.
**Conversion** (`iPoWV1Conversion.sol` / `ipow-conversion`, §1–§2) is a
permissionless, staked-auction native↔Bitcoin swap — the base primitive,
live on every EVM network in §9.5's table plus Solana devnet. **Beta**
(`BetaHub.sol`/`BetaVault.sol`/`beta-factory`, §6–§8) is a genuine
consumer of Conversion's *design pattern* (Bitcoin-anchored statements,
bonded parties, permissionless judging) but not of Conversion itself —
no CPI between them. A user moves value onto a chain via Conversion,
independently and under their own control, then locks it via
`BetaVault.deposit()`/`lock_sol` — two decoupled steps, not one CPI
chain (§9.1 explains why that CPI was tried and abandoned).
`BetaMint.sol`/`BetaToken.sol`/`BasketBridge.sol` (Ethereum) and
`beta-mint` (Solana) — an *earlier* attempt at funding Beta through
Conversion CPI directly — are removed/unused; see §9.1 before assuming
anything below that mentions them is still live. `ipow`'s own original
single-fixed-operator Conversion (fused inside the `ipow`/`iPoWV1`
program itself, a different mechanism from the permissionless-auction
one this doc mostly describes) is separate and untouched.

## 1. Conversion: the base primitive

A permissionless, staked-auction native↔Bitcoin swap. Two directions,
symmetric:

- **`token→bitcoin`** (`commit_token_to_bitcoin`/`commitTokenToBitcoin`,
  renamed from `commit_native_to_bitcoin`/`commitNativeToBitcoin` — the
  instruction always accepted any SPL/ERC20 mint via `token_mint`/
  `tokenAddr`, "native" only ever meant "the default case"): a user locks
  value (SOL/ETH or a supported SPL/ERC20 token), naming a Bitcoin
  receive-script (`user_program`/`userProgram`) they want paid. Released
  to whoever wins the claim auction and then proves, via a real Bitcoin
  header + Merkle proof, that they paid it. Optionally carries a
  **bundle**: up to 3 `extra_tokens`/`extraTokens` (`{mint, amount}`
  pairs) beyond the primary `token_mint`/`native_amount` slot — several
  different tokens locked by the same commit, all released or refunded
  together, settled by the *same single* `bitcoin_amount` proof. Every
  extra mint must be distinct from each other and from the primary
  (`DuplicateTokenMint`) and non-default (`InvalidTokenMint` — native
  SOL/ETH can only ever be the primary slot); an empty bundle is
  identical to the pre-rename behavior. This is the *auctioned* bundle
  path — `bitcoin→token` bundles also exist, via a different mechanism
  in each case; see the "Multi-token" note below.
- **`bitcoin→token`** (`commit_bitcoin_to_token`/`commitBitcoinToToken`,
  renamed from `commit_bitcoin_to_native`/`commitBitcoinToNative`): the
  winning claimant self-escrows the payout amount themselves at claim
  time, released to the committer once *they* prove a real Bitcoin
  payment landed at the claimant's registered receive-script.
  **Single-token** (`extra_tokens` empty — the everyday case): `native_
  amount` is the field claimants compete on, higher wins (see "Auction
  lifecycle" below). **Bundle** (Solana only, `extra_tokens` non-empty —
  up to 3 extras beyond the primary, same shape as `token→bitcoin`'s own
  bundles): ranking "claimant A offers 100 USDC + 5 SOL" against
  "claimant B offers 90 USDC + 6 SOL" has no natural ordering without a
  price oracle, so a bundle claim flips which side is fixed vs.
  competed — the whole token wishlist (primary + every `extra_tokens`
  entry) becomes the fixed ask instead, and claimants compete on
  `bitcoin_amount` alone, **lower** wins (see "Auction lifecycle" below
  for the exact mechanics). EVM mirror not yet built.

### Auction lifecycle

- **`propose_claim_conversion`/`proposeClaimConversion`** — permissionless,
  competes on **rate**, not stake. `required_bond` (set by whoever
  committed — they bear the risk, so they set the security budget) is a
  fixed anti-griefing bond, refunded in full to an out-bid claimant; it
  never decides who wins. On Solana, it's only ever required for
  `token→bitcoin`: that claimant never self-escrows anything on-chain
  (only a later-proven promise to pay real BTC), so the stake is their
  *only* skin-in-the-game. `bitcoin→token` (single-token or bundle) posts
  no stake at all — the claimant self-escrows real collateral right here
  instead, and that collateral's destination is identical whether they
  complete the duty or abandon it (released via proof or force-claim,
  always to the user either way), so a separate stake would be redundant
  with the exposure the self-escrow already creates. `commit_bitcoin_to_
  token` doesn't even accept a `required_bond` param anymore; `conversion.
  required_bond`/`staked_bond` simply stay 0 for that direction. EVM
  hasn't had this change ported yet — `iPoWV1Conversion.sol` still
  requires and posts a stake for every direction. What wins a claim is
  `proposed_rate_amount`, and the rules split three ways:
  - **`token→bitcoin`**: how much real BTC this claimant will pay the
    user (`bitcoin_amount`) — higher wins, and it covers the whole bundle
    if one exists (the locked side, never competed on).
  - **`bitcoin→token`, single-token**: how much native/SPL/ERC value
    they'll front for the fixed required Bitcoin proof (`native_amount`)
    — higher wins.
  - **`bitcoin→token`, bundle** (Solana only): the token side (primary +
    every `extra_tokens` entry) is the fixed ask instead — claimants
    compete purely on `bitcoin_amount`, and **lower** wins (they're
    underbidding how little BTC the user needs to pay for the same fixed
    bundle). A claimant self-escrows the *entire fixed bundle* on every
    claim, win or out-bid — never `proposed_rate_amount`, which is the
    competed BTC side here, not a fund amount.

  In every case, the committer's own commit-time value is the ask —
  meeting it exactly wins a first claim; every later out-bidding claim
  must strictly beat the current best (`BelowAskedRate`/`AboveAskedRate`
  on a bad first claim, `RateNotBetter` on a bad out-bid).
  `submit_proof_cache`/`submitBitcoinMerkleProofWithTx` enforces whatever
  rate wins (`out_value_sats >= conversion.bitcoin_amount`), so a
  claimant cannot renege on it after winning.
  Also carries the claimant's own Bitcoin receive-script registration for
  `bitcoin→token` (mandatory, globally unique per contract instance via
  `used_program_pda`/`usedIPoWPrograms` — the same script can never be
  claimed twice). For `bitcoin→token`, this is also where the claimant
  **self-escrows** their own proposed `native_amount` (single-token) or
  the fixed bundle (bundle), pre-finalize only (`!window_started`) — see
  "No liquidity pool" below. This is real, escrowed value, so it (like
  the escrow itself) only ever moves pre-finalize; a round reopened after
  a claimant defaults inherits the already-real amount as-is — for a
  bundle, the new claimant picks up the exact same escrowed bundle
  without re-funding it at all (`window_started` gates the self-escrow/
  bundle loop, so it simply doesn't run on a reopen; Solana `bitcoin→
  token` posts no stake either, so there's nothing else for the new
  claimant to front). For `token→bitcoin`, `bitcoin_amount` is never
  backed by an
  on-chain escrow — only a later-proven promise — so it stays updatable
  regardless of `window_started`. For a `bitcoin→token` bundle,
  `bitcoin_amount` updates inside the same `!window_started` gate as the
  bundle self-escrow itself (same reasoning as `native_amount` above) —
  a reopened bundle claim inherits the frozen rate as-is too, it cannot
  be renegotiated post-default.
- **`finalize_claim_conversion`/`finalizeClaimConversion`** — permission-
  less, quiet-period gated (`CLAIM_QUIET_PERIOD_SEC`). Locks in the
  winner, starts the duty window. `window_start_height`/`windowStartHeight`
  is captured here, once, from the current Bitcoin header-relay tip, and
  never reset on a later reopen — a user's downstream refund/force-claim
  deadline cannot be pushed out by a claimant handoff.
- **`reclaim_expired_conversion`/`reclaimExpiredConversion`** — permission-
  less. If no real commitment exists yet, refunds the claimant's stake and
  reopens cleanly. If a real commitment already exists (a `token→bitcoin`
  deposit, or a `bitcoin→token` self-escrow), the stake is forfeited —
  on Solana, paid straight to `conversion.user` immediately, right here
  (compensation for the delay, not held for whoever eventually finishes —
  neither guaranteed-resolution path below depends on a replacement
  claimant ever showing up, so there was nothing to reward by holding it).
  `required_bond` escalates to match regardless (a separate anti-repeat-
  griefing measure, unrelated to who the forfeit goes to), and the
  conversion reopens for a new claimant without re-touching the already-
  real value — a second claimant competing for a forfeited duty does not
  double-fund it. On Solana, this only has anything to forfeit for
  `token→bitcoin` — `bitcoin→token`'s `staked_bond` is always 0 (see
  above), so this whole branch is a no-op when one of those reopens; the
  guarantee that a second claimant doesn't need to re-fund the already-
  real self-escrow still holds regardless. EVM hasn't had this change
  ported yet — `iPoWV1Conversion.sol` still forfeits into a `bounty` field
  that pays out to whichever claimant eventually completes the duty,
  same as this design used to work on Solana too.
- **`submit_proof_cache`/`submitBitcoinMerkleProofWithTx`** — checks a
  real Bitcoin header (via the chain's own header relay) and Merkle
  proof, checks the payment amount/script match what was committed, and
  pays out.

### Guaranteed resolution

- **`token→bitcoin`**: if the claimant never proves, permissionless
  **`refund_no_proof_native_to_bitcoin`/`refundNoProofNativeToBitcoin`**
  returns the deposit once the proof window passes
  (`window_start_height + PROOF_BLOCKS_WINDOW` — a **block-height** check
  against the header relay's tip, so it needs the tip to have advanced
  that far, from any header-relay activity, not necessarily anything
  specific to this conversion).
- **`bitcoin→token`**: if the claimant never proves, permissionless
  **`claim_native_operator_expired`/`claimNativeOperatorExpired`**
  ("force-claim") pays the committer the already-self-escrowed amount
  unconditionally. This check is purely **time-based**
  (`operator_duty_expires_at` vs. wall-clock `Clock::get()`) — no
  dependency on any Bitcoin header ever being relayed.

Every path terminates with real value correctly resolved: a real
proof-gated payout, or a real forfeiture/refund. Conversion never
releases more than what was actually deposited or self-escrowed.

### No liquidity pool — self-escrow only

There is no governance-funded pool. `bitcoin→token`'s payout is entirely
backed by whichever claimant wins the auction personally self-escrowing
their own proposed `native_amount` at `propose_claim_conversion` time —
symmetric with how `token→bitcoin` already has the *user* front real
value via `deposit_conversion`/`depositApprovedConversion`.
`Pool.total_reserved`/`totalReservedAmount` are bookkeeping only, never
load-bearing for correctness.

### Multi-token

`token_mint`/`tokenAddr` (default = native SOL/ETH) selects the asset a
given conversion moves; every value-moving instruction branches once on
it — native uses a plain transfer, anything else a real SPL
`transfer_checked` CPI (classic Token and Token-2022, cross-checked
against the mint's actual owner) or `SafeERC20`. The auction stake and
commit fee are always native SOL/ETH regardless of what the conversion
itself moves — a separate pot of money from the self-escrowed/deposited
`native_amount`/`bitcoin_amount`, which follows `token_mint`/`tokenAddr`
and can be an SPL/ERC20 token. Deposit-direction transfers measure the
actual amount received via a before/after balance delta
(`transfer_value_in`/`_pullToken`) rather than trusting the requested
amount, so a fee-on-transfer token cannot silently under-collateralize
an already-fixed promise.

**Bundles**: `Conversion.extra_tokens`/`extraTokens` holds up to 3 extra
`{mint, amount}` pairs beyond the primary slot — same shape everywhere,
but reached via three different mechanisms depending on direction:

- **`token→bitcoin`** (both chains, auctioned — §1's everyday path):
  `deposit_conversion`/`depositApprovedConversion`, `submit_proof_cache`/
  `submitBitcoinMerkleProofWithTx`, and `refund_no_proof_native_to_
  bitcoin`/`refundNoProofNativeToBitcoin` each loop it, moving every
  extra token the same way (and at the same time) as the primary.
- **`bitcoin→token`, direct-Bitcoin** (Solana only, auctioned — §1's
  "Auction lifecycle"): `propose_claim_conversion` self-escrows the
  fixed bundle (and refunds an out-bid claimant's) via the same loop
  shape, just inbound instead of at deposit time; `submit_proof_cache`'s
  `bitcoin→token` branch and `claim_native_operator_expired` pay it out.
- **`bitcoin→token`, cross-network** (both chains, no auction — §2's
  `open_bundle_tunnel`/`openBundleTunnel`): the opener self-funds the
  fixed bundle atomically in one call; the same `submit_proof_cache`/
  `claim_native_operator_expired` loops above pay it out — they're
  agnostic to how a conversion's `extra_tokens` got populated.

On Solana, since Anchor's typed `Accounts` struct can't size to a
variable-length token list, each extra token's accounts travel via
`ctx.remaining_accounts`, in the same order as `extra_tokens` — 5
accounts per extra (`mint`, `token_program`, `escrow_vault`,
`escrow_ata`, one token account) everywhere except `propose_claim_
conversion`'s bundle loop, which needs 6 (both a refund-out and an
escrow-in token account, since out-bidding needs both directions in the
same call). Because `remaining_accounts` bypass Anchor's automatic
`#[account(seeds = ...)]` validation, every extra token's `escrow_vault`
PDA is manually re-derived and checked (`InvalidRemainingAccount` on
mismatch) before any transfer — a wrong or attacker-supplied remaining
account is a fund-safety bug, not a UX one, so every one of these loops
is covered by its own test asserting a mismatched account reverts rather
than misdirecting funds. `Pool.total_locked_deposits`/`total_reserved`
bookkeeping is not updated for each extra token's own per-mint pool
(never load-bearing for correctness, per above) — only the
real transfers matter.

### Security model

Bitcoin's proof-of-work can only prove a fact about *itself* — that a
specific payment happened — never about another chain's state. Every
release of value in Conversion is gated on exactly that one fact,
checked independently by whichever chain is paying out, against real
value that already moved (a deposit or a self-escrow) before any proof
is possible. Nothing here trusts a claim about what happened elsewhere;
it only ever checks Bitcoin proving something about Bitcoin.

## 2. Tunnels: `bitcoin→token` without an auction

Two independent tunnel mechanisms, one per generation of the design,
both built on the same idea — skip `propose_claim_conversion` entirely
and have the payer self-fund the destination side directly, gated only
on a real Bitcoin proof (§1's security model still applies unchanged).

### `ipow`'s tunnel: `operator_open_tunnel` (Solana, single fixed operator)

`add_network`/`SupportedNetwork` is a 4-chain protocol registry —
`network_id`: 1=Hedera, 2=Ethereum, 3=Solana, 4=Polkadot — governance-
gated, with matching post-deploy config scripts on all four chains
(`solana/scripts/init.ts`, `ethereum/`/`hedera/`/`polkadot/scripts/
configure.ts`, all sharing one `NETWORK_REGISTRY`).

`operator_open_tunnel` is `bitcoin→token` underneath (`is_native_to_
bitcoin = false`): the single fixed `global_state.operator` opens it
unilaterally, reserving the payout from the operator's own pool
liquidity (`available_liquidity >= reserve`, checked against the
program's real escrow balance) rather than a per-conversion self-escrow.
It carries a real `ipow_receive_program` Bitcoin script (SHA256+
uniqueness checked, same as a normal claim's registration), sets
`window_started`/`window_start_height` immediately, and still requires a
real proof via `submit_proof_cache` to settle. "Native→native" describes
the user-facing shape (pay in, get paid out on another chain, no Bitcoin
ever touched by the user) — underneath, real Bitcoin still settles both
legs: the user's `token→bitcoin` payment is claimed by the operator, and
that operator proving the same real Bitcoin payment against their own
registered script releases `native_amount` to `dest_address` on the
other chain.

`network_id`/`network_address`/`dest_address` are first-class fields on
the `Conversion` account itself — real, on-chain correlation between a
chain-A leg and its intended chain-B counterpart, which §5's basket-mint
design does not have (see its own limitation note).

### `ipow-conversion`'s tunnel: `open_bundle_tunnel` / `openBundleTunnel` (both chains, permissionless, multi-token)

Built directly on §1's Conversion (not on `ipow`'s design above), and
differs from `operator_open_tunnel` in two ways: it's **permissionless**
— neither contract has a fixed-operator concept anywhere else, so anyone
may open one, self-funding it themselves — and it **supports a token
bundle**, up to 3 `extra_tokens`/`extraTokens` beyond the primary slot,
the same `TokenAmount` shape §1's `token→bitcoin` bundles use.

This is the *only* way `bitcoin→token` supports more than one token:
`commit_bitcoin_to_token`/`commitBitcoinToToken` (§1) never can, because
its rate-competed field would become multi-dimensional and incomparable
across claimants without a price oracle. Removing the auction sidesteps
that — the opener self-funds the whole fixed bundle atomically in one
call (primary + every extra token, via the same self-escrow mechanism
§1's bundles use: Solana's `remaining_accounts`, same as the outbound
bundle loops but inbound instead of at deposit time; EVM's `_pullToken`
in a loop), becomes `responsible_operator`/`responsibleOperator`
immediately, with `status = Approved` and `window_started`/
`windowStarted = true` set in that same call — no separate propose/
finalize step. Restricted to `network_id != 0`/`networkId != 0`: a
direct-Bitcoin, single-token `bitcoin→token` conversion already has
`commit_bitcoin_to_token`'s auctioned path, so this isn't a second route
to the same thing.

Both `submit_proof_cache`/`submitBitcoinMerkleProofWithTx`'s
`bitcoin→token` branch and `claim_native_operator_expired`/
`claimNativeOperatorExpired`'s guaranteed-resolution force-claim loop
`extra_tokens`/`extraTokens` to settle or force-release a bundle, the
same pattern §1's `token→bitcoin` payout loops already use.
`reclaim_expired_conversion`/`reclaimExpiredConversion` needs no bundle-
awareness — it's pure bond/status bookkeeping that never touches
escrowed value, so if an opener goes dark the conversion simply becomes
claimable again by a normal claimant at the already-escrowed amount,
same as any other reopened `bitcoin→token` duty.

**Network registry, one per chain, governance-gated:**

- **Solana**: `add_network`/`remove_network`, one `SupportedNetwork` PDA
  per `network_id` (`seeds = [b"network", network_id]`). `add_network`
  rejects `network_id == 0`, `min_addr_len == 0`, or `min_addr_len >
  max_addr_len` (`InvalidNetworkConfig`); re-registering an already-
  active `network_id` fails via Anchor's own `init` (refuses to
  re-initialize an existing PDA). `remove_network` closes the PDA,
  refunding its rent to governance.
- **Ethereum**: `addNetwork`/`removeNetwork`, `mapping(uint256 =>
  NetworkConfig) public networkConfigs` — same validation, and the same
  `NetworkConfig` struct shape and `InvalidNetworkConfig`/
  `IncorrectNetwork`/`IncorrectNetworkAddress` error names `iPoWV1.sol`
  already uses for its own (separate) registry, minus `iPoWV1`'s
  `SELF_NETWORK_ID` guard — nothing in this contract needs "this chain's
  own network id". `Conversion.networkAddress` sits at the *end* of the
  Solidity struct, after `bounty` rather than next to `networkId`, so it
  doesn't shift any other field's position: `BetaMint.sol`/
  `BasketBridge.sol`'s locally-redeclared `conversions()` interfaces only
  decode a prefix of the tuple and are unaffected by a new trailing
  field.

Both chains' registries are 1:1.

## 3. Cross-chain linking: two Conversion legs, one real Bitcoin payment

Two independently-deployed Conversion instances (one per chain), linked
only by both referencing the same real Bitcoin transaction — no new
trust mechanism beyond §1:

- Chain A's `token→bitcoin` locks value there, released once someone
  proves (via Chain A's own header relay) they paid Chain A's registered
  script.
- Chain B's `bitcoin→token` has a claimant self-escrow value there,
  released once someone proves (via Chain B's own header relay,
  independently) that the same real transaction paid Chain B's
  registered script.

A claimant free to register any script on the `bitcoin→token` leg can
make it match whatever the `token→bitcoin` leg expects — so one real
Bitcoin payment, submitted separately to each chain's own header relay,
independently satisfies both. Neither chain reads or trusts the other's
state; each only checks a fact about Bitcoin. Script uniqueness
(`bitcoin→token`'s guard) is enforced per contract instance, so the
same script can be Chain A's check and Chain B's registration with no
collision; `token→bitcoin` has no uniqueness guard at all, so a script
can be reused freely for outbound legs.

**What this does not do**: verify that the party registering matching
scripts on both legs is honest, or that they even intend to link them.
The protocol accepts whatever script bytes a caller supplies — it has no
independent notion of "the address the user really wants," and no
awareness that another chain exists at all. Getting the scripts to
actually match is the job of whoever orchestrates both legs (a user
calling raw Conversion directly, or an app built on top); get it wrong
and both legs still resolve on their own (proof or recovery), they just
do not link as intended. This is a correctness concern for the
orchestrator, not a fund-safety risk.

## 4. BETA, same-chain: `beta-mint` / `BetaMint.sol`

Single-asset (native SOL / native ETH), same-chain mint + redeem, no
cross-chain hop. A shared **vault PDA** (Solana, `seeds = [b"beta_vault"]`)
or the `BetaMint` contract itself (Ethereum — `msg.sender` is
automatically the contract on any call it makes, no PDA ceremony needed)
acts as `conversion.user` on every Conversion these programs initiate —
a pooled-reserve model: solvency is "minted BETA supply ≤ the vault's
real native balance."

- **Mint**: `initiate_mint`/`initiateMint` CPIs `commit_bitcoin_to_token`
  signed by the vault, recording `MintRequest{recipient, amount,
  claimed}`. The auction (propose/finalize) runs directly against
  Conversion, unmodified, by whoever wants to claim it.
  `submit_proof_cache`/`submitBitcoinMerkleProofWithTx` requires its
  signer to equal `conversion.user` (the vault) — a PDA can only "sign"
  via a CPI from its own program — so `claim_and_mint`/`claimAndMint`
  folds proof submission and minting into one instruction,
  permissionless, callable by anyone holding the real proof.
- **Redeem**: `initiate_redeem`/`initiateRedeem` burns BETA 1:1
  (`burn_amount == native_amount`), then CPIs `commit_token_to_bitcoin`
  signed by the vault. Once the auction finalizes, permissionless
  `fund_redeem_reserve`/`fundRedeemReserve` CPIs `deposit_conversion`/
  `depositApprovedConversion`, moving the vault's reserve into
  Conversion's escrow. The winning claimant proving payment (raw,
  unmodified) pays the user's real Bitcoin address and collects the
  deposited reserve as reward — redemption complete, no further
  BETA-side step.

**Guaranteed resolution**: `claim_mint_after_expiry`/`claimMintAfterExpiry`
force-claims (if still `Approved`, past duty window) or mints against an
already-force-claimed conversion — real value landing in the vault via
force-claim always has a path to becoming BETA, not just via proof.
Symmetric on redeem: `reclaim_redeem_after_expiry`/
`reclaimRedeemAfterExpiry` triggers the no-proof refund if needed and
re-mints the burned amount back to the original burner (`RedeemRequest{
burner, amount, recovered}`) — redemption failing does not leave the
caller's burned BETA simply gone. A second call to either recovery path
reverts (`AlreadyClaimed`).

## 5. BETA, cross-chain basket: two-hop, minted on Solana

> **Superseded by §6.** Kept as the record of what `beta-mint`/`BasketBridge.sol`
> currently implement and of the gap named in its last paragraph.

Extends `beta-mint` with a second minting path backed by a basket split
across both chains — part native SOL locked on Solana, part native ETH
permanently locked on Ethereum via a new contract, `BasketBridge.sol`.
Four Conversion legs; one real Bitcoin payment can link all four (script
uniqueness is per-contract, and outbound legs have no uniqueness guard
at all, per §1).

**User-facing surface stays on Solana only** — deposit, then later mint;
Ethereum is entirely the operator/arbitrageur's concern:

1. **Leg1** (`initiate_basket_mint`, Solana `token→bitcoin`): locks the
   user's deposit, records `BasketMintRequest{recipient, beta_amount,
   leg1_tx_id, leg4_tx_id, leg4_registered, claimed}`.
2. **Leg2** (`BasketBridge.commitLeg2` + raw `proposeClaimConversion`/
   `finalizeClaimConversion`, Ethereum `bitcoin→token`): an arbitrageur
   self-escrows native ETH, `BasketBridge` itself as `conversion.user` so
   the payout lands there, not an arbitrary caller's wallet.
3. **`BasketBridge.claimLeg2AndForward`**: releases Leg2's escrow into
   `BasketBridge`, permanently retains `keepAmount` (`totalLocked` —
   basket collateral #1), and immediately commits **Leg3**
   (`token→bitcoin`) for the remainder, to bridge back to Solana.
4. **Leg3** auction resolves directly against Conversion; `fundLeg3Deposit`
   (mirrors `fundRedeemReserve`) moves the forwarded amount into escrow.
5. **Leg4** (`initiate_basket_leg4`, Solana `bitcoin→token`): a claimant
   self-escrows the returning amount — basket collateral #2, landing in
   `beta-mint`'s own vault.
6. **`claim_basket_and_mint`**: permissionless, folds Leg4's `submit_
   proof_cache` CPI with a direct state check that Leg1 already shows
   `Completed`/`proofVerified: true`, then mints.

**Guaranteed resolution, both legs**:

- **`claim_basket_mint_after_expiry`**: if Leg4's claimant never proves,
  force-claims (purely time-based, no header dependency; the resulting
  conversion shows `Completed, proofVerified: false`) and still mints,
  once Leg1 independently shows `Completed`/`proofVerified: true`.
- **`reclaim_basket_deposit_after_expiry`**: if Leg1's claimant never
  proves, refunds the original deposit straight back to the recipient —
  native SOL, not BETA, since no BETA was ever minted or burned for this
  request — once the header relay's tip crosses the proof window
  (`window_start_height + PROOF_BLOCKS_WINDOW`, block-height based, same
  as §1's `token→bitcoin` refund). Both recovery paths reject a second
  call (`AlreadyClaimed`).

**What this guarantees**: BETA is never minted against unmoved value —
every mint follows real value already landing (via proof or forced
forfeiture) on Solana's own, independently Bitcoin-verified legs.
**What it does not guarantee**: that Ethereum's Leg2/Leg3 actually
happened. Solana cannot read Ethereum's contract state directly, and
this design has no cross-chain oracle. That link is economic, not
cryptographic — an arbitrageur only profits by completing both sides,
and if they do not, Leg4's forced forfeiture still leaves the vault (and
BETA's backing) solvent regardless of Ethereum's actual state. Closing
this gap would need a genuine cross-chain state proof, which nothing
here currently provides.

**Known limitation**: no on-chain correlation between Solana's
`leg1_tx_id`/`leg4_tx_id` and Ethereum's `txId2`/`txId3` — tracked
off-chain by whoever orchestrates a request. §2's tunnel mechanism
already carries this kind of correlation as first-class on-chain fields
and is a direct precedent for closing this gap. A real multi-user
version would need a correlation id threaded through (e.g. via
`userProgram`'s bytes) or an off-chain coordinator — not built.

## 6. BETA v2: one fungible BETA on Solana, backed across chains — Bitcoin-anchored statements, adjudicated where the fact lives

**Built (2026-09-21), not deployed**: Solana `programs/beta-factory`
(18 litesvm tests), Ethereum `contracts/BetaVault.sol` (13 hardhat tests),
and the header-relay change in §6.9 on both chains. The operator service
does not yet build anchors or run the auditor loop (§6.13).

Supersedes §5 as the target design. §5's closing paragraph names the gap
exactly: Solana cannot read Ethereum, so nothing on Solana can tell an
honest "1 ETH locked on Ethereum" from a fabricated one. Every design
that tried to close that gap with Bitcoin payments, derived scripts,
round trips, escrows, timers or entry-point restrictions failed the same
test — the honest and cheating runs produce byte-identical inputs on
Solana. This section does not pretend to give Solana a view of
Ethereum. It does something narrower and provable:

> **Every cross-chain statement an operator makes is a Bitcoin
> transaction. The chain that owns the fact the statement is about
> adjudicates it — with code, permissionlessly — and slashes a bond it
> holds. The chain that cannot see the fact acts on the statement
> provisionally, rate-limited, and honors vetoes that are themselves
> Bitcoin transactions adjudicated the same way.**

A lie is still *accepted* by the blind chain. It is *proven and punished
by code* on the other chain within Bitcoin-confirmation latency, the
punishment is denominated in the asset the lie was about, and the
punishment funds the redemptions the lie created. No governance decides
truth. Humans hold at most a pause button, and §6.7 replaces even that
with bonded auditors.

**Trust tier, stated plainly**: this is *punished*, not *rejected*.
Conversion (§1) rejects lies because Solana can check Bitcoin. Here the
lie about Ethereum executes on Solana and is punished on Ethereum. The
upgrade that makes it *rejected* — a zk light client, §6.11 — replaces
one instruction and leaves everything else in this section unchanged.

### 6.0 Reading guide and end-to-end walkthrough

**The one-sentence version**: a user locks SOL on Solana and ETH on
Ethereum; a bonded operator writes "lock N exists" into a Bitcoin
transaction; Solana mints against that transaction; Ethereum checks the
same transaction against its own vault and slashes the operator if it
lied. Redeem runs the same way in reverse. Nothing ever asks one chain to
read another; everything either chain acts on is a Bitcoin fact or a local
fact.

**Who does what**

| Role | Holds | Does | Can lose |
|---|---|---|---|
| User X | SOL, ETH, BETA | `lock_sol`, `deposit`, `approve_pending`, `burn_redeem` | nothing to anyone else's misbehavior — only time |
| Operator | a Bitcoin UTXO (its *statement chain* head), SOL bond on A, ETH bond on B | anchors MINT / RELEASE / CANCEL; streams headers | its bonds, per lie, on the chain that owns the fact |
| Auditor (any number) | its own statement chain, SOL + ETH bonds | anchors VETO; submits other parties' anchors for a bounty | its bond for a false veto |
| Anyone | — | `process_anchor`/`processAnchor`, `exercise_mint`, `executeRelease`, `skip_anchor`, `relay-header` after 30 min | tx fees; earns bounties |
| Governance | one key per chain | sets numbers, approves operators, pause | never decides truth |
| Solana factory A | SOL reserve, BETA mint | judges facts about Solana; acts provisionally on MINT | — |
| Ethereum vault B | ETH reserve, insurance | judges facts about Ethereum; acts provisionally on RELEASE | — |
| Bitcoin | — | orders and publishes every statement; both chains read it natively | — |

**Sequence: mint 1 BETA**

```
 User X            Solana A               Bitcoin              Ethereum B            Operator
   |  lock_sol(nonce=1,1u) |                    |                    |                    |
   |---------------------->| pending[X,1]       |                    |                    |
   |  deposit(X,1,1u,D) ---------------------------------------------->| lock[N]=PENDING   |
   |  approve_pending(1,N) |                    |                    |                    |
   |---------------------->| pending.eth_lock=N |                    |                    |
   |                       |                    |   anchor A1 = spend(head) + OP_RETURN H(MINT{N,X,1,1,D})
   |                       |                    |<---------------------------------------------|
   |                       |                    |  (confirmed in block h)                 |
   |            process_anchor(MINT, A1, proof h)                     |                    |
   |  anyone ------------->| chain head := A1   |                    |                    |
   |                       | predicate(X,1,N) ✓ |                    |                    |
   |                       | status = Queued    |                    |                    |
   |            exercise_mint(A1)               |                    |                    |
   |  anyone ------------->| window ok → mint   |                    |                    |
   |<-- 1 BETA ------------| reserve += 1 SOL   |                    |                    |
   |            processAnchor(MINT, A1, proof h) ---------------------->| chain head := A1  |
   |  anyone                                                          | lock[N] matches ✓ |
   |                                                                  | lock[N] = FINAL   |
```

If the operator's statement had named a lock that does not exist, the
Solana column is identical up to the mint, and the Ethereum column ends
with `slash(eth_bond) → insurance; operator.dead`, after which any
auditor anchors a dead-veto that pauses A.

**Sequence: redeem 1 BETA**

```
 Holder Y           Solana A               Bitcoin              Ethereum B            Operator
   |  burn_redeem(1u, 0xY) |                    |                    |                    |
   |---------------------->| burn 1 BETA        |                    |                    |
   |<-- 1 SOL -------------| burn[id]={0xY,1u}  |                    |                    |
   |                       |                    |   anchor A2 = spend(A1:0) + OP_RETURN H(RELEASE{N,id,0xY,1})
   |                       |                    |<---------------------------------------------|
   |            processAnchor(RELEASE, A2) ---------------------------->| lock[N] FINAL ✓   |
   |  anyone                                                          | queue: pay after T_rel unless held
   |            process_anchor(RELEASE, A2)     |                    |                    |
   |  anyone ------------->| burn[id] exists ✓  |                    |                    |
   |                       | burn.claimed=true  |                    |                    |
   |            executeRelease(A2) after T_rel --------------------------->| pay 1 ETH → 0xY |
   |<-- 1 ETH ------------------------------------------------------------|                 |
```

If there was no burn, Solana's column ends with `slash(sol_bond)`, and
an auditor's VETO anchor (its own chain) holds the Ethereum payout in the
meantime.

**Timeline of one anchor** (typical numbers): user txs, seconds; anchor
broadcast to one Bitcoin confirmation, ~10 min; header relayed to both
chains, seconds after; `process` on each chain, seconds; a false MINT is
slashed on Ethereum in the same `process` call that would have finalized
it — the gap between "lie accepted on Solana" and "lie paid for on
Ethereum" is one Bitcoin block plus relay latency, which is what the
rate cap (§6.7) is sized against.

### 6.1 Components

```
SOLANA  (mint chain)                 BITCOIN                ETHEREUM  (reserve chain)
BetaFactory A                        statement bus          BetaVault B
  reserve_sol, BETA mint             (both chains read it   lock[N], insurance_eth
  pending[X]                          natively via their    operator: eth_bond, anchor_utxo
  operator: sol_bond, anchor_utxo     own header relays)    auditors[i]: eth_bond, anchor_utxo
  auditors[i]: sol_bond, anchor_utxo                        rate window, release queue
  rate window, mint queue
  paused
```

- **Operator** (one, for BETA stage): makes `MINT` and `RELEASE`
  statements. Registered on *both* chains with the same Bitcoin
  outpoint `anchor_utxo` and a bond on each: `eth_bond` in ETH on B,
  `sol_bond` in SOL on A.
- **Auditors** (any number, permissionless to join): make `VETO`
  statements. Registered on both chains the same way, each with its own
  `anchor_utxo` and bonds. Also the parties who *submit* other people's
  anchors for adjudication (bounty-paid; §6.6).
- **Users**: lock SOL on A, deposit ETH on B, co-sign their own mint.
- **Conversion (§1–3)** is the ramp — how a user with only SOL gets
  ETH onto Ethereum, or leaves — and is otherwise not involved.

### 6.2 Statement chains

Every registered party has a **statement chain**: a linear sequence of
Bitcoin transactions where each spends output 0 of the previous one.

Anchor transaction rules (checked identically on both chains):
- `input[0]` spends the party's currently registered `anchor_utxo`.
- `output[0]` is the new `anchor_utxo` (any script the party controls).
- `output[1]` is `OP_RETURN <ver:1> <kind:1> <H(statement):32>`.
- Anything else in the tx is ignored.

Properties that follow, with no signature verification anywhere:
- **Unforgeable authorship** — nobody else can spend the party's UTXO.
- **Unhideable** — the chain is public and linear; a party cannot show
  one chain an anchor and withhold it from the other, because *anyone*
  can submit any anchor to either chain.
- **Ordered** — each chain advances `anchor_utxo` only by processing
  anchors in sequence, so "the operator's history up to here" is a
  well-defined, identical object on both chains.
- **Reuses existing verifiers** — tx inclusion is `submit_proof_cache`
  / `submitBitcoinMerkleProofWithTx` machinery; output parsing already
  exists (`parse_output_at`); input outpoint parsing is a few lines.

The preimage of `H(statement)` is supplied by whoever submits the anchor
to a chain. A party that anchors a hash and never reveals the preimage
has stalled its own chain; after `T_skip` anyone may `skip_anchor`,
which advances the pointer with no effect (§6.6 keeps skipped anchors
auditable later).

### 6.3 Statements

```
MINT    { N, X, eth_amount, deadline D, sig_X }      -- "B.lock[N] = {X, eth_amount, D}, X consents"
RELEASE { N, to_eth_addr, burn_id }                  -- "A burned BETA under burn_id for lock N, pay to"
VETO    { target: party, anchor_txid, reason }       -- "that anchor is false; pause acting on it"
CANCEL  { N }                                        -- "MINT N will not be exercised; let B refund"
```

`sig_X` is user X's Solana signature over `{N, X, eth_amount, D}` — the
operator cannot mint *for* anyone who did not ask (§6.8).

### 6.4 Mint

1. **User, Solana**: `lock_sol(1 SOL)` → `pending[X]` with expiry `D`.
2. **User, Ethereum**: `deposit(X, D){1 ETH}` → `lock[N] = {X, 1 ETH, D,
   PENDING}`. Refundable by X after `D + margin` unless FINAL (§6.5).
3. **User, off-chain**: signs `sig_X`, hands it to the operator. Before
   signing, X has seen both of their own locks land — the Conversion
   pattern: the party at risk verifies before committing.
4. **Operator, Bitcoin**: anchors `MINT{N, X, 1 ETH, D, sig_X}`.
5. **Anyone, Solana**: `process_anchor(tx, proof, statement)`:
   - Bitcoin checks (§6.2); `now < D`; `sig_X` valid; `pending[X]`
     exists with matching amount and `D`; `!paused`.
   - Enqueue into the **mint queue** (§6.7). When capacity allows:
     mint 1 BETA → X, `pending[X] → reserve_sol`, `eth_claims += 1`.
   - A statement whose *Solana* predicate is false (`pending[X]`
     missing, bad `sig_X`) is a lie about Solana: slash `sol_bond`
     (§6.6) and advance the pointer with no mint.
6. **Anyone, Ethereum**: `process_anchor(tx, proof, statement)`:
   - Bitcoin checks; then compare to `lock[N]`.
   - Match, and anchor's Bitcoin block time `< D` → `lock[N].FINAL`.
   - Mismatch (no such lock, wrong X/amount/D, already FINAL) → **slash
     `eth_bond` → `insurance_eth`**, bounty to submitter, operator
     `dead` on B.

### 6.5 Redeem

1. **Holder, Solana**: `burn_redeem(1 BETA, to_eth_addr)` → burn; pay
   1 SOL from `reserve_sol` immediately; `eth_claims -= 1`; write
   `burn[burn_id] = {to_eth_addr}`.
2. **Operator, Bitcoin**: anchors `RELEASE{N, to_eth_addr, burn_id}`
   where `N` is any FINAL, unreleased lock.
3. **Anyone, Ethereum**: `process_anchor` → Bitcoin checks; `lock[N]`
   FINAL and unreleased → enqueue into the **release queue** with delay
   `T_rel`. After `T_rel` with no honored VETO: pay 1 ETH to
   `to_eth_addr`, mark RELEASED.
4. **Anyone, Solana**: `process_anchor` → Bitcoin checks; `burn[burn_id]`
   must exist with matching `to_eth_addr` and be unclaimed → mark
   claimed. **Otherwise it is a lie about Solana: slash `sol_bond`.**

Ethereum cannot see the burn. The release is therefore *delayed* and
*vetoable* (§6.7), and the lie is adjudicated on Solana where the burn
fact lives.

`lock[N]` refund: X may `refund(N)` after `D + margin` if not FINAL. If
FINAL, the ETH belongs to the pool. A MINT that was anchored but that
Solana refused (predicate false) is compensated to X from the slashed
`sol_bond` at the governance ratio (§6.7); the lock stays FINAL and its
ETH joins the pool — the only path where a price ratio touches funds,
and it has no attacker profit (an operator can only reach it by burning
its own `sol_bond`, and only against a user who signed `sig_X`).

### 6.6 Adjudication and slashing

The invariant: **a statement is judged only by the chain that owns the
fact it asserts.**

| Statement | Fact about | Judged on | Slashes | Blind chain's provisional action |
|---|---|---|---|---|
| MINT | Ethereum (`lock[N]`) *and* Solana (`pending[X]`, `sig_X`) | B for the Ethereum half, A for the Solana half | `eth_bond` on B / `sol_bond` on A | A mints (rate-limited) |
| RELEASE | Solana (`burn[id]`) | A | `sol_bond` | B pays after `T_rel` unless vetoed |
| VETO | whichever chain the vetoed anchor is judged on | that chain | vetoer's bond on that chain if the vetoed anchor was in fact true | the *other* chain pauses acting on the target |
| CANCEL | none | — | — | B lets X refund N |

Slash rules:
- Slashed `eth_bond` → `insurance_eth` inside B. B's solvency is
  `real locks + insurance_eth ≥ outstanding claims`; the fake units'
  future redemptions are paid from the slash. Same asset as the lie —
  no price needed.
- Slashed `sol_bond` → Solana insurance, used for §6.5's compensation
  path and for any SOL-side shortfall.
- Submitter bounty: a fixed fraction of any slash. Auditing is
  permissionless and paid; watchtowers are expected to be bots.
- A `dead` operator's anchors are still processed for *audit* (to slash
  further lies) but no longer *exercised* (no mints, no releases).
- `skip_anchor`ed anchors remain auditable: `audit_skipped(txid,
  preimage)` slashes if the revealed statement is false. A party cannot
  escape by withholding a preimage, because every *exercised* statement
  has its preimage on-chain by construction.

### 6.7 Bounding exposure without a human judge

The blind chain acts on lies before they are punished, so exposure per
adjudication latency must be bounded and funded.

- **Rate limit (mint queue)**: A exercises at most `R_mint` ETH of MINT
  per Bitcoin window `W`. Excess anchors wait in order; they are not
  rejected. Ethereum's slash for a false MINT lands within ~1
  confirmation of the anchor (permissionless headers, §6.9, plus one
  live auditor), so `eth_bond ≥ R_mint × k` for small `k` covers every
  fake that can be minted before the operator is `dead` on B.
- **Release delay + veto**: B pays a RELEASE only after `T_rel`. Any
  registered auditor may anchor `VETO{operator, anchor_txid}`; B honors
  a veto from a live, bonded auditor by holding that release. The veto
  is then judged **on Solana**: if `burn[id]` really existed, the
  vetoer's `sol_bond` is slashed and B may pay; if not, the operator's
  `sol_bond` is slashed and B cancels the release. `sol_bond ≥ R_rel ×
  k` for the fake-release exposure, where `R_rel` is B's per-window
  release cap. The SOL/ETH ratio for that sizing is a governance
  parameter — the one place a ratio is unavoidable, and it sizes a
  bond, never moves funds.
- **Post-slash pause without governance**: after B slashes the operator,
  A does not know. Any auditor anchors `VETO{operator, *}` (a "dead"
  veto); A honors it by pausing mints from that operator. The veto is
  judged **on Ethereum**: `operator.dead` is a local fact there. A false
  dead-veto slashes the vetoer's `eth_bond`. So the breaker is
  1-of-n honest-and-alive auditors, each with skin in the game, and a
  wrong pause costs the pauser.
- **Governance** keeps only: set `R_mint`, `R_rel`, `W`, `T_rel`,
  `T_skip`, `margin`, the SOL/ETH sizing ratio, auditor minimum bonds;
  and register/replace the operator. It never decides whether a
  statement was true.

### 6.8 User-side protections (independent of everything above)

- `sig_X` on every MINT: a lone operator cannot mint for an X who did
  not ask, and cannot reach the compensation path against anyone who
  did not sign.
- X signs only after seeing `pending[X]` and `lock[N]` land — both are
  X's own transactions on chains X can read.
- Disjoint windows: A mints only before `D`; B refunds only after `D +
  margin`, only if not FINAL; B finalizes only on an anchor whose
  Bitcoin block time is `< D`. An offline operator strands nobody.
- `burn_redeem` pays the SOL half instantly and locally; only the ETH
  half waits on the operator, and a lie about it is slashed on the
  chain that can see the burn.

### 6.9 Header relays: operator-first, permissionless fallback (built)

Before this change `ipow`'s `commit_global_header` and `iPoWV1`'s
`commitGlobalBitcoinHeader80` were operator-only, and — more importantly
for §6 — neither checked that a new header *linked* to the tip
(`prev_hash == tip.hash`) or kept the tip's `nBits` inside a difficulty
epoch. PoW was validated against each header's *own* bits, so any real
Bitcoin header from any height, or a header mined at `POW_LIMIT` (~2³²
hashes), would have passed. Harmless with a trusted operator; fatal for
any permissionless submitter.

Both relays now enforce, for **every** submitter including the operator:
- extending the tip (`height == tip + 1`) requires the tip header
  (Solana: the `prev_header` account; Ethereum: storage) and
  `prev_hash == tip.hash` (`PrevAndTipUnmatch`);
- inside an epoch, `nBits == tip.nBits` (Solana `BitsMismatch`, Ethereum
  `InvalidRetarget`); at an epoch boundary the existing retarget rule.

And they open the *extension* path only, to anyone, once the tip has
sat unextended for `PERMISSIONLESS_HEADER_DELAY` (30 min; Solana
`HeaderNotStale`, Ethereum `HeaderNotStale`). Anchoring a fresh relay and
jumping ahead stay operator-only. The operator keeps the fast path and
its own confirmation policy; it can no longer delay an audit by more than
30 minutes, which is what §6.6's slashing latency assumes.

**Deliberately not built: reorg handling.** Heights stay immutable on
both relays. A stale sibling block committed at the tip (by anyone, once
the fallback is open) leaves the relay stuck at that height with no
replace path. The 30-minute gate makes this a race against an absent
operator rather than a free DoS, and a real orphan at exactly that height
is rare, but it is a liveness hole. Closing it means either a bounded
tip-replacement rule that also invalidates proofs cached against the
replaced header, or a hash-keyed header DAG with best-tip-by-work — a
separate change to the base relay.

Operator service: `core/operator/.../svm_provider.rs` now passes the
tip header account (`prev_header`) when extending by one.

### 6.10 Attack table

| Attack | What happens |
|---|---|
| Operator anchors MINT for a lock that does not exist | A mints (within `R_mint`); B slashes `eth_bond` → insurance; operator `dead` on B; auditor dead-vetoes A; fake units redeem from insurance |
| Operator anchors MINT with wrong amount / D / X | same |
| Operator = user, colludes with itself | identical to above — self-collusion is just a false MINT and is slashed the same way; `sig_X` gains it nothing |
| Operator anchors MINT for an X who never asked (no valid `sig_X`) | A refuses to mint and slashes `sol_bond` (false Solana predicate). If X had actually deposited on B, B has finalized a lock that will never mint; the operator must anchor CANCEL so X can refund, else the slashed `sol_bond` compensates X (§6.5). No attacker profit on any branch |
| Operator anchors RELEASE with no burn | B queues; auditor vetoes; A judges: no `burn[id]` → operator `sol_bond` slashed; B cancels |
| Operator anchors RELEASE, no auditor alive for `T_rel` | B pays; A slashes `sol_bond` when the anchor is processed; loss bounded by `R_rel × k`, covered by `sol_bond` — the one path that depends on an auditor being alive for *safety*, hence the delay and the cap |
| Auditor vetoes a true RELEASE | A judges: burn exists → auditor's `sol_bond` slashed; release proceeds |
| Auditor dead-vetoes a live operator | B judges: not dead → auditor's `eth_bond` slashed; A resumes |
| Operator anchors on Bitcoin, submits to A, withholds from B | anyone submits to B; bounty |
| Operator withholds a preimage | cannot exercise; `skip_anchor` after `T_skip`; still auditable later |
| Third party tries to forge an anchor | cannot spend the party's `anchor_utxo` |
| Operator stalls headers | §6.9 — permissionless headers |
| Operator keeps minting after B slashed it, before the dead-veto lands | bounded by `R_mint` per window; funded by `eth_bond` |
| Operator lies faster than `R_mint` allows | it cannot — A exercises in order, capped |

### 6.11 Trust statement and upgrade path

What must hold for BETA to stay fully backed:
1. Ethereum's and Solana's own consensus (as for anything on them).
2. Bitcoin's PoW, as for Conversion.
3. `eth_bond ≥ R_mint × k` and `sol_bond ≥ R_rel × k` (economic).
4. At least one honest auditor alive within `T_rel` (existential, 1-of-n
   — for the release direction's safety and for the post-slash pause).

What is *not* assumed: that the operator is honest; that governance
judges anything; that any price feed is correct.

Upgrade: replace B's role in adjudicating MINT with a zk light client on
A (§C1 in the design discussion): `process_anchor` for MINT additionally
requires a proof of Ethereum finality + storage proof of `lock[N]`. The
lie then fails on A instead of being punished on B; `eth_bond`, the
mint queue and the dead-veto become unnecessary for that direction.
RELEASE follows when a Solana light client on Ethereum is feasible.
Nothing in §6.1–6.5's account or statement shapes changes.

### 6.12 Decisions taken in the build

- **OP_RETURN layout**: `6a 22 | ver=0x01 | kind | sha256(statement)` —
  36-byte script, output index 1, input 0 spends the party's registered
  outpoint. Witness-serialized anchors are rejected; the submitter strips.
- **MINT carries the Solana pending slot explicitly**: `{eth_lock_id,
  sol_user, nonce, units, deadline}`; `pending` is keyed `(user, nonce)`
  so a user can run several mints at once. §6.3's `sig_X` is an on-chain
  `approve_pending(nonce, eth_lock_id)` signed by X rather than an
  ed25519 signature carried inside the statement.
- **Party identity** is a 32-byte `party_id` registered on both chains,
  each chain mapping it to its local key/address. Operators need
  governance approval (Solana: governance co-signs `register_party`;
  Ethereum: `approveOperator`); auditors are permissionless.
- **Amounts** are whole-BETA `units`; each chain holds the composition
  (`sol_per_unit` lamports, `eth_gwei_per_unit`/`ethWeiPerUnit`).
- **Rate limit never rejects**: a MINT judged true on Solana is
  `Queued`, locks its pending slot against expiry, and `exercise_mint`
  runs when the window (clocked by `ipow`'s relay tip) has room.
- **Vetoes**: dead-veto → `paused_until` on the target operator
  (Solana), judged on Ethereum (`operator.dead`); veto on a queued MINT →
  `held_until` (Solana); veto on a queued RELEASE → `heldUntil` on
  Ethereum with a `vetoHoldFeeWei` taken from the auditor's Ethereum
  bond per renewal (so holding forever has a linear cost on the chain
  that can't judge it), while the truth is judged on Solana (`burn`
  exists → vetoer slashed; not → operator slashed, vetoer rewarded from
  insurance).
- **Finalize/skip windows**: Ethereum finalizes a MINT only if revealed
  within `tFinSecs` of the anchor's Bitcoin block time and before `D`;
  Solana `skip_anchor` needs the header to be `t_skip_secs` old, with
  `t_skip ≥ 2·tFin` enforced on Ethereum's params.
- **Compensation path** (false Solana predicate with a real Ethereum
  lock): slashed `sol_bond` pays the named user at
  `comp_lamports_per_unit` — the one place a SOL/ETH ratio touches funds,
  governance-set, no attacker profit.
- **Multi-operator** is already supported by shape (per-party chains and
  bonds; shared pool); only governance approval gates it.

### 6.12b Built after the live runs (2026-09-21, later the same day)

- **Stranded queued mint** (found by asking "does it always end normally
  for the user?"): a MINT judged true and queued on Solana whose operator
  is then retired could neither exercise (dead party) nor expire (queued)
  nor be re-anchored (queued ⇒ predicate false). Now `Pending.queued_by`
  records the queuing party; a live operator's MINT for the same slot is
  true if the submitter passes the retired party (`prior_party`, must be
  `dead`), which re-queues the slot under the new anchor (`exercise_mint`
  requires `queued_by == anchor.party`); and `expire_pending` cancels a
  queued slot once its queuing party is dead. Tests: one end-to-end case
  covering both paths.
- **Reward pool**: veto rewards on both chains come from a governance-
  funded pool (`fund_rewards` / `fundRewards`), never from insurance. The
  slash therefore fully backs the units the lie created (minus the
  submitter bounty). Sizing rule, final form:
  `bond ≥ cap × unit ÷ (1 − bounty_bps/10 000)` per chain; the reward pool
  is a separate, small, top-up-able budget.
- **Insurance releases** and the **auditor retirement rule** (§6.12).
- **Daemon** — `scripts/beta_daemon.ts`: `ROLE=watchtower` follows every
  registered party's statement chain on Bitcoin (esplora outspends from
  the on-chain head), relays headers, finds each anchor's statement
  (local `scripts/statements/` store, else reconstructed from chain state
  — Deposited events × pendings, burns × lock ids, parties × anchors),
  submits it to both chains via the two CLI drivers, exercises queued
  mints, executes due releases, and with `AUDITOR_ID` anchors VETOs and
  dead-vetoes where the chains' verdicts disagree. `ROLE=operator` watches
  Ethereum deposits and Solana burns and anchors MINT/RELEASE for
  `OPERATOR_ID` (insurance release when no FINAL lock fits). Broadcasts
  only with `BROADCAST=yes`; `ONCE=1` runs a single cycle.
- **Redeployed**: Sepolia `BetaVault` v2 `0xd06153e65e3e7f37cB7E5FfCE52Ed6F585571c43`
  (same relay `0xB8ab…1588`), devnet `beta_factory` upgraded in place
  (slot 501907644). Fresh parties on both chains: operator `0x03…03`
  (chain head `02f8cf39…:0`), auditor `0x04…04` (head `0f5e1d2c…:0`);
  reward pools funded (0.002 ETH / 0.05 SOL). The retired parties'
  bonds (0.0135 ETH in the v1 vault, 0.19 + 0.1 SOL on devnet) stay
  where they are — dead parties cannot withdraw, by design.

### 6.13 Not built yet

- **Operator service integration — done for the MINT path (2026-09-24),
  both EVM and Solana.** `scripts/beta_daemon.ts` (the standalone v2-era
  script this bullet originally pointed at) is removed — superseded, not
  folded in. `core/operator`'s own `beta_operator.rs`/`beta_registry.rs`
  now drive claim → relay → exercise directly: real BTC wallet, Redis
  chain-head tracking, and (for Solana specifically) the *existing*
  Conversion-role header streamer reused unmodified by pointing it at
  Beta's own `ipow` program id, mirroring the same trick already used for
  EVM's `iPoWV1`. Covers every EVM network with a `beta_networks.*` entry
  plus Solana's `beta-factory` (a new `SvmBetaHubAdapter`, same
  `BetaHubAdapter` trait EVM already implements). **Still not covered**:
  RELEASE/redeem statements, and the auditor/"watchtower" role
  (VETO/dead-veto/ALIVE) — `beta_daemon.ts` handled both; neither has a
  `core/operator` equivalent yet. Compiled and unit-tested; no live
  `processAnchor`/`process_anchor`/`exercise_mint` transaction has yet
  been sent by this new code on any network.
- **Relay reorg handling** (§6.9).
- **Batching**: several statements per anchor via a Merkle root in the
  OP_RETURN, once anchor volume justifies it.
- **Parameter values** for a deployment (`R_mint`, `R_rel`, windows,
  bonds, fees) and the zk audit path (§6.11).

### 6.14 Runbook: running it for real (devnet + Sepolia + Bitcoin mainnet)

The relays in this repo track **Bitcoin mainnet**; anchors are real
mainnet transactions (dust-sized, outputs back to the operator's own
wallet so nothing is unrecoverable). Tooling built for this:

- `programmable-network/solana/scripts/beta_factory_e2e.ts` — every
  factory action by `ACTION=…` (see its header), plus `relay-header`
  for `ipow`.
- `programmable-network/ethereum/ignition/modules/BetaVault.ts` — deploys
  a fresh `iPoWV1` (with §6.9) and `BetaVault` on Sepolia.
- `programmable-network/ethereum/scripts/beta_vault_e2e.mjs` — every
  vault action by `ACTION=…`, plus `relay-header`.
- `core/operator/crates/core/examples/anchor_statement.rs` — turns a
  statement (hex) into a signed anchor on the operator's chain; dry-run
  unless `ANCHOR_STATEMENT_CONFIRM=yes`.

One mint, end to end:

```
# deploy (once)
solana program deploy target/deploy/ipow.so --program-id target/deploy/ipow-keypair.json -u devnet
solana program deploy target/deploy/beta_factory.so --program-id target/deploy/beta_factory-keypair.json -u devnet
npx hardhat ignition deploy ignition/modules/BetaVault.ts --network sepolia

# configure (once): operator party P on both chains, chain head = a real UTXO the operator holds
ACTION=init npx ts-node scripts/beta_factory_e2e.ts
ACTION=register-operator PARTY_ID=$P ANCHOR_TXID=$UTXO_TXID ANCHOR_VOUT=$VOUT BOND_SOL=0.2 npx ts-node scripts/beta_factory_e2e.ts
ACTION=approve-operator PARTY_ID=$P node scripts/beta_vault_e2e.mjs
ACTION=register PARTY_ID=$P KIND=0 ANCHOR_TXID=$UTXO_TXID ANCHOR_VOUT=$VOUT BOND_ETH=0.01 node scripts/beta_vault_e2e.mjs

# user
ACTION=lock NONCE=1 UNITS=1 npx ts-node scripts/beta_factory_e2e.ts
ACTION=deposit SOL_USER=0x<wallet pubkey hex> NONCE=1 UNITS=1 node scripts/beta_vault_e2e.mjs   # → lockId N (deadline must match)
ACTION=approve NONCE=1 ETH_LOCK_ID=$N npx ts-node scripts/beta_factory_e2e.ts

# operator
ACTION=statement-mint NONCE=1 ETH_LOCK_ID=$N npx ts-node scripts/beta_factory_e2e.ts            # → STMT hex
cargo run --example anchor_statement -- $UTXO_TXID $VOUT 1 $STMT 294 70                          # dry run, then with ANCHOR_STATEMENT_CONFIRM=yes
# wait for 1 confirmation at height h; relay h to both chains (jump allowed while idle)
ACTION=relay-header HEIGHT=$h npx ts-node scripts/beta_factory_e2e.ts
ACTION=relay-header HEIGHT=$h node scripts/beta_vault_e2e.mjs

# anyone
ACTION=process PARTY_ID=$P TXID=$A1 STATEMENT=$STMT npx ts-node scripts/beta_factory_e2e.ts      # Queued
ACTION=exercise PARTY_ID=$P TXID=$A1 npx ts-node scripts/beta_factory_e2e.ts                     # BETA minted
ACTION=process PARTY_ID=$P TXID=$A1 STATEMENT=$STMT node scripts/beta_vault_e2e.mjs              # lock FINAL (or slash)
```

Redeem: `ACTION=burn` → `ACTION=statement-release` → anchor (kind 2,
spending `$A1:0`) → relay → `process` on both → `execute-release` after
`T_rel`.

Note the deposit's `deadline` must equal the Solana pending lock's
`deadline` byte-for-byte (the MINT statement carries one value both
chains check), so create the Ethereum lock with the deadline the Solana
lock printed.

### 6.15 Worked example with numbers

Test composition: 1 BETA = 0.01 SOL + 0.001 ETH. Operator bonds 0.2 SOL
on A and 0.01 ETH on B. Rate cap 100 units per 6-block window.

- Honest mint: X locks 0.01 SOL and 0.001 ETH; BETA supply 1; A reserve
  0.01 SOL; B `totalLocked` 0.001 ETH, `lock[N]` FINAL.
- Operator lies about 3 units (no locks on B): A mints 3 BETA (within
  cap); B's `processAnchor` finds no `lock[N]`, slashes
  `3 × 0.001 = 0.003 ETH` from the 0.01 ETH bond → 0.0027 ETH insurance
  + 0.0003 ETH bounty to whoever submitted; operator dead on B. B's
  solvency: `real locks (0) + insurance 0.0027 ≥ claims 3 × 0.001 =
  0.003`? No — 0.0027 < 0.003, because the bounty came out of the same
  slash. Sizing rule, therefore: `eth_bond ≥ cap × ethWeiPerUnit /
  (1 − bounty_bps/10 000)`, i.e. the bond must cover the cap *plus* the
  bounty. With cap 100 units at 0.001 ETH and 10 % bounty, that is
  `≥ 0.111 ETH`. (The test deployment's 0.01 ETH bond therefore pairs
  with a cap of 9 units, not 100 — set `MINT_CAP=9` on devnet.)
- False release of 1 unit: B queues a 0.001 ETH payout for `T_rel`; A
  slashes `1 × comp_lamports_per_unit = 0.01 SOL` from the SOL bond to
  insurance; an auditor's veto holds B's payout until it lapses and
  renews it while the lie stands (each renewal costs the auditor
  `vetoHoldFeeWei` on B and earns `veto_reward` on A).

## 7. BETA v3: optimistic settlement — per-action escrow, one-week challenge, attestations instead of timers

Supersedes §6's provisional-action rules (rate cap, `T_rel`, `T_pause`,
veto hold fees). Everything else in §6 — statement chains, "a claim is
judged only by the chain that owns the fact", slashing, insurance,
insurance releases, re-anchoring, header relays — stays exactly as is.

### 7.1 The change in one paragraph

Every provisional action (a mint on Solana, a payout on Ethereum) now
lives inside a **challenge window** `T_challenge` (one week). It executes
early only when a bonded party **attests** to it with a per-action
**escrow** in the acting chain's asset; otherwise it executes at the end
of the window if no **veto** stands. A veto has no clock — it stands
until a bonded **clear** contradicts it. Every attest, veto and clear is
a statement about the *other* chain's fact and is judged there like any
other statement: false ones are slashed, true vetoes are rewarded. At the
end of the window the acting chain settles on the standing claims: a held
action is cancelled and its attester's escrow is forfeited; an unheld one
is final and the escrow returns. Security model: **1-of-N honest checker
within a week**, with a cheater's damage bounded by its own escrow.

### 7.2 Statements

```
MINT    = 0x01 | eth_lock_id u64 | sol_user 32 | nonce u64 | units u64 | deadline i64    (65)
RELEASE = 0x02 | eth_lock_id u64 | burn_id u64 | to 20 | units u64                       (45)
VETO    = 0x03 | target_party_id 32 | target_txid_le 32  (all-zero txid = dead-veto)      (65)
CANCEL  = 0x04 | eth_lock_id u64                                                          (9)
ATTEST  = 0x05 | target_txid_le 32     "that MINT/RELEASE is true; I escrow for it"       (33)
CLEAR   = 0x06 | target_txid_le 32     "that MINT/RELEASE is true; lift the veto"         (33)
ALIVE   = 0x07 | target_party_id 32    "that operator is not dead on Ethereum; un-pause"  (33)
```

Who judges which (the chain owning the fact), and what the *other* chain
does with it provisionally:

| statement about | judged on | if false | if true | acting (blind) chain |
|---|---|---|---|---|
| MINT | Ethereum (`lock[N]`) | operator slashed, **and its attester** | FINAL | Solana: queue; exercise on ATTEST or at window end |
| RELEASE | Solana (`burn`) | operator slashed, **and its attester** | burn claimed | Ethereum: queue; pay on ATTEST (from escrow) or at window end (from vault) |
| ATTEST of X | where X is judged | attester slashed | — | acting chain: reserve escrow from attester's bond; act now |
| VETO of X | where X is judged | vetoer slashed | vetoer rewarded (pool) | acting chain: `held = true`, no expiry |
| CLEAR of X | where X is judged | clearer slashed | — | acting chain: `held = false` |
| dead-VETO | Ethereum (`dead`) | vetoer slashed | rewarded | Solana: operator paused, no expiry |
| ALIVE | Ethereum (`dead`) | claimer slashed | — | Solana: un-pause |

### 7.3 Mint (Solana acts, Ethereum judges)

1. `process_anchor(MINT)`: Solana predicate as §6.4 → `Queued`,
   `challenge_until = now + T_challenge`.
2. Fast path — `process_anchor(ATTEST)` by any bonded party (the operator
   itself, its own auditor identity, anyone): reserves
   `units × comp_lamports_per_unit` from the attester's bond into the
   anchor's `escrow`; `exercise_mint` is allowed immediately → user has
   BETA within minutes.
3. Slow path — nobody attests: `exercise_mint` allowed once
   `now ≥ challenge_until` and not held. Free, one week.
4. Challenge — `VETO` sets `held`; `CLEAR` lifts it. Both are judged on
   Ethereum against `lock[N]`.
5. `settle_mint` after `challenge_until`:
   - not held → escrow (if any) returns to the attester's bond; the mint
     is final.
   - held → if exercised, the escrow is **forfeited to insurance** (it
     backs the unit that should not exist, at the governance SOL/ETH
     ratio); if not yet exercised, the anchor is **cancelled** and the
     pending slot un-queued so the user can re-anchor or expire.

Ethereum's side: `processAnchor(MINT)` still finalizes or slashes at
once when submitted; a false MINT also slashes any party whose ATTEST
names it (whether the ATTEST was processed before or after).

### 7.4 Release (Ethereum acts, Solana judges)

1. `processAnchor(RELEASE)`: Ethereum predicate as §6.5 → `Queued`,
   `challengeUntil`.
2. Fast path — `processAnchor(ATTEST)`: reserves `units × ethWeiPerUnit`
   from the attester's ETH bond and **pays the user from it now**. The
   vault has paid nothing yet.
3. Slow path — `executeRelease` after `challengeUntil` if not held: pays
   from the vault (lock / insurance).
4. `VETO` holds, `CLEAR` lifts — judged on Solana against the burn.
5. `settleRelease` after `challengeUntil`:
   - not held → the vault reimburses the attester (lock released /
     insurance drawn); final.
   - held → reimbursement cancelled: the lock returns to FINAL (or the
     insurance reservation is released). **The vault never paid for a
     false release**; the attester paid its accomplice with its own money
     and is slashed on Solana besides.

### 7.5 What was removed and why

- Rate cap / mint window: exposure is per-action escrow, not a rate.
- `T_rel`, `T_pause`, veto hold fee and renewals: vetoes and pauses
  stand until contradicted by a bonded claim; time is only the window.
- Nothing about bonds' *existence* changed: bonds still pay for lies and
  serve as the escrow pool. Their *sizing* stops being a safety
  parameter — a party can only attest what it can escrow.

### 7.4a-bis EVM/SVM parity check (2026-09-22)

Prompted by "does the EVM and SVM already match 1:1" — verified by reading
both, not assumed. They don't fully match, and shouldn't in every place:

- **Intentionally asymmetric, both correct**: `approve_pending` has no
  Ethereum analog (Solana's two-step lock-then-approve vs Ethereum's
  one-call `deposit`); a Solana MINT-attest locks a real SOL escrow
  (Solana is blind, the escrow buys early release) while an Ethereum
  MINT-attest costs nothing (Ethereum judges MINT truth directly and
  instantly, nothing to gate early); Solana carries two dead v2 params
  for layout stability (in-place upgrades) that Ethereum's clean-redeploy
  `Params` struct never needed.
- **Confirmed gap, not yet built**: Ethereum has no
  `audit_skipped_release` equivalent — nothing lets an anchor Ethereum
  skipped ever be revisited if its preimage later surfaces. Left open.
- **Bug found and fixed**: `_processAttestOrClear`'s MINT branch set
  `mintAttester[target] = partyId` unconditionally on every call, so a
  second attester silently overwrote the first — only whoever attested
  *last* bore the fan-out slash if the mint proved false; every earlier
  attester walked away. Guarded to lock in only the first attester,
  matching Solana's `attested_by` semantics. New test:
  `only the first attester of an unresolved MINT is recorded…` (EVM suite
  now 115).

**Deployed the same day.** Solana `beta_factory` upgraded in place again
(slot 502471525 — needed a second `program extend` for the fee-mechanism
growth; a first deploy attempt failed mid-write leaving 5 stray buffer
accounts holding ~7.75 SOL, closed to reclaim rent before retrying with
`--with-compute-unit-price 1000`). All existing config/party/pending
state survived untouched, as expected from an in-place upgrade.
Ethereum: fresh `BetaVault` v4 (Sepolia can't upgrade in place) at
`0xDEF84990e07cBDd0Ac08D1343754D8BE833fE380`, same relay
`0xB8ab960D1121F33B48b4086aBFCDD8B750081588`; `tFinSecs`/`tSkipSecs`
carried forward at their raised values (4h / 8h). Operator `0x03…03`
(0.01 ETH bond) and auditor `0x04…04` (0.005 ETH bond) re-registered with
their existing Bitcoin chain heads; reward pool funded 0.003 ETH.
Deliberately **not** touched: v3 vault
`0x799e3B35ba0fCC8DB67Ceba453017d7C883eBcE8` still holds lock #2's
pending RELEASE — its payout completes there, independently, once its
own challenge window closes (~2026-09-29); redeploying doesn't
invalidate it, since it's a separate contract at a separate address.
Drivers/daemon updated to prefer v4 by default.

### 7.4b Acceleration fee (MINT side, built 2026-09-22)

An ATTEST buys the user *speed*, not correctness — the operator's bond
already covers correctness on every claim, attested or not (§7.6). Speed
needs its own incentive, separate from the escrow that's actually at risk
for a wrong claim. Design settled on: **the user who wants speed pays for
it directly**, not a governance subsidy — matching how real fast-
withdrawal systems price liquidity provision.

- `lock_sol` takes an optional `attest_fee`, transferred into a new
  `fees` PDA (`seeds = [b"fees"]`) alongside the SOL being locked;
  `Pending.attest_fee` records it.
- `process_anchor`'s `ATTEST`-of-`MINT` branch (non-redundant case) pays
  the fee to the attesting party's owner **immediately**, the same
  instant the escrow is reserved — not deferred to settle, since the fee
  compensates the service of accelerating (rendered now), independent of
  whether the claim later proves true (that risk is the escrow's job,
  forfeited separately at settle if wrong).
- `exercise_mint`'s slow-path branch (`attested_by` still empty) refunds
  any unspent `attest_fee` to the user — nobody earned it if nobody
  accelerated them.
- Redundant attests (§7.4a) return before touching the fee, so a second
  attest of an already-accelerated mint never double-pays.

Deliberately scoped to MINT only for now. On RELEASE the fee would
naturally be posted on Solana (at `burn_redeem`) while the attester's
real service is on Ethereum; the clean hook is Solana's own processing of
a RELEASE-ATTEST anchor (which already runs, currently a no-op on
success) — left as follow-up rather than built same-day. Tests:
`attest_fee_is_paid_to_the_attester_immediately_not_deferred_to_settle`,
`attest_fee_is_refunded_to_the_user_when_nobody_accelerates_the_mint`
(+2, suite now 25).

### 7.5b Built and deployed (2026-09-22)

- Solana `beta-factory` v3 upgraded in place on devnet (slot 502011808):
  statement kinds 5–7, `challenge_until`/`held`/`attested_by`/`escrow`/
  `settled` on `ProcessedAnchor` (new layout — older anchor accounts are
  history and skipped by tooling via a `dataSize` filter), persistent
  pause (`paused_until = i64::MAX` until ALIVE), `settle_mint`; `t_pause_
  secs` field re-read as `t_challenge_secs` (set to 7 days by governance).
  22 tests.
- Ethereum `BetaVault` v3 on Sepolia `0x799e3B35ba0fCC8DB67Ceba453017d7C883eBcE8`
  (same relay): `tChallengeSecs`, ATTEST pays from the attester's bond,
  `settleRelease`, MINT-attester fan-out slashing via `mintAttester`,
  MINT vetoes / ALIVE judged locally, no hold fees, no rate cap. 114 EVM
  tests. Parties `0x03…03` (operator, 0.004 ETH) and `0x04…04` (auditor,
  0.002 ETH) re-registered with the same chain heads; reward pool 0.001
  ETH. The v2 vault's live bonds were unbonded and withdrawn to fund it.
- Drivers: `settle` / `statement-target` / `statement-veto` / `set-params`
  (Solana), `settle-release` / `fund-rewards` / `withdraw-bond` (EVM).
  Daemon: attests verified mints/releases (fast path), vetoes lies, settles
  closed windows, self-attests the operator's own MINTs (`SELF_ATTEST=no`
  to disable), retries Esplora with a fallback host.
- **First live v3 mint (2026-09-22, block 968030).** Sepolia v3 lock #1
  (0.001 ETH, nonce 4) + devnet pending nonce 4 approved for it. Operator
  `0x03…03` anchored MINT `b019417d…7aa8` and, on its own chain right
  behind it, ATTEST `a00829ba…8097` (both RBF-bumped from a 0.47 sat/vB
  first attempt to 2 sat/vB after ~2 h unconfirmed — the wallet couldn't
  fund the 4 sat/vB "fastest" rate for both). Relays jumped to 968030
  (Sepolia needed the epoch-start header 967680 recorded first —
  `EpochFirstMissing` otherwise; jumping avoids ~117 sequential header
  txs, which the deployer could not have paid for). Devnet: MINT
  `Queued` → ATTEST reserved 0.01 SOL from the operator's bond (0.2 →
  0.19) → `exercise_mint` **immediately** (supply 3, `eth_claims` 3,
  reserve 0.03 SOL). Sepolia: MINT processed 38 min after its block
  (inside `tFin`) → lock #1 FINAL; ATTEST recorded `mintAttester =
  0x03…03`. Escrow settles back to the bond after the 7-day window
  (`ACTION=settle`).
  Lessons: (1) Esplora's `/tx/:id/status` returns `{"confirmed":false}`
  for *unknown* txids too — use `/tx/:id` (404) or `/outspend` to tell
  "in mempool" from "not broadcast"; the daemon's confirmation polls only
  trust `"confirmed":true`. (2) `tFin` = 1 h is tight when the anchor
  confirms during an outage; the daemon should relay+process the moment
  it sees a confirmation, and `tFin` deserves to be a few hours.
  (3) `relayTo` in the daemon extends header by header; it needs the
  jump path (with epoch-start pre-relay on the EVM) when far behind.
  → Done the same day: the daemon jumps when the gap exceeds `JUMP_GAP`
  (6) and no Conversion is open, pre-relaying the EVM epoch-start header;
  `tFin` raised to 4 h and `t_skip`/`tSkip` to 8 h on both chains.
- **First daemon-driven mint, and the first independent attester
  (2026-09-22, blocks 968040/968046).** Sepolia v3 lock #2 + devnet nonce
  5. The operator daemon found the match and anchored MINT
  `4079c614…841c` by itself (block 968040); its self-ATTEST could not be
  funded, so the mint waited. After a top-up the **auditor** loop saw
  "verified on Ethereum, unattested" and anchored ATTEST `5f901eca…fe22`
  from the auditor's chain — and then, every cycle until that anchor was
  confirmed and processed, three more identical ATTESTs (`1bfbc2b9…`,
  `ea6f2303…`, `61f509cb…`; 1,244 sats wasted). Fixes: the daemon keeps a
  statement-hash ledger and never anchors the same statement twice; the
  program treats a redundant ATTEST as a no-op that only advances the
  attester's chain (previously `AlreadyAttested` would have wedged it).
  With those in place the watchtower relayed 968041–968046 (extend),
  processed all four attests on both chains (first carries the escrow:
  auditor bond 0.1 → 0.09 SOL; the rest no-ops) and `exercise_mint`
  succeeded: **BETA #4 minted on an independent attester's escrow**
  (supply 4, `eth_claims` 4, reserve 0.04 SOL; Sepolia lock #2 FINAL,
  `mintAttester = 0x04…04`). Escrow returns to the auditor at settle
  (~2026-09-29). Also: Esplora's `/tx/:id/status` reports unknown txids as
  `{"confirmed":false}`; the daemon's "in mempool" checks now use
  `/tx/:id` (404) or `/outspend`.
- **Slow-path redeem in flight (2026-09-22, block 968053).** Burn #1 on
  devnet (1 BETA → 0.01 SOL back instantly); operator RELEASE
  `4971e4fa…ac90` (lock 2, burn 1, 300 sats), deliberately unattested.
  Relays *jumped* to 968053 (one tx per chain — first live use of the
  daemon's jump path). Devnet: burn #1 claimed, anchor Exercised.
  Sepolia: `QueuedRelease`, unheld, unpaid, `challengeUntil` 1790636424
  (2026-09-29 ~20:20 UTC) → `executeRelease` then pays 0.001 ETH from
  the vault with no attester ever involved; `settleRelease` is a no-op.
  Together with the two mint escrows settling ~09-28/29 this is the
  first live exercise of the window-close logic. Daemon spend caps
  (`MAX_FEE_SATS` 600, `MAX_ANCHORS_PER_HOUR` 4, `MAX_SATS_PER_HOUR`
  1500) were added after the duplicate-attest incident.

### 7.6 Trust statement (v3)

Honest users are never harmed and never wait on a clock (an attester
fronts). A cheating operator is punished by code if anyone submits its
statements to the judging chain within `T_challenge`; its damage is
bounded by its own escrow even if nobody does. Residual assumptions:
one honest checker per week, the SOL/ETH ratio used for mint escrows
(a week of price risk on units that should not exist), and the header
relays. The zk audit path still turns "caught" into "rejected" and
removes the first assumption; nothing here changes shape for it.

## 8. BETA v4: multi-network, multi-token composition (design)

Generalizes §6–§7 from a fixed "1 SOL + 1 ETH" pair to an arbitrary,
governance-registered basket: any number of components, each on any
reserve chain, any number of components per chain. Reuses Conversion's
own bundle pattern (docs/DESIGN_V2.md §1, `TokenAmount`), widened by one
field since a composition spans chains where Conversion's bundles never
leave one.

### 8.1 The one realization that keeps this simple

**A component behaves exactly like today's single ETH leg.** It has its
own lock on its own reserve chain, its own MINT-equivalent statement,
its own ATTEST/VETO/CLEAR/challenge-window, judged entirely by the chain
it lives on — nothing about §7's mechanism changes per component. The
only two new things are: a **composition registry** (what components
exist, and how much of each backs one unit), and **`exercise_mint` gates
on every component being confirmed, not just one**. Multi-token-on-one-
network falls out for free: two components with the same `network_id`.

### 8.2 Composition

```rust
pub struct Component {
    pub network_id: u64,     // 0 = Solana (local, no cross-chain judging needed)
    pub token_id: [u8; 32],  // 0 = native; else a Pubkey or zero-padded EVM address
    pub amount_per_unit: u64,
}
#[max_len(16)]  // MAX_NETWORKS(4) * MAX_TOKENS_PER_NETWORK(4) — §8.11
pub components: Vec<Component>,   // governance-registered per composition_id, same generic shape as Conversion's bundles
```

Every component with `network_id == 0` is a Solana-local leg: it's the
hub's own local lock, verified with zero cross-chain judging — exactly
what `lock_sol` already does today, generalized in §8.11 to allow more
than one (native SOL plus SPL tokens). Every other component is a
"remote leg," judged on its own chain's vault contract (a
`BetaVault`-shaped contract already deployed identically to
Ethereum/Polkadot/Hedera in this repo).

### 8.3 Generalized state (Solana hub)

```rust
pub struct Pending {
    user, nonce, units, deadline, composition_id,
    approved: bool,
    component_lock_id: [u64; 4],   // per-component lock id on that component's own chain
    component_final: [bool; 4],    // that component's own MINT-equivalent judged true
    queued, queued_by, attest_fee,
}
```

### 8.4 Statements generalize by one field

```
MINT = 0x01 | composition_id u64 | component_index u8 | lock_id u64 | sol_user 32 | nonce u64 | units u64 | deadline i64
```

One `MINT` anchor per non-local component — each anchored on that
component's own chain, judged there, exactly like §7.3 today. Solana's
`process_anchor` marks `component_final[component_index] = true` on a
true verdict; `ATTEST`/`VETO`/`CLEAR` all target one `(composition_id,
component_index)` pair and are otherwise unchanged from §7.2.

### 8.5 `exercise_mint` generalizes to "all components ready"

```
allowed = NOT held (on every component)
      AND for every registered component i:
            component_final[i]
        AND (attested_by[i] != empty OR now >= challenge_until[i])
```

Each component keeps its own independent escrow, challenge window, and
settle — a slow component doesn't block a fast one from being attested,
it only blocks the *overall* mint from exercising until it too is ready.

### 8.6 RELEASE and redeem generalize the same way

`burn_redeem` records one `Burn` per redeemed unit, same as today;
`RELEASE` becomes per-component too (`burn_id` + `component_index`), so
each reserve chain pays out its own component from its own vault,
independently, exactly as §7.4 does today for the single ETH leg.

### 8.7 What changes on each reserve-chain vault

Each `BetaVault`-shaped contract needs exactly one addition to support
multiple tokens on its own chain: **ERC20 support alongside native**,
mirroring Conversion's own native-or-SPL branch —
`token_id == 0` → `payable`/native as today; else `transferFrom`/
`transfer` against the given token address. `Lock`/`Params` gain a
`token_id` field; nothing else in the vault's judging logic changes,
since a token-denominated component is judged exactly like a native one.

### 8.8 What this doesn't change

The trust statement (§7.6) is unchanged per component — still optimistic,
still bounded by that component's own escrow, still "judged where the
fact lives." A basket spanning N chains is exactly N independent
instances of the same mechanism, gated together by the hub. No new
trust is introduced by adding components; the composition registry is
the only new governance surface.

### 8.9 Build order

1. Solana hub: composition registry, generalized `Pending`/
   `ProcessedAnchor`, per-component MINT/ATTEST/VETO/CLEAR, generalized
   `exercise_mint`/`settle_mint`. (Provable with the vaults already
   deployed — e.g. a composition of `[SOL, ETH, USDC-on-Ethereum]`
   already exercises both axes: multi-network via SOL+ETH, multi-token-
   per-network via ETH+USDC.)
2. ERC20 support on `BetaVault.sol` (§8.7) — ethereum/polkadot/hedera all
   get it for free, since they already deploy the same contract.
3. Redeem side (§8.6), symmetric to mint.
4. Deploy a `BetaVault` to Polkadot/Hedera to prove true 3+-network
   compositions live (the contract is already identical there; this is
   configuration, not new code).

### 8.10 Built and tested (Solana hub, 2026-09-23)

§8.9 step 1 is done: `beta-factory`'s Solana side is generalized and
`cargo test -p beta-factory` passes (26/26), plus a workspace-wide
`cargo check` regression pass. Actual shapes differ slightly from §8.2's
sketch (a `Vec` per composition/pending rather than fixed `[T; 4]`
arrays, and a single `queued_by: [u8; 32]` rather than a `component_final`
bool array) but the semantics match:

- `Composition { id, components: Vec<Component> }` (governance-only
  `register_composition`, `#[max_len(4)]`, exactly one `network_id == 0`
  component required).
- `Pending` gained `composition_id`, `remote_lock_id: Vec<u64>`,
  `remote_anchor_txid: Vec<[u8; 32]>` (one slot per non-local component,
  in composition order), and `queued_by: [u8; 32]` (replacing the old
  single `queued` bool — "one basket, one carrier": every remote
  component of one mint must be anchored by the same operator, so a
  takeover resets every cached txid at once rather than splicing partial
  progress across operators).
- `ProcessedAnchor` (MINT) gained `composition_id: u64` and
  `component_index: u8`; `eth_lock_id` renamed `lock_id` everywhere
  (MINT/RELEASE/CANCEL) since it's no longer Ethereum-specific.
- `exercise_mint` now takes 0–3 optional remote `ProcessedAnchor`
  accounts (`remote_0/1/2`, `MAX_COMPONENTS - 1`) and requires every one
  `pending.remote_count()` names to be `Queued`, unheld, and (attested or
  past its own challenge window) before minting — proven with a real
  three-network composition test (`[SOL, network 1, network 2]`): exercise
  refuses with only one remote leg ready and succeeds once both are,
  independently attested on their own simulated chains.
- Acceleration fee (§7.4b) generalizes as "paid to the first
  non-redundant ATTEST of any one component" rather than the harder
  "last component to become ready" (that would need checking every
  sibling's readiness from inside a single component's own ATTEST,
  which isn't built this pass) — refunded at `exercise_mint` only if
  nobody ever attested anything.
- RELEASE/CANCEL statements are unchanged beyond the field rename —
  redeem-side generalization (§8.6) is still design-only.

**Not done, deliberately deferred**: §8.9 steps 2–4 (ERC20 support on
`BetaVault.sol`, RELEASE/redeem-side generalization, Polkadot/Hedera
deployment) and, therefore, not deployed live — the current Solana
program at `3GUPbVjjBRNEYafHprb2fnh6Wzyif9a4gWphSrhkPVEf` (devnet) still
speaks the pre-§8 statement/account shapes; this section is built and
tested locally only.

### 8.11 Full symmetric revision: Solana may hold more than one local token too (2026-09-23)

§8.10 required exactly one `network_id == 0` component per composition
— Solana could verify only a single local asset directly. This revision
makes Solana symmetric with every remote network: up to
`MAX_TOKENS_PER_NETWORK` (4) tokens on any one network, up to
`MAX_NETWORKS` (4) distinct networks, so `MAX_COMPONENTS` rises from 4
to 16 (`4 × 4`). `register_composition` now requires at least one local
leg (not exactly one) and rejects any network exceeding the per-network
token cap, any composition exceeding the network cap, or two components
naming the same `(network_id, token_id)` pair.

Built and tested (`cargo test -p beta-factory`, 28/28):

- **New SPL-token lock/unlock machinery on the hub** — the missing piece
  §8.10 didn't need, since it only ever had one native SOL local leg.
  `lock_sol` and `expire_pending` each gained three typed optional
  account triples (`local_spl_0/1/2`, each `{mint, user_ata, vault_ata}`)
  covering `MAX_TOKENS_PER_NETWORK - 1` non-native local legs. At lock
  time, each SPL leg's vault ATA (owned by a new `token_vault_authority`
  PDA) is created idempotently and the user's tokens transferred in,
  alongside the existing native-SOL transfer for whichever component (if
  any) has `token_id == 0`; at expire time the same ATAs refund back to
  the user, PDA-signed. `exercise_mint` needed no equivalent change —
  local funds are already secured atomically by `lock_sol`'s own
  transfers, so exercising never moves them again, only the global
  native-lamport tallies (`pending_lamports`/`reserve_lamports`), which
  now key specifically off the one possible native-SOL component rather
  than "whichever local component happens to exist." An SPL local leg's
  reserve is simply its own vault ATA balance — no separate global
  tally, unlike native SOL's `reserve_lamports`.
- **A composition with zero remote legs needs no operator at all** — a
  gap this revision made reachable (previously every composition had
  exactly one remote leg in practice): if `pending.remote_count() == 0`,
  `exercise_mint` skips the operator/party gate entirely (its `party`
  account is now `Option`) and mints as soon as the lock is approved,
  since every leg was already verified directly by `lock_sol`'s own
  transfers — nothing remains to relay across Bitcoin's statement bus.
- Proven end to end with a composition locking native SOL *and* a test
  SPL token as two local legs in one `lock_sol` call, exercised
  permissionlessly with no `party`/`remote_N` accounts at all, and a
  matching `expire_pending` test confirming both legs refund in full
  (including the PDA-signed SPL-side transfer, a code path distinct from
  the user-signed lock-time transfer).

**Still not done**: the reserve-chain side of the same symmetry (a
remote network locking more than one token needs the ERC20 support
already deferred in §8.9 step 2); Token-2022 mint support (only classic
SPL Token local legs are handled); and, as before, redeem-side
generalization and additional-network deployment.

### 8.12 Flat budget replaces the 4×4 grid; `exercise_mint` widened to match (2026-09-23)

§8.11's `MAX_NETWORKS(4) × MAX_TOKENS_PER_NETWORK(4)` grid treated every
network as needing the same per-network cap, and — found while reviewing
it afterward — `exercise_mint` was never actually widened to match: it
only ever declared three named `remote_0/1/2` accounts, so a composition
with a fourth remote component could be *registered* but would panic
(array index out of bounds) the moment anyone tried to *exercise* it.
Registration's ceiling and execution's real capability had drifted apart.

This revision replaces the grid with one shared budget,
`MAX_COMPONENTS = 8`, spent however a recipe likes — 8 networks at one
token each, one network at several and the rest skipped, any mix —
except local (Solana) legs, which keep their own smaller sub-budget,
`MAX_LOCAL_COMPONENTS = 4`. That asymmetry is real, not arbitrary: a
local leg costs a CPI transfer inside `lock_sol`/`expire_pending`, while
a remote leg costs only a cheap account read inside `exercise_mint` —
they don't share a transaction's account budget with each other, since
they're checked in different instructions, but each instruction still
needs to survive its own worst case. `register_composition` now checks
`1 <= components.len() <= 8` and `1 <= local_count <= 4`, with no
separate per-remote-network sub-cap at all.

`exercise_mint` gained `remote_3` through `remote_6` (seven total,
`MAX_REMOTE_COMPONENTS = MAX_COMPONENTS - 1`), closing the gap this
section opened with. `lock_sol`/`expire_pending` needed no change —
`local_spl_0/1/2` already covered `MAX_LOCAL_COMPONENTS - 1`.

Proven with a composition spending the *entire* budget on seven
distinct remote networks (1 local + 7 remote = 8): exercise refuses
until all seven are independently anchored and attested, then succeeds
and marks all seven `Exercised`. `cargo test -p beta-factory`: 29/29.

### 8.13 ERC20 support on the Ethereum vault (§8.9 step 2, 2026-09-23)

The reserve-chain half of §8.11/§8.12's symmetry: `BetaVault.sol` can
now hold more than native ETH, mirroring `lock_sol`'s SPL-token support.
This contract judges only its own leg — it never needed a composition
registry of its own, just to parse past the new fields:

- **MINT statement parsing updated to the current 74-byte encoding**
  (`kind | compositionId u64 | componentIndex u8 | lockId u64 | solUser
  32 | nonce u64 | units u64 | deadline i64`, DESIGN_V2 §8.4) — overdue
  regardless of ERC20, since the Solana hub's own MINT statement already
  changed shape in §8.10 and the deployed vault would otherwise misparse
  it. `compositionId`/`componentIndex` are Solana-hub routing this
  contract doesn't judge; it parses past them and keeps matching
  `lockId` exactly as before.
- **`deposit(token, solUser, nonce, units, deadline)`** — `token ==
  address(0)` is native ETH exactly as before; any other address must be
  governance-registered first via `setTokenParams(token, {amountPerUnit,
  slashWeiPerUnit})`. `amountPerUnit` is what a depositor pays and a
  refund pays back, in the token's own smallest unit; `slashWeiPerUnit`
  is separate because bonds are always native ETH, so even a lie about
  an ERC20 lock is punished in wei. ERC20 transfers go through the same
  before/after-balance `_pullToken` pattern `iPoWV1Conversion` already
  uses, so a fee-on-transfer token can't overstate what really arrived.
- **`Lock` gained `token` and a frozen `amount`** — the exact amount
  actually pulled in at deposit time, so a refund always pays back
  exactly that, never a live-recomputed rate; a governance rate change
  or deregistration after the fact can never strand or shortchange an
  already-locked depositor. `totalLocked` became a per-token mapping.
- **RELEASE stays native-ETH-only, deliberately, and fails closed**:
  `_processRelease` now reverts `ReleaseTokenUnsupported` for any lock
  whose `token != address(0)`, rather than silently paying out the wrong
  asset. This matches Solana's own state — §8.6's redeem-side
  generalization is still design-only there too — so an ERC20-backed
  unit can mint but has no release path through this contract yet.

Built and tested (`npx hardhat test`, 23/23 in `BetaVault.test.ts`, 120/120
across the whole Ethereum suite): a registered ERC20 lock finalizes on a
true MINT and pulls exactly `amountPerUnit * units`; an unfinalized one
refunds back in the same token with no ETH movement beyond the caller's
own gas; a false MINT against an ERC20 lock slashes the operator in wei
at the registered rate, leaving the lock's own token balance untouched;
and RELEASE against an ERC20-backed lock reverts while a native lock's
RELEASE path is unaffected. Deployed to Sepolia as `BetaVault` v5 (below)
and, per §8.14, to five more networks.

### 8.14 `BetaVault` deployed across the programmable-network structure (2026-09-23)

Every network gets its own `programmable-network/<name>/` package —
own `package.json`, `hardhat.config.ts`, `contracts/`, `ignition/
modules/`, matching the existing `hedera/`/`polkadot/` convention —
rather than bolting extra network entries onto the `ethereum/` package's
config (a first pass did exactly that by mistake; reverted once caught).
Hedera and Polkadot already had an `iPoWV1` relay deployed from earlier
work, so only `BetaVault` needed adding there; four brand-new networks
(Base, Robinhood Chain, Tempo, Hyperliquid) had neither, so each needed
`iPoWV1` deployed fresh too, extending the iPoW protocol network ID
registry: 1=Hedera, 2=Ethereum, 3=Solana (reserved), 4=Polkadot,
**5=Base, 6=Robinhood Chain, 7=Tempo, 8=Hyperliquid (HyperEVM)**. Every
new `BetaVault` also got a `MockERC20` test token deployed alongside it,
purely to exercise §8.13 — not part of the protocol.

**Live and verified (real deposit/lock or equivalent smoke-test, not
just "the deploy transaction succeeded"):**

| Network | Package | `iPoWV1` | `BetaVault` |
|---|---|---|---|
| Hedera Testnet | `hedera/` | `0x36D7…96CF3` (pre-existing) | `0xba35203CD4389A66926e2280C66F7DD0646DBF35` |
| Polkadot Hub TestNet | `polkadot/` | `0x2dD2…88aDb` (pre-existing) | `0x4caE61D96FbF49eC853Ad91853Eb2F4b669D04CC` |
| Base Sepolia | `base/` | `0xB7054E399E31A2cFE181c4fD59C7235562a6d45d` | `0x6354779b4Dbb564c712ea91c179eCF521C15BE73` |
| Robinhood Chain testnet | `robinhood/` | `0x53e1291BdAff473694BbbB8DD257f9844e5f9F3c` | `0x35e564d74B90a3A5bfcA8Dec65b1325C83d2e822` |

Hedera's `BetaVault` deploy uses tinybar-scaled (8-decimal) params, not
the usual 18-decimal weibar convention `msg.value` reads as everywhere
else — see `hedera/ignition/modules/BetaVault.ts`'s own comment; getting
this wrong means every `deposit()`/`registerParty()` reverts.

**Both networks' blockers are now resolved (§8.15 covers Tempo's in
detail, §8.16/§8.17 cover Hyperliquid's):**

- **Hyperliquid testnet (HyperEVM, chain ID 998)**: resolved — see §8.16
  and §8.17. Both `iPoWV1` and `BetaVault` are live there via a
  router+facets split, not the plain monolithic contracts.
- **Tempo testnet (Moderato, chain ID 42431)**: resolved — see §8.15's
  update. `iPoWV1` `0x53e1291bdaff473694bbbb8dd257f9844e5f9f3c`,
  `BetaVault` `0xb7054e399e31a2cfe181c4fd59c7235562a6d45d`, `MockERC20`
  `0xe8780640839860f9049132606d18c916956a44a5` — all bytecode- and
  state-verified, not just receipt-trusted.

### 8.15 Tempo (Moderato): what's different, and getting `BetaVault` genuinely deployed (2026-09-23)

Tempo is not a standard-gas EVM chain, and naive Hardhat/ethers deploys
fail against it for real architectural reasons, not bugs:

- **Fee-token transactions.** Every transaction carries a `feeToken` and
  is paid by a `feePayer` (a sponsor), via a Tempo-specific transaction
  type (`0x76`) — confirmed directly from a transaction receipt's
  `feeToken`/`feePayer` fields. Plain EIP-1559 `maxFeePerGas`/
  `maxPriorityFeePerGas` transactions don't fit this model; Tempo's
  public testnet sponsor endpoint (`sponsor.moderato.tempo.xyz`, no API
  key) covers gas so no real funds are needed at all.
- **Two-dimensional nonces.** Nonce key `0` is the standard sequential
  "protocol nonce"; keys `1+` are independent, concurrently-usable user
  lanes; key `maxUint256` is a special "expiring" one-shot lane that
  `viem`'s Tempo client silently defaults to whenever `feePayer: true`
  is set with no explicit `nonceKey` (`node_modules/viem/tempo/
  chainConfig.ts`'s `useExpiringNonce` logic).
- **A real, confirmed bug: contract-creation addresses ignore the nonce
  lane entirely**, deriving only from the raw nonce number. Proven with
  a completely fresh, never-used lane (`nonceKey=99, nonce=0`): it still
  reported the exact address (and bytecode) of the *first* contract ever
  created by that sender, whatever lane that used. Since any sender's
  first successful create typically lands on nonce 0 in some lane, every
  later create that also lands on nonce 0 in a *different* lane silently
  collides — the transaction reports `status: success` but deploys
  nothing, and the already-resident bytecode is untouched.
  `iPoWV1`'s deploy (via the default expiring lane) was this sender's
  first create and succeeded; both attempts at `BetaVault` (once via the
  same expiring lane, once via an explicit lane at nonce 0) collided
  with it and silently no-opped, confirmed by checking deployed bytecode
  length at the reported address rather than trusting the receipt.
- **An unexplained anomaly**, not yet root-caused: in one run, a
  transaction built with `MockERC20`'s bytecode ended up with a receipt
  whose on-chain code matches `BetaVault`'s size instead. Recorded here
  rather than guessed at.

**Resolved (2026-09-23), by working around the bug rather than fixing
it** (it's a genuine Tempo chain-level issue, not something this repo
controls) — the rule that emerged from getting both `BetaHub` (§8.19)
and now `BetaVault` genuinely deployed here: **never trust a Tempo
deploy script's reported address.** Always independently derive
candidate `CREATE` addresses for the sender's nearby raw nonces
(`ethers.getCreateAddress({from, nonce})`, a handful in each direction)
and check which one actually holds code of the expected deployed-
bytecode length — then confirm it's genuinely well-formed by reading
real contract state (`governance()`/`ipowHeaders()` matching what was
just deployed, not just a length match, since a coincidental same-
length collision with something unrelated is possible in principle).
Re-running `tempo/scripts/deploy_viem.mjs` (unchanged from the attempt
that surfaced the original bug) this way confirmed `BetaVault` and
`MockERC20` both genuinely landed and are correctly configured — see
the addresses in this section's own "Live and verified" table above.
The nonce-lane bug itself remains unfixed and is still worth reporting
to Tempo's team (their Moderato testnet is only months old); the
"unexplained anomaly" above was never reproduced again and may simply
have been an earlier instance of this same address-misattribution
pattern. This same verify-don't-trust discipline applied to every
Tempo-targeted deploy this session; the raw-script experiments
themselves live in `tempo/scripts/deploy_raw.mjs` (the
failed plain-ethers attempt, kept for the record) and
`tempo/scripts/deploy_viem.mjs` (the viem/Tempo-native script, still
worth treating every future deploy through it with the same
verify-don't-trust discipline).

### 8.16 Hyperliquid's real blocker, and the router+facets split that fixes it (2026-09-23)

HyperEVM testnet's block gas limit is 3,000,000 — a hard capacity
constraint, confirmed directly against the RPC
(`eth_estimateGas` without fee fields, since Hyperliquid's own gas token
is HYPE not ETH): plain `iPoWV1` alone needs ≈4,792,000 gas to deploy,
driven by its deployed bytecode size (EIP-170's code-deposit cost, ~200
gas/byte, dominates large-contract deployment). No amount of funding
fixes this — it's a per-transaction ceiling, not a balance problem.
Splitting bytecode storage across multiple transactions doesn't help
either: a single atomic `CREATE` still needs its full code-deposit cost
paid in one transaction. The only real fix is shrinking what gets
deployed as one contract.

**The fix, on this network only: a Diamond-style (EIP-2535-inspired)
router+facets split**, built with the same security bar as the rest of
this repo — the user's explicit question going in was "not compromise
security right?":

- A shared storage base contract (`contracts/base/iPoWV1Storage.sol`)
  that every facet and the router inherit identically, guaranteeing
  identical Solidity storage-slot assignment — critical, since facets
  only ever execute via `delegatecall` against the router's own storage.
  `NATIVE_DECIMALS`/`SELF_NETWORK_ID` moved from `immutable` to regular
  storage here: immutables are baked into a contract's own bytecode at
  construction and don't propagate through `delegatecall`, so each
  facet would otherwise read its own meaningless zero value.
- Three facets (`iPoWV1AdminFacet`, `iPoWV1ConversionEntryFacet`,
  `iPoWV1ConversionSettlementFacet`) — split along genuinely
  non-overlapping helper dependencies (verified by listing every
  function's exact internal-helper call graph before splitting), not an
  arbitrary line count. A cross-facet reach-in (the header relay's retry
  loop, which needs to call into the Settlement facet's
  `_tryFinalizeProof`) goes through an external self-call via
  `address(this)` — the router's own dispatch — since an `internal`
  Solidity function can't cross a `delegatecall` boundary between
  separately-deployed contracts; this needed one new external wrapper,
  `tryFinalizeProof`, on the Settlement facet.
- **`iPoWV1Router` is deliberately immutable**: all three facet
  addresses are set once in the constructor: no admin function exists to
  swap one later. This closes off the standard security criticism of
  general-purpose (mutable) Diamond patterns — a compromised or
  malicious upgrade swapping in bad logic — entirely, at the cost of
  never being able to fix a facet bug in place (a genuine tradeoff,
  accepted deliberately).
- Dispatch is by explicit `bytes4` selector comparison (one `if` per
  known selector, not a mapping/storage-backed table) — cheaper, and
  every entry is individually visible for review rather than hidden
  behind a constructor-populated mapping.
- `BetaVault`'s own dependency on this relay
  (`globalTipHeight`/`globalHeightToHashLE`/`globalHeaders`) is
  satisfied directly by the router's own inherited public getters — no
  delegatecall needed for reads, since those are plain public state
  variables on the router itself.

**Verification.** The entire existing 83-test suite (originally written
against plain `iPoWV1`) runs unchanged against the split by attaching
the *monolithic* `IPoWV1` ABI to the *router's* address
(`IPoWV1__factory.connect(routerAddress, signer)` — the standard
proxy/diamond testing technique: logic contract's interface, proxy's
address) via an `IPOW_SPLIT_DEPLOY=1` env-var toggle in
`test/helpers/deploy.ts`. All 83 pass identically in both modes. Three
new tests in `test/iPoWV1.RouterSplit.test.ts` target the one piece of
logic that's genuinely new (not a byte-for-byte copy of the monolithic
contract's own code) — the cross-facet `tryFinalizeProof` call path —
checking it routes correctly, is safe to call on a nonexistent proof,
and correctly leaves a not-yet-arrived proof pending rather than
mishandling it. (A full "a real header arrives and auto-finalizes an
already-pending proof" integration test isn't included: it would need a
second genuinely-mined valid-PoW Bitcoin header chained onto genesis,
and this suite deliberately only ever uses the real, independently-
verified genesis header — a gap that's pre-existing for the monolithic
contract too, not introduced by the split.)

**Deployed gas costs** (all comfortably under the 3,000,000 limit, vs
plain `iPoWV1`'s ≈4.79M):

| Contract | Deployed bytecode | Deploy gas |
|---|---|---|
| `iPoWV1AdminFacet` | 9,331 bytes | ≈1.87M |
| `iPoWV1ConversionEntryFacet` | 9,096 bytes | ≈1.82M |
| `iPoWV1ConversionSettlementFacet` | 8,454 bytes | ≈1.69M |
| `iPoWV1Router` | 3,074 bytes | ≈0.61M |

**Live on HyperEVM testnet (chain ID 998), deployment ID
`hyperliquid-router`:**

| Contract | Address |
|---|---|
| `iPoWV1AdminFacet` | `0x53e1291BdAff473694BbbB8DD257f9844e5f9F3c` |
| `iPoWV1ConversionEntryFacet` | `0xF43DF008d31995690C75982937a368545953564A` |
| `iPoWV1ConversionSettlementFacet` | `0x35e564d74B90a3A5bfcA8Dec65b1325C83d2e822` |
| `iPoWV1Router` | `0xB7054E399E31A2cFE181c4fD59C7235562a6d45d` |

Every other network keeps the plain, unmodified `iPoWV1.sol` — this
split exists only in `programmable-network/hyperliquid/`.

### 8.17 `BetaVault` hits the same wall — the same split, applied again (2026-09-23)

Deploying `BetaVault` against the freshly-live `iPoWV1Router` (§8.16)
hit the identical blocker: plain `BetaVault` needs ≈4.0M gas to deploy
(deployed bytecode 16,921 bytes; code-deposit cost alone — 200 gas/byte
— is ≈3.38M, already over the 3,000,000 limit before any constructor
execution or calldata cost is added). Confirmed the same way as §8.16:
direct `eth_estimateGas` against the real deploy transaction. Optimizer
tuning doesn't meaningfully help here either — `runs: 1` (vs the
project's default `runs: 200`) only shrank deployed bytecode from
16,921 to 16,827 bytes, nowhere near enough.

**The same router+facets pattern, applied to `BetaVault`:**

- `contracts/base/BetaVaultStorage.sol` — every error, type, event,
  state variable (`ipowHeaders` changed from `immutable` to regular
  storage, same reason as §8.16), and, unlike the `iPoWV1` split, every
  shared *internal helper function* too (`_validate`, `_pay`,
  `_slashRateFor`, `_pullToken`, `_payOut`, `_refund`, `_drawVault`,
  `_slash`, `_verifyAndAdvance`, `_readVarInt`, `_parseAnchor`,
  `_processMint`, `_processRelease`, `_processVeto`,
  `_processAttestOrClear`). Putting shared helpers directly in the
  common base (rather than hand-duplicating one, as §8.16 did for a
  couple of small `iPoWV1` helpers) relies on solc's dead-code
  elimination (enabled here via `viaIR: true`): a facet's compiled
  bytecode only includes the inherited internal functions it actually
  calls, so this costs nothing per-facet, is provably DRY (a single
  source of truth, not two copies that could silently drift), and needs
  no cross-facet external self-calls at all — every helper is directly
  available, by inheritance, to whichever facet needs it.
- Two facets, split along the same "genuinely non-overlapping" logic
  used for `iPoWV1`: `BetaVaultCoreFacet` (governance, party
  registration/bonding, deposit/refund, the v3 slow-path release
  settlement functions `executeRelease`/`settleRelease`, reward
  funding) and `BetaVaultAnchorFacet` (`processAnchor`, `skipAnchor`,
  and everything the statement-chain anchor pipeline needs). All
  reads/writes still land on the router's own storage regardless of
  which facet executed them, so `executeRelease` reading an anchor that
  `processAnchor` wrote — across what are, for gas-budget purposes, two
  different facets — needs no cross-facet call at all.
- `BetaVaultRouter` — same immutability decision as `iPoWV1Router`:
  `coreFacet`/`anchorFacet` addresses fixed at construction, no upgrade
  path. Its `receive()` (plain ETH transfers topping up `insurance`) is
  implemented directly on the router rather than dispatched: `receive`
  carries no calldata to dispatch on a selector, so this needed real
  logic on the router itself, not a `delegatecall`.

**Verification**, same rigor as §8.16: `ethereum/test/BetaVault.test.ts`
(the full existing behavioral suite for plain `BetaVault` — locks,
MINT/RELEASE/VETO/CANCEL/ATTEST/CLEAR/ALIVE anchor judging, the v3
challenge window, and §8.12's ERC20 local-leg support) ported into this
package as `test/BetaVault.test.ts`, deployed through a new
`deployBetaVault` helper toggled by `BETAVAULT_SPLIT_DEPLOY=1` (same
pattern as `IPOW_SPLIT_DEPLOY`, attaching the monolithic `BetaVault` ABI
to the router's address). All 109 tests in this package (the 83 `iPoWV1`
tests + 3 `RouterSplit` tests + 23 ported `BetaVault` tests) pass
identically under all four combinations of the two toggles — plain/
plain, split/plain, plain/split, and split/split (the actual live
configuration) — confirming the two splits are independent and
compose safely.

**Deployed gas costs** (all comfortably under the 3,000,000 limit, vs
plain `BetaVault`'s ≈4.0M):

| Contract | Deployed bytecode | Deploy gas |
|---|---|---|
| `BetaVaultCoreFacet` | 6,938 bytes | ≈1.61M |
| `BetaVaultAnchorFacet` | 12,149 bytes | ≈2.75M |
| `BetaVaultRouter` | 2,598 bytes | ≈0.89M |

**Live on HyperEVM testnet (chain ID 998), deployment ID
`hyperliquid-betavault-router`, `ipowHeaders` pointed at §8.16's
`iPoWV1Router`:**

| Contract | Address |
|---|---|
| `BetaVaultCoreFacet` | `0x24765955eCbffAaACB48a8c3747975d6C750C075` |
| `BetaVaultAnchorFacet` | `0x6354779b4Dbb564c712ea91c179eCF521C15BE73` |
| `BetaVaultRouter` | `0x554Fe13e4a5d0931e7c8F7d3E74Dea8Ca04C244a` |
| `MockERC20` (test token, not part of the protocol) | `0x806Ae4940f1a68e5371D27f9C8cb9528872aeBd1` |

Post-deploy sanity check against the live router (plain read calls,
routed through `fallback()` to `BetaVaultCoreFacet`'s inherited
getters): `governance()` = the operator wallet, `ipowHeaders()` = the
`iPoWV1Router` address above, `paused()` = false, `params().
ethWeiPerUnit` = the deployed value, `nextLockId()` = 1 — all as
expected for a fresh deployment. Every other network keeps the plain,
unmodified `BetaVault.sol`.

### 8.18 `BetaHub` — an EVM chain acting as a mint hub, not just a spoke (2026-09-23)

Every EVM chain (via `BetaVault.sol`) has only ever played "spoke": it
locks value and judges its own leg's MINT claim, but never mints
anything itself — only Solana's `beta-factory` can mint BETA.
`BetaHub.sol` (new, `programmable-network/ethereum/contracts/`) lets an
EVM chain also act as a hub: mint its own token, backed by a
governance-registered composition of legs on that chain (local,
permissionless) and on other EVM chains (remote, judged the same way
`BetaVault.sol` already judges its own leg).

**Decided scope.** Per-chain tokens for now, not one global fungible
supply — each hub mints its own independent ERC20 (`HubToken`, deployed
as `"iBETA"` for this pilot — not `"Beta"/"BETA"`, already
`BetaToken.sol`'s name for the unrelated `BetaMint` mechanism), with the
explicit intent that a future bridging layer could unify them later.
Solana-as-a-remote-leg is out of scope (would need a new Solana "spoke"
program mirroring `BetaVault.sol`'s judging logic) —
`registerComposition` fails closed on `networkId == 3` (Solana).
Pilot network: `programmable-network/ethereum/` (no block-gas-limit
constraint, unlike Hyperliquid's §8.16/§8.17 situation).

**No new cross-chain trust primitive.** Verified directly in
`beta-factory/src/process_anchor.rs`'s `Statement::Mint` branch: a
remote leg's finality is never proven cross-chain — each chain
independently re-verifies Bitcoin-inclusion of the same operator-posted,
Bitcoin-anchored statement via its own local header relay, and judges
only its own local bookkeeping. Trust is bonding + a challenge window,
not a cross-chain message. This means `BetaVault.sol` already works
unmodified as a remote leg for `BetaHub` — `Lock.solUser` is an opaque
`bytes32`, never compared except byte-for-byte, so a deposit destined
for an EVM hub just passes `solUser = bytes32(uint256(uint160(hubUser)))`
instead of a Solana pubkey.

**Structure** (all new files, nothing existing touched):
`contracts/libraries/AnchorChainLib.sol` (pure Bitcoin-tx byte-parsing,
shared — extracted from `BetaVault.sol`'s own parsing, which stays
unrefactored since it's already deployed on six live networks),
`contracts/base/HubPartyRegistry.sol` (bonded party/bond lifecycle,
deliberately a fresh hub-local registry, not shared with any
`BetaVault`'s own `parties` and not Solana's global pool — every
`BetaVault` deployment already has its own independent registry; this
keeps that per-deployment trust isolation for hubs too),
`contracts/base/HubCompositionRegistry.sol` (governance-only,
immutable-per-id `Composition`/`Component` registry — `MAX_COMPONENTS
=8`/`MAX_LOCAL_COMPONENTS=4`, mirroring Solana; `Component.amountPerUnit`
is the only rate table needed, no separate `setTokenParams`-style
registry, since registration is already governance-gated and
immutable), `contracts/base/HubAnchorJudge.sol` (statement-chain
judging — `KIND_VETO`/`ATTEST`/`CLEAR`/`ALIVE` reused near-verbatim from
`BetaVault.sol`, retargeted at a queued MINT *component* instead of a
queued *release*; `KIND_RELEASE`/`CANCEL` unimplemented, fail-closed,
matching Solana's own §8.6 design-only redeem generalization),
`contracts/BetaHub.sol` (final contract: `Pending` lifecycle —
`lockLocal`/`approvePending`/`exerciseMint`/`settleMint`/`expirePending`
— plus the one genuinely new predicate, `_processRemoteMint`, translated
field-for-field from `process_anchor.rs`'s verified `Statement::Mint`
branch), `contracts/HubToken.sol` (mirrors `BetaToken.sol`'s shape:
immutable minter, no admin, but with `name_`/`symbol_` constructor
params instead of a hardcoded pair).

**Two real corrections found during implementation** (verify, don't
assume, even against an already-approved plan): (1) `BetaVault.sol`'s
actual `KIND_MINT`-targeting ATTEST/CLEAR branch has no escrow or
fast-payout at all (only its `KIND_RELEASE` branch does) — a MINT
component's payout (the mint itself) only ever happens collectively at
`exerciseMint`, so there's nothing to fast-pay per component; `settleMint`
correspondingly does nothing for a clean (never-held) component, matching
`BetaVault.settleRelease`'s own silent-no-op behavior in that case. (2)
An early draft let `skipAnchor`'s caller supply `tSkipSecs` directly as
an argument — a real bug (anyone could pass `0` and skip instantly,
defeating the wait entirely); fixed by moving it into governance-set
`Params` like every other timing constant, read via a virtual hook
matching the rest of the contract's pattern.

**Verification.** `test/BetaHub.test.ts` (new, reusing
`test/BetaVault.test.ts`'s chain-agnostic scaffolding — the Bitcoin
statement-chain tx builders and `iPoWV1` storage cheat-codes need zero
changes) covers: composition registry validation, local-only
compositions minting with no operator at all, native+ERC20 local-leg
locking and refund-on-expiry (blocked while claimed by a live operator,
allowed once retired), the false-MINT predicate slashing to the named
victim (not insurance — the genuine divergence from `BetaVault._slash`,
which has no identifiable victim to name), operator takeover after
retirement resetting every leg, `exerciseMint`'s incomplete/not-ready
gating and exact mint amount, an ATTESTed component unblocking
`exerciseMint` early, VETO holding and CLEAR releasing a queued
component, `settleMint` un-queuing a held-and-never-cleared component,
and the true/false `KIND_ALIVE` cases. The end-to-end test deploys two
independent `BetaVault` instances as remote spokes and mints through
both statement chains — proving the "EVM chain as hub, backed by other
EVM chains as spokes" story concretely, not just in the individual-unit
sense. All 134 tests in the package pass (120 pre-existing + 14 new),
confirming nothing existing was touched. `BetaHub`'s deployed bytecode
is 19,118 bytes (77.8% of EIP-170's 24,576-byte cap) — no block-gas
constraint on this network either.

**Live on Sepolia** (deployment ID `BetaHubModule`, reusing the same
`iPoWV1` relay every `BetaVault` version on this network has,
`0xB8ab960D1121F33B48b4086aBFCDD8B750081588`): `BetaHub`
`0x4222DE34E0160F63cb5a614a766B96f0b5a34D23`, its `HubToken` ("iBETA")
`0x20FdA397DA9107662bC465db05e179f16251A46A`. Post-deploy sanity check
against the live contract (plain reads): `governance()` = the operator
wallet, `ipowHeaders()` = the relay address above, `SELF_NETWORK_ID()` =
2 (Ethereum), `paused()` = false, `token()` → a `HubToken` whose
`minter()` correctly resolves back to `BetaHub`'s own address — all as
expected for a fresh deployment. No composition registered yet, no
mint exercised — deploying the contract is as far as this pass goes;
registering a real composition and running a real multi-chain mint
through it is follow-up work.

### 8.19 `BetaHub` rolled out to every EVM network (2026-09-23)

Same `BetaHub`/`HubToken` contracts as §8.18, copied into every other
EVM package in this repo (`hedera/`, `polkadot/`, `base/`, `robinhood/`,
`tempo/`, `hyperliquid/`) and deployed against each network's own
already-live `iPoWV1` relay — no existing contract touched on any of
them. Hedera's params are tinybar-scaled (8 decimals), matching its
`BetaVault` deploy's own convention (`msg.value` inside executing
contract code is tinybar-scaled on Hedera's HSCS); every other network
uses the standard 18-decimal weibar convention.

**Hyperliquid needed the same router+facets split as §8.16/§8.17,
applied a third time.** `BetaHub`'s deployed bytecode (19,118 bytes,
~3.82M gas of code-deposit cost alone) exceeds HyperEVM testnet's
3,000,000 block gas limit the same way `iPoWV1` and `BetaVault` did.
New Hyperliquid-only files: `contracts/base/BetaHubStorage.sol` (unlike
the reference `HubPartyRegistry`/`HubCompositionRegistry`/
`HubAnchorJudge` abstract-base chain — built for single-contract
inheritance, not delegatecall — this flattens everything into one
shared storage/logic layer, since a facet split needs one identical
storage layout every facet and the router inherit, and drops the
virtual-hook indirection those abstract bases used, reading `params.xyz`
directly instead), `contracts/facets/BetaHubGovernanceFacet.sol`
(governance/party/bond/composition registry),
`contracts/facets/BetaHubAnchorFacet.sol` (statement-chain judging —
the biggest facet, 11,776 bytes/≈2.64M gas, ~12% margin under the
limit), `contracts/facets/BetaHubMintFacet.sol` (the `Pending` mint
lifecycle), `contracts/BetaHubRouter.sol` (immutable dispatcher, same
pattern as the other two routers). Verified via the same proxy-
attachment technique and `BETAHUB_SPLIT_DEPLOY=1` toggle as the prior
two splits: all 123 tests in the package (109 pre-existing + 14
`BetaHub` tests) pass identically under every combination of the three
split toggles, including all three at once — the exact configuration
now live.

**Tempo needed the same viem-native, fee-sponsored deploy path already
built for `iPoWV1`/`BetaVault` there** (`scripts/deploy_viem_betahub.mjs`,
extending `scripts/deploy_viem.mjs`'s pattern) — plain Hardhat/ethers
still can't speak Tempo's transaction model. **A new, more concerning
variant of the previously-documented nonce-lane address-derivation bug
surfaced here**: the deploy transaction's own receipt reported the
wrong `contractAddress` (nonce 2's address — already occupied by an
unrelated, earlier `MockERC20` deploy, whose code never actually
changed) while `BetaHub` had genuinely landed at nonce 6's address
instead. Caught by refusing to trust the receipt and instead
independently deriving `ethers.getCreateAddress({from, nonce})` for a
range of nearby nonces and checking which one actually held code of the
expected length — then confirming via live reads
(`governance()`/`ipowHeaders()`/`SELF_NETWORK_ID()`, and the deployed
`HubToken`'s `minter()` correctly resolving back to it) that the
contract is genuinely well-formed at that address. Not fixed (a Tempo
chain-level issue, same as the original bug) — the deploy script's own
header comment now warns never to trust its reported address on this
chain.

Robinhood Chain testnet's RPC endpoint
(`rpc.testnet.chain.robinhood.com`) briefly failed TLS handshakes
outright (`SSL_ERROR_SYSCALL` immediately after ClientHello) — the same
class of interference diagnosed earlier this session (an ISP-level
content filter, not a real endpoint problem) — but cleared on retry;
deployed normally once connectivity returned.

**Live deployments:**

| Network | `BetaHub` (or router) | `HubToken` ("iBETA") |
|---|---|---|
| Ethereum Sepolia | `0x4222DE34E0160F63cb5a614a766B96f0b5a34D23` | `0x20FdA397DA9107662bC465db05e179f16251A46A` |
| Hedera Testnet | `0x3cDba300797351664dEE4D5F0D3F7Be28e186bdA` | `0x09698f483149189f8A59e165f718fB339131eF6C` |
| Polkadot Hub TestNet | `0x754FC037B3d2aBf0b63A57a549F21755442a9a16` | `0xC0208386aCBFA9a18F20a7D76E3C8D71154C135b` |
| Base Sepolia | `0x24765955eCbffAaACB48a8c3747975d6C750C075` | `0xc52610b59E5eF7d8460dA06EA79DeeE7c62ebb83` |
| Tempo (Moderato) | `0x24765955eCbffAaACB48a8c3747975d6C750C075` (verified live state, not the misreported receipt address — see above) | `0xc52610b59E5eF7d8460dA06EA79DeeE7c62ebb83` |
| Hyperliquid (HyperEVM testnet) | Router `0x4C5769e3213496a0641E139e2F0E94ce7625374C` (facets: Governance `0x900D54050f9Fe47ca56cC67A281947Da2EeE2cD1`, Anchor `0x8b9efF66C7D93816Eadf702cC6cE273e8B2b03e7`, Mint `0xC40003975B6ff70E46999Bac8f3b06a9908Fb333`) | `0xa5aA9d972659A9cB6F425cb3B3f1E117a6289775` |
| Robinhood Chain testnet | `0xB7054E399E31A2cFE181c4fD59C7235562a6d45d` | `0x8928104EC0Df63203749BC5bfD61f71527B04f0b` |

No composition registered and no mint exercised on any of these yet —
every deployment above is verified only via constructor-state reads
(`governance`/`ipowHeaders`/`SELF_NETWORK_ID`/`token`/`paused`, and on
Tempo, `HubToken.minter()` resolving back correctly). Registering a
real cross-network composition and exercising a real mint through it is
follow-up work, same as noted for the Sepolia pilot in §8.18.

### 8.20 First real 5-network BETA mint, using genuine Bitcoin (2026-09-23)

The follow-up work §8.19 flagged — a real composition, exercised for
real — is done, using the *original* Solana `beta-factory` hub (not any
of the new `BetaHub` deployments), with legs on Solana itself (local)
plus Ethereum, Base, Robinhood Chain, and Hyperliquid (remote) — Tempo
excluded (no `BetaVault` deployed there — §8.15/§8.19) and Hedera/
Polkadot excluded per the user's own scoping. Every anchor is a real,
mined Bitcoin mainnet transaction, not a self-mined low-difficulty
header or a test cheat-code.

**Composition id=2** registered on the live composition-capable
`beta_factory` (`BkNb9JfNbfbn3rMiA3j3z2pnFdNibxiYWWsKZ9VdzoD1` —
composition id=1 there is an earlier scratch run with placeholder
network ids, left untouched, since a composition is immutable once
registered): local leg = a fresh devnet SPL token
(`9GLcTUA6FY2waGqVQJkmnYJNC4FESQcS7JpDubiUbEpc`), remote legs =
Ethereum (2), Base (5), Robinhood Chain (6), Hyperliquid (8) — 5
components total, 1 unit.

**A real Bitcoin operational lesson, caught before any harm**: every
statement was originally chained into *one* linear sequence (each
transaction spending the previous one's output) for speed, on the
assumption any system watching the chain could "catch up" through it.
That's true for Solana's hub (which does process every link in order,
by design — one party, one basket) but **not** for each EVM spoke's own
`BetaVault`: its `Party.anchorTxidLE` pointer can only ever advance by
one link at a time from wherever it was registered, and it has no way
to skip past links meant for other chains. Only the first EVM leg in
the chain (Ethereum) could process directly; Base/Robinhood/Hyperliquid's
`processAnchor` calls reverted with `NotOnStatementChain` — caught
immediately (a revert is atomic, no partial state, no funds moved) and
understood before retrying. Fixed by registering a second party
(`0x06…`) on each of those three chains specifically, pointed at the
correct intermediate outpoint their own statement actually spends — safe
because each `BetaVault` deployment's `Party` registry is entirely
local storage, independent of every other chain's (and of Solana's own
copy, which kept using `0x05` throughout, since it alone needed to walk
the whole chain). A real design confirmation, not just a bug: a shared
multi-spoke statement chain needs either one Bitcoin transaction per
spoke branching from a common ancestor, or (as done here after the
fix) per-spoke party identities layered onto one physical chain — not
one linear sequence assumed to work for every watcher.

**Bitcoin mechanics, confirmed for real**: 8 total real transactions (4
`Statement::Mint`, one per remote leg, plus 4 self-`Statement::Attest`
— broadcast in the same chained batch so everything could confirm
together rather than needing a second wait cycle for the challenge-
window fast path) all landed in the same real block, height 968203.
Confirmed a real, previously-undocumented Tempo-unrelated wrinkle:
Solana's own `process_anchor` needs roughly 1.4M compute units against
a real block (the default 200k budget under-runs), and a `processAnchor`
carrying a full 12-level Merkle branch (384 bytes) plus a raw Bitcoin
transaction exceeds Solana's 1232-byte transaction limit as a legacy
transaction — fixed with a `ComputeBudgetProgram` instruction and a v0
transaction using an Address Lookup Table for the 9 accounts identical
across all 8 calls.

**Result**: `exercise_mint` minted **1 BETA** to the operator wallet
(`G8vkKtPzQfm8dTVnFcgFis5LgRHqwCk2bT29731sXU6D`), backed by 1 real
locked SPL token unit (Solana) + 4 real `Final` locks (Ethereum lockId
2, Base/Robinhood/Hyperliquid lockId 1 each) — every leg independently
verified via its own chain's own state (lock `state == 2`/`Final`,
`pending` account closed after exercise, SPL vault balance decreased by
exactly 1 unit). The Bitcoin anchor transactions themselves are public
and independently checkable on any block explorer at height 968203.

### 8.21 EVM-hub, Tempo-spoke BETA mint, and Tempo's real value model (2026-09-23)

A second real cross-chain mint, this time using an EVM chain as the hub
instead of Solana: Ethereum Sepolia's `BetaHub`
(`0x4222DE34E0160F63cb5a614a766B96f0b5a34D23`) as hub, Tempo's
`BetaVault` as the sole remote leg — a 2-network composition (1 local
Ethereum leg + 1 remote Tempo leg), anchored by a second real,
independently mined Bitcoin transaction chain (not the §8.20 chain).

**A genuine, previously-unknown chain-level constraint surfaced mid-flight:**
Tempo's custom `0x76` transaction type unconditionally rejects any
transaction carrying native `value` — confirmed via two independent
tests (a plain value transfer, and `registerParty{value: bond}` on the
already-deployed native-value `BetaVault`), both reverting identically
with `"Revm error: value transfer in Tempo Transaction not allowed"`.
Tempo has no native-ETH-style value at all; every real value movement
on Tempo goes through its actual fee/settlement ERC20, **PathUSD**
(`0x20c0000000000000000000000000000000000000`, 6 decimals) — confirmed
as the chain's real `feeToken` (visible in `eth_getTransactionByHash`)
and via direct `name()`/`symbol()`/`decimals()`/`balanceOf()` reads.

This is not fixable by changing how a script calls the existing
contract — it required a new, Tempo-only contract variant,
**`BetaVaultPathUSD.sol`** (`programmable-network/tempo/contracts/`),
converting `BetaVault.sol`'s bond/payout mechanics from native value to
PathUSD ERC20 transfers (`registerParty`/`topUpBond`/`fundRewards` pull
via `safeTransferFrom` instead of `payable`; `_pay` uses
`safeTransfer` instead of a native call; no `receive()`). All
anchor-judging logic (`processAnchor`, merkle verification, veto/attest
handling) is byte-identical to `BetaVault.sol` — only the token-movement
primitives changed, mirroring the precedent already set by Hyperliquid's
gas-driven router/facet split: a network-specific variant where the
network's own real constraints force it, not a redesign of the shared
logic. Deployed and independently verified (never trust a Tempo
receipt's reported address — scanned nearby raw nonces for real
deployed bytecode) at
**`0x8b9efF66C7D93816Eadf702cC6cE273e8B2b03e7`**.

**Flow**: local `lockLocal` + `approvePending` on Ethereum's `BetaHub`;
`registerParty`/`deposit` (PathUSD) on Tempo's `BetaVaultPathUSD`; a
real Bitcoin statement chain (MINT + ATTEST) mined and broadcast, then
independently merkle-verified and committed via each chain's own
header relay; `processAnchor` on both Tempo (MINT) and Ethereum
(ATTEST, then MINT) confirmed the anchors — Tempo lock `state == 2`
(Final) verified directly. Because the MINT anchor's operator had
already ATTESTed, `BetaHub.exerciseMint` bypassed the 7-day challenge
window (per its own `attested || block.timestamp >= challengeUntil`
gate) and minted immediately, without waiting for 2026-09-30.

**Result**: `exerciseMint` minted **1.0 iBETA**
(`0x20FdA397DA9107662bC465db05e179f16251A46A`) to
`0x9784B80BC7b95753f2a2732649F3a1136b4c2BF1`
(tx `0x588dac8a04911820c991b80e44d3261ec0dfdb12a6148780aba19ee4f8cc7f6a`,
111,927 gas) — independently verified via `token.balanceOf` (1.0),
`pending` slot deleted, and the anchor's status reading back
`Exercised`. This is the first BETA mint backed by a chain with no
native value transfer at all, settled entirely through a real ERC20
stablecoin rather than native ETH-equivalent value.

## 9. Decoupling cross-chain conversion from locking, and full EVM/SVM parity (2026-09-24)

### 9.1 Abandoned: CPI from Beta into Conversion, as the funding mechanism for a lock

Two separate eras of the same idea — fund a `BetaHub`/`BetaVault` lock by
having a contract drive `iPoWV1Conversion`/`ipow-conversion` via CPI, so a
user only ever touches the mint chain — were built and then removed:

- **The original same-chain/basket design** (§4/§5 above): `BetaMint.sol`
  (mint funded directly through a bitcoin→native `iPoWV1Conversion`
  auction) and `BasketBridge.sol` (a two-hop Solana↔Ethereum basket using
  the same CPI). Already superseded by the statement-chain v2/v3/v4
  design (`BetaVault`/`BetaHub`/`beta-factory`) before this session
  started; their Solana-side counterpart (`beta-mint`, a genuinely
  separate program from `beta-factory`) was left running but unused.
- **This session's own attempt**: `BetaCompositionFundingIntent.sol` (the
  mint-chain leg) + `BetaVaultConversionConnector.sol` (a same-chain
  connector funding one remote leg via real Bitcoin) + a Rust "fan-out
  relayer" (`funding_relay.rs`) meant to settle every leg of a
  composition from one Bitcoin transaction, permissionlessly.

Stress-testing the second design surfaced two real fund-safety gaps in
`iPoWV1Conversion.sol` itself, not just the new connector contracts:

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
  stays the user's own responsibility, using `iPoWV1Conversion`'s
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
system — never used Conversion CPI at all) and `iPoWV1Conversion.sol`
itself (a standalone, independently-useful base primitive, not something
built for this experiment).

### 9.2 `iPoWV1Conversion` rolled out to every EVM network, and cross-registered

Before this pass, the new permissionless-auction `iPoWV1Conversion.sol`
only existed on Ethereum and Hyperliquid (the latter via the same
router+facets split `iPoWV1`/`BetaHub`/`BetaVault` already needed
there). Deployed fresh to Hedera, Polkadot, Base, and Robinhood — each
gets its own dedicated `iPoWV1` header-relay instance too, deliberately
separate from that network's own Beta `iPoWV1` (same reasoning as
§8.19's own header-source split), mirroring Ethereum's own precedent of
not reusing Beta's instance.

**Ethereum's own `iPoWV1Conversion` was also redeployed**, not reused:
the original instance (`0x9637cc…`) turned out to predate
`addNetwork`/`networkConfigs`/`openBundleTunnel` entirely — confirmed by
comparing deployed bytecode length (10,641 bytes) against the current
source's (13,918 bytes), not by assumption. It had 5 real committed
conversions (4 already `Completed`, 1 small stuck `Committed` one — the
already-known "never claimed" gap from §9.1, not new risk), so nothing
of value was lost redeploying. Ignition's own deployment journal got
stuck mid-redeploy on a phantom transaction record (a tooling issue, not
a chain issue — confirmed via independent nonce/balance checks showing
nothing had actually been broadcast) and was resolved with an isolated
`--deployment-id` rather than `--reset` (which would have touched every
other tracked module's state for the chain, not just this one).

**Every one of the 7 EVM `iPoWV1Conversion` deployments now has
`addNetwork` called for all 6 *other* networks** (`minAddrLen =
maxAddrLen = 20`, since every leg so far is EVM-address-shaped) — before
this, `openBundleTunnel` would have reverted `IncorrectNetwork` for
almost every real cross-network call, on every deployment including
Ethereum's, since (with the exception of two stale Hedera/Polkadot
entries on the old Ethereum instance) nothing had ever called
`addNetwork` at all. The base auction flow (commit/claim/proof) never
needed this registry — only the bundle-tunnel path does.

### 9.3 Tempo gets its own `iPoWV1Conversion` variant, and a `BetaHub` that actually works

Tempo's hub role had been left unconfigured (§8.21 only wired it as a
spoke) because the plain `BetaHub.sol` uses native value for bonds the
same way `BetaVault.sol` originally did, which Tempo hard-rejects.
Confirmed structurally, not just theorized: `registerParty` requires
`msg.value >= minOperatorBond`, and since Tempo rejects any
nonzero-`value` transaction outright, no party could ever be registered
on Tempo's plain `BetaHub` — a live on-chain read confirmed zero parties
existed there.

**`BetaHubPathUSD.sol`** fixes this via inheritance rather than full
duplication (unlike `BetaVaultPathUSD.sol`, which had to duplicate
`BetaVault.sol` wholesale since it was never split into reusable bases):
`HubPartyRegistry._pay` was marked `virtual` — a no-op for every other
network's `BetaHub`, which doesn't override it — so `BetaHubPathUSD`
(still inheriting the *unchanged* `HubCompositionRegistry`/
`HubAnchorJudge`) can redirect every inherited payout path through one
PathUSD choke point. `registerParty`/`topUpBond` gain new PathUSD-taking
overloads (different arity, so genuine overloads, not overrides) — the
inherited payable originals stay present but permanently unreachable,
the same "dead path, not a live footgun" choice `BetaVaultPathUSD.sol`
already made for its own native branch.
Deployed and verified (never trusting the receipt — Tempo's own
documented bug struck again; the real address came from independently
scanning nearby CREATE nonces) at
**`0x900D54050f9Fe47ca56cC67A281947Da2EeE2cD1`**
(`HubToken`/"iBETA" `0x5E14002470a5c0FfD431BA151e4D22a678C16b6C`), with
a real operator party registered (5 PathUSD bond) against a real,
currently-unspent Bitcoin UTXO as its starting anchor.

**`iPoWV1ConversionPathUSD.sol`** is a bigger conversion than the hub's:
unlike `BetaVault`/`BetaHub`, `iPoWV1Conversion` already has a fully
flexible `tokenAddr` choice for the *conversion value itself*
(`address(0)` = native, left as a provably-dead branch on Tempo, same
precedent again) — what's hard-coded to native value regardless of
`tokenAddr` is narrower: the **commit fee**, the **auction stake**, and
the **bounty** it can accumulate into. `proposeClaimConversion` gained
an explicit `stakeAmount` parameter (it can no longer be inferred from
`msg.value`, which now serves only the still-native bitcoin→native
self-escrow case). Deployed and verified at
**`0xC40003975B6ff70E46999Bac8f3b06a9908Fb333`**, cross-registered with
the other 6 networks like every other instance.

### 9.4 Solana Beta automation, and genuine EVM/SVM parity

`core/operator` previously automated Beta's claim → relay → exercise
loop for EVM only (a deliberate early scope decision); Solana's
`beta-factory` — the real, live, composition-capable hub used in the
real 5-network mint (§8.20) — had zero Rust automation. A new
`SvmBetaHubAdapter` (`ipow-svm`'s `beta` module) implements the same
chain-agnostic `BetaHubAdapter` trait the EVM adapter already does,
against the real IDL (`beta-factory`'s own, copied in for
`declare_program!`). Same MINT-only scope as the EVM engine — VETO/
ATTEST/CLEAR/ALIVE remain manual, auditor-triggered actions on both
chains.

Two things worth recording precisely:

- **Header freshness needed no new code at all.** The existing
  `SolanaStreamingAdapter` (built for Conversion's own Solana role)
  already does exactly what's needed — real Bitcoin-PoW header fetching
  plus `commit_header` submission — so Beta's own registry just points
  a synthetic `SolanaConfig` at Beta's `ipow` program id
  (`EsmGbkui9ZFC6Fch9J6xoyNRZSp6TwfvcjS88beP1Vem`) and reuses it
  unmodified, the exact same trick `beta_registry.rs` already used for
  EVM's `EvmStreamingAdapter`.
- **A real bug was caught while wiring this, not shipped**: the
  `BetaHubAdapter` trait's `user` field was `[u8; 20]` (EVM-address-
  shaped) — truncating a real 32-byte Solana pubkey into that would have
  silently corrupted it. Widened to `[u8; 32]` throughout (the wire
  format already used 32 bytes, zero-padded on the EVM side — a
  correctness fix, not a new capability), a small, mechanical, 5-call-
  site ripple. Confirmed independently: the MINT-statement builder was
  *unconditionally* running every hub's `pending.user` through
  EVM-specific unpadding logic regardless of which chain the hub
  actually ran on — harmless while only EVM hubs existed, silently wrong
  the moment a Solana hub's own claim/relay path used it. Fixed by
  constructing the statement directly from the already-correctly-shaped
  wide value instead.

`BetaNetworkHandle.evm_network: EvmNetwork` became a plain
`network_label: String` — every real usage was logging only, confirmed
by grep before changing it, not assumed.

**Housekeeping the same session**: a stale, expired, never-claimed test
`Pending` (composition id 1, nonce 1, from the original devnet
composition-support validation run) was cleared via a real, permissionless
`expire_pending` call, refunding its locked SOL back to the same wallet.
`beta-factory` now has zero live `Pending` accounts — a clean slate,
not evidence anything was ever broken.

### 9.5 Corrected addresses (supersedes stale entries in "Current deployments" below)

| Network | `iPoWV1Conversion` (permissionless) | Beta hub | Beta spoke |
|---|---|---|---|
| Ethereum Sepolia | `0xA1a25C810ff2BCBb7c5CdAc4Fd56A72aD7160C0B` (redeployed; old `0x9637cc…` abandoned, not migrated) | `0x4222DE34E0160F63cb5a614a766B96f0b5a34D23` | `0x30A386AEc5aD0afAcC6C8624b47ABbB719596a6b` (v5) |
| Hedera | `0xF5FE37E0bAE715FC61D6FF0f724c4c7E070F8057` | `0x3cDba300797351664dEE4D5F0D3F7Be28e186bdA` | `0xba35203CD4389A66926e2280C66F7DD0646DBF35` |
| Polkadot Hub | `0x02A1A50e251f468cF42f7d0e6Ca0d37C52fcb111` | `0x754FC037B3d2aBf0b63A57a549F21755442a9a16` | `0x4caE61D96FbF49eC853Ad91853Eb2F4b669D04CC` |
| Base Sepolia | `0xa43f67C41Fc6e8474278d5a7C1540862CC2a16bA` | `0x24765955eCbffAaACB48a8c3747975d6C750C075` | `0x6354779b4Dbb564c712ea91c179eCF521C15BE73` |
| Robinhood Chain | `0xaFBdaC4A4e7428C4bA1Bc7E95B7D4A33B2F564f5` | `0xB7054E399E31A2cFE181c4fD59C7235562a6d45d` | `0x35e564d74B90a3A5bfcA8Dec65b1325C83d2e822` |
| Hyperliquid | router `0xB7054E399E31A2cFE181c4fD59C7235562a6d45d` | router `0x4C5769e3213496a0641E139e2F0E94ce7625374C` | via same router |
| Tempo (Moderato) | `0xC40003975B6ff70E46999Bac8f3b06a9908Fb333` (PathUSD) | `0x900D54050f9Fe47ca56cC67A281947Da2EeE2cD1` (PathUSD) | `0x8b9efF66C7D93816Eadf702cC6cE273e8B2b03e7` (PathUSD) |
| Solana devnet | `ipow-conversion` `FbwXLABMqUeRPw85C7MpMS2LQi4mveXR9a9fA79DfS3V` (no operator automation — permissionless by design) | `beta-factory` `BkNb9JfNbfbn3rMiA3j3z2pnFdNibxiYWWsKZ9VdzoD1` | n/a — Solana has no spoke role |

`BasketBridge`/`BetaMint`/`BetaToken` (Ethereum) and `beta-mint`
(Solana) no longer exist / are unused — see §9.1.

## Current deployments (devnet/Sepolia, for reference — not production)

**This section is a historical log, not current state — several
addresses/contracts below are since superseded or removed. See §9.5 for
the corrected, currently-live address table.**

- Solana devnet: `ipow` `EsmGbkui9ZFC6Fch9J6xoyNRZSp6TwfvcjS88beP1Vem`,
  `ipow_conversion` `FbwXLABMqUeRPw85C7MpMS2LQi4mveXR9a9fA79DfS3V`,
  `beta_mint` `D9RgvEFXetbXKkaKZd4EzrmUyjA8VPLHn8fYwkLUKW3R`.
- Ethereum Sepolia: `iPoWV1` `0x3a6b4B540BAc87056618696dc3fE6929c28e9b6C`,
  `iPoWV1Conversion` `0x9637cc434B81e6ed711D97dfD6A66ec0dDF6c41d`,
  `BasketBridge` `0x8978fC779962C3eDa2Ed0b9ECd7E542ce96a3c63`.
- **BETA v2 (2026-09-21)** — Sepolia: `iPoWV1` (with §6.9)
  `0xB8ab960D1121F33B48b4086aBFCDD8B750081588`, `BetaVault`
  `0xee41B6d9D622C7eab420fc37Be61695DC7422413`. Operator party
  `0x01…01` registered with chain head = the operator wallet's UTXO;
  user lock #1 (1 unit = 0.001 ETH, deadline 1790537898) is **FINAL**,
  finalized by the real Bitcoin-mainnet anchor
  `51d6b13a034ddda3e6ef94a0af8fcf4d9073c930ff05c6cf50977f19ec76dc50`
  (block 967886, relayed to the fresh Sepolia relay as its anchor
  header). Solana devnet `beta_factory`
  `3GUPbVjjBRNEYafHprb2fnh6Wzyif9a4gWphSrhkPVEf` (BETA mint
  `C2ZELAMyd6ukCBJKH4xE3wqBba6hJQykw6waaV13Ukhw`, operator party PDA
  `EPwUzmEkajar93y12pXX8SXRAx3e9KgyhTwuE2kEUCLF`, `MINT_CAP=9`): the same
  anchor was processed (`Queued`) and exercised — **1 BETA minted**,
  `reserve_lamports` 0.01 SOL, `eth_claims_units` 1, supply 1. The
  devnet `ipow` program is still the pre-§6.9 binary (needs `solana
  program extend … 6000` before the upgrade lands); block 967886 was
  relayed with the legacy account layout (`RELAY_LEGACY`). Compute:
  `process_anchor` on a real block needs ~1.4M CU; the driver sets the
  budget explicitly.
- First real cross-chain BETA: 1 unit backed by 0.01 SOL on devnet +
  0.001 ETH FINAL on Sepolia, both finalized by one Bitcoin-mainnet
  anchor, 2026-09-21.
- Devnet `ipow` upgraded to the §6.9 binary (slot 501553310, after
  `solana program extend … 10240`; note `target/deploy/ipow-keypair.json`
  is *not* the `EsmG…` key — upgrade with `--program-id EsmG…`). The
  new linkage path was exercised live: 967887 committed as an extension
  of 967886 with `prev_header`.
- Redeem leg in flight: `burn_redeem(1)` done on devnet (burn id 0,
  0.01 SOL returned, supply 0); RELEASE statement
  `0200…0001` (lock 1, burn 0, to `0x9784…2BF1`, 1 unit); anchor built
  and signed — txid `f85336e006b16fd4ac4991a0fbdb96e056f0bed7a66e3316fadf70c7001af8e5`,
  spending the head `51d6b1…:0` plus a 2,564-sat funding input from the
  Polkadot BTC wallet (index 0), change to the operator's main wallet.
  Broadcast by Kelwin; confirmed in block 967894. Both relays extended
  967887→967894 header by header (15 linked extensions through the §6.9
  checks). Sepolia `processAnchor` queued the release (tx `0x44207d9c…`);
  devnet `process_anchor` judged the burn true (burn 0 claimed, party seq
  2, operator alive); after `T_rel`, `executeRelease` paid 0.001 ETH to
  `0x9784…2BF1` (tx `0x2d5bcd19…927d`). Final state: Sepolia lock 1
  `Released`, `totalLocked` 0, insurance 0, operator bond intact; devnet
  reserve 0, `eth_claims_units` 0, BETA supply 0. **Full mint → redeem
  round trip completed on real infrastructure, 2026-09-21.**
- **Adversarial round (2026-09-21, block 967913)** — five pre-signed
  anchors (`scripts/adversarial_anchors.txt`, broadcast by Kelwin with
  `scripts/broadcast_anchors.sh`; auditor party `0x02…02` registered on
  both chains with chain head A3:2), processed in order on both chains:

  | anchor | statement | devnet `beta_factory` | Sepolia `BetaVault` |
  |---|---|---|---|
  | A3 `aafee334…` | honest MINT lock 2 | Queued → Exercised (1 BETA) | lock 2 FINAL |
  | A4 `b2022b1e…` | **MINT lock 99 (no such lock)** | Queued → Exercised — the lie mints on the blind chain (BETA supply 2, reserve 0.02 SOL) | **Slashed**: 0.001 ETH from the bond (0.0009 → insurance, 0.0001 bounty), operator `dead` |
  | A5 `a3f1ee4a…` | auditor dead-veto | operator `paused_until` set (mints held) | judged "dead" = true → auditor rewarded 0.0009 ETH (insurance-capped) |
  | A6 `02f8cf39…` | **RELEASE lock 2, burn 7 (no such burn)** | **Slashed**: 0.01 SOL from the bond (0.009 → insurance, 0.001 bounty), operator `dead` | queued (lock 2 FINAL) — but `executeRelease` refuses: `NotReady` → `Held` → `PartyDead` |
  | A7 `0f5e1d2c…` | auditor VETO on A6 | target Slashed → auditor rewarded 0.009 SOL (insurance-capped) | release held until `heldUntil`; hold fee 0.0005 ETH to insurance |

  Every lie was accepted where it could not be checked and punished by
  code where it could, with the bond flowing into insurance in the lied-
  about asset. Solvency after the round: Sepolia `totalLocked` 0.001 ETH
  (lock 2, real) + insurance 0.0005 ETH against the one unbacked unit
  (0.001 ETH) — short by 0.0005 because the dead-veto reward and the
  submitter bounty were paid out of the same slashed 0.001, exactly the
  sizing rule §6.15 derives (bond must cover cap ÷ (1 − bounty) *plus*
  the veto reward budget; the test bond was deliberately the minimum).
  Two follow-ups from the live run: (1) the hold fee had retired the
  auditor on Sepolia because its bond was exactly the minimum — rule
  changed to "retire only when the next fee can't be paid" (+1 test);
  (2) `veto_reward` should come from a governance-funded pool, not the
  same slash that has to back the fake units. Governance topped Sepolia
  insurance up by 0.0005 ETH (tx `0x6be528da…`): backing 0.002 = claims
  0.002 again.
- **Insurance releases (built after the round, not redeployed).** The
  contract had no way to *redeem* a unit that no FINAL lock backs — a
  RELEASE naming a nonexistent lock is, correctly, a lie. Added: `RELEASE`
  with `lockId = 0` means "pay from insurance". Ethereum's half of the
  predicate is `insurance − insuranceReserved ≥ amount` (a local fact);
  it queues, delays and is vetoable exactly like a lock release, reserving
  the amount so it can't be promised twice; the burn is judged on Solana
  unchanged (`beta-factory` never reads the lock id). So a holder of the
  unbacked unit burns on Solana, the operator anchors `RELEASE{0, burn,
  to, 1}`, and the slashed bond pays them — the pool is whole at every
  point, and lying about insurance capacity is slashed like any other
  Ethereum-side lie. (+1 test; EVM suite 112.)
- **§8.12/§8.13 composition support deployed fresh (2026-09-23)** — both
  sides needed a new address rather than an in-place upgrade: Solana's
  account layout changed non-additively (fields inserted mid-struct, not
  just appended), which would have misread the old `beta_factory`'s
  still-live, still-unsettled `ProcessedAnchor`/`Pending` accounts (two
  pending settles were due ~2026-09-29 with real escrowed SOL); Ethereum
  contracts were never upgradeable in the first place, matching the
  v2/v3/v4 pattern of a fresh address per breaking `BetaVault` change.
  The old devnet `beta_factory` (`3GUPbVjjBRNEYafHprb2fnh6Wzyif9a4gWphSrhkPVEf`)
  is untouched and keeps running for its own sake.
  - **Solana devnet**: fresh `beta_factory` **`BkNb9JfNbfbn3rMiA3j3z2pnFdNibxiYWWsKZ9VdzoD1`**
    (slot 502532698), initialized fresh (new BETA mint
    `CWVb5kuEmbA5ZYzzXDfQVupNiWLsn2f7BE3gJbEEgLiP`). Live-verified via a
    one-off registration/deploy script (since removed as superseded
    scratch tooling — the on-chain state this produced is independent of
    the script's own continued existence): registered composition
    id 1 spanning 6 components — local SOL plus 5 *named* remote
    networks (Base `8453` and Arbitrum `42161`, their real mainnet chain
    IDs; Hyperliquid/Robinhood/Tempo as clearly-marked placeholder IDs
    `1_000_000_00{1,2,3}` — no verified real chain ID for those three,
    and no live vault contract deployed to any of the five) — then
    really `lock_sol` + `approve_pending`'d 1 unit of real devnet SOL
    against it (tx signatures in the script's own output). This proves
    the flat-budget registry and lock/approve mechanics live on a real
    deployment; it does not exercise the mint, which needs real
    Bitcoin-anchored MINT statements per remote leg (an operator with
    funded UTXOs) — a separate, bigger undertaking, not run here.
    **Update (2026-09-24)**: this test pending (composition id 1, nonce
    1) sat past its deadline, unclaimed, until permissionlessly expired
    via `expire_pending` — refunding the locked SOL back to the same
    wallet and closing the account. `beta-factory`'s own live state is
    back to zero `Pending` accounts; composition id 1 itself (the
    registry entry) is untouched and still registered, just unused.
  - **Ethereum Sepolia**: fresh `BetaVault` **v5**
    `0x30A386AEc5aD0afAcC6C8624b47ABbB719596a6b` (same relay
    `0xB8ab…1588`; `ignition/modules/BetaVaultV5.ts`), plus a test
    `MockERC20` ("Mock BETA-leg USD" / mUSD)
    `0xA1304b12E594aD13a62e9d7e66Cd38826288Bc44` deployed alongside it
    purely to exercise the new path. Live-verified: `setTokenParams`
    registered mUSD (1 mUSD/unit, 0.001 ETH/unit slash rate), then a
    real `deposit(mUSD, …)` locked 1 mUSD (tx `0xcc26fd13…`) — lock #1
    reads back `token = mUSD`, `amount = 1e18`, `state = Pending`, and
    the vault's own mUSD balance is 1.0, confirming the ERC20 leg really
    moved real (testnet) tokens, not just that registration succeeded.
    Deployer balance ~0.03 ETH before this; no parties registered yet on
    v5.
