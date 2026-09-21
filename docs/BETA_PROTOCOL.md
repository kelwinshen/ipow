# BETA: one token, backed on several chains, settled through Bitcoin

*End-to-end design and record of the live runs. Companion to
`docs/DESIGN_V2.md` (the working spec, §6–§7) and the interactive page
"Two Judges, One Bitcoin".*

---

## 1. The problem in one paragraph

We want a single fungible token on Solana — BETA — where each unit is
backed by real native assets sitting on other chains: 1 BETA = 1 SOL on
Solana + 1 ETH on Ethereum (and later DOT, HBAR, …). The obstacle is
absolute: **no chain can read another chain's state.** Solana cannot check
that ETH is locked on Ethereum, and Ethereum cannot check that BETA was
burned on Solana. Every "clever flow" that tries to get around this with
round trips, derived addresses, escrows, or timers fails the same test:
the honest run and the cheating run produce byte-identical inputs on the
blind chain, so no rule on that chain can tell them apart.

What every chain in this system *can* read is **Bitcoin** — each one runs
its own PoW-validated header relay and verifies Bitcoin transaction
inclusion natively. That is the only shared ground truth, and the whole
design is built on using it correctly.

## 2. The three layers

| Layer | What it does | Trust |
|---|---|---|
| **Conversion** (`ipow-conversion`, `iPoWV1Conversion.sol`) | Moves value between a chain and Bitcoin by auction; two legs on two chains linked by one Bitcoin payment give chain-A → chain-B transfers | Trustless: the party at risk (the user) sees both chains and commits only after the counterparty's value is escrowed; each chain's release is a Bitcoin fact it verifies itself |
| **Statement bus** (`beta-factory`, `BetaVault.sol`, Bitcoin) | Carries *claims* across chains as Bitcoin transactions; the chain that owns the fact judges the claim by code; the blind chain acts provisionally | Optimistic: a lie executes on the blind chain but is proven and punished by code on the other, within a window, bounded by the liar's own escrow |
| **BETA** | The product on top: lock SOL + lock ETH → mint; burn → get both back | Inherits the two above |

Conversion cannot carry truth (it moves money); the statement bus cannot
move money (it carries truth). BETA needs both.

## 3. The one rule

> **A claim is judged only by the chain that owns the fact it asserts.**

"1 ETH is locked on Ethereum for lock #N" is judged on Ethereum, which can
read its own vault. "BETA was burned on Solana under burn #id" is judged
on Solana. The chain that *cannot* see the fact never tries to judge it —
it acts on the claim provisionally (queues it, inside a challenge window,
vetoable), and relies on the owning chain to slash the claimant if the
claim was false.

Because every claim is a Bitcoin transaction, both chains read the same
bytes, in the same order, and neither can be shown something the other
was not.

## 4. Statement chains: how a party "says" something

Every registered party (operators, auditors) has a **statement chain** on
Bitcoin: a linear sequence of transactions where each spends output 0 of
the previous one. To make a claim, the party spends its current chain
head and writes the claim into an `OP_RETURN`:

```
input[0]  = the party's registered head (only its key can spend it)
output[0] = 294 sats back to its own address → the next head
output[1] = OP_RETURN  ver=0x01 | kind | sha256(statement)
```

Properties, with no signature verification anywhere:
- **Unforgeable** — nobody else can spend the party's UTXO.
- **Unhideable** — anyone can submit any anchor to either chain for judging,
  and gets a bounty if it was a lie.
- **Ordered** — both chains advance a party's pointer only in sequence, so
  "the party's history up to here" is one well-defined object on both.
- **Cheap** — a statement costs one dust output plus a fee (~300 sats in
  the live runs).

The statement kinds and who judges them:

| kind | bytes | says | judged on | acting (blind) chain |
|---|---|---|---|---|
| MINT | 65 | lock #N (user X, nonce, units, deadline) exists on Ethereum | Ethereum | Solana queues; exercises on ATTEST or after the window |
| RELEASE | 45 | burn #id exists on Solana; pay `to` from lock #N (or insurance if N = 0) | Solana | Ethereum queues; pays on ATTEST (from escrow) or after the window |
| ATTEST | 33 | "that MINT/RELEASE is true, and I escrow for it" | wherever the target is judged | act now; escrow held until settle |
| VETO | 65 | "that anchor is false" (all-zero txid = "that operator is dead") | wherever the target is judged | hold / pause, with no expiry |
| CLEAR | 33 | "that anchor is true; lift the veto" | wherever the target is judged | lift the hold |
| ALIVE | 33 | "that operator is not dead on Ethereum" | Ethereum | Solana un-pauses |
| CANCEL | 9 | "MINT #N will not happen; let the depositor refund" | — | Ethereum refunds the pending lock |

## 5. Who is who

| Role | Holds | Does | Can lose |
|---|---|---|---|
| User | SOL, ETH, BETA | locks SOL (`lock_sol`), deposits ETH (`deposit`), consents (`approve_pending`), burns to redeem | only time — never money to anyone else's misbehavior |
| Operator | a chain head, a SOL bond on Solana, an ETH bond on Ethereum | anchors MINT / RELEASE / CANCEL; usually also ATTESTs its own claims for instant UX | its bond and escrow, per lie, on the chain that caught it |
| Auditor (anyone) | a chain head, bonds | ATTESTs claims it has verified (earning the fast path), VETOs lies, CLEARs false vetoes | its bond for a false statement of its own |
| Anyone at all | — | submits anchors for judging (bounty), exercises mints, executes releases, relays headers after 30 min | nothing |
| Governance | one key per chain | sets numbers, approves operators, pauses | never decides truth |

In the live runs every role was the same wallet. That is fine for
mechanics and meaningless for security: the "one honest checker"
assumption is only real once a checker is someone else.

## 6. Mint, end to end (v3)

```
User X            Solana A               Bitcoin              Ethereum B            Operator / Auditor
  | lock_sol(nonce,1u)  |                    |                    |                    |
  |-------------------->| pending[X,nonce]   |                    |                    |
  | deposit(X,nonce,1u,D) --------------------------------------->| lock[N] PENDING   |
  | approve_pending(nonce, N)                |                    |                    |
  |-------------------->| consent            |                    |                    |
  |                     |                    |  A1 = MINT{N,X,nonce,1u,D}   <----------| operator
  |                     |                    |  A2 = ATTEST{A1}             <----------| operator or auditor
  |        process(A1)  | predicate ✓ → Queued (challenge window opens)  |    |
  |        process(A1) ------------------------------------------>| lock[N] matches → FINAL
  |        process(A2)  | escrow reserved from attester's bond      |    |
  |        process(A2) ------------------------------------------>| attester recorded
  |        exercise(A1) | mint 1 BETA → X   (now, not in a week)   |    |
  |   ...one week...    |                    |                    |    |
  |        settle(A1)   | not held → escrow back to attester's bond |    |
```

- **Fast path**: any bonded party ATTESTs → exercise immediately. The
  attester's escrow (`units × comp` SOL) is locked for the window.
- **Slow path**: nobody attests → exercise after the window, free.
- **Challenge**: a VETO (judged on Ethereum) holds the mint; a CLEAR lifts
  it. Neither expires; only a contrary bonded claim changes them.
- **Settle** (after the window): unheld → escrow returns; held → escrow
  forfeited to insurance (it backs the unit that should not exist); a
  held mint never exercised is cancelled and the user's SOL freed.

## 7. Redeem, end to end (v3)

```
Holder Y          Solana A               Bitcoin              Ethereum B            Operator / Auditor
  | burn_redeem(1u, 0xY)|                    |                    |                    |
  |-------------------->| burn 1 BETA; pay 1 SOL now; burn[id]     |                    |
  |                     |                    |  R1 = RELEASE{N,id,0xY,1u}   <----------| operator
  |        process(R1)  | burn[id] exists ✓ → claimed              |                    |
  |        process(R1) ------------------------------------------>| lock[N] FINAL → Queued (window)
  |   fast: ATTEST{R1} ------------------------------------------>| pay 0xY now FROM THE ATTESTER'S ESCROW
  |   slow: executeRelease after the window ---------------------->| pay 0xY from the vault
  |   settleRelease after the window ----------------------------->| reimburse attester (unheld) / cancel (held)
```

The vault never pays for a false release: on the fast path the attester
fronted its own money and, if a veto stands at settle, is simply not
reimbursed (and is slashed on Solana, where the burn fact lives).

## 8. What happens when someone lies

| Lie | Accepted on | Caught on | Consequence |
|---|---|---|---|
| MINT for a lock that doesn't exist | Solana mints (blind) | Ethereum | operator bond slashed in ETH → insurance; **its attester slashed too**; operator retired; auditor dead-vetoes Solana |
| RELEASE for a burn that never happened | Ethereum queues (blind) | Solana | operator bond slashed in SOL; attester (if any) not reimbursed; Ethereum never pays |
| False VETO / false CLEAR / false ALIVE | — | the fact's chain | the claimant's bond slashed; true vetoes rewarded from a governance pool |
| Silence | — | — | nothing to slash; users refund on timeouts; anyone may extend headers after 30 min |

"Retired" = that party id is dead on that chain: its bond is gone and its
key can't act. The same person returns as a new party with a fresh bond.
Retiring rather than "slash a bit and continue" keeps the escrow math
honest.

## 9. The trust statement, precisely

- **Conversion is reject-tier**: a lie fails, because the chain can verify
  Bitcoin.
- **BETA is punish-tier, optimistic**: a lie executes on the blind chain
  and is proven false by code on the other within the challenge window.
  It is safe if **one honest party submits the lying anchor within a
  week** — a permissionless, bounty-paid action — and even if none does,
  the damage is bounded by the liar's own escrow.
- Residual assumptions: one honest checker per week; the SOL/ETH ratio
  used for mint escrows (a week of price risk on units that should not
  exist); the header relays keep moving.
- **Honest users are never harmed** and never wait on a clock (an attester
  fronts). A cheating operator always pays. When bonds are undersized the
  bill lands on BETA holders as a group — the sizing rule is
  `bond ≥ cap × unit ÷ (1 − bounty)` plus a reward budget.
- The zk audit path (Ethereum finality + storage proof verified on
  Solana) turns "punished" into "rejected" by replacing one instruction;
  nothing else changes.

## 10. What was built

| Component | Where | Tests |
|---|---|---|
| Solana `beta-factory` | `programmable-network/solana/programs/beta-factory` | 23 litesvm |
| Ethereum `BetaVault.sol` | `programmable-network/ethereum/contracts/BetaVault.sol` | 17 (EVM suite 114) |
| Header relay change (both chains) | `ipow/commit_header.rs`, `iPoWV1.sol` | prev-hash linkage, in-epoch nBits, permissionless extension after 30 min |
| Anchor builder | `core/operator/crates/core/examples/anchor_statement.rs` | dry-run by default |
| Drivers | `solana/scripts/beta_factory_e2e.ts`, `ethereum/scripts/beta_vault_e2e.mjs` | every instruction by `ACTION=` |
| Daemon | `solana/scripts/beta_daemon.ts` | watchtower / operator / auditor loops, jump relays, statement ledger, spend caps |

Deployed: Solana devnet `3GUPbVjjBRNEYafHprb2fnh6Wzyif9a4gWphSrhkPVEf`
(BETA mint `C2ZELAMyd6ukCBJKH4xE3wqBba6hJQykw6waaV13Ukhw`); Sepolia
`BetaVault` v3 `0x799e3B35ba0fCC8DB67Ceba453017d7C883eBcE8` on relay
`0xB8ab960D1121F33B48b4086aBFCDD8B750081588`. Test composition: 1 unit =
0.01 SOL + 0.001 ETH.

## 11. The live record (Bitcoin mainnet, 2026-09-21/22)

| Block | Anchor | Statement | Solana devnet | Sepolia |
|---|---|---|---|---|
| 967886 | `51d6b13a…dc50` | MINT lock 1 (v2) | minted 1 BETA | lock 1 FINAL |
| 967894 | `f85336e0…f8e5` | RELEASE lock 1 (v2) | burn 0 claimed | paid 0.001 ETH after 10 min |
| 967913 | `aafee334…f81b` | MINT lock 2 | minted | FINAL |
| 967913 | `b2022b1e…a3e5` | **MINT lock 99 (lie)** | minted (blind) | **slashed 0.001 ETH**, operator retired |
| 967913 | `a3f1ee4a…9a95` | auditor dead-veto | operator paused | judged true → auditor rewarded |
| 967913 | `02f8cf39…62de` | **RELEASE burn 7 (lie)** | **slashed 0.01 SOL**, operator retired | queued → refused (`NotReady`→`Held`→`PartyDead`) |
| 967913 | `0f5e1d2c…777c` | auditor VETO on the lie | judged true → rewarded | payout held |
| 968030 | `b019417d…7aa8` + `a00829ba…8097` | v3 MINT + operator ATTEST | **exercised immediately** (escrow 0.01 SOL) | FINAL, attester recorded |
| 968040 / 968046 | `4079c614…841c` / `5f901eca…fe22` | **daemon-anchored** MINT + **independent auditor** ATTEST | exercised immediately (auditor escrow) | FINAL, `mintAttester = 0x04…` |
| 968053 | `4971e4fa…ac90` | v3 RELEASE, slow path | burn 1 claimed | queued, unattested; payout due 2026-09-29 |

Every row is one Bitcoin transaction read independently by two chains that
never spoke to each other, each reaching the verdict its own facts
support.

## 12. What it cost, and what went wrong

- Total Bitcoin spent across every live run: under 6,000 sats, including
  1,244 sats lost to a daemon bug (it re-anchored the same ATTEST every
  cycle until the first confirmed; fixed with a statement ledger, a
  program-side no-op for redundant attests, and hard spend caps).
- Esplora's `/tx/:id/status` returns `{"confirmed":false}` for txids it
  has never seen — treat only `"confirmed":true` as information.
- `tFin` (the window in which a MINT anchor must be processed on
  Ethereum) was 1 h and nearly missed during an explorer outage; now 4 h.
- Relays must jump, not extend, when far behind; the EVM jump needs the
  epoch-start header first.
- Bounties and veto rewards must not come out of the slash that backs the
  fake units — they now come from a separate governance pool.

## 13. Open

- An auditor run by someone who is not the operator.
- Multi-operator live; multi-component compositions (per-chain vaults,
  per-component locks).
- The daemon inside `core/operator` with a real pre-flight.
- The zk audit path.
