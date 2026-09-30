# iPoW protocol

**Status: approved design, being built. Not live.** The owner approved
this document as the spec for the new contracts on 2026-09-28. The
system that is live today is described in [`ipow.md`](ipow.md) and
[`ipow-implementation.md`](ipow-implementation.md). The build plan is
[`../drafts/ipow-build-plan.md`](../drafts/ipow-build-plan.md). What is
built so far is listed in "Build status" at the end.

The project owner decided 101 items on 2026-09-28 and 2026-09-29. Each
rule below names the decision it comes from, as (D1) to (D101). The index
at the end lists all 101. Anything the owner has not decided is listed under "Not decided"
or "To verify during the detailed design".

## Goal

iPoW is a general layer that any programmable network uses to
communicate with any other, with Bitcoin's chain of blocks as the shared
source of truth. Applications such as Beta and Conversion are built on
top of it. They hold no operator logic of their own.

## Terms

| Term | Meaning |
|---|---|
| Protocol | The iPoW contracts on a network: the light client, the operators, the jobs |
| Application | A contract that uses the protocol. It opens jobs |
| Light client | The store of Bitcoin blocks on a network |
| Operator | Whoever locks a bond and takes jobs |
| Bond | The money an operator locks in the protocol. Stake means the same |
| Job | One piece of work an application asks for on one network |
| Escrow | The part of an operator's bond that secures one job |
| x | The escrow amount an application submits for a job |
| y | The value the application needs back if the operator fails. x is 125% of y |
| Tagged transaction | The Bitcoin transaction an operator publishes. The tag names the transfer it settles |
| Chain head | A Bitcoin coin registered under an operator's name. Each tagged transaction spends it and creates the next one |
| Anchor | The Bitcoin block an operator jumps to for a job |
| Window | The Bitcoin blocks that follow the anchor: 30 for a job with 6 confirmations, at most 100 |
| Epoch | 2,016 Bitcoin blocks. Every block in an epoch has the same difficulty |
| Epoch start | The first 6 blocks of an epoch |
| Settlement | A tagged transaction that both networks check for themselves |
| Claim | A tagged transaction that states a fact about another network |
| Challenge period | The time in which the proof of a claim can be challenged before the claim counts as official |
| Guardian | Whoever proves an operator's failure to the protocol |
| Attester | Whoever locks its own money in place of an operator's escrow |

## 1. Protocol and application

| Layer | Contains | Secured by |
|---|---|---|
| Protocol, light client | The Bitcoin blocks. Operators anchor and stream | Proof-of-work checks, the jump guards, the punishment for a missed duty |
| Protocol, operators | The bond, the job, the auction, the duty, the fees, the punishment, and a general way to carry a claim: challenge period, attest, slashing | Bond, slashing, challenge period |
| Application | Its own messages, what they mean and whether they are true, the money users receive, the amounts | The protocol it builds on |

- The protocol handles programmable network to programmable network. It
  moves no BTC for users. Bitcoin carries the operators' transactions
  only. (D4)
- Beta and Conversion are applications. The operator role, its bond and
  its punishment belong to the protocol. (D5)
- The money a user receives is supplied by the application. The protocol
  handles only the operator's bond. (D51)
- An application may add Bitcoin conversion on top of the protocol. (D8)
- Anyone can build an application and register it with the protocol. No
  approval. (D63)
- Only a registered application can open a job. (D83)

## 2. Light client

### 2.1 One shared store per network

- Each network has one light client, shared by all operators. (D48)
- Every Bitcoin block has a unique id, calculated from its content.
  Blocks are stored by that id. (D48)
- Every block names the block before it.
- No block is stored under a number below a lowest number, fixed when
  the contract is deployed: about 4 weeks below Bitcoin's height at that
  moment. Bitcoin is far past such numbers, so a label like 0 can only
  come from someone cheating. The 4 weeks leave room for the 2,016
  questions for a parent (D92). (D93)
- An honest operator anchors only on a block stored under its real
  number. The contract cannot check a number, so someone may store real
  blocks under a false one. An anchor with a false number can meet a
  false epoch boundary when a guardian asks for parents, and then the
  operator cannot answer. The operator service must check the number
  against Bitcoin before it anchors.
- Anyone can add a block that passes the checks, not only operators,
  provided the light client keeps forks apart, so that one party's
  blocks do not disturb what another party needs. (D64) Whether storing
  blocks by id achieves that is still to verify, see V1.
- A fake block never replaces a real one. It is stored beside it, on its
  own branch.

```
block 100 -- 101 -- 102 -- 103        real blocks, streamed by operator A
        \-- 101' -- 102'              fake blocks, streamed by operator B
```

| | Shared? |
|---|---|
| The storage. A block already stored is not stored twice | Yes |
| The epoch start | Yes |
| The anchor, the window and the duty of a job | No |
| Responsibility and punishment | No |

When an operator wins a job it chooses the anchor for that job. The
proof must be in a block that continues from that anchor. A job is
checked against its own operator's branch only.

An operator never has to finish another operator's work. If operator A
abandons its window, operator B anchors at a newer block and streams
from there. Only A is punished, when its deadline passes. (D48)

| Situation | Who is harmed | Who pays |
|---|---|---|
| An operator streams fake blocks and does nothing with them | Nobody | That operator, for the mining |
| An operator streams fake blocks hoping another builds on them | Nobody. The other takes the block's id from Bitcoin, and the fake has a different id | That operator |
| An operator uses fake blocks to prove its own job | The application behind that job | That operator, from its escrow |

How the contract stores a block. The contract cannot prove a block
number, so the number is stated by whoever submits an epoch start or a
jump. A block is stored under three values together: its id, its stated
number, and the time of the first block of its epoch.

| Who | What it states | Result |
|---|---|---|
| Two honest operators | The same values for the same block | One shared entry. The block is not stored twice |
| Someone who states a wrong number for a real block | Other values | An entry of its own. The honest entry is not touched |

An entry holds no link to the entry before it. The block before is found
from the id that every Bitcoin block carries of its parent. An entry is
therefore the same whether it arrived by a jump, by an epoch start or by
streaming.

### 2.2 The jump

The loop of the protocol is anchor, duty, punish. (D2)

1. An operator jumps the light client to an anchor.
2. The anchor starts a window, 30 blocks by default. The operator
   streams them without gaps.
3. An operator that fails its duty is punished economically.

A jump is guarded by the protocol itself. The user does not need to
check the anchor. (D3)

| Guard | Rule | Stops | Decision |
|---|---|---|---|
| Epoch start | The first 6 blocks of the epoch are in the light client, and the anchor has the same difficulty as them | An operator lying about difficulty | D44 |
| Which epoch start counts | One whose difficulty is at least half of the highest epoch start recorded in the last 4 weeks. The contract measures the 4 weeks by the time written in the first block of each epoch start, in whole days. Anyone may submit one | A cheap fake epoch start, and someone blocking honest jumps | D45 |
| Epoch start age | At most 4 weeks older than the anchor | The epoch start of an old epoch with a low difficulty | D57 |
| Minimum difficulty | 2^45, for the anchor and for every other block the light client stores | Cheap fake blocks where no real epoch start is recorded yet | D38 |
| Anchor age | At most 2 hours | A real but old block used as an anchor | D39 |
| Duty | It must continue from the anchor until the deadline | Any anchor the operator cannot build on | D3, D14 |

An epoch start is not a claim and has no challenge period. It counts
from the moment it is in the light client, for anchors up to 4 weeks
younger than it.

An honest operator pays only the cost of streaming 6 real blocks, once
per epoch. Every operator shares the result.

The contract cannot check that the 6 blocks are the first of an epoch,
because a block number cannot be proven. Any 6 real blocks in a row of
the last 4 weeks pass. What an epoch start pins is a difficulty that
Bitcoin used in the last 4 weeks.

If Bitcoin's difficulty falls by more than half within 4 weeks, the new
real epoch start does not count until the older ones are more than 4
weeks old. The contract compares by whole days, so this can take up to a
day more than 4 weeks. This has not happened in Bitcoin's history.

Continuity, meaning each block names the one before it:

| Where | Checked? |
|---|---|
| Inside the epoch start, its 6 blocks | Yes |
| Inside a window, from the anchor onward | Yes. When the window crosses into a new epoch, the new difficulty is checked by Bitcoin's own rule (D72) |
| Between the epoch start and the anchor | No. The blocks in between are skipped |

What an operator can try:

| The operator | Result |
|---|---|
| Anchors on an old real block | Rejected at once by the anchor age |
| Anchors on a real recent block but states a wrong block number | Nothing, until the window reaches the end of the epoch, the stated one or the real one. There the difficulty check fails and the operator cannot continue. Corrected on 2026-09-28: the earlier text said the next blocks stop fitting at once, which the tests showed to be wrong |
| Anchors on a fake block | Every following block must also be fake, and each costs real mining. If it stops, it fails its duty and is slashed |

The anchor is tied to the epoch by its difficulty, by the 4-week limit,
and by its own age of at most 2 hours.

D45 was changed twice on 2026-09-28. First from "the higher difficulty
counts", which let someone block every jump in an epoch. Then from "the
highest one recorded for that epoch" to "the highest one recorded in the
last 4 weeks", because the contract cannot know which epoch a block
belongs to: a block number cannot be proven. With "for that epoch" a
forger could name an epoch that has no real epoch start recorded.

A proof of the block number was considered and dropped. For a real block
a wrong number only hurts the operator. For a fake block the operator
builds the block itself and can write any number into it. The block
number of an anchor and of an epoch start is therefore stated by whoever
submits it, and the contract does not prove it.

Crossing into a new epoch (D72). A window may cross the point where
Bitcoin changes its difficulty. The light client calculates the new
difficulty the way Bitcoin does, from the time of the first block of the
old epoch and the time of its last block, and accepts the next block
only with that difficulty. There is no gap in service at an epoch
change.

### 2.3 Why these numbers

Anchor age of 2 hours. Every Bitcoin block carries the time it was
mined. The protocol compares that time with the network's current time.
An operator cannot change the time in a real block without mining it
again. The window starts at the anchor, so an old anchor uses up part of
the window before the job starts.

| Oldest anchor allowed | Blocks already past, at most | Blocks left in the proof range of 25 | Honest operators rejected because Bitcoin was slow |
|---|---|---|---|
| 1 hour | About 6 | About 19 | About once every 3 days |
| 2 hours | About 12 | About 13 | About once every 3 years |
| 3 hours | About 18 | About 7 | Practically never |

The rejection figures count only slow blocks. The time written in a
block can also be somewhat off, which makes rejections a little more
frequent.

With 2 hours, a new transaction has at least 13 blocks, about 2 hours,
to be confirmed inside the proof range.

Epoch start age of 4 weeks. An epoch lasts 2,016 blocks: 14 days at 10
minutes per block, about 20 days at 14 minutes, 28 days at 20 minutes.

Minimum difficulty of 2^45, changed from 2^44 on 2026-09-28. Bitcoin's
difficulty on 2026-09-28 is about 2^46.9. The largest fall in Bitcoin's
history was about 45%, in 2021.

| Minimum | Compared with 2026-09-28 | Bitcoin can fall this much before the light client stops | Rough cost to forge 6 blocks | Rough cost to forge 12 blocks |
|---|---|---|---|---|
| 2^44 | 7.5 times lower | 87% | About 2.5 BTC | About 5 BTC |
| 2^45 | 3.8 times lower | 73% | About 5 BTC | About 10 BTC |
| 2^46 | 1.9 times lower | 47% | About 10 BTC | About 20 BTC |

The last column is kept from the discussion of the minimum. A forger
mines 7 blocks, see section 2.5.

### 2.4 Forks and confirmations

- A proof needs 6 confirmations, counted the way Bitcoin counts them:
  the proof's own block is the first, and 5 more blocks are on top. The
  contract enforces it. (D15)
- 6 is the default. A job may request more. (D41)
- For each confirmation above 6, the window grows by one block. The
  proof range stays blocks 1 to 25. (D68)
- The window is at most 100 blocks, so a job requests at most 76
  confirmations. (D70)
- A fork harms only the operator that streamed the dropped block. The
  real chain never continues from it, so that operator cannot finish its
  window and fails its duty. (D16)

The window of a job with 6 confirmations is 30 blocks, numbered 1 to
30:

| Blocks | May hold the proof? | Purpose |
|---|---|---|
| 1 to 25 | Yes | Proof range |
| 26 to 30 | No | Only confirm the blocks before them |

With more confirmations:

| Confirmations | Window | Proof range | Blocks that only confirm |
|---|---|---|---|
| 6 | 30 blocks | 1 to 25 | 26 to 30 |
| 7 | 31 blocks | 1 to 25 | 26 to 31 |
| 12 | 36 blocks | 1 to 25 | 26 to 36 |
| 76 | 100 blocks, the maximum | 1 to 25 | 26 to 100 |

D68 is the owner's own proposal of 2026-09-28.

When a proof is submitted the contract checks:

1. The proof's block is inside the proof range of the window.
2. Five blocks, or more when the job requests more confirmations,
   exist in the light client on top of that block, each
   naming the one before it.

Today the operator service only streams blocks that are already buried,
through its `high_finality_confirmed_block` setting. The contract cannot
see that setting. An operator may still wait before streaming, for its
own protection.

### 2.5 What an attack costs

Costs are rough. One real Bitcoin block earns about 3.125 BTC. The
figures assume mining power can be rented at the market rate and ignore
transaction fees.

| What someone tries | Blocks to mine | Cost |
|---|---|---|
| Forge a proof when a real epoch start is recorded | 7 at half the real difficulty | About 11 BTC |
| Forge a proof when no real epoch start is recorded yet | 7 at the minimum of 2^45 | About 6 BTC |
| Block honest jumps for 4 weeks and up to a day more | 6 above twice the real difficulty | About 37 BTC |

Why 7 blocks. The 6 blocks of a forger's own epoch start name each other
in order, so the first serves as the anchor and the next five as blocks
of the window. One more block completes 6 confirmations. The owner
accepted this on 2026-09-28. (D75) An earlier version of this table said
12 blocks, about 19 BTC and about 10 BTC.

The table is for a job with 6 confirmations. A job whose value is above
the cost of forging requests more confirmations. With the maximum of 76
confirmations a forger mines 77 blocks: about 64 BTC where no real
epoch start is recorded, about 120 BTC where one is. Above that value,
confirmations alone do not cover a job. A forged proof can also be
challenged, see section 6.3.
A job that already has its anchor keeps it when someone blocks new
jumps.

Where a window crosses into a new epoch (D72), a forger chooses the
times written in its own blocks. It can make the blocks after the change
cost about half. The forger takes a real epoch start that is almost 4
weeks old, mines the anchor at that difficulty, states it as the last
block of the epoch, and mines 6 blocks at half of it. That is the work
of 4 blocks at the difficulty of 4 weeks ago, about 12.5 BTC when the
difficulty did not change. It is cheaper than the 7 blocks above only
when the difficulty rose by more than about 14% in those 4 weeks. No
block goes below the minimum difficulty of 2^45. The owner accepted the
crossing on 2026-09-28. An earlier version of this text said this route
is never cheaper, which the second review of the contract showed to be
wrong.

## 3. Operator

- Anyone can become an operator by locking a bond. No approval. (D12)
- There is no minimum to register, because taking a job requires
  locking escrow. (D43)
- An operator leaves freely. (D43) Only free bond can be withdrawn, as
  the owner stated on 2026-09-28. Bond locked as escrow stays locked
  until the lock of that job ends, see section 4.4.
- One operator identity does both jobs: it streams blocks and publishes
  tagged transactions. (D13)
- An operator locks a bond and nothing else. Escrow is the part of that
  bond that secures one job. (D31)
- An operator needs a bond on every network where it takes jobs.
- The bond, the escrow and both fees are in the network's own coin.
  (D84)

Chain head:

- Every tagged transaction must come from the operator. It spends the
  operator's chain head. This applies to settlements and to claims.
  (D30)
- Each operator has one chain head per network. A network processes
  only the messages meant for it. (D34)

| Part | Rule |
|---|---|
| Registration | The operator registers a separate chain head on each network it works on |
| A message for one network | Spends the operator's chain head for that network |
| A message for two networks | One Bitcoin transaction that spends both chain heads |
| What each network checks | Only that its own chain head was spent |

With a single chain head for all networks, every network would have to
process every message in order, including messages meant for others.
The live run of 2026-09-23 met this problem.

## 4. Job

### 4.1 Opening a job

| The application states | Rule | Decision |
|---|---|---|
| The escrow x | x is already 125% of y. Never zero. At least 5 times the commitment fee | D19, D36, D40, D55, D56 |
| The escrow fee | 0.5% of x by default. May be lower or higher, at most 100% of x | D22, D25, D77 |
| The confirmations | 6 by default. May be more, up to the maximum | D41, D70 |

The job also pays the commitment fee, see section 5.

Without escrow an operator could abuse the job at no cost. (D55)

One job has one escrow, locked on the network where the job is opened.
A network can only slash what is locked on it. The application decides
whether to open a job on one network or on both. Each job has its own
escrow and its own fees. (D52)

A false proof can cause a loss on either network of a transfer:

| Where the operator cheats | What happens | Who loses |
|---|---|---|
| A false proof on network A | A releases the user's value, but B never pays | The user, on A |
| A false proof on network B | B pays out, but A never releases the locked value | The application, on B |

Either side may open the destination job of a transfer. (D21)

| Case | Who pays the fee on the destination network |
|---|---|
| Claim, such as a Beta mint | The user, who acts on both networks |
| Transfer, destination job opened by the user | The user pays there |
| Transfer, destination job opened by the operator | Taken out of what the user receives there |

A user who is new to the destination network holds nothing there, so
the operator opens the job. The application shows the user the
operator's destination job before the user locks anything on the
source.

### 4.2 Auction

The operator for a job is always chosen by auction. (D29)

| Step | What happens | Decision |
|---|---|---|
| 1 | The job requests the escrow x. That is the minimum bid | D32 |
| 2 | An operator bids by locking x or more from its free bond | D19, D32 |
| 3 | Another operator may outbid it by locking at least 0.1% more. The outbid operator's bond becomes free again | D32, D76 |
| 4 | The operator that locked the most wins | D32 |
| 5 | Bidding is open for 15 minutes. The winner is locked in once 1 minute passes with no better bid | D37 |
| 6 | Bidding ends 15 minutes after the job was opened, or 1 minute after the last bid when that is earlier. A bid in the last minute does not extend the 15 minutes | D82 |

If no operator has enough free bond, the job is not processed. (D19) An
application may split a large transaction into smaller jobs.

A job with no bid when the auction closes expires. The fee and the
user's locked value are returned. (D61) The owner agreed to "the fee".
Both fees are meant here: no operator did the job, so under D23 no
operator earns either of them.

The escrow fee is always calculated on x, not on the larger amount an
operator chose to lock. The commitment fee is not auctioned. (D33)

Example: a job requests 1 ETH of escrow. The escrow fee is 0.005 ETH.

| Operator | Locks | Result | Escrow fee if it does the job |
|---|---|---|---|
| A | 1 ETH | Outbid | None |
| B | 1.5 ETH | Wins | 0.005 ETH |

An operator that locks more earns no more, and that bond cannot secure
other jobs in the meantime.

### 4.3 Duty

- For a job with 6 confirmations the window is 30 blocks and the
  deadline is 1 day. (D14) D14 was changed on 2026-09-28 by D68 and D69.
  Before, 30 blocks and 1 day held for every job.
- The protocol calculates the window and the deadline from the number of
  confirmations the job requests. Nobody sets them directly. (D14, D68,
  D69)
- The deadline grows linearly with the window: 48 minutes for each
  block, which is 1 day for 30 blocks. (D69)
- The deadline starts when the auction winner is locked in. (D60)
- The operator's Bitcoin transaction must carry a tag. The tag names the
  transfer it settles. A BTC amount is optional, and the protocol never
  requires one. (D6)
- A duty is fulfilled only by proving the tagged transaction inside the
  window. Streaming blocks alone never fulfils a duty. Every direction
  follows this pattern. (D9)
- The duty ends when the proof is accepted. The operator does not have
  to stream the rest of the window. (D54)
- A proof after the deadline is rejected. (D27)

The tag (D6, D80):

| Part | Rule |
|---|---|
| Who makes the tag | The application, when it opens a job. It is 32 bytes. The protocol does not look inside it |
| What an application puts in it | For a transfer, a hash of the transfer's details: source network, source transfer id, destination network, destination leg id |
| A transfer with a job on each network | The application gives both jobs the same tag |
| What each network checks | The operator's Bitcoin transaction carries that tag in an `OP_RETURN` output, spends the operator's chain head for this network, and was not used for another job on this network |
| One tag, one job | An application cannot open two jobs with the same tag on one network |

Each job has its own auction, so the two jobs of one transfer can have
two different operators. Each operator then publishes its own Bitcoin
transaction with the tag. When the same operator wins both, it publishes
one transaction that spends both chain heads. If one operator fails, its
escrow on that network pays the application there. (D80)

- One transaction settles one job on a network. It cannot settle two.
- A tagged transaction is new, so it cannot exist in an old block.
- The protocol checks the tag only, so operators need BTC for fees only.

Example for a job with 6 confirmations: the operator's transaction is
confirmed in block 3, and blocks 4 to 8 are the 5 blocks on top. After block 8 the proof is accepted, and
the operator streams nothing more for that job.

| For a job with 6 confirmations | Blocks | Normal time |
|---|---|---|
| Time to get the transaction confirmed | 25 | About 4 hours |
| Blocks on top | 5 | About 50 minutes |
| Whole duty | 30 | About 5 hours |

Why 30 blocks and 1 day, for a job with 6 confirmations:

| Bitcoin speed | Time to mine 30 blocks | Spare time before the deadline | Chance an honest operator fails by bad luck |
|---|---|---|---|
| Normal, 10 minutes per block | 5 hours | 19 hours | Practically never |
| Slowest in history, 14 minutes | 7 hours | 17 hours | Practically never |
| 20 minutes | 10 hours | 14 hours | About 1 in 140 million |
| 30 minutes | 15 hours | 9 hours | About 1 in 460 |

With a longer window (D68, D69). The deadline grows as fast as the
window, so an
honest operator is not at more risk:

| Confirmations | Window | Deadline | Time to mine at 10 minutes per block | Chance an honest operator fails by bad luck, at 30 minutes per block |
|---|---|---|---|---|
| 6 | 30 blocks | 24 hours | 5 hours | About 1 in 460 |
| 12 | 36 blocks | 28.8 hours | 6 hours | About 1 in 1,100 |
| 36 | 60 blocks | 48 hours | 10 hours | About 1 in 30,000 |
| 76 | 100 blocks | 80 hours | 16.7 hours | About 1 in 7 million |

An honest operator finishes as soon as the blocks exist, so the deadline
only matters when an operator has failed.

Congestion an honest operator can meet:

| Kind | Effect | Handling |
|---|---|---|
| Bitcoin blocks arrive slowly | The blocks of the window take longer to exist | The spare time |
| Bitcoin fees spike | The tagged transaction waits unless the operator pays more | The proof range gives time to raise the fee. The cost belongs in the operator's fee |
| The programmable network is congested or stops | The operator cannot submit blocks or its proof while the clock runs | The spare time. Solana has had outages of 17 to 19 hours |

No decision makes an exception for a network that was down. Under D27
and D53 an operator that misses the deadline for that reason loses the
full escrow of that job.

### 4.4 After the job

| Job | The money stays locked | Decision |
|---|---|---|
| Every job | 36 hours after the job, or 1.5 times the deadline of the job when that is longer. A proof made on a fake branch can still be challenged during that time | D49, D50, D71 |
| A job that makes a claim | Also for the whole challenge period | D66 |

For a claim, an attest does not shorten the lock. It changes only who
locks the money: the operator, or the attester in its place. (D66)

The application sets the length of the challenge period, between 36
hours and 7 days. The protocol holds the locked money. (D17, D28,
confirmed by the owner on 2026-09-28)

## 5. Fees

A job pays two fees, both at the protocol. (D22)

| Fee | Pays the operator for | Amount | Decision |
|---|---|---|---|
| Commitment fee | Streaming blocks and publishing its transaction | Amount of work x current price x 1.5, calculated on-chain when the job is opened | D20, D24 |
| Escrow fee | The bond it puts at risk for the job | 0.5% of x by default. A job may request lower or higher | D22, D25, D33 |

- Every job pays the commitment fee, even a job that carries no value.
  (D20)
- Each network calculates its own commitment fee. (D22)
- Both fees go to the operator that does the job correctly. (D23)
- Fees always go to the operator recorded for the job, never to whoever
  sent the transaction. (D46)
- A higher escrow fee gives operators a reason to take that job first.
  (D25)
- No person sets a price. Every network's transaction cost is read or
  computed by the contract. (D58)
- The user pays the fees when the job is opened. The protocol holds
  them until the job ends. (D65) D67, decided later on 2026-09-28, makes
  this exact: until the lock of the job ends. When the operator opens the
  destination job, the fee is taken out of what the user receives.
  (D21)
- The price is read at the moment the job is opened, so the user cannot
  know the exact commitment fee before and sends a little more. What is
  sent above the fees is kept for the operator, as part of the
  commitment fee of the job. It returns to the user with the fees when
  the job expires or the operator fails. (D79)
- The minimum escrow of D56 is calculated on the fee of the moment, not
  on what was sent.
- When an operator fails, both fees return to the user. (D62)
- The fees are handed to the operator when the lock of the job ends, see
  section 4.4, and not earlier. An operator proven wrong during the lock
  is paid nothing. The attester is paid at the same moment. (D67)

The operator's costs do not depend on the value of a job: streaming the
blocks of the window, the fee for its tagged Bitcoin transaction, and submitting
the proof. Today the fee is a percentage of the value, so a job with no
value would pay nothing.

A transfer from network A to network B makes the operator stream blocks
and submit a proof on both. Neither network can see the other's prices.

The price is read when the job is opened, and the operator streams over
the following hours. The safety margin of 1.5 covers a price that rises
in between. (D24)

An operator does not accept a job whose fees are too low. That job gets
no bid and expires. (D61)

Example for a job with a window of 30 blocks, on a network where the
commitment fee is 0.0044 ETH:

| x | Commitment fee | Escrow fee | Total |
|---|---|---|---|
| 1 ETH | 0.0044 ETH | 0.005 ETH | 0.0094 ETH |
| 0.022 ETH, the minimum of 5 times the commitment fee | 0.0044 ETH | 0.00011 ETH | 0.00451 ETH |

The commitment fee is calculated for the window of the job, and grows
linearly with it. (D69) A quick operator keeps the difference. When
several jobs use the same anchor, the operator streams the blocks once
and collects the fee from each job.

The minimum escrow of 5 times the commitment fee (D56) uses the fee of
that job, so it grows with the window too. For a window of 100 blocks it
is about 3.3 times the minimum of a window of 30 blocks. The owner has
not been asked about this effect.

Amount of work. The work for one block is a constant in the contract,
measured once. The amount of work for a job is that constant times the
blocks of its window, plus a fixed part for the proof. On the current
`iPoW.sol`, measured on 2026-09-28: about 150,000 gas per block, about
4.4 million gas for 30 blocks. The proof submission is not measured. The
new light client must be measured again.

Price of work. Checked on 2026-09-28 by asking each network's public
test endpoint to run code that returns the price a contract sees.

| Network | Price a contract can read | Result |
|---|---|---|
| Ethereum, Sepolia | The base fee | Readable |
| Base, Sepolia | The base fee, and the system contract that reports the cost of posting data to Ethereum | Both readable |
| Robinhood Chain, testnet | The base fee, and the Arbitrum system contract that reports the cost of posting data to Ethereum | Both readable |
| Hyperliquid, testnet | The base fee | Readable |
| Polkadot Hub, testnet | The base fee. It is a fixed value | Readable |
| Tempo, Moderato | The base fee | Readable |
| Hedera, testnet | The base fee reads as zero. The network fixes its price in US dollar terms, and a system contract converts dollar amounts to HBAR | Computable. The system contract answers |
| Solana | The basic transaction fee and the storage cost are fixed and known. Not tested. The extra fee paid during congestion cannot be read by a program | Computable from fixed values |

Only the test networks were checked.

On Solana every stored block is its own account, which must hold rent,
about 0.0018 SOL. The rent is not spent: a block or an epoch start can be
closed 8 weeks after it was stored, and its rent goes back to whoever
stored it. No rule ever needs a block that old: an anchor is at most 2
hours old, the questions for a parent go back at most 2,016 blocks (about
4 weeks when Bitcoin is slow), and the lock of a job runs at most about
10.5 days after its anchor. So the commitment fee on Solana counts only
the transaction fees: 5,000 lamports per signature, one per block of the
window and 20 for the rest, times 1.5. For a window of 30 blocks that is
0.000375 SOL. (D101)

## 6. Punishment

### 6.1 What is slashed

- A slash takes the escrow of the failed job, not the operator's whole
  bond. (D26)
- A missed duty is slashed 100%, the same as cheating. A user loses
  opportunity when an operator misses its deadline. (D53)
- If the operator locked more than x to win the auction, the amount
  above x is not slashed. It returns to the operator's free bond.

| Failure | Punishment |
|---|---|
| The operator misses its deadline | The full escrow of the job is slashed |
| The operator cheats: a proof on a fake branch | The full escrow of the job is slashed |

A vault bond is slashed for a false record (section 11.4, D109).

### 6.2 Where the slashed money goes

Slashed money has two destinations. This applies to both failures: no
proof before the deadline, and a proof on a fake branch. (D7, D94)

| Share | Goes to | Decision |
|---|---|---|
| 80% of x | The application that was harmed | D7, D36 |
| 20% of x | The guardian | D35 |

The application submits x, already set to 125% of y. (D36, D40)

| | In terms of x | In terms of y | Example, y = 100 |
|---|---|---|---|
| Escrow the application submits | x | 1.25 y | 125 |
| Compensation to the application | 0.8 x | y | 100 |
| Reward to the guardian | 0.2 x | 0.25 y | 25 |
| Escrow fee | 0.005 x | 0.00625 y | 0.625 |

The guardian is paid from the operator's money. The application is made
whole. The protocol receives x from the application and splits the
slash 80% and 20%. (D40)

The minimum escrow is 5 times the commitment fee. The guardian's 20%
then at least covers what it costs the guardian to prove a failure.
(D56)

A guardian claims its reward in two steps. It registers a sealed note of
its proof, then reveals the proof. (D47)

| Case | Rule |
|---|---|
| The operator of the job reports its own missed duty as the guardian | Allowed. It takes back the 20%, so a missed duty costs it 80% of x. The application still receives 80% of x, which is 100% of y (D85) |
| Someone seals notes for every job in advance and reports a missed duty first | Accepted. For a missed duty the guardian shows only the job, so the note cannot tell who found it. It decides only who receives the 20% (D86) |

### 6.3 Challenging a proof

A fake branch mined at the right difficulty passes every check. The real
Bitcoin chain keeps growing with the world's mining power behind it, and
a fake branch cannot keep up. (D49)

A block number cannot be proven, so it cannot say which branches
compete. Every Bitcoin block names its parent, and that cannot be faked.
An honest operator's branch is real Bitcoin, so to beat it someone must
out-mine Bitcoin. A guardian cannot use real blocks from a later time
against it, because those blocks do not name the same parent.

| Part | Rule |
|---|---|
| Who can challenge | A guardian |
| When two branches compete | When they start from the same parent block. The guardian shows a block that names the same parent as the operator's anchor, or as a block between the anchor and the proof (D81) |
| Made-up blocks on top of a real proof block | Not challenged. A branch may start only at the proof's block or below it. If the proof's block stays in Bitcoin, the transaction is real and nothing is wrong. If Bitcoin later drops it, real Bitcoin has another block at its height, and a guardian challenges from there: the operator is slashed and the application receives 80% of x. An operator that makes up the confirmations only proves earlier, at the risk of its escrow. Allowing a branch to start above the proof's block would let a guardian win against an honest operator whose newest block Bitcoin dropped in an ordinary reorganisation, though its transaction is real (D103) |
| Who wins | The branch with more mining work when the lock ends. Until then anyone may add blocks to either branch (D81) |
| The anchor names a parent that is not a Bitcoin block | The guardian asks for the parent. Anyone may show it, within 12 hours. An honest operator shows a real block and pays only gas. A forger must mine one more fake block each time. If nobody shows it in time, the proof is false (D81) |
| The guardian's deposit | A guardian puts up a deposit to challenge, equal to the commitment fee of the job. If the challenge fails, the deposit goes to the operator (D81, D90) |
| Several challenges | Several guardians may challenge one job at the same time. The operator is slashed if any one of them wins. A guardian whose challenge is still open when another has slashed the operator gets its deposit back (D88) |
| An honest operator that is down | If nobody answers a question for a parent within 12 hours, the operator is slashed even when it is honest. It must keep its service running (D90) |
| The deposit grows with each question | The first question, for the parent of the anchor, costs 1 times the commitment fee. The question for the block before that costs 2 times, and question k costs k times. Harassing an honest operator becomes expensive fast, and every deposit goes to the operator. A forger pays one fake block per answer, about 0.8 BTC, which stays far above the guardian's cost (D91) |
| How far back | At most 2,016 questions per job, about 2 weeks of Bitcoin blocks. Real blocks in that range pass the minimum difficulty, so an honest operator can always answer. A forger that survives all of them has mined 2,016 fake blocks (D92) |
| What it shows | A branch in the light client with more mining work than the branch the operator used |
| Result | The operator's proof is declared false. The escrow is slashed: 80% to the application, 20% to the guardian |
| Time to challenge | The lock is fixed when the proof is accepted: 36 hours after the job, or the challenge period of a claim (D50, D66). It never moves. No challenge opens in its last 12 hours, so every challenge is decided within it (D99) |
| How many questions fit | Questions come one after another, and an operator may take up to 12 hours to answer each. With no question in the last 12 hours (D99), a lock of 36 hours surely fits 2 questions, and a third only when each question is asked in the block right after the previous answer. A lock of 7 days fits about 13. A dishonest operator whose fake blocks connect to nothing real therefore mines at least 2 more blocks on a settlement: at least 9 blocks, about 7 to 12 BTC. That is about the cost D75 accepted. The limit of 2,016 questions (D92) is reached only with a very long lock |
| An honest operator in a competing-branch challenge | Either side may add blocks until the last second of the lock, with no time for the other side to answer. The operator's side counts only the blocks someone has shown for it. So an honest operator must keep adding the real blocks Bitcoin produces to its side, on its own, until the lock ends. If it does, a guardian can only win with more mining work than real Bitcoin. The operator service must do this for every open challenge (D99) |
| The first deposit | Equal to the commitment fee of the job, with no minimum. On a job opened while the network was very quiet, the first questions cost little. The deposit grows with each question (D91), every deposit goes to the operator, and only a few questions fit in a lock (D99), so this cannot cost an honest operator anything (D100) |
| Several guardians win | Whoever first resolves a winning challenge receives the 20%, even the operator itself from another address. The application still receives its 80% (D98) |

One fake branch can prove the jobs of one operator that share an anchor:
its transactions follow each other on the operator's chain, and all can
sit in one fake block. The cost of forging, about 11 BTC, is then shared
by those jobs. The challenge of a proof by a branch of real Bitcoin
blocks is the defence. (D87)

### 6.4 Examples

Missed duty, for a job with 6 confirmations. The operator anchors at
block 968,000. Its window is 30 blocks and its deadline is 1 day. It stops at 968,010 without proving
its transaction. After the deadline a guardian shows that the duty was
not fulfilled. The operator loses its escrow, the guardian receives the
reward, and the user's locked value and both fees return to the user.
(D62)

Fake proof. The operator does not publish the application's message on
real Bitcoin. It proves it on blocks it mined itself. A guardian asks
for the parent of the anchor, or shows real Bitcoin blocks that split
from the same parent. The operator's escrow is slashed and the guardian
receives the reward.

## 7. Claims and attest

### 7.1 The operator carries the application's message (D94)

The operator never states a fact. It carries the message the application
wrote, and nothing else. The application puts its message in the tag of
the job (D80), so the protocol refuses a transaction that carries any
other message. Whether the message is true is the application's own
business: its own contract wrote it.

The operator is punished only for failures the protocol can prove by
itself, without reading any application's contract:

| Failure | How the protocol proves it |
|---|---|
| No proof before the deadline | The deadline passed with no proof (D53) |
| A different message | Cannot happen. The tag must match exactly, or the proof is refused |
| A proof on fake Bitcoin blocks | A guardian's challenge (section 6.3) |

| | Settlement | Claim |
|---|---|---|
| Example | A transfer from network A to network B | "Lock #99 exists on network A, so mint on network B" |
| Who writes the message | The application | The application, on the network that holds the fact |
| Is the value in place on both sides before the message? | Yes | No |
| When the receiving network acts | When the proof is accepted | When the claim is official: attested, or its challenge period has passed (D11) |
| What the challenge period is for | Not used | Time for guardians to challenge the proof before the receiving network acts |

A settlement still depends on the light client holding real Bitcoin
blocks. The jump guards protect that.

### 7.2 Challenge period

- The protocol sets limits for the challenge period. Each application
  sets the value for each of its claims, when it registers, and cannot
  change it afterwards. (D17)
- An application registers at most 32 kinds of claim. (D78)
- The limits are 36 hours and 7 days. (D28)
- A claim becomes official when it is attested, or when its challenge
  period has passed. The protocol reports that state. What the
  application does with an official claim is the application's matter.
  (D11)
- Without an attester, a claim becomes official when its lock has ended
  with no challenge open. The lock covers the whole challenge period, and
  no new challenge can open after it, so the claim never stops being
  official. (D97)

The minimum of 36 hours is longer than the 1-day deadline, so a guardian
can always get the blocks in and prove. A job with more than 6
confirmations has a longer deadline, up to 80 hours. Its challenge
period is at least 1.5 times its deadline: 120 hours for a deadline of
80 hours. An application still registers the challenge period of a claim
once (D17), and the protocol uses the longer of the two. (D71)

The challenge period of a claim starts when the operator's proof is
accepted. (D74)

The challenge period is the time an honest guardian has to notice a
proof on fake Bitcoin blocks and challenge it (section 6.3).

With attest, the cost of a long period falls on the attester's locked
money. At a 10% yearly cost of money, 36 hours costs 0.041% of the value
and 7 days costs 0.192%.

### 7.3 Attester

An attester takes the operator's place for a job. (D42)

| Step | What happens |
|---|---|
| 1 | The operator has locked its escrow for a job |
| 2 | An attester locks the same amount in its place |
| 3 | The operator's escrow becomes free, and the operator can use it for other jobs |
| 4 | The attester's money stays locked for the rest of the challenge period (D66) |

| At the end of the period | Result |
|---|---|
| Nobody proved the proof fake | The attester takes back its money, with its reward |
| A guardian proved the proof fake | The attester's money is slashed as the operator's would have been: 80% to the application, 20% to the guardian |

The escrow for a job is always locked by someone: first the operator,
then the attester. Only one amount is at stake for a job.

Once an attester has stepped in, the operator has nothing locked for
that job. If the proof is proven fake, the attester pays, and the
operator loses only its share of the fee. The attester checks that the
operator's proof is on real Bitcoin before it attests: every block of the
proof is in Bitcoin's best chain, at the height the proof states for it,
with the epoch time of that height. A real block stated at another height
can make a question for its parent impossible to answer, and the proof is
then false. (D102)

The attester locks x, the amount the operator needs to escrow. It does
not lock the larger amount the operator bid in the auction. (D73)

The attester earns 40% of the escrow fee, which is 0.2% of x, when it
keeps x locked for the whole lock. It earns less the later it steps in,
in proportion to the time it kept x locked: stepping in halfway earns
20%, stepping in at the last second earns almost nothing. The operator
receives the rest. (D42, D96)

Anyone may attest, the operator of the job too. An application sees who
attested, and may wait for an attester other than the operator if it
wants one. (D95)

| | Nobody attests | Someone attests |
|---|---|---|
| Operator | 0.5% of x | 0.3% of x |
| Attester | None | 0.2% of x |

An attester that locks x for 1 week earns 0.2% of x. Over a year that is
about 10.4%, or about 10.95% when the attester adds each reward to the
money it locks next.

## 8. Front-running

| Where | Can someone get ahead of another? | Does it harm the protocol? |
|---|---|---|
| Auction | An operator outbids another by locking more | No. That is the auction working |
| Streaming blocks | An operator submits a block just before another does | No. Blocks are shared and stored once |
| Epoch start | A forger submits a fake one first | No. The real one counts whenever it is submitted (D45) |
| Submitting a proof | Someone copies an operator's proof and submits it first | Not possible. Only the operator of the job submits its proof (D89). Fees go to the operator recorded for the job (D46) |
| Guardian's reward | Someone copies a guardian's proof and submits it first | No. The guardian claims in two steps (D47) |
| Bitcoin messages | Nobody. Only the operator can spend its chain head | No |

## 9. No person in control

- Anyone can become an operator. (D12)
- Anyone can register an application. (D63)
- Anyone can add a block to the light client. (D64)
- No person sets a price. (D58)
- No key can change or upgrade the contracts after they are deployed. A
  new version is a new deployment. (D59)
- The name is iPoW. No version suffix in contract, file or binding
  names. Done in code. (D1)

## 10. Applications on top

This section describes applications, not the protocol. It shows that the
protocol is enough to build them.

### 10.1 Transfer between two programmable networks

A user moves 1 ETH from Ethereum to Solana.

1. The user locks 1 ETH on Ethereum.
2. The application holds the matching SOL on Solana, from its own funds.
3. An operator wins the job and locks its escrow.
4. The operator publishes one Bitcoin transaction tagged with the
   transfer.
5. Ethereum finds the transaction through its light client and releases
   the ETH to the application.
6. Solana finds the same transaction and the application pays the SOL to
   the user.

If the operator never publishes the transaction, neither side releases
and the user's ETH is returned after the deadline. The deadline is 1
day for a job with 6 confirmations.

### 10.2 Beta mint

A user locks 1 ETH on Ethereum and wants BETA minted on Solana. Solana
has nothing in place and cannot see Ethereum. Beta's own contract on
Ethereum sees the lock and writes the message "lock #99 exists". The
operator carries it. The claim becomes official when it is attested or
when its challenge period has passed, and Beta on Solana then mints.
Beta would set a challenge period of 7 days.

Replaced on 2026-10-01 by the protocol's vault (section 11): BETA is a
basket of receipts on one network.

### 10.3 Bitcoin conversion

The application checks the BTC amount, because the payment is the value
being moved. (D8)

| Direction | What is in place first | The operator's tagged transaction | Who may prove it |
|---|---|---|---|
| Token to Bitcoin | The user's tokens | A payment of the BTC amount to the user's address | The operator |
| Bitcoin to token, user paid | The tokens for the user, held by the application | A receipt that spends the BTC the user paid | The operator, or the user with their own payment proof (D10) |
| Bitcoin to token, user never paid | The tokens for the user, held by the application | A close, saying no payment arrived | The operator |

In a Bitcoin to token conversion, the operator's receipt or the user's
own proof both release the tokens. (D10) Two other options were
considered and not chosen: only the sender proves, and only the operator
publishes.

A close is a claim defined by the conversion application. The protocol
does not know what a close is. It offers only the general claim:
challenge period, attest, slashing. An application that only moves value
between programmable networks has no close. (D18) The user may disprove
it during the challenge period by proving their payment, and the
application then gives the user the tokens. That is the application's
own rule. The protocol does not slash the operator for it, because the
operator only carried the close (D94). A close would set a challenge
period of 36 hours.

The user needs no gas on the programmable network unless they choose to
submit their own proof.

A payment that lands in block 26 or later cannot be proven. The
application shows the user an earlier cut-off than block 25.

## 11. Vault and receipts

Decided by the owner on 2026-10-01 (D104 to D119). The design and the
discussion behind it are in
[`../drafts/ipow-vault-claims.md`](../drafts/ipow-vault-claims.md).

Locking an asset on one network and receiving a receipt for it on
another is part of the protocol, as opening a job is (D104). A user locks
1 ETH on Ethereum and receives 1 vETH on Solana; burning 1 vETH on Solana
pays 1 ETH on Ethereum. An application such as BETA uses receipts on one
network and never crosses networks itself.

### 11.1 What it promises

A network cannot read another network's state, and nothing on Bitcoin
can prove what a contract on another network did: only a Bitcoin key
writes on Bitcoin. A payment on Bitcoin is proven with no trust
(section 10.3). A lock on another network is only **safe while at least
one honest guardian watches**, and is never called trustless (D105).

### 11.2 The parts

| Part | What it is |
|---|---|
| Vault | A contract, or program, of the protocol on each network, deployed with it, with no owner and no key (D59). It is separate from the protocol contract because that contract is at Ethereum's size limit (D104) |
| Receipt | A token on one network for an asset locked in the vault on its home network. vETH on Solana has 9 decimals, so a lock of ETH is a whole number of gwei |
| Record | A fact written by a vault: LOCK, REQUEST, CANCEL, BOND or EXIT (section 11.5) |
| Batch | The records an operator carries in one message. One statement: if any record is false, the whole message is false (D114) |
| Pair chain | One per operator and pair of networks, beside its chain heads (D106) |
| Vault bond | What an operator locks in the vault, in the asset it vouches for, beside its protocol bond (D110, D117). Apart from it, the operator keeps deposit money in the vault, in the network's coin, for the flat deposits of its claims, so that a deposit never changes what a BOND record states |

### 11.3 The pair chain

| Part | Rule |
|---|---|
| Registration | A Bitcoin transaction makes the first coin of the chain, at output 0 or any output it names, and carries `sha256("iPoW pair" ‖ Ethereum vault (20 bytes) ‖ operator on Ethereum (20) ‖ Solana vault (32) ‖ operator on Solana (32))` in an `OP_RETURN`. Each vault accepts it only from the address named for its network, so both networks know the same operator (D106) |
| A message | A Bitcoin transaction that spends the chain's current coin and carries `sha256("iPoW vault" ‖ batch)` in an `OP_RETURN`. The next coin is the output with the same number as the input (N23) (D107) |
| Order | Each vault processes a chain's messages in order and never skips one. Anyone may submit the next message with its batch (D107, D108) |
| Real Bitcoin | A registration or message counts only when its block is below a real block: the proof block of a protocol job whose escrow is at least a minimum fixed when the vault is deployed (D118) and whose lock has ended with no challenge won and no slash, or a block the vault already recorded as real. The light client's walk shows the link. A fake block is never below a real one, so nobody can forge a message of another operator, or hide one (D108) |
| Size (D119) | A message is false when its Bitcoin transaction is longer than 1,024 bytes, its batch longer than 2,048 bytes, or it carries more than 32 REQUEST and CANCEL records together. Every message that counts can then be judged on every network, within one Solana transaction and its buffer. A larger one cannot be processed on Solana, and the operator's chain stops there; Ethereum judges it false and slashes the bond there, and refuses the chain's claims |
| No job for a while | Anyone may open a job through the vault, paying its fees, to make a real block (D116). It asks the minimum escrow of D118. If its operator is slashed, the application's share of the escrow goes to the backing of the receipt |

### 11.4 Bonds

| Part | Rule |
|---|---|
| Where | In the vault, in the asset it vouches for: ETH on Ethereum for records about ETH locks, vETH on Solana for records about vETH burns (D110, D117) |
| BOND | A BOND record states a chain's bond on one network, once per network. The vault holding it checks it against the free bond and keeps it locked; the other network counts it after 7 days of objections. Until then no claim of the chain that moves value opens there (D110) |
| 125% | On the acting network, the open claims of a chain, every batch still in its 7 days, are at most 80% of the bond stated for them (D110) |
| Slash | A false record proven where its fact lives slashes the whole bond there: 80% backs the receipt lied about, 20% goes to whoever submitted the message. When the operator submitted it, the 20% backs the receipt too (D109). A vETH bond is slashed by burning its 80%: fewer receipts for the same ETH |
| Leaving | An EXIT message ends the chain. The bond on a network becomes free once that vault has processed every message before EXIT and every claim of the chain there has ended (D112) |

### 11.5 Records

| Record | Judged by the network that holds the fact | Acted on by the other network, after 7 days of objections |
|---|---|---|
| LOCK (Ethereum: lock #99, amount, Solana recipient, fee) | The lock exists with exactly this record, returned or not: Solana issues nothing for a lock it marked never usable, so an honest message that arrives after a return is not a lie. True: the first message carrying it earns its fee. False: slash | Issues the receipt, once per lock, never for a lock it marked never usable |
| REQUEST (Solana: burn #7, amount, Ethereum address, fee) | The burn exists with exactly this record. True: the first message earns its fee. False: slash | Pays the asset, once per request number, from any accepted claim that carries it. Solana writes no request to address zero, so Ethereum slashes one |
| CANCEL (Solana: lock #99) | Solana marked lock #99 never usable, at the request of its recipient. Solana gives up only a lock it learned from a LOCK record in an accepted claim: a claim not yet decided may carry a false LOCK record, which could otherwise give up any lock, so a CANCEL of a lock that does not exist on Ethereum is false and slashed there | Returns the lock to its owner, with its fee if no message earned it; the fee can then never be earned |
| BOND (a network, an amount) | The bond is locked there. False: slash. A later BOND of a chain for the same network is true when it states the same amount, and changes nothing; another amount is false | Counts the first one carried in a claim that opened; a BOND in a message whose claim could not open can be carried again. A later one is not acted on, and is judged where its fact lives |
| EXIT | Ends the chain on both networks | Ends the chain |

A lock leaves the vault only for an official REQUEST or CANCEL. No timer
returns it (D112). A user attaches a fee and picks no operator (D113). An
operator whose bond was slashed on a network earns no more fees there; a
fee it would have earned waits for another operator.

A LOCK record names the lock by its number, so an operator carries it
only once the lock's Ethereum block is final: a reorganisation of
Ethereum could give the number another lock, and the record would then
be false.

A batch is its records one after another. Numbers are big-endian, and
amounts and fees are in gwei (9 decimals). A batch that does not parse
is false: its hash is what the operator wrote.

| Record | Bytes |
|---|---|
| LOCK | `01` ‖ lock number (8) ‖ amount (8) ‖ Solana recipient (32) ‖ fee (8) |
| REQUEST | `02` ‖ request number (8) ‖ amount (8) ‖ Ethereum address (20) ‖ fee (8) |
| CANCEL | `03` ‖ lock number (8) |
| BOND | `04` ‖ network, 1 for Ethereum and 2 for Solana (1) ‖ amount (8) |
| EXIT | `05`, and nothing after it |

### 11.6 The objection window

The acting network cannot judge a record from another network (D111).

| Part | Rule |
|---|---|
| The claim | The acting records of one message are one claim. The operator's flat deposit is taken from its deposit money. No claim opens when the chain was refused or proven false here, when its open claims would pass 80% of its counted bond, or when its deposit money cannot pay the deposit; its acting records can be carried again in another message |
| Who may object or answer | Anyone, with the flat deposit: 5 times what one full objection costs on that network, measured once built and approved by the owner before deployment |
| Effect | An objection holds the claim. An answer lifts the hold and restarts the 7 days in full |
| End | The claim is decided when the last objection or answer has stood for 7 days |
| Decided true | The claim acts. The objecting side's deposits are shared evenly by the answering side's deposits, the operator's among them; what does not divide evenly backs the receipt. Each winner collects its own share, so no number of deposits can stop a claim from being decided |
| Decided false | The claim is refused. The answering side's deposits, with the operator's, are shared by the objecting side. Every other claim of the chain on this network is refused when decided, and no new one opens. A claim of a chain proven false on this network is refused too |

### 11.7 Fast paths

Not in the first version (D115).

| Direction | Attester | If the claim is false |
|---|---|---|
| Lock to receipt | Locks 1.25 receipts on the acting network; the receipt is issued at once | 1 receipt of it is burned in place of the one wrongly issued, 0.25 goes to the guardian |
| Receipt to asset | Pays the user now from its own asset; the vault repays it after 7 days | The vault never repays it |

### 11.8 First version

ETH locked on Ethereum, vETH on Solana, native coins only, batching, the
slow paths. Then the fast paths. Tokens, with a bond in each token, and
more networks (stage 7) later (D117).

## Holes in the current code this proposal closes

These come from reading the code. No test demonstrates them yet.

| Hole | Closed by |
|---|---|
| A jump accepts a block at any difficulty | The jump guards, section 2.2 |
| A jump accepts a real block from any past height | The anchor age, and the duty rule |
| The jump guard does not see Conversion or Beta activity | Each job has its own window |
| The tip can move backward | Blocks stored by id |
| The light client's operator has no bond and replaces itself | Operators join by bond, no person in control |
| Two light clients per EVM network | One shared light client per network |
| The operator picks its own duty deadline | The protocol calculates the deadline from the confirmations the job requests |
| A failed Native to Bitcoin duty costs the operator nothing | A missed duty is slashed 100% |
| `iPoWConversion.sol` lets one payment complete two conversions | The tag names the transfer |
| A conversion that nobody claims stays open forever | The auction closes after 15 minutes, and a job with no bid expires (D37, D61) |
| Beta operators are approved by one governance key | Anyone can become an operator |

## Not decided

N1 to N11 were decided by the owner on 2026-09-28: N1 is D60, N2 is D61,
N3 is D62, N4 is D67, N5 is D68 to D70, N6 is D66 with D17 and D28
confirmed, N7 is D63, N8 is D64, N9 is D65, N10 is D71, N11 is confirmed
in D69 and D70.

N12 is D75. N18 is D76. N19 is D77. N20 is D78. N21 is D79. N13 is D82. N14 is D83.
N15 is D84. N16 is D80. N17 is D81.

### Readings made while building the duty and the proof

The contract needed these and no decision covers them. The code follows
the reading in the middle column until the owner says otherwise.

| # | Point | Reading in the code |
|---|---|---|
| N22 | How an operator's first chain head is registered (D30, D34) | By a Bitcoin transaction that names the operator, the network and the contract in its `OP_RETURN`. Nobody can register a coin of someone else |
| N23 | Which output is the next chain head | The output with the same number as the input that spent the chain head |
| N24 | Registering a chain head again | Not possible. An operator that loses its coin starts again as a new operator |
| N25 | A tagged transaction that came after its deadline has spent the chain head | The operator itself moves its chain head past that transaction. Nobody else can, because someone could move it past a transaction the operator is about to prove. That transaction can no longer settle a job |
| N26 | How the anchor of a job is chosen (D39) | The operator of the job names it in a step of its own. The block must be in the light client and at most 2 hours old at that moment. A block that was streamed can be an anchor, not only one that was jumped to |
| N27 | The guardian's sealed note (D47) | The note is sealed in an earlier block than the report. It is made from the guardian's address, the job, what the guardian will show, and a secret. The first report with a valid note is paid |
| N28 | The challenge of a proof (D49, D81) | Several challenges of a job at the same time (D88). The lock never moves, and no challenge opens in its last 12 hours (D99). When the lock ends, the branch with more work wins, and the operator's branch wins when the work is equal. A guardian whose challenge wins receives its deposit back and the 20% |
| N31 | Where a competing branch may part from the operator's branch | At the anchor or at a block after it, up to the block of the proof (D81). Not below the anchor. The two blocks where the branches part are compared by the parent they name, not by the block number or epoch time each side stated |
| N32 | Several challenges of one job at the same time | Decided: D88. Built and tested |
| N33 | How deep a guardian may ask for parents | Decided: D91 and D92. Built and tested |
| N34 | Section 8 says anyone may submit a proof. N29 says only the operator | Decided: D89 |
| N36 | When a message counts as official (D11) | A settlement is official once its proof is accepted. A claim: see D97. Never after a slash |
| N37 | Which jobs an attester may take (D42) | Any job whose proof was accepted and whose lock has not ended, settlement or claim. The first attester is the only one. It needs x of free bond, and uses the same bond as operators |
| N35 | Adding blocks during a competing-branch challenge | Blocks go on top of any block shown before on that side, so nobody can freeze a side by showing a block that goes nowhere. Either side may add blocks until the lock ends |
| N29 | Who submits the proof of a job | Only the operator of the job. The proof names the branch that is judged when a guardian challenges, so nobody else may choose it. Changed on 2026-09-28 after the review found that anyone could prove the operator's transaction on a fake branch they mined |
| N30 | Keeping a job's tag apart from a chain head registration | The transaction of a job carries `sha256("iPoW job", tag)`, not the tag itself. A registration carries `sha256("iPoW chain head", network, contract, operator)`. The two can never be equal, whatever tag an application chooses. A transaction that was already used can never be registered |

## To verify during the detailed design

| # | Question |
|---|---|
| V1 | Answered for Ethereum by stage 1: yes, see section 2.1 and the tests. Open for Solana |
| V2 | Answered by stage 2: the work of each branch is added up as blocks are shown, a walk of at most 100 blocks at a time, so a branch can grow without limit during the lock |
| V4 | Answered by stage 2: one transaction settles one job, and the jobs of one operator on one network are proven in the order of its chain. Each transaction spends the output of the one before |
| V5 | On Hedera, does the contract take the fixed price from a constant or from the price its own transaction pays? Where the price reads as zero, the commitment fee is zero and the minimum escrow of D56 is 1 of the smallest unit. `iPoWProtocol.sol` must not be deployed on such a network as it is |
| V6 | Do the main networks expose the same prices as the test networks? |
| V7 | What is the amount of work for the commitment fee in the new light client? Measured on 2026-09-28 on the Ethereum package: about 76,000 gas for each block when 29 blocks are streamed in one call, about 109,000 gas for one block alone, about 175,000 gas for a jump, about 524,000 gas for an epoch start, about 250,000 gas to walk 30 blocks back. The cost of a jump does not grow with the number of epoch starts recorded. The proof is not built yet. Naming the anchor of a job costs about 111,000 gas and the proof about 216,000 gas, measured with a Merkle proof of 1 level. `iPoWProtocol.sol` uses 80,000 gas for each block and 520,000 gas as the fixed part: the jump, the anchor and the proof. This text assumes work for one block x blocks of the window, plus a fixed part for the proof. The owner confirmed the rate of the deadline, not this formula |
| V11 | When the operator opens the destination job (D21), who states the number of confirmations? |
| V14 | A call that is not a transaction often runs at a price of zero, so a caller cannot read the commitment fee that way and a gas estimate for opening a job is about 20,000 too low. `commitmentFeeAt` gives the fee for a stated price. The operator service and the applications must send more than the fee and state the gas. What is sent above the fee is kept for the operator (D79) |
| V13 | A transaction of exactly 64 bytes inside a real block lets someone prove a transaction that is not in the block. It needs about 2^70 hashes and a miner that includes the transaction. The check in the light client covers only the other direction. To settle before an application checks a BTC amount (D8) |
| V12 | Is 36 hours enough for a guardian to show a branch with more work, when the fake branch is up to 100 blocks long? Depends on V2 |

## Decision index

| # | Decision | Section |
|---|---|---|
| D1 | The name is iPoW, with no version suffix | 9 |
| D2 | The jump stays. The loop is anchor, duty, punish | 2.2 |
| D3 | A jump is guarded by the protocol itself. The user does not check the anchor | 2.2 |
| D4 | The protocol handles programmable network to programmable network and moves no BTC for users | 1 |
| D5 | Beta and Conversion are applications. Locking and receipts across networks are the protocol's vault (D104) | 1, 11 |
| D6 | The operator's Bitcoin transaction must carry a tag. A BTC amount is optional | 4.3 |
| D7 | Slashed money compensates the application and rewards whoever proved the failure, for both kinds of failure: no proof before the deadline, and a proof on fake blocks (changed by D94) | 6.2 |
| D8 | An application may add Bitcoin conversion and checks the BTC amount | 1, 10.3 |
| D9 | A duty is fulfilled only by proving the tagged transaction inside the window | 4.3 |
| D10 | In a Bitcoin to token conversion, the operator's receipt or the user's own proof both release the tokens | 10.3 |
| D11 | A claim becomes official when it is attested or when its challenge period has passed. A vault claim: D111 | 7.2 |
| D12 | Anyone can become an operator by locking a bond | 3 |
| D13 | One operator identity streams blocks and publishes tagged transactions | 3 |
| D14 | The window is 30 blocks and the deadline is 1 day, fixed by the protocol. Since D68 and D69 this holds for a job with 6 confirmations | 4.3 |
| D15 | A proof needs 6 confirmations, its own block and 5 on top, and must be in blocks 1 to 25 | 2.4 |
| D16 | A fork harms only the operator that streamed the dropped block | 2.4 |
| D17 | The protocol sets limits for the challenge period and each application sets the value per claim at registration | 7.2 |
| D18 | A close belongs to the conversion application | 10.3 |
| D19 | The application requests the escrow for each job. An operator needs that much free bond | 4.1, 4.2 |
| D20 | Every job pays a commitment fee, even a job that carries no value | 5 |
| D21 | Either side may open the destination job of a transfer | 4.1 |
| D22 | A job pays two fees at the protocol: the commitment fee and the escrow fee | 5 |
| D23 | Both fees go to the operator that does the job correctly | 5 |
| D24 | The commitment fee is amount of work x current price x 1.5, calculated on-chain | 5 |
| D25 | The escrow fee is 0.5% by default and a job may request lower or higher | 5 |
| D26 | A slash takes the escrow of the failed job, not the whole bond | 6.1 |
| D27 | A proof after the deadline is rejected | 4.3 |
| D28 | The challenge period is between 36 hours and 7 days | 7.2 |
| D29 | The operator for a job is always chosen by auction | 4.2 |
| D30 | Every tagged transaction spends the operator's chain head | 3 |
| D31 | An operator locks a bond and nothing else. Escrow is the part that secures one job | 3 |
| D32 | Operators compete on the escrow they lock. x is the minimum and the most wins | 4.2 |
| D33 | The escrow fee is calculated on x, not on the larger amount locked | 4.2, 5 |
| D34 | Each operator has one chain head per network. For the vault, also one pair chain per pair of networks (D106) | 3 |
| D35 | The reward to the guardian is 20% of the slash | 6.2 |
| D36 | The application requests escrow of 125% of the actual value | 6.2 |
| D37 | The auction is open for 15 minutes, and the winner is locked in after 1 minute with no better bid | 4.2 |
| D38 | The minimum difficulty for a jump is 2^45 | 2.2 |
| D39 | An anchor may be at most 2 hours old | 2.2 |
| D40 | The application submits x, already 125% of y. The escrow fee is calculated on x | 6.2 |
| D41 | A job requires 6 confirmations by default and may request more | 2.4 |
| D42 | An attester takes the operator's place and earns 40% of the escrow fee | 7.3 |
| D43 | Anyone can join and leave as an operator freely, with no minimum to register | 3 |
| D44 | A jump needs the epoch start, and the anchor has the same difficulty | 2.2 |
| D45 | An epoch start counts if its difficulty is at least half of the highest one recorded in the last 4 weeks. Changed on 2026-09-28 from "recorded for that epoch" | 2.2 |
| D46 | Fees always go to the operator recorded for the job | 5 |
| D47 | A guardian claims its reward in two steps | 6.2 |
| D48 | Each network has one shared light client, with blocks stored by id and one window per job | 2.1 |
| D49 | A proof made on a fake branch can be challenged by a guardian | 6.3 |
| D50 | The escrow stays locked for 36 hours after a job | 4.4 |
| D51 | The money a user receives is supplied by the application, or by the protocol's vault (D104) | 1 |
| D52 | One job has one escrow, locked on the network where the job is opened | 4.1 |
| D53 | A missed duty is slashed 100%, the same as cheating | 6.1 |
| D54 | The duty ends when the proof is accepted | 4.3 |
| D55 | A job never has zero escrow | 4.1 |
| D56 | The minimum escrow is 5 times the commitment fee | 4.1, 6.2 |
| D57 | The epoch start may be at most 4 weeks older than the anchor | 2.2 |
| D58 | No person sets a price | 5, 9 |
| D59 | No key can change or upgrade the contracts | 9 |
| D60 | The deadline starts when the auction winner is locked in. Decided as "the 1-day deadline", before D69 | 4.3 |
| D61 | A job with no bid when the auction closes expires, and the fee and the user's locked value are returned | 4.2, 5 |
| D62 | When an operator fails, both fees return to the user | 5, 6.4 |
| D63 | Anyone can build an application and register it with the protocol. No approval | 1, 9 |
| D64 | Anyone can add a block to the light client, provided forks are kept apart so that one party's blocks do not disturb what another party needs | 2.1, 9 |
| D65 | The user pays the fees when the job is opened, and the protocol holds them until the job ends. D67 makes this exact | 5 |
| D66 | For a claim, the money stays locked for the whole challenge period, attested or not. An attest changes only who locks it: the operator or the attester | 4.4, 7.3 |
| D67 | The fees are handed to the operator and the attester when the lock of the job ends. An operator proven wrong during the lock is paid nothing | 5 |
| D68 | For each confirmation above 6, the window grows by one block. The proof range stays blocks 1 to 25 | 2.4, 4.3 |
| D69 | The deadline and the commitment fee grow linearly with the window. The deadline is 48 minutes for each block | 4.3, 5 |
| D70 | The window is at most 100 blocks, so at most 76 confirmations | 2.4 |
| D71 | The lock after a job and the shortest challenge period are at least 1.5 times the deadline of the job | 4.4, 7.2 |
| D72 | A window may cross into a new epoch. The light client checks the new difficulty by Bitcoin's own rule | 2.2 |
| D73 | An attester locks x, the amount the operator needs to escrow | 7.3 |
| D74 | The challenge period of a claim starts when the operator's proof is accepted | 7.2 |
| D75 | A forger's own epoch start also serves as its anchor and window, so forging a proof with 6 confirmations takes 7 mined blocks. Accepted. A job with a large value requests more confirmations | 2.5 |
| D76 | A better bid locks at least 0.1% more than the best bid | 4.2 |
| D77 | The escrow fee a job requests is at most 100% of x | 4.1 |
| D78 | An application registers at most 32 kinds of claim | 7.2 |
| D79 | What is sent above the fees of a job is kept for the operator, as part of the commitment fee | 5 |
| D80 | The application makes the tag and gives the same tag to the job on each network of a transfer. Each job has its own operator. One Bitcoin transaction settles one job on a network | 4.3 |
| D81 | Two branches compete when they start from the same parent block. The branch with more work at the end of the lock wins. A missing parent must be shown within 12 hours. A guardian's deposit equals the commitment fee and goes to the operator when the challenge fails | 6.3 |
| D82 | Bidding ends 15 minutes after the job was opened, or 1 minute after the last bid when that is earlier | 4.2 |
| D83 | Only a registered application can open a job | 1 |
| D84 | The bond, the escrow and both fees are in the network's own coin | 3 |
| D85 | The operator of a job may report its own missed duty as the guardian. The application still receives 80% of x | 6.2 |
| D86 | For a missed duty, the first guardian with a valid sealed note receives the 20%, even if it sealed notes in advance for every job | 6.2 |
| D87 | One fake branch may forge the proofs of several jobs of one operator. The challenge of a proof is the defence | 6.2 |
| D88 | Several guardians may challenge the proof of one job at the same time. The operator is slashed if any one of them wins | 6.3 |
| D89 | Only the operator of a job submits its proof | 8 |
| D90 | When a guardian asks for a parent, the operator earns the deposit by answering within 12 hours, and is slashed if nobody answers. The operator must keep its service running. Confirms D81 | 6.3 |
| D91 | Question k for a parent costs k times the commitment fee | 6.3 |
| D92 | At most 2,016 questions for a parent per job | 6.3 |
| D93 | No block is stored under a number below a lowest number fixed at deployment, about 4 weeks below Bitcoin's height at that moment | 2.1 |
| D94 | The operator only carries the application's message and is punished only for failures the protocol proves by itself: no proof before the deadline, or a proof on fake blocks. Whether a message is true is the application's business. Nobody declares a claim false. The vault's records are the exception: the operator vouches for them (D109, D111) | 7.1 |
| D95 | Anyone may attest, the operator of the job too. The application sees who attested | 7.3 |
| D96 | The attester's share of the escrow fee is 40% times the part of the lock during which it kept x locked. The operator receives the rest | 7.3 |
| D97 | Without an attester, a claim becomes official when its lock has ended with no challenge open, so it never stops being official | 7.2 |
| D98 | When several challenges win, whoever first resolves one receives the 20%, even the operator itself. The application's 80% keeps the protocol secure | 6.3 |
| D99 | The lock is fixed when the proof is accepted and never moves. No challenge opens in its last 12 hours | 6.3 |
| D100 | The first deposit of a question equals the commitment fee of the job, with no minimum | 6.3 |
| D101 | On Solana a stored block or epoch start can be closed 8 weeks after it was stored, and its rent goes back to whoever stored it. The commitment fee counts only transaction fees | 5 |
| D102 | Before it attests, an attester checks that the proof's blocks are real Bitcoin blocks at the heights and epoch times the proof states, not only real blocks. Agreed by the owner on 2026-09-29 | 7.3 |
| D103 | A competing branch starts at the proof's block or below it, never above: made-up blocks on top of a real proof block are not challenged. The owner chose this on 2026-09-29, over changing the contracts to allow it | 6.3 |
| D104 | Locking an asset on one network and receiving a receipt on another is part of the protocol: a vault on each network, deployed with the protocol, separate from the protocol contract because that contract is at Ethereum's size limit. Owner, 2026-10-01 | 11 |
| D105 | The vault is described as safe while at least one honest guardian watches, never as trustless | 11.1 |
| D106 | Each operator that carries vault records has one pair chain per pair of networks, registered by one Bitcoin transaction that names it on both, accepted by each vault only from the address named for it | 11.3 |
| D107 | A vault message spends the pair chain's coin and carries the hash of one batch. Each vault processes a chain in order and never skips a message | 11.3 |
| D108 | A vault message counts when its block is below a real block: the proof block of a job whose lock ended with no challenge won and no slash, or a block the vault recorded as real. Anyone may submit it. Owner, 2026-10-01, over a challenge game of the vault's own | 11.3 |
| D109 | A false record proven on the network that holds its fact slashes the operator's whole vault bond there: 80% backs the receipt, 20% to whoever submitted the message, or to the backing when the operator did | 11.4 |
| D110 | A vault bond is in the asset it vouches for. A chain's open claims on the acting network are at most 80% of the bond stated for them | 11.4 |
| D111 | The acting network holds a batch as a claim for 7 days of objections, with a flat deposit; an answer restarts the 7 days; the side left unanswered for 7 days wins | 11.6 |
| D112 | A lock leaves the vault only for an official REQUEST or CANCEL, never by a timer. A bond leaves after an EXIT message, once every earlier message and claim is done | 11.4, 11.5 |
| D113 | A user attaches a fee and picks no operator. The fee goes to the first operator whose message carrying the record is judged true | 11.5 |
| D114 | A message carries a batch of records, and is false if any record in it is false | 11.2 |
| D115 | The fast paths, an attester in the receipt or an attester fronting the asset, come after the first version | 11.7 |
| D116 | Anyone may open a job through the vault to make a real block | 11.3 |
| D117 | The first version: ETH on Ethereum, vETH on Solana, native coins, batching, slow paths. The vault bond is in the vault, not in the protocol contract | 11.8 |
| D118 | Only a job whose escrow is at least a minimum fixed at the vault's deployment certifies real blocks: 1 ETH proposed on Ethereum, the amount on Solana set with it; both approved by the owner before deployment. One unchallenged cheap job would otherwise expose every vault bond. Owner, 2026-10-01 | 11.3 |
| D119 | A vault message is false when its Bitcoin transaction is longer than 1,024 bytes, its batch longer than 2,048 bytes, or it carries more than 32 REQUEST and CANCEL records: every message that counts can be judged on every network. Solana takes a large message through a buffer. Owner, 2026-10-01 | 11.3 |

## Build status

| Stage | State |
|---|---|
| 1. The light client | Built and tested on the Ethereum package on 2026-09-28: `contracts/protocol/iPoWLightClient.sol`, tests in `test/iPoWLightClient.test.ts`. Open: V13. Not deployed. Not built for other networks |
| 2. Operators and jobs | Built and tested on the Ethereum package on 2026-09-28 and 2026-09-29: `contracts/protocol/iPoWProtocol.sol`, tests in `test/iPoWProtocol.test.ts` and `test/iPoWProtocol.Duty.test.ts`. Built: the bond, registration of an application, opening a job, both fees, the auction, the deadline, a job that nobody takes, the chain head, the anchor of a job, the proof of the tagged transaction, the lock, paying the operator, the slash of a missed duty, the guardian's two steps, and the challenge of a proof. Not deployed |
| 3. Claims and attest | Built and tested on the Ethereum package on 2026-09-29, in `contracts/protocol/iPoWProtocol.sol`, tests in `test/iPoWProtocol.Duty.test.ts`. Not deployed |
| 4. Solana | Built and tested on 2026-09-29: `programmable-network/solana/programs/ipow-light-client` and `programs/ipow-protocol`, with the same rules as Ethereum. Differences made by Solana: a walk of up to 100 blocks is done in steps of about 25 blocks and finishes only at the exact block it names; the protocol reads finished walks instead of walking; streaming takes up to 7 blocks per call and needs a compute limit above Solana's default; an account is created even when someone sent lamports to its address first; blocks can be closed after 8 weeks (D101); a competing-branch challenge needs no separate walk from the anchor, because the walk from the proof's block down to where the branches part already shows that block lies between them. Not built on Solana: the test harness that lowers the limit of 2,016 questions, so D92 is tested only on Ethereum. Not deployed |
| 5. The operator service | Built and tested on 2026-09-29 as a new program, `core/node` (the old `core/operator` stays for the old contracts). One program runs any mix of the operator, guardian and attester roles on any number of networks, each role on each network as its own task. Adapters for Ethereum and Solana. When the operator holds jobs with the same tag on several networks, one Bitcoin transaction spends all their chain heads (sections 3 and 4.3); a network not ready within one Bitcoin block of the first sends its own. On Solana, a proof too large for one transaction (a transaction for several networks, or a block of more than 4,096 transactions) goes through an address lookup table, closed afterwards to take its rent back. Tested on a local Hardhat network and an in-process Solana with the contracts and programs of stages 1 to 4, against a Bitcoin held in memory; the explorer client read Bitcoin mainnet once. Not run against a live network |
| 6. The applications | Conversion built and tested on 2026-09-29, on Ethereum (`contracts/apps/Conversion.sol`) and Solana (`programs/conversion`), and in the node's operator; design and the owner's decisions in [`../drafts/ipow-conversion-app.md`](../drafts/ipow-conversion-app.md). It asks the lowest escrow, since the application itself keeps the user's coin safe. BETA built and tested on Solana on 2026-10-01 as a basket of receipts (section 11): `programs/beta-basket`, design in [`../drafts/ipow-beta-app.md`](../drafts/ipow-beta-app.md). Not deployed |
| 6b. The vault | Built and tested on 2026-10-01 on Ethereum (`contracts/protocol/iPoWVault.sol`, tests in `test/iPoWVault.test.ts`) and Solana (`programs/ipow-vault`, tests in `programs/ipow-vault/tests/test_vault.rs`). Differences made by Solana: the deposit, the minimum certifying escrow and the Ethereum vault's address are set once by the program's upgrade authority when it initializes the vault, as Ethereum's constructor sets them, and the authority is then removed (D59); the owner approves the amounts before deployment; a message acts on at most 10 LOCK records here; a transaction and batch too large for one Solana transaction with their proof are uploaded to a buffer first; a slash burns 80% of the vETH bond, which backs vETH as the ETH it stands for; the deposits of a claim with no winner, and what does not divide evenly among winners, stay in the vault, and the application's share of a slashed checkpoint job is not collected, since there is no ETH here to back; a processed message's block is not recorded as real, so a later message may need its own walk; a receipt is issued to a token account its recipient owns. The node's roles built and tested on 2026-10-01 in `core/node` (`crates/node/src/vault.rs`, tests in `crates/node/tests/vault_local.rs` on a local Hardhat network and an in-process Solana with one Bitcoin in memory): the operator registers its pair chain, keeps its bond and deposits, carries final locks, burns and give-ups in batches, submits each message to both vaults once a real block is above it, opens a checkpoint job when none is, and answers objections to its claims; the guardian checks every claim against the other network and objects to a false one. Not built: a guardian that brings a message hidden from one network to it, so the lie is proven there; the fast paths (D115). Not deployed |
| 7. The other networks | Not built |
| 8. Deployment | Not done |
