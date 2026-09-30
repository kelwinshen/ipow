# Conversion on the new protocol (proposal, stage 6)

**Status: approved by the owner on 2026-09-29 (questions C1 to C5 below). Built and tested on Ethereum (`contracts/apps/Conversion.sol`, `test/Conversion.test.ts`), on Solana (`programs/conversion`, `tests/test_conversion.rs`), and in the node's operator (`core/node/crates/node/src/swaps.rs`, tests in `core/node/crates/node/tests/conversion_local.rs`). Not deployed.** It is the first
application of stage 6 in [`ipow-build-plan.md`](ipow-build-plan.md), on
the protocol of [`../design/ipow-protocol.md`](../design/ipow-protocol.md)
(section 10.3, D8, D10, D18, D36, D51, D94). The questions in the last
section are open for the owner.

## What it does

A user swaps a network's coin for real BTC, or real BTC for a network's
coin. Conversion is an application: a contract (on Solana, a program)
registered with the protocol like any other (D63). It opens jobs, and
the protocol's operator carries its messages on Bitcoin.

## Who is who

| Role | Does |
|---|---|
| User | Asks for a swap, at a price they set |
| Operator | Wins the job in the protocol's auction, and is also the other side of the swap: it pays or receives the BTC, and receives or supplies the coin |
| Guardian | As in the protocol: punishes a missed duty or a proof on fake blocks |

The operator is the other side because section 10.3 asks for it: for
Bitcoin to token, "a receipt that spends the BTC the user paid". Only the
owner of the address the user paid can spend that BTC.

## Token to Bitcoin: the user sells 1 ETH for BTC

1. Alice locks 1 ETH in Conversion, names her Bitcoin address, and the
   least BTC she accepts: 0.05 BTC. She pays the job's fees.
2. Conversion opens a job. Its tag names this swap.
3. An operator, Bob, wins the auction. His node only bids on swaps whose
   price suits him.
4. Bob's tagged Bitcoin transaction pays Alice 0.05 BTC or more, in one
   output, and Bob proves it.
5. Once the proof's lock has ended (36 hours) with no challenge won,
   anyone hands Conversion that transaction. Conversion checks it is the
   one the protocol accepted for the job, and that it pays Alice enough.
   Bob receives the 1 ETH. He waits for the lock because a proof on
   made-up blocks can be shown false until then; with the lowest escrow
   (C3), only Conversion keeps Alice's ETH safe.

If Bob's transaction does not pay Alice enough, or the job fails, Alice
takes her 1 ETH back.

## Bitcoin to token: the user buys 1 ETH with BTC

1. Alice asks for 1 ETH for 0.05 BTC. She pays the job's fees.
2. Conversion opens a job, as a claim with a challenge period of 36
   hours (for a close, below).
3. Bob wins and anchors. Within 30 minutes of the end of the auction,
   and while his anchor is at most 30 minutes old, he locks 1 ETH in
   Conversion and names a Bitcoin address he has never used, in
   Conversion or on Bitcoin.
4. Only then, Alice pays 0.05 BTC to Bob's address, in one output, before
   the payment deadline: 12 blocks after the job's anchor. The app shows
   her the deadline.
5. Bob's tagged transaction spends Alice's payment: the receipt. Once it
   is proven, anyone hands Conversion the receipt and Alice's payment, and
   Alice receives the 1 ETH. This must happen before Bob can take his ETH
   back (below): Alice's wallet, or any watcher, does it.

| If | Then |
|---|---|
| Bob does not lock the 1 ETH in time | The swap is cancelled. Alice has paid nothing on Bitcoin; she loses the job's fees |
| Alice never pays | Bob's tagged transaction is a close: no payment. It must be mined after the payment deadline. Once its lock has ended with no challenge won, Bob takes his 1 ETH back. An attest does not shorten this: Alice keeps the whole lock to prove a payment |
| Bob's close is mined before the payment deadline | It does not count as a close. Bob does not take his 1 ETH back through it |
| Alice paid, and Bob sends a close anyway | Her payment is in a block below Bob's close, on the chain Bob proved, which the protocol's guardians watch. She proves that, and receives the 1 ETH at once (D10) |
| Bob sends nothing and misses his deadline, is slashed, or closes within the payment blocks | Alice proves her payment in blocks on top of the parent of Bob's anchor (so a reorganisation that dropped the anchor does not strand her), with as many confirmations as the job asks. She receives the 1 ETH at once. Faking this means mining that many blocks at Bitcoin's full difficulty, about 19 BTC for 6. It needs Bob to have failed first, so one made-up branch cannot be used against swaps whose operators did their duty; each swap also has a size limit |
| Nobody proves anything | 36 hours after the job's deadline Bob takes his 1 ETH back. Until then Alice may prove her payment |
| Bob is slashed, in either direction | The application's share of his escrow, 80% of x (D7), goes to the swap's user. Anyone may pass it on, once per swap |

A payment counts once. Bob's address is new for each swap, so a payment
to it belongs to that swap only. Bob's own coins must never be at that
address: a transaction of his that spends one would read as a receipt.

## Limits of the Solana program

- A Solana transaction holds at most 1,232 bytes. The user's payment is
  sent inside the instruction that proves it, so a payment with many
  inputs may not fit; an address lookup table makes room for a payment of
  a few inputs. The app asks the user to pay from few coins.
- The records of a swap and of an operator's named script are not closed:
  their rent stays locked.
- A token can carry rules of its own (a permanent delegate, a freeze
  authority, a transfer hook) that could move or freeze what Conversion
  holds. Conversion does not check a token's rules; an operator's node
  only takes swaps of tokens its settings list. The same holds for ERC-20
  tokens on Ethereum.

## Open questions for the owner

| # | Question | Recommendation |
|---|---|---|
| C1 | Is the operator the other side of the swap, as above? | Decided 2026-09-29: yes |
| C2 | Who sets the price? | Decided 2026-09-29: the user proposes it; operators bid only on swaps they find worth it |
| C3 | The escrow x | Decided 2026-09-29: the protocol minimum (5 times the commitment fee). In a conversion the Conversion contract itself keeps the user's coin safe: it is refunded, or, for Bitcoin to token, locked by the operator before the user pays. The 125% of D36 stays the rule for transfers between programmable networks, where the escrow is what repays a loss |
| C4 | How the user's own payment proof (D10) is protected | Decided 2026-09-29: it must lie on the chain the protocol protects (below Bob's close, or on top of Bob's anchor at full difficulty), with a payment deadline and a size limit per swap. No challenge or attest inside Conversion |
| C5 | Which coins? | Decided 2026-09-29: the network's own coin and tokens (ERC-20, SPL) |
