# Superseded: BETA same-chain and cross-chain-basket designs

**Status: superseded/removed. Not authoritative. Kept for historical
context on why the current statement-bus design (see
[`../design/ipow.md`](../design/ipow.md) and
[`../design/ipow-implementation.md`](../design/ipow-implementation.md))
looks the way it does.** Both designs below predate the Bitcoin-anchored-
statement approach and were superseded by it before that work started.
Their code (`BetaMint.sol`, `BasketBridge.sol`, Solana's `beta-mint`
program) has since been removed entirely — see
[`abandoned-cpi-funding-attempt.md`](abandoned-cpi-funding-attempt.md).

## BETA, same-chain (`beta-mint` / `BetaMint.sol`)

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

## BETA, cross-chain basket: two-hop, minted on Solana

Extends the same-chain design with a second minting path backed by a
basket split across both chains — part native SOL locked on Solana, part
native ETH permanently locked on Ethereum via a second contract,
`BasketBridge.sol`. Four Conversion legs; one real Bitcoin payment can
link all four (script uniqueness is per-contract, and outbound legs have
no uniqueness guard at all).

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
   `BasketBridge`, permanently retains `keepAmount` (basket collateral
   #1), and immediately commits **Leg3** (`token→bitcoin`) for the
   remainder, to bridge back to Solana.
4. **Leg3** auction resolves directly against Conversion; `fundLeg3Deposit`
   (mirrors `fundRedeemReserve`) moves the forwarded amount into escrow.
5. **Leg4** (`initiate_basket_leg4`, Solana `bitcoin→token`): a claimant
   self-escrows the returning amount — basket collateral #2, landing in
   the same-chain design's own vault.
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
  request — once the header relay's tip crosses the proof window. Both
  recovery paths reject a second call (`AlreadyClaimed`).

**What this guaranteed**: BETA was never minted against unmoved value —
every mint followed real value already landing (via proof or forced
forfeiture) on Solana's own, independently Bitcoin-verified legs.
**What it did not guarantee**: that Ethereum's Leg2/Leg3 actually
happened. Solana cannot read Ethereum's contract state directly, and
this design had no cross-chain oracle. That link was economic, not
cryptographic — an arbitrageur only profited by completing both sides,
and if they didn't, Leg4's forced forfeiture still left the vault (and
BETA's backing) solvent regardless of Ethereum's actual state. This
exact gap — Solana having no way to verify a fact about Ethereum's real
state — is precisely what the current statement-bus design in
`ipow.md`/`ipow-implementation.md` was built to close.

**Known limitation**: no on-chain correlation existed between Solana's
`leg1_tx_id`/`leg4_tx_id` and Ethereum's `txId2`/`txId3` — it was tracked
off-chain by whoever orchestrated a request. A real multi-user version
would have needed a correlation id threaded through, or an off-chain
coordinator — never built before this design was superseded.
