# First live run of the new protocol: Sepolia, 2026-10-02

A dated record of the first job taken end to end on a test network: what
happened, in order, what it cost, and what it teaches about the SDK and
Greatwall.finance, the user-facing app built on it. Not a spec: the design
is [`../design/ipow-protocol.md`](../design/ipow-protocol.md). Times are UTC.
Every figure below was read back from the chains.

## What ran

- **The deployment:** the new protocol, deployed the same day on seven EVM
  test networks and Solana devnet (spec, Build status row 8). This run used
  Sepolia only.
- **The node** (`core/node`, settings `core/node/node.testnet.yml`, started
  with `core/node/run-testnet.sh`), running as operator and guardian. On
  Solana it had no bond, so its operator only waited.
- **One key, both sides:** the same key, `0x9784…2BF1`, opened the job and
  took it as operator. A self-test: the protocol allows it, and a second
  key can play the user later.
- **Real Bitcoin:** the operator's wallet `bc1q7hyq…5syx` held 14,182 sats.
  The light client only accepts real Bitcoin, so there is no testnet mode.
- **The job:** a vault checkpoint (D116) with 6 confirmations. It is the
  simplest job: the operator writes one tagged Bitcoin transaction and
  proves it; no user payment is involved.

## Timeline

| Time | Who | Where | What happened | Cost |
|---|---|---|---|---|
| 19:00:24 | Owner | Sepolia | Locked a 0.1 ETH bond (`lockBond`). The node never locks a bond itself | 45,655 gas |
| 19:01:19 | Node | — | Started; saw the bond | — |
| 19:01–19:03 | Node | Bitcoin explorer | Three tries to read the wallet's coins timed out (mempool.space is slow from this network); the fourth worked | — |
| 19:03:53 | Node | Bitcoin | Sent the operator's first chain head `d24d40…0112`: a 546-sat coin to itself and an OP_RETURN naming it, 8 sat/vB | 1,470 sats |
| 19:03–19:16 | — | Bitcoin | No block for 58 minutes in all, since 18:18. Nothing to do but wait | — |
| 19:16:34 | Bitcoin | Block 969,622 | The chain head confirmed | — |
| 19:17:36 | Node | Sepolia | Recorded the six blocks that start Bitcoin's current difficulty period in the new light client (`addEpochStart`) | 519,050 gas |
| 19:18:00 | Node | Sepolia | Jumped the light client to Bitcoin's best block (`jump`) | 170,456 gas |
| 19:18:12 | Node | Sepolia | Walked one block back to the chain head's block (`extendBack`) | 104,654 gas |
| 19:18:36 | Node | Sepolia | Registered the chain head (`registerChainHead`); log: `chain head registered` | 132,417 gas |
| 19:20:00 | User | Sepolia | Opened job 1 through the vault (`openCheckpoint`). Quoted at 19:19: 0.012388 ETH paid. The escrow came out at 0.03201 ETH, not the 0.03059 quoted: the base fee rose before the transaction was mined | 274,413 gas + 0.012388 ETH |
| 19:20:48 | Node | Sepolia | Bid on job 1, locking 0.03201 ETH of its bond; 48 seconds after the job opened | 100,665 gas |
| 19:21:48 | — | Sepolia | The auction ended, a minute after the last bid (D37); the job was assigned, and its 24-hour deadline counts from here | — |
| 19:22:48 | Node | Sepolia | Jumped the light client to block 969,623 | 101,675 gas |
| 19:23:24 | Node | Sepolia | Anchored job 1 at block 969,622, one below the best (`anchorJob`) | 111,439 gas |
| 19:23:33 | Node | Bitcoin | Sent the tagged transaction `f68b80…2404`: it spends the chain head, makes the next one, and carries the job's tag, at 5.5 sat/vB | 1,394 sats |
| 19:29:30 | Bitcoin | Block 969,624 | The tagged transaction confirmed: 1 of the 6 confirmations | — |
| until 2026-10-03 19:21:48 | — | Sepolia | Job 1's deadline. The proof needs block 969,629 (6 confirmations), then the node proves (`proveJob`; log: `proven`) | *to be recorded* |
| proof + 36 h | Node | Sepolia | The lock ends; the node settles (log: `settled`): its bond is freed and it is paid both fees | *to be recorded* |

## What it cost

| | Bitcoin | Sepolia (about 1 gwei) |
|---|---|---|
| One-time operator set-up: chain head, epoch start, jump, walk, registration | 1,470 sats | 926,577 gas, about 0.00094 ETH |
| The user's job | — | 0.012388 ETH paid (refunded if nobody takes it), plus 274,413 gas |
| The operator's work on the job so far: bid, jump, anchor, tagged transaction | 1,394 sats | 313,779 gas, about 0.00033 ETH |
| The proof and the headers streamed for it | — | *to be recorded* |

## What the user went through

The user here only opened a checkpoint. The same steps carry over to a
Conversion swap or a vault lock.

1. **The price moved between quote and send.** The quote at 19:19 gave an
   escrow of 0.03059 ETH. The job, mined at 19:20, had 0.03201 ETH,
   because the base fee rose. The script paid twice the commitment fee as a
   margin, so the job opened. What is paid above the fees goes to the
   operator (D79), so the margin is real money to the user.
2. **The first wait was short.** An operator bid 48 seconds after the job
   opened. The auction allows up to 15 minutes.
3. **The second wait was Bitcoin's.** The tagged transaction went out 3.5
   minutes after the bid, then waited 6 minutes for a block. The 6
   confirmations take about an hour more on average. Earlier the same day,
   Bitcoin went 58 minutes without a block.
4. **Nothing told the user any of this.** The only feedback was reading the
   chains by hand: the job's status number, the Bitcoin explorer and the
   node's log.

## What the operator went through

- **A bond before anything.** The node never locks one itself, so it sat
  idle until the owner locked one (by design: D84 limits).
- **A one-time set-up per network:** a chain-head transaction on Bitcoin,
  then on a brand-new light client the epoch start, a jump and a walk. All
  of it waits for one Bitcoin block.
- **Flaky explorers.** mempool.space timed out three times in a row; the
  node retried and went on. During the deployment, blockstream.info answered
  `curl` but not Python's HTTP client from the same machine.
- **Noise.** With no bond on Solana, the node logged the same warning every
  30 seconds.

## Lessons for the SDK

| Need | Why, from this run |
|---|---|
| A quote with a stated margin, showing the expected escrow and fees and what the margin costs | The price moved between quote and send (the escrow rose 4.6% in one minute), and the margin is not refunded (D79) |
| Job status by name and stage (`Auction`, `Assigned`, `Proven`, `Settled`, `Expired`, `Slashed`), with what each stage waits for | The raw status is a number, and the order is not the obvious one (3 is `Assigned`). A job with a bid is still `Auction` for a minute after it (D37): "waiting for a bid" would be wrong then |
| The Bitcoin side: the tagged transaction's id, its confirmations against the 6 needed, and an explorer link | Most of the time is spent waiting on Bitcoin |
| Estimates in time, from Bitcoin's real block times, not "10 minutes" | One gap was 58 minutes; another block came 8 seconds after the one before |
| Reads that retry, and a second explorer to fall back on | Base Sepolia's endpoint answered from before a deployment; mempool.space timed out |
| Each network's units and sending rules hidden behind one API | Hedera counts HBAR in 8 decimals inside contracts but 18 over its RPC; Tempo pays fees in PathUSD and needs its own transaction type (viem); Polkadot's gas is about 1/8 of Ethereum's |
| The deadline and the refund guarantee, stated | If the operator fails, the escrow is slashed and both fees return (D62); the user should see that before paying |

## Lessons for Greatwall.finance

- **A progress tracker with real stages and times:** Opened, Taken by an
  operator (with the time), Written on Bitcoin (with a link), confirmations
  1 to 6, Proven, Settled. Every stage in this run has a timestamp, and the
  app can show them.
- **One number up front:** "you pay 0.0124 ETH; if no one takes it in 15
  minutes it all comes back", with the fee breakdown one click away.
- **Name the slow part:** "waiting for Bitcoin blocks (about 10 minutes
  each, sometimes much longer)", so a 58-minute gap does not look like a
  failure.
- **Never ask the user for Bitcoin words.** The user never saw a txid,
  vout or block height here unless they went looking; the app should link
  out rather than explain.

## Not yet recorded

The proof, the settlement after the 36-hour lock, and their costs are added
here when they happen.
