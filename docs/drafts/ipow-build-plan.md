# Build plan for the iPoW protocol

**Status: plan, not a spec.** It plans the work described in
[`../design/ipow-protocol.md`](../design/ipow-protocol.md), which the
owner approved as the spec on 2026-09-28. Section numbers, D and V
numbers below refer to that file.

On 2026-09-28 the owner said: forget the current contracts and build
the new design. The stage that tested the current contracts is dropped.
The owner answered B2 to B4 the same day.

## Rules for every stage

| Rule | Reason |
|---|---|
| The new contracts are new files. The current contracts are not tested, fixed or changed. They stay in the repo until stage 8, see B3 | The owner's instruction of 2026-09-28. The live deployments and their tests keep working while the new code is built |
| A stage is done when its tests pass and the `reviewer` agent has run | The repo rules in `CLAUDE.md` |
| If the code cannot follow a decision, the work stops and the owner decides | The repo rule on divergence |
| Nothing is committed or deployed without the owner asking | The repo rules on commits and deployments |

The spec for the new code is `docs/design/ipow-protocol.md`. The
`design-alignment` skill checks the new code against it.

## Stages

| Stage | What is built | Where | Depends on |
|---|---|---|---|
| 1 | The light client | Ethereum package | Nothing |
| 2 | Operators and jobs | Ethereum package | Stage 1 |
| 3 | Claims and attest | Ethereum package | Stage 2 |
| 4 | Stages 1 to 3 as Solana programs | Solana package | Stage 3 |
| 5 | The operator service | `core/operator` | Stages 3 and 4 |
| 6 | The applications: Conversion, then the vault (a protocol part, section 11), then BETA | Ethereum and Solana packages | Stage 5 |
| 7 | The other networks | Base, Robinhood, Polkadot, Hedera, Hyperliquid, Tempo | Stage 6 |
| 8 | Test network deployment, live run, canonical docs, removal of the old contracts | All | Stage 7 |

The new contracts are built on Ethereum first. Base, Robinhood, Polkadot
and Hedera then use the same source. Hyperliquid and Tempo need their
own variants.

## Before stage 1

These need no code and can change the design, so they come first.

| Item | Why now |
|---|---|
| V6: do the main networks expose the same prices as the test networks? | A "no" changes how the commitment fee is calculated (D58) |
| V5: Hedera's price source | It shapes the price interface of stage 2 |
| Solana's price of work. It was not tested, and the fee paid during congestion cannot be read by a program | The same |
| Whether Solana's accounts can store blocks on branches | V1 is otherwise answered for Ethereum only, and stage 4 finds out too late |

## Tests that replace the dropped stage

The holes listed in the proposal were found by reading the current code.
They are not tested on the current code. Each one is tested on the new
code instead:

| Hole in the current code | Test on the new code | Stage |
|---|---|---|
| A jump accepts a block at any difficulty | A jump below the minimum difficulty, or with a difficulty different from the epoch start, is rejected | 1 |
| A jump accepts a real block from any past height | A jump to a block older than 2 hours is rejected | 1 |
| The tip can move backward | A block is stored by its id and never replaces another | 1 |
| Inside an epoch, a block with a different difficulty is accepted | The same attack is rejected | 1 |
| The jump guard does not see Conversion or Beta activity | A jump while another job is open is allowed by design. The test shows that the open job's proof is not affected | 2 |
| One Bitcoin payment completes two conversions | One tagged transaction cannot fulfil two jobs | 2 |
| The same | One payment by a user cannot release two conversions | 6 |
| A conversion that nobody claims stays open forever | A job with no bid expires and the fee returns | 2 |
| The same | A conversion that nobody takes returns the user's value | 6 |
| The operator picks its own duty deadline | The protocol calculates the deadline | 2 |
| A failed duty costs the operator nothing | A missed duty is slashed 100% | 2 |

## Stage 1. The light client

| Part | Decisions |
|---|---|
| Blocks stored by id, on branches. Anyone can add a block | D48, D64 |
| Each block passes the proof-of-work check, names the block before it, and has the difficulty of its epoch | Sections 2.1 and 2.2. The proposal has no decision for the proof-of-work check of a single block. It is taken from the current code |
| Epoch start: 6 blocks, at least half of the highest recorded, at most 4 weeks older than the anchor | D44, D45, D57 |
| Jump: minimum difficulty 2^45, anchor at most 2 hours old | D3, D38, D39 |
| A window may cross into a new epoch | D72 |
| A fork harms only the operator that streamed the dropped block | D16 |
| Reading a block and its confirmations, for stage 2 | D15, D41 |

Questions answered in this stage: V1 and the per-block part of V7.
Crossing into a new epoch is D72.

Open: who may call a jump before operators exist in stage 2. Proposed:
in stage 1 anyone may, and stage 2 ties the jump to a job.

Tests: the stage 1 rows of the table above. Two operators stream two
branches and neither disturbs the other. A real epoch start counts after
a fake one was submitted first.

## Stage 2. Operators and jobs

| Part | Decisions |
|---|---|
| Bond: lock, withdraw free bond. One identity streams and publishes | D12, D13, D31, D43 |
| Chain head, one per network | D30, D34 |
| Job: escrow x, minimum escrow, one escrow per job and network, confirmations, window, deadline | D14, D19, D40, D52, D55, D56, D68 to D70 |
| Either side opens the destination job. The fee is taken out of what the user receives when the operator opens it | D21 |
| Fees: commitment fee from the network's price, escrow fee, held by the protocol, paid to the operator recorded for the job | D20, D22, D23, D24, D25, D33, D46, D58, D65 |
| Auction | D29, D32, D37, D60, D61 |
| Duty: proof of the tagged transaction inside the proof range | D6, D9, D15, D27, D54 |
| Slash, and the guardian's claim in two steps | D7, D26, D35, D47, D53, D62 |
| Challenge of a proof by a branch with more work. Lock of 36 hours or 1.5 times the deadline. Payment of the fees to the operator | D49, D50, D67, D71 |
| No key can change or upgrade the contracts | D59 |

Questions answered in this stage: V2, V4, V8, V11, V12, V13, and the
proof part of V7.

Open: whether a job can be opened only by a registered application.
Registration is D63, built in stage 3. Proposed: registration moves to
stage 2.

Tests: the stage 2 rows of the table above. One test for each row of the
table in section 8, front-running.

## Stage 3. Claims and attest

| Part | Decisions |
|---|---|
| The challenge period of each claim, set at registration, at least 1.5 times the deadline, starting when the proof is accepted | D17, D28, D71, D74 |
| A claim becomes official when attested or when its period has passed | D11 |
| The lock lasts the whole challenge period | D66 |
| Attester: takes the operator's place, locks x, earns 40% of the escrow fee, is slashed in its place, is paid when the lock ends | D42, D67, D73 |

The attester's amount is D73. The start of the challenge period is D74.

## Stage 4. Solana

The same three stages as programs, with the same tests.

## Stage 5. The operator service

| Mode | What it does |
|---|---|
| Operator | Bids, streams, publishes the tagged transaction, proves |
| Guardian | Watches jobs, proves a missed duty, a false claim or a fake branch |
| Attester | Checks an operator's work, then attests |

The current service has the operator's logic in `chain_operator.rs` and
Beta's logic in `beta_operator.rs`. How much of it is kept is decided
when stage 3 is done and the contract interface is fixed.

The parts that belong to one application, such as the BTC payment of a
conversion, are added in stage 6.

## Stage 6. The applications

| Application | Built on the protocol as | Decisions |
|---|---|---|
| Conversion | Section 10.3. It checks the BTC amount, defines the close, and accepts the user's own payment proof | D8, D10, D18, D36, D51 |
| The vault | Section 11: a protocol part beside the protocol contract, on Ethereum then Solana, then the node's operator (carrying batches), guardian (objecting) roles | D104 to D117 |
| BETA | A basket of receipts on Solana, with creator fees: [`ipow-beta-app.md`](ipow-beta-app.md). Replaces section 10.2 | D104 |

Conversion is first because it is smaller.

Tests: the stage 6 rows of the table above.

## Stage 7. The other networks

| Network | Extra work |
|---|---|
| Base, Robinhood | The cost of posting data to Ethereum is part of the network's price. How it enters the formula of D24 is open |
| Polkadot | None known |
| Hedera | V5 |
| Hyperliquid | The contracts are split, because of the block gas limit |
| Tempo | PathUSD variants, because Tempo rejects native value |

## Stage 8. Deployment and documents

1. Deploy to the test networks. Read every address and setting back
   from the network, including that no key can change the contracts
   (D59).
   Before the vault (section 11): the mainnet amounts are approved
   (D121 deposit: 0.03 ETH, 0.015 SOL; D118 minimum certifying escrow:
   1 ETH, 1 SOL). Test networks are deployed with smaller amounts, chosen
   at deployment, so that a test run needs little test ETH; the Ethereum vault is deployed first, and its address is
   given to the Solana vault's initialize, which only its upgrade
   authority can call; the authority is removed right after, and its
   removal read back from the network, as for every program (D59).
2. Record the addresses in each network's README.
3. Live run with real Bitcoin transactions, recorded in
   [`live-run-log.md`](live-run-log.md).
4. Rewrite `docs/design/ipow.md`, `docs/design/ipow-implementation.md`
   and `SECURITY.md` from what was built.
5. Remove the current contracts, programs and their operator code.

## Open for the owner

| # | Question | Recommendation |
|---|---|---|
| B1 | Are the stages in the right order? | Yes, as above |
| B2 | Names of the new contracts | Decided: `iPoWLightClient.sol` for stage 1, `iPoWProtocol.sol` for stages 2 and 3 |
| B3 | When are the current contract files deleted? | Decided: when the new ones work. Until then they stay untouched |
| B4 | Which document is the spec while stages 1 to 7 are built? | Decided: `docs/design/ipow-protocol.md`, marked "being built". The two current documents stay as the description of what is live |
