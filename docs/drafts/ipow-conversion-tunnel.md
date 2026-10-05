# Conversion between programmable networks: the tunnel (proposal)

**Status: approved by the owner on 2026-10-03 (Q1 to Q7 below). Built on
2026-10-03, not deployed:** T1 and T2 in the Conversion
contract and program, with tests; T3 in the SDK on EVM networks
(`provePayment`), not on Solana; T4 in the node (the tunnel API, the
promise to a tunnel buy's sell, read from the buy's memo, the sell's
window), its token buys on Solana not yet; T5 in Greatwall's
Convert between EVM networks, and from Solana into them; into Solana it
waits for T3 there (Q3: a buy opens to testers only with the user's own
proof). Live once each network's Conversion is redeployed (the Solana
program too) and the node's `tunnel_api` is set.

One Conversion application then handles every pair: a programmable
network against Bitcoin (one leg, built) and a programmable network against
another (two legs, this proposal). It extends Conversion
([`ipow-conversion-app.md`](ipow-conversion-app.md), built) so that a user
converts an asset on one programmable network into an asset on another (SOL
on Solana into AAPL on Ethereum), the way the old protocol's conversion
layer did, on the new protocol's jobs, auctions and light clients.

## The idea: two legs, one Bitcoin payment

The old protocol linked "two Conversion legs, one real Bitcoin payment"
([`../design/ipow-implementation.md`](../design/ipow-implementation.md),
"Cross-chain linking"): a sell on network A and a buy on network B, where
the one Bitcoin payment that completes the sell is also the payment the buy
waits for. Neither network reads the other; each checks only a fact about
Bitcoin, through its own light client.

Conversion already has both legs. A tunnel is a buy on B whose operator's
Bitcoin address is the address the sell on A pays:

| Leg | Network | What it is | Already in Conversion |
|---|---|---|---|
| The buy | B (Ethereum) | The operator locks AAPL for the user and names a fresh Bitcoin address S | `buy`, `fund` |
| The sell | A (Solana) | The user locks SOL, to be paid at least the buy's sats at S | `sell`, with S as its script |
| The payment | Bitcoin | The sell's operator's tagged transaction pays S | The sell's proof |
| The receipt | Bitcoin | The buy's operator's tagged transaction spends that payment | The buy's proof, `completeBuy` |

No protocol change, and no new trust: every step is a Bitcoin fact a light
client checks.

## The flow: SOL on Solana into AAPL on Ethereum

| Step | Who | Where | What happens |
|---|---|---|---|
| 1 | The user's app | | Asks an operator for a tunnel: SOL amount, AAPL wanted, the sats between them |
| 2 | The user, one signature | Ethereum | Opens the buy at the quote (`buy`: the AAPL to themselves, paying the job's fees, Q5), its memo naming the SOL sale that will pay it |
| 2b | The operator | Ethereum | Wins the buy, locks the AAPL and names S (`fund`). The buy's payment blocks are now fixed: the 12 after its anchor |
| 3 | The user's app | Ethereum, read only | Checks the funded buy: for this user, the AAPL amount, S, the payment blocks |
| 4 | The user, one signature | Solana | Sells SOL for at least the buy's sats, paid to S, mined no later than the buy's last payment block (T1) |
| 5 | An operator | Solana, Bitcoin | Wins the sell; its tagged transaction pays S within the payment blocks |
| 6 | The buy's operator | Bitcoin, Ethereum | Its tagged transaction spends the payment (the receipt), proven on Ethereum; anyone completes the buy and the user receives the AAPL |
| 7 | The sell's operator | Solana | After its proof's lock (36 hours), receives the SOL |

When one operator takes both legs, the payment goes from its wallet to its
own address S and back with the receipt: it spends only the two
transactions' Bitcoin fees, about 600 to 800 sats at 1 sat/vB (2026-10-03).

## When it does not go through

| Case | Result | Rule |
|---|---|---|
| Nobody takes the sell | The user gets the SOL back (`refundSell`); the buy's operator closes and takes its AAPL back | Today's rules |
| The sell's payment is mined after the buy's last payment block | It no longer pays the buy. The sell refunds the user | **T1, new** |
| The sell's payment pays less than the buy's sats | The buy is not paid; the sell refunds the user (a proven payment too small) | Today's rules |
| The buy's operator never proves its receipt, or closes anyway | The user proves the payment themselves and receives the AAPL | **D10, needs the SDK (T3)** |
| The sell's operator pays S but never proves | The user gets the AAPL (the buy is paid) and the SOL back (the sell's job failed); the sell's operator loses | Today's rules |
| Either operator is slashed | Its escrow's share goes to that leg's user | Today's rules |

The user cannot lose both: the SOL leaves only for a payment to S within the
payment blocks, and that payment gives the user the AAPL, by the receipt
or by the user's own proof.

## The changes

| # | Change | Where |
|---|---|---|
| T1 | A sell may name a latest Bitcoin block for its payment. `completeSell` refuses a proven transaction in a later block (`Duty.proofBlock.height`), and `refundSell` refunds it. Zero keeps today's behaviour | Conversion, EVM and Solana |
| T2 | A buy may name its recipient: the coin goes to them, the fees are the opener's (`buyFor`). Built, then set aside by Q5: the user opens their own buy and pays its fees; `buyFor` stays in the contract and program for an operator that wants to open buys for its users | Conversion, EVM and Solana |
| T3 | The user's own payment proof (D10), which Conversion already accepts (`proveMyPayment`), built in the SDK: the payment's block stored in the light client, and a walk of blocks on top | SDK |
| T4 | The operator's node runs tunnels: lists what it trades, answers a request with a quote, reads the user's buy's memo and promises its sell (Q7), funds the buy, takes the sell, pays S, proves both | Node |
| T5 | Greatwall's Convert: any asset on any network into any other; a tunnel is the buy's transaction, then the sell's once the funded buy is shown | Greatwall, SDK |

The old protocol did not need T1 and T2: one fixed operator ran both legs,
so they lined up by themselves. With an auction per job, each leg may have
its own operator, and the contract keeps them in line.

## Asking for a tunnel

The node serves `GET /assets` and `GET /quote`
(`core/node/crates/node/src/tunnel.rs`). `/assets` lists what the operator
trades on each network (the coin and the tokens, with a symbol, decimals
and its prices in sats): the app's token pickers and estimates come from
it, so the app keeps no prices of its own. Either side of a tunnel may be
a token (`fromToken`, `token`), as a sell or a buy of that token; on
Solana the node buys only SOL so far. On the test networks the operator's
prices are fixed and representative (Bitcoin only carries the real
transaction): a tenth of the market, and the mock RWA tokens of
`programmable-network/ethereum/deployments/ethereum-testnet-rwa.json`.

The API is not for browsers: with `tunnel_api.key_env` set, a caller
sends the key in `x-tunnel-key`. Greatwall calls it from its server
(`/api/tunnel`), which holds the key. The SDK's `tunnel.ts` is the
client (`TunnelApi`), the buy's memo (`tunnelMemo`), the estimates at
the operator's prices, and `sellPaying`, which finds the sale that pays a
funded buy's address on the source network: the two legs are linked from
the chain, not from an app's own record.

A tunnel is asked for in two steps, one signature: the app takes a quote
(which also says whether the operator could take the tunnel now, so the
user pays no fee for nothing); the user opens the buy on the destination
at that quote, with their own wallet and fees (`buy`, the amount to their
own address), with the sale's terms in the buy's memo (the SDK's
`tunnelMemo`: the source network, the coin sold, the amount). The node
reads the memo as it considers the buy and takes it as a tunnel's when it
is at the operator's prices for both legs, within `max_sats`, and within
what the operator can fund and pay; refused, the buy is left to expire,
and the user ends it in one transaction. The terms are the buy's owner's
by construction: nobody else can write its memo. From then on the buy's sell is
promised (Q7). A buy the node refuses is an ordinary buy, ended by its own
rules. Before the user signs the sell, the app checks the funded buy
against its record (the recipient, the coin, the amount, the sats) and
that no sell of theirs already pays its address.

## Timing

Each leg waits for its confirmations. With 6 (the default), a tunnel takes
about 1 to 2 hours: the buy's auction (15 minutes) and funding, the sell's
auction, the payment, and the receipt's confirmations. The payment must be
mined within 12 blocks (about 2 hours) of the buy's anchor, which leaves the
sell's auction and payment about an hour and a half.

## Cost on the test networks

Every tunnel moves real BTC, even between test networks, within Conversion's
cap (100,000 sats on the test networks). With one operator on both legs the
BTC returns to it, and only fees are spent. The operator's node takes only
the jobs it finds worth it, so it can price the fees in, cap sizes and refuse
when tested too often.

## Alternatives considered

Three designs were drafted on 2026-10-03 and dropped for this one:

| Design | Why not |
|---|---|
| A settlement (spec section 10.1): one Bitcoin transaction settling a job on each network | Neither network can see the other's coin is in place, so one order of steps leaves the user or the maker exposed, covered only by an escrow of 125% of the value on each side (D36) |
| Two separate swaps through the user's Bitcoin wallet | Not linked: the user holds real BTC between them, signs on three networks, and is exposed if the second swap fails |
| The vault, then an order on the receiving network that swaps the receipt | Sound, but a separate application beside Conversion, and fast only with attesters running |

A programmable network sees only Bitcoin, so a delivery on another
programmable network can reach it only as a statement, which the protocol
does not vouch for (D94). The tunnel avoids statements: the payment that
links the legs happens on Bitcoin.

## Decisions

| # | Question | Decision |
|---|---|---|
| Q1 | Is the tunnel the way Conversion links programmable networks? | Yes, owner, 2026-10-03: "like the old conversion, with the new protocol" |
| Q2 | T1 and T2 change the Conversion contract and program | Approved, owner, 2026-10-03 |
| Q3 | Build D10 in the SDK (T3) before tunnels open to testers? | Yes, owner, 2026-10-03 |
| Q4 | Confirmations on the test networks | 6, the default, owner, 2026-10-03 |
| Q5 | Who pays the buy's job fees? | Revised by the owner on 2026-10-04: the user opens the buy and pays its fees, as for any buy ("cannot be that free"). An asked-for tunnel then costs its user the fee, not the operator; the operator only registers the buy (Q7). `buyFor` (T2) stays in the contract and program, unused by Greatwall. The node's `price_fees` setting still lets an operator price a buy's fees into its quote; off on the test networks |
| Q6 | How does the app ask for a tunnel? | The node's own API: a quote, then the funded buy. Owner, 2026-10-03 |
| Q7 | Is a quote binding? | Yes, owner, 2026-10-03: the node keeps each tunnel it opened and takes its sell on the terms quoted (the amount, the sats, paying the funded buy's address), whatever its price is by then. With these further rules, owner, 2026-10-03: the sell's window must be exactly the buy's payment blocks; one sell per tunnel, until that one is refunded, and a second sell to the same address goes out without the payment; no other sell with a payment window is taken (a payment mined after a window the user chose would refund the sell and leave the operator's BTC paid); kept in memory, so after a restart the tunnel's sell is refused and its user refunded |
