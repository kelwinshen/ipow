# Vault and receipts in the protocol (proposal)

**Status: approved by the owner on 2026-10-01 and moved into section 11
of [`../design/ipow-protocol.md`](../design/ipow-protocol.md), which is
authoritative.** Two changes were made on the way, both approved by the
owner: a message counts once its block is below a real block of a
finished job, instead of the report and challenge of P1 and P2 below
(D108); and the vault is a separate contract of the protocol with the
bonds in it, because the protocol contract is at Ethereum's size limit
(D104, D117). This document keeps the discussion. Earlier status: A protocol extension. The
owner chose on 2026-10-01 to make locking and minting across networks a
primitive of the protocol, instead of an application: "lock here, receive
a receipt there" is offered by the protocol, as opening a job is. It
replaces the application version of this document, approved earlier the
same day; the owner's decisions on that version that still apply are kept
under "Decisions". It changes rules of
[`../design/ipow-protocol.md`](../design/ipow-protocol.md), listed under
"Changes to the approved design". Nothing is changed there until the
owner approves this proposal. BETA ([`ipow-beta-app.md`](ipow-beta-app.md))
stays an application, built from receipts.

## What it promises

A network can never read another network's state, and nothing on Bitcoin
can prove what a contract on another network did: only a Bitcoin key
writes on Bitcoin, never a contract.

| Fact | How it is protected | Guarantee |
|---|---|---|
| A payment on Bitcoin (Conversion) | Proven by a contract through the light client | No trust |
| A lock on Ethereum, acted on by Solana | Objections on Solana for 7 days, and a proof on Ethereum that slashes the operator's bond there, in ETH | Safe while at least one honest guardian watches |

The documents call the second row "safe while one honest guardian
watches", never "trustless".

## The parts

| Part | What it is |
|---|---|
| Vault | Part of the protocol on every network. It holds locked assets, issues and burns receipts, and judges records about its own network. No owner, no key (D59) |
| Receipt | A token on one network for an asset locked in the vault on its home network: 1 vETH on Solana for 1 ETH locked on Ethereum |
| Record | A fact written by a vault: "lock #99: 1 ETH, 1 vETH to this Solana address, fee f", or "request #7: pay 1 ETH to this Ethereum address, fee f" |
| Batch | Every record an operator gathered since its last message. One message carries one batch |
| Pair chain | For each pair of networks, a chain of Bitcoin transactions of one operator, followed by both networks. It is separate from the operator's chain heads (D34) |
| Operator | The protocol's operator. Carrying records is part of its work; users pick nobody |
| Guardian | Anyone who objects to a false claim, reports a message a network has not seen, or challenges a proof on fake blocks. Paid from the loser |
| Attester | Locks the claim's own asset on the acting network to make a claim act at once |

## The pair chain

Each operator that carries records has one pair chain per pair of
networks (V5), in the first version only Ethereum and Solana.

| Part | Rule |
|---|---|
| Registration | One Bitcoin transaction makes the first coin of the chain and names the operator's address on both networks in its `OP_RETURN`. Each network accepts it only from the address it names there, as a chain head today (N22). Both networks then know the same operator on both sides |
| A message | A Bitcoin transaction that spends the chain's current coin and carries the Merkle root of one batch in an `OP_RETURN`. The next coin is the output with the same number as the input (N23) |
| Order | Each network processes a pair chain's messages in order and never skips one |
| BOND | The first message states the bond the operator keeps in the protocol on each network for this chain: "10 ETH on Ethereum". On Ethereum the protocol reserves it, or finds the statement false. On Solana it is a claim with 7 days for objections, like any other |
| EXIT | Ends the chain. A network refuses every later message. The reserved bond becomes free once the network has processed every message before EXIT, and every claim it made has ended |

## How a message reaches a network

A network must not act on a message that exists only on fake blocks.
The light client checks the work of blocks, not the signatures in a
transaction, so on blocks someone mined themselves, anyone can "spend"
an operator's coin. The protocol already solved this for jobs (N29, D89,
section 6.3); a message reaches a network in one of two ways, both
protected the same way.

| Route | Who submits | Protection against fake blocks |
|---|---|---|
| Proof | The operator itself, naming its branch, with a fresh anchor and 6 confirmations, as a job's proof (D39, D89) | Section 6.3 as for a job: a guardian asks for parents or shows a heavier branch; the operator's branch must win |
| Report | Anyone, when the operator has not proven a message on this network. The reporter names its branch as the operator would | Section 6.3 with the roles swapped: the operator, or anyone, may ask the reporter for parents or show a heavier branch. The reporter's branch must win |

A message counts on a network once its lock has ended with no challenge
won: 36 hours, as a settlement (D50). The loser of a challenge loses its
deposit to the winner, as today.

A report makes hiding impossible: a message shown to Solana and never
proven on Ethereum is reported to Ethereum by a guardian. A report on
fake blocks cannot frame an honest operator: its real chain is heavier,
and the report loses.

## What each network does with a record

| Record | The network that holds the fact judges it | The network that acts on it |
|---|---|---|
| LOCK #99 (Ethereum) | Against lock #99. True: the operator earns the lock's fee, for the first message that carries it. False: the operator's whole bond on Ethereum is slashed | Holds its batch as a claim for 7 days of objections, then issues vETH. Issues a receipt for a lock once, and never for a lock it marked never usable |
| REQUEST #7 (Solana) | Against burn #7. True: the operator earns the fee. False: the operator's whole bond on Solana is slashed | Holds its batch as a claim for 7 days of objections, then pays ETH. Pays a request once |
| CANCEL #99 (Solana) | Solana marked lock #99 never usable | Ethereum returns the lock to its owner after 7 days of objections |
| BOND | Against the bond reserved there | The other network counts it after 7 days of objections |

A batch is one statement: if any record in it is false, the whole
message is false. The true records of a refused batch are carried again
in a later one.

## Bonds

- The bond is the operator's protocol bond, reserved for the pair chain
  by its BOND message (D31). For a native coin, the coin of the home
  network is the asset: ETH on Ethereum covers claims about ETH. No
  price is needed.
- The bond is 125% of what it vouches for, as a job's escrow is 125% of
  what the application needs back (D36, D40): on the acting network, the
  operator's open claims of this chain, every batch still in its 7 days,
  never exceed 80% of its BOND, counted in the asset. With 10 ETH
  reserved, an operator carries up to 8 ETH at a time.
- A proven false record slashes the whole reserved bond (V2), split as a
  job's escrow (D7, D35): 80% to the backing of the receipt lied about,
  which covers everything the operator could have lied about, and 20% to
  the reporter or guardian who brought the message. When the operator
  proved its own false message, the 20% goes to the backing too.
- On the acting network, the operator and anyone who objects or answers
  puts down the flat deposit of V1.

## The objection window

The acting network cannot judge a record from another network. The side
left unanswered for a whole window wins, and an honest side always
answers.

| Part | Rule |
|---|---|
| Who may object or answer | Anyone, with the flat deposit |
| Effect of an objection | The claim is held |
| Effect of an answer | The hold is lifted, and the 7 days restart in full (V4) |
| Decided true | The claim acts. The objecting side's deposits go to the answering side |
| Decided false | The claim is refused. The operator's deposit and the answering side's deposits go to the objecting side. The network refuses the rest of the chain |

## Mint, redeem and a lie

### ETH to vETH (Alice)

1. **Ethereum:** Alice locks 1 ETH in the vault with a fee: lock #99.
2. Carl, an operator with a pair chain and 10 ETH reserved, which lets
   him carry up to 8 ETH at a time, puts lock
   #99 in his next batch and writes its root on his pair chain.
3. **Solana:** Carl proves the message. After 36 hours it counts, and its
   batch is a claim for 7 days. Nobody objects. Alice receives 1 vETH.
4. **Ethereum:** Carl proves the same message. After 36 hours it counts.
   Lock #99 is real: Carl earns the fee.

### vETH to ETH (Bob)

1. **Solana:** Bob burns 1 vETH with a fee: request #7.
2. Carl carries it the same way. Ethereum pays Bob 1 ETH after the 7
   days; Solana finds request #7 real and Carl earns the fee.

### A lie (Mallory)

1. Mallory, an operator with 5 ETH reserved, so 4 ETH of open claims at
   most, writes a batch with "lock #100: 4 ETH" and proves it on Solana
   only.
2. **Solana:** Gina objects. Nothing is issued unless Mallory answers
   every objection for good.
3. **Ethereum:** Gina reports the message. After 36 hours it counts. Lock
   #100 does not exist: Mallory's 5 ETH is slashed, 4 ETH to vETH's
   backing and 1 ETH to Gina.
4. If nobody objected on Solana and 4 fake vETH were issued, the 4 ETH
   of the slash back them in full, in ETH.

### A forgery (Mallory against Carl)

Mallory mines 6 fake blocks with a message "from" Carl claiming "lock
#100", and reports it to Ethereum. Carl shows the real, heavier chain,
where his coin was spent differently or not at all. The report loses,
Mallory's deposit goes to Carl, and nothing of Carl's is slashed.

## Fast paths

| Direction | Attester | If the claim is false |
|---|---|---|
| ETH to vETH | Locks 1.25 vETH on Solana; vETH is issued at once | 1 vETH of it is burned in place of the one wrongly issued, 0.25 goes to the guardian |
| vETH to ETH | Pays Bob 1 ETH now from its own ETH; the vault repays it after 7 days | The vault never repays it |

At first nobody holds vETH, so every mint takes the slow path. Fast
redemption works from the first day.

## Many networks and tokens

Each asset crosses on its own pair of networks, with its own pair chains
and bonds in its own asset: no pair needs a price, and a lie that slipped
through on one pair affects only that pair's receipt. Tokens (USDC, RWA
tokens) come after native coins: a bond for a token is reserved in that
token. A network added later needs the light client and the vault there
(stage 7).

## BETA on it

BETA is an application: a basket of up to 8 parts on one network, for
example 1 SOL + 1 vETH, with creator fees. See
[`ipow-beta-app.md`](ipow-beta-app.md).

## Changes to the approved design

| Rule today | Proposed |
|---|---|
| D5, D51: value moves only through applications, and the money a user receives is supplied by the application | The protocol's vault holds locked assets and issues receipts |
| D94: the operator only carries the application's message; nobody declares a claim false | For vault records, the operator vouches for its batch. The home network judges it and slashes a false one; the acting network can refuse it by an objection that stands for a full window |
| D11: a claim is official when attested or when its challenge period has passed | For vault records: when attested, or when the 7 days have ended with no objection standing |
| D34: each operator has one chain head per network | Also one pair chain per pair of networks, for vault records |
| D89: only the operator of a job submits its proof | For pair-chain messages, anyone may also report one, protected by section 6.3 with the roles swapped |
| Section 6.1: an escrow is slashed for a missed deadline or a fake branch | Also: the bond reserved for a pair chain, for a false record |
| Section 7.3: an attester locks the network's coin | For vault records, the attester locks the claim's own asset, 1.25 times its amount |

Jobs, auctions, fees and every other rule stay as they are.

## Decisions

Kept from the application version (the owner, 2026-10-01):

| # | Decision |
|---|---|
| V1 | The flat deposit on the acting network, for the operator's claim and to object or answer: 5 times what one full objection costs a guardian there, measured once built and approved by the owner before deployment |
| V2 | A proven false record slashes the whole reserved bond |
| V4 | An answer restarts the full 7 days |
| V5 | One pair chain per pair of networks |
| V6 | First version: ETH on Ethereum, vETH on Solana, native coins, batching. Slow paths first, then fast paths. Tokens after; more networks in stage 7 |
| V7 | The user attaches a fee and picks nobody. The fee goes to the first operator whose message carrying the record is judged true |

Decided on this version:

| # | Question | Decision |
|---|---|---|
| P1 | How long before a message counts on a network, for the fork challenge? | **Decided by the owner on 2026-10-01**: 36 hours, as a settlement (D50). It fits inside the 7 days of objections |
| P2 | Who may report a message? | **Decided by the owner on 2026-10-01**: anyone, with a deposit equal to a job's commitment fee (D81), which goes to the operator if the report loses |
| P3 | When an operator proves its own false message on the home network, who gets the 20%? | **Decided by the owner on 2026-10-01**: the bond is 125% of what it vouches for, and a slash is split 80% to the backing and 20% to the guardian, as a job's escrow. With no guardian, the 20% goes to the backing too; the liar never gets any of it back |
