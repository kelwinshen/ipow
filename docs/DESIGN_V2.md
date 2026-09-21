# Design v2: Conversion, and BETA built on top of it

Current-state reference only — describes what the code does, not how it
got there. Two chains, three programs/contract-sets per chain:
`ipow-conversion` (Solana) / `iPoWV1Conversion.sol` (Ethereum) as the base
primitive; `beta-mint` (Solana) / `BetaMint.sol` + `BetaToken.sol`
(Ethereum) as same-chain BETA; `BasketBridge.sol` (Ethereum) as the
Ethereum-side half of cross-chain basket BETA, minted on Solana.
`ipow`'s own original single-fixed-operator Conversion (inside the `ipow`
program itself) is a separate, untouched implementation — everything
below is `ipow-conversion`, a sibling program, except §5.

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

- **Operator service integration**: `scripts/beta_daemon.ts` (§6.12b) is
  the loop, but it runs as a standalone script driving the CLI tools;
  folding it into `core/operator` (Redis state, the existing BTC wallet
  and header streamer) is still to do. It has run dry cycles against live
  state only — no anchor has yet been produced by the daemon end to end.
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

### 7.6 Trust statement (v3)

Honest users are never harmed and never wait on a clock (an attester
fronts). A cheating operator is punished by code if anyone submits its
statements to the judging chain within `T_challenge`; its damage is
bounded by its own escrow even if nobody does. Residual assumptions:
one honest checker per week, the SOL/ETH ratio used for mint escrows
(a week of price risk on units that should not exist), and the header
relays. The zk audit path still turns "caught" into "rejected" and
removes the first assumption; nothing here changes shape for it.

## Current deployments (devnet/Sepolia, for reference — not production)

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
