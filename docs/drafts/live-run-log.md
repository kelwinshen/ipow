# Live-run log

**Status: historical record, not spec.** A dated account of specific
deploys, live transactions, incidents, and bugs found while building and
running this system for real. Not authoritative for current behavior —
see [`../design/ipow.md`](../design/ipow.md) and
[`../design/ipow-implementation.md`](../design/ipow-implementation.md)
for that. Kept because the *lessons* here (a caught bug, a chain-level
quirk, a timing constant raised after a near-miss) explain why the
current design has specific numbers and rules, even after the narrative
detail itself stops mattering day to day.

## 2026-09-21: first BETA statement-bus runs (v2, timer-based)

Solana `beta-factory` and Ethereum `BetaVault.sol` deployed for the first
time (not the current design — the original rate-cap/timer-window
version, superseded the next day by the attestation/challenge-window
design below). Test composition: 1 unit = 0.01 SOL + 0.001 ETH.

Live record (Bitcoin mainnet):

| Block | Statement | Result |
|---|---|---|
| 967886 | MINT lock 1 | Minted 1 BETA; lock 1 FINAL |
| 967894 | RELEASE lock 1 | Burn claimed; paid 0.001 ETH after 10 min |
| 967913 | MINT lock 2 | Minted; FINAL |
| 967913 | MINT lock 99 (a deliberate lie) | Minted on the blind chain anyway; **slashed 0.001 ETH** on Ethereum; operator retired |
| 967913 | auditor dead-veto | Operator paused on Solana; judged true → auditor rewarded |
| 967913 | RELEASE burn 7 (a deliberate lie) | **Slashed 0.01 SOL** on Solana; operator retired; Ethereum's queued payout refused |
| 967913 | auditor VETO on the lie | Judged true → rewarded; payout held |

Total Bitcoin spent across every live run in this whole log: under 6,000
sats, including 1,244 sats lost to a daemon bug (below).

**Stranded queued mint, found by asking "does it always end normally for
the user?"**: a MINT judged true and queued on Solana whose operator was
then retired could neither exercise (dead party) nor expire (queued) nor
be re-anchored (queued ⇒ predicate false). Fixed same day: a queued
slot now records which party queued it; a live operator's MINT for the
same slot can pass the retired party to re-queue it under the new
anchor; an `expire_pending` path cancels a queued slot once its queuing
party is dead.

**Lesson learned, became a rule**: veto rewards on both chains now come
from a governance-funded pool, never from insurance — otherwise the
slash wouldn't fully back the units the lie created.

## 2026-09-22: v3 (attestation/challenge-window) replaces v2's timers

Solana `beta-factory` upgraded in place (slot 502011808); Ethereum fresh
`BetaVault` v3 deployed (Sepolia can't upgrade in place). This is the
design documented as current in `ipow.md`/`ipow-implementation.md`.

**First live v3 mint (block 968030)**: operator anchored MINT and, right
behind it on its own chain, self-ATTEST (both fee-bumped after ~2 hours
unconfirmed — the wallet couldn't initially fund the "fastest" fee rate
for both). Relays needed to jump to 968030, since Sepolia required the
epoch-start header recorded first. Devnet: MINT queued → ATTEST reserved
escrow from the operator's bond → exercised **immediately**. Sepolia:
MINT processed 38 minutes after its block → lock FINAL.

**Lessons that became rules**: (1) Esplora's `/tx/:id/status` endpoint
returns `{"confirmed":false}` for *unknown* txids too, not just
unconfirmed ones — use `/tx/:id` (404) or `/outspend` to distinguish "in
mempool" from "never broadcast." (2) The MINT-processing window (`tFin`)
was 1 hour, which is tight if an anchor confirms during an explorer
outage — raised to 4 hours. (3) Header relay must be able to jump, not
just extend one at a time, when far behind, and the EVM side needs its
epoch-start header pre-relayed before a jump — both built the same day.

**First daemon-driven mint, and first independent attester (blocks
968040/968046)**: the operator's own daemon anchored a MINT by itself;
its self-ATTEST couldn't be funded, so the mint waited. An independent
auditor's loop saw "verified on Ethereum, unattested" and anchored its
own ATTEST — then, every cycle until that anchor confirmed, anchored
three more *identical* ATTESTs (1,244 sats wasted, since a statement
whose target was already attested was still being re-anchored). **Fixed
same day**: the daemon keeps a statement-hash ledger and never re-anchors
the same statement twice; the on-chain program treats a redundant ATTEST
as a harmless no-op instead of erroring. With those in place, the mint
succeeded on the independent attester's own escrow. Spend caps (max fee,
max anchors/hour, max sats/hour) were added to the daemon after this
incident.

**First slow-path redeem (block 968053)**: a RELEASE anchored
deliberately unattested, to exercise the window-close path for real —
paid from the vault after the challenge window with no attester
involved. First live use of the relay's jump path for headers, and the
first real test of settlement with nothing fast-tracked.

**Acceleration fee added (MINT side only)**: an ATTEST buys the user
*speed*, not correctness — the operator's bond already covers correctness
on every claim, attested or not. Speed gets its own, separate incentive:
the user who wants speed pays for it directly (an optional `attest_fee`
on `lock_sol`, paid to whoever accelerates them immediately, refunded if
nobody does). Deliberately scoped to MINT only; the same hook on RELEASE
was identified as a natural follow-up, not built same-day.

**EVM/SVM parity check, done by reading both, not assuming**: found and
fixed a real bug where a second attester of an already-attested MINT
silently overwrote the first in storage — only the *last* attester would
have borne a fan-out slash if the mint proved false, letting the real
first attester walk away. Fixed to lock in only the first attester,
matching the Solana side's existing semantics. Also confirmed one
genuine, still-open gap: Ethereum has no equivalent of Solana's
`audit_skipped_release` — nothing lets a skipped anchor be revisited if
its preimage later surfaces, on the Ethereum side specifically.

## 2026-09-23: composition (multi-network, multi-token)

Solana hub generalized to support an arbitrary composition of components
across networks and tokens (built and tested incrementally over the
day — a flat per-composition budget of 8 components, up to 4 of them
local, replaced an earlier, more rigid 4-networks-×-4-tokens grid after
review found the grid and the mint-time account limit had silently
drifted apart).

**Tempo and Hyperliquid deploy blockers, found and resolved the same
day**: Tempo's contract-creation addresses turned out to ignore the
sender's nonce lane (a confirmed chain bug — see
`ipow-implementation.md`'s Tempo section for the rule this produced:
never trust a Tempo deploy receipt). Hyperliquid's HyperEVM testnet block
gas limit turned out to be too small for the plain contracts to even
deploy — resolved with the router+facets split, applied in turn to
`iPoW`, `BetaVault`, and `BetaHub` as each was rolled out to that
network.

**`BetaHub` built and deployed to every EVM network** — an EVM chain
acting as a mint hub for the first time, not just a spoke. Two real bugs
caught during implementation, before deployment: (1) an early draft let
a caller supply the anchor-skip timeout directly as an argument, meaning
anyone could pass zero and skip instantly — fixed by moving it into
governance-set parameters like every other timing constant. (2) The
MINT-targeting ATTEST/CLEAR branch was checked against the wrong
assumption about fast-payout behavior — corrected after re-reading the
reference contract's actual behavior rather than assuming symmetry with
its RELEASE branch.

## 2026-09-23: first real multi-network BETA mints, using genuine Bitcoin

**First 5-network mint**: Solana's `beta-factory` as hub, with legs on
Solana (local) plus Ethereum, Base, Robinhood Chain, and Hyperliquid
(remote) — every anchor a real, mined Bitcoin mainnet transaction. A real
operational lesson caught before any harm: statements were originally
chained into one linear sequence for speed, which works for the hub
(which processes every link in order) but not for each spoke's own
`Party.anchorTxidLE` pointer, which can only advance one link at a time
and has no way to skip links meant for other chains — the first attempt
had three of four remote legs revert with a "not on this chain's
statement chain" error. A revert here is atomic (no partial state, no
funds moved) and was caught immediately; fixed by registering a second
party per affected chain, pointed at the correct intermediate outpoint.
Result: 1 BETA minted, backed by 1 real locked local unit plus 4 real
finalized remote locks, independently verified against each chain's own
state.

**First EVM-hub, Tempo-spoke mint**: Ethereum's `BetaHub` as hub, Tempo's
`BetaVault` as the sole remote leg. Surfaced the confirmed, previously-
undocumented fact that Tempo rejects any transaction carrying native
value outright — not fixable by changing how a script calls the existing
contract; required the PathUSD-denominated `BetaVaultPathUSD.sol`
variant documented in `ipow-implementation.md`. Because the MINT anchor's
operator had already self-attested, the mint exercised immediately rather
than waiting for the full challenge window.

The scripts that drove these two runs were one-off and hard-coded to them
(Bitcoin block 968203, parties 0x05/0x06, composition 2, the old
BetaVault), and were removed on 2026-10-09:
`programmable-network/ethereum/scripts/live_multi_network_{evm,commit_and_process,fix_party}.mjs`
and `programmable-network/solana/scripts/live_multi_network_{mint,solana_process}.ts`.
They remain in git history at commit `9f5b691`.

## 2026-10-03: Conversion redeployed for the tunnel (T1, T2)

Conversion with `sellInWindow` and `buyFor`
([`ipow-conversion-tunnel.md`](ipow-conversion-tunnel.md)), deployed by
the owner. None of the contracts replaced held a swap.

| Network | New Conversion | Replaced | Unused |
|---|---|---|---|
| Sepolia | `0x5fdFfea2E1e03E6Eb376Aa6d679A4C8b62bf6da1` | `0x9dF80412e01720F904f5059559eD6f66c7558803` | `0x7082Eba69c5804773E6731EeA3103154025e7fEC` |
| Base Sepolia | `0x6CCCA5c12689DC5d14F8C540c7c0C9a20731e8C1` | `0x6c5d75F830C002f4B73b4625F3E2bFaeC50b5D2E` | `0xCA39c4e5d4f938A380E61475bF4CeF8D1B7d6190` |
| HyperEVM testnet | `0x6CCCA5c12689DC5d14F8C540c7c0C9a20731e8C1` | `0x6c5d75F830C002f4B73b4625F3E2bFaeC50b5D2E` | `0xCA39c4e5d4f938A380E61475bF4CeF8D1B7d6190` |
| Robinhood testnet | `0x2Dd456fCe7B3574AbD76b2899d3106CaB6ff5b9B` | `0xb856906fEBAFBB21A06DdC35E9BCe476139A86BA` | |
| Solana devnet | program `9Argk3M83p8t5pWG92PhwEhYzysWt5n82siEWyb29w7D` upgraded in place, slot 507034248 | | |

Read back: `verify-deployments.ts` matches every contract of all seven
EVM test networks with its build; each new Conversion
and each unused one holds 17,792 bytes of code and no swap; the Solana
program's code is byte for byte `target/deploy/conversion.so` (SHA-256
`3c9324c0...`), with `SellInWindow` and `BuyFor` in it.

Friction, for the SDK and Greatwall:
- The script ran twice per network: the first run's contracts are left
  unused (listed in each deployment as `unusedConversions`), and the
  second recorded the first as the one it replaced, corrected by hand.
- The Solana upgrade first failed: the CLI extended the program by 6,352
  bytes, below the 10,240 the loader requires; `solana program extend`
  by 10,240 first, then the deploy, went through.
- Robinhood's public RPC failed TLS (`UNABLE_TO_GET_ISSUER_CERT_LOCALLY`):
  the deploy packages now use Alchemy's endpoints for Base, HyperEVM,
  Robinhood and Tempo (each checked by its chain id first), and Robinhood
  was redeployed through it, once.
- Hedera, Polkadot and Tempo kept the old Conversion: its code was added
  to `deployments/source-a9493c7/` so the check compares them with it.

## 2026-10-04: the first tunnel, SOL on devnet into AAPL on Sepolia

The first conversion between two programmable networks on the new protocol
([`ipow-conversion-tunnel.md`](ipow-conversion-tunnel.md)), run from
Greatwall's Convert by the owner, with one operator node on both legs.

| UTC | Step | Where |
|---|---|---|
| 01:07 | The user opens buy 8 (0.36 AAPL for 900 sats) and pays its job fee | Sepolia |
| 01:08 | Greatwall registers the buy, signed by its owner; the node promises its sale | node |
| 01:10 | The node bids; the auction ends a minute later | Sepolia |
| 01:19 | Anchored at Bitcoin block 969784; the AAPL locked, address `bc1qtyq93a0…` named | Sepolia |
| 01:32 | The user signs sale 1 (0.06 SOL, paid to that address in blocks 969785 to 969796); the node wins it | Solana devnet |
| block 969786 | The node pays 900 sats (`93d979a1…`), and its receipt spends them (`ac335dbc…`), in the same block | Bitcoin |
| 03:25 | Buy 8 proven and completed: 0.36 AAPL to the user | Sepolia |
| 03:26 | Sale 1 proven; the node collects the SOL after its lock (2026-10-05 15:26) | Solana devnet |

Two hours from the user's first signature to the AAPL; the user signed
twice on Sepolia (the buy, the registration's message) and once on Solana.
Cost to the operator: the two Bitcoin transactions' fees, from a wallet of
about 14,000 sats; everything else came back to it.

What it taught, fixed the same day:
- The node read a buy's job fees with `feesFor`, which many RPCs run at a
  price of zero: the protocol refused the buy (`FeesNotPaid`). It now
  prices with `commitmentFeeAt` and the network's price, as the SDK does.
- Tunnels opened by the operator for the user (T2) cost the operator a job's
  fees whenever the user walked away: three such buys were left before the
  owner decided the user opens and pays for the buy (Q5, revised). Those
  jobs also locked most of the operator's 0.1 ETH bond, so a later buy
  found no bidder: the bond was raised to 0.2 ETH.
- The node's API cut connections at 30 seconds, less than a Sepolia
  transaction takes: two buys were opened and never recorded.
- mempool.space does not answer from the owner's network; the app's own
  explorer calls had no timeout and hung the page. Everything now goes
  through the SDK's client, blockstream first, 8 seconds each.
- A Solana send whose confirmation web3.js gave up on after 30 seconds had
  landed; the SDK now asks for its status for up to two minutes.
- The app had kept the two legs linked only in the browser; the SDK now
  finds a buy's sale on chain (`sellPaying`), and the page records it.
- The page: a timer read the swap before it loaded; the source side of a
  tunnel's buy showed the sats as if they were SOL and no address; a
  completed tunnel read "Converting" after the operator's lock on the sale,
  which is not the user's wait.

## 2026-10-04: Conversion redeployed again (one-transaction close, the buy's user script)

By the owner, once per network this time. The old contracts keep their
swaps (Sepolia's eight, among them the first tunnel's buy); the node
watches the new ones from here.

| Network | New Conversion | Replaced |
|---|---|---|
| Sepolia | `0xFB82B968e1529E740588aAE0F6379c956BF6683C` | `0x5fdFfea2E1e03E6Eb376Aa6d679A4C8b62bf6da1` |
| Base Sepolia | `0x8A5EB387b8b1CBd5cBAFBE2088aCE6d589eb4F59` | `0x6CCCA5c12689DC5d14F8C540c7c0C9a20731e8C1` |
| HyperEVM testnet | `0x8A5EB387b8b1CBd5cBAFBE2088aCE6d589eb4F59` | `0x6CCCA5c12689DC5d14F8C540c7c0C9a20731e8C1` |
| Robinhood testnet | `0x6c5d75F830C002f4B73b4625F3E2bFaeC50b5D2E` | `0x2Dd456fCe7B3574AbD76b2899d3106CaB6ff5b9B` |

Read back: `verify-deployments.ts` matches all seven networks. Left on
the old Sepolia contract: swap 2, a buy the operator opened for a user
and funded (0.36 AAPL locked), reclaimable by anyone 36 hours after its
deadline; the node no longer watches it, so it is reclaimed by hand.

## 2026-10-04: Conversion redeployed a third time (the buy's memo)

By the owner, once per network. A tunnel is now one transaction: the buy
carries the sale's terms in its memo, read by the node. The previous
contracts keep their swaps (Sepolia's one).

| Network | New Conversion | Replaced |
|---|---|---|
| Sepolia | `0x3896b0B95D853655A62CCb83079509716E77962d` | `0xFB82B968e1529E740588aAE0F6379c956BF6683C` |
| Base Sepolia | `0x07eFF65A853f36cBA8FEFbC495eb6A0D98d26a75` | `0x8A5EB387b8b1CBd5cBAFBE2088aCE6d589eb4F59` |
| HyperEVM testnet | `0x07eFF65A853f36cBA8FEFbC495eb6A0D98d26a75` | `0x8A5EB387b8b1CBd5cBAFBE2088aCE6d589eb4F59` |
| Robinhood testnet | `0xf35A1C4A9dE7B7FffF62c7cEfda2ac91a71cB6cc` | `0x6c5d75F830C002f4B73b4625F3E2bFaeC50b5D2E` |

Read back: `verify-deployments.ts` matches all seven networks. Three
redeploys in a day is the cost of deciding the flow on a live run; the
contract is small enough that each took minutes.

## 2026-10-05: Greatwall's official mock RWA tokens, Sepolia and devnet

By the owner, once per network (`scripts/deploy-mock-rwa.ts` in each
package). Ten stand-ins per network for real tokenized assets (rwa.xyz),
replacing the four "(test)" tokens of 2026-10-04 as the set Greatwall
lists and the node prices; the old four stay deployed and recorded.

Sepolia (`deployments/ethereum-testnet-rwa.json`): MockRWA named
"<asset> (Sepolia Greatwall)" — XAUT, STRCx, IVVon, QQQon, NVDAon, SLVon,
IEFAon, AMDon, SOFIon, HIMSon — each with 1,000 minted to the operator
and registered with the vault as assets 5–14; faucet 5 a claim (XAUT: 1).
Thirty transactions, no failures. Read back: code, vault asset number
and operator balance by the script; name, symbol and faucet amount of
two by hand. An ERC-20 carries no logo, so Greatwall serves the images
(`public/assets/rwa/<symbol>.png`), a token list (`/api/tokenlist`) and
an "Add to wallet" button (EIP-747 `wallet_watchAsset` with the image).

Solana devnet (`testnet-rwa.json`): SPL mints named "<asset> (GW Devnet)"
— ONyc, MSTRx, CRCLx, SPYx, TSLAx, GLDx, GOOGLx, SPCXx, QQQx, HOODx — each
in one transaction with a Metaplex Token Metadata account (name, symbol,
URI `https://greatwall.finance/assets/rwa/solana/<symbol>.json`, which
Greatwall serves with the image), 1,000 minted to the operator, then
registered with the vault's pair with Ethereum as assets 5–14. The first
run failed in simulation ("Name too long": Token Metadata allows 32
bytes, "(Devnet Greatwall)" took 20 of them), landing nothing; the names
were shortened and the script now checks the limits before sending.
Devnet's public RPC rate-limited (429) through the run; every call
retried and succeeded. Read back: mint authority, decimals, operator
balance, and each metadata account's name, symbol and URI by the script.

HyperEVM testnet (`deployments/hyperliquid-testnet-rwa.json`), the same
script with `--big-blocks`: nine, named "<asset> (HyperEVM Testnet
Greatwall)" — thBILL, USDH, GMEx, MUx, USDHL, SKHYx, NBISx, IWMx, CRWVx —
vault assets 1–9 (its vault had none). Eight deployed at 0.1 gwei; the
ninth was refused for funds: the node's fee estimate had jumped to 500
gwei (0.32 HYPE for one deploy, against 0.078 held), and the script had
written its record only at the end, so the eight addresses existed only
in the terminal. The record was written from that output by hand; the
script now saves after every token, and on a big-block network caps the
fee at twice `eth_bigBlockGasPrice` instead of taking the estimate.
Minutes later the big-block price read 0.1 gwei again and the ninth went
through. No HYPE faucet exists; none was needed.

Base Sepolia (`deployments/base-testnet-rwa.json`): ten, named "<asset>
(Base Sepolia Greatwall)" — GLDY, JTRSY, AUDD, JSPXA, JSPX, bTSLA, USTBL,
wtNKE, AMZN.d, wtMCD — vault assets 1–10. The endpoint is a pool of
nodes that lag each other by a block: the first mint was estimated by a
node that had not seen the deploy (22,946 gas) and ran out; later, reads
right after a registration returned 0 from such a node (recorded as
asset −1 for six tokens). GLDY, deployed but not minted, was added to the
record by hand; the script now records a token the moment it is
deployed, mints whenever the operator holds none (so a stopped run
continues), waits for the code and the registration to be visible, and
sets the gas of a mint (120k; measured 69k) and a registration (300k;
measured 101k) rather than estimating. The record's asset numbers were
then corrected from chain. Read back on chain by hand, all ten: name,
symbol, faucet amount, minter, the operator's 1,000, vault asset.

Robinhood testnet (`deployments/robinhood-testnet-rwa.json`): ten, named
"<asset> (Robinhood Testnet Greatwall)" — USDG, SPYr, NVDAr, SPCXr, METAr,
COINr, USOr, GMEr, PLTRr, SGOVr (the stock tokens have no ticker on
rwa.xyz; the symbol is the stock's ticker with an r) — vault assets 1–10,
with the hardened script: thirty transactions, no failures, every name,
symbol, faucet, minter, balance and asset number read back by the script.

Tempo testnet (`deployments/tempo-testnet-rwa.json`), from the Tempo
package's own `scripts/deploy-mock-rwa.ts`, since ethers cannot send
Tempo's transactions: three, named "<asset> (Tempo Testnet Greatwall)" —
EURC, USDY, syrupUSDC, faucet 100 each — vault assets 1–3, fees in
PathUSD, each sent in the ordinary nonce lane with the address worked out
from the nonce and the code read there (implementation doc, Tempo
section); every receipt named the same address. Nine transactions, no
failures; read back by the script. Greatwall's Tempo wallet now reads
these tokens' balances and claims the faucet with an ordinary MetaMask
transaction, as its sends already do; whether Tempo accepts that from
MetaMask is not yet tried in a browser.

Arbitrum was asked for next and parked: it is not an iPoW network (D133
fixes the eight numbers, and both vaults refuse a ninth), so adding it is
a design decision and a deployment with test funds the deployer does not
hold there yet; see the memory note of the day.

Greatwall lists all five EVM sets and the Solana one, with logos. The
node's testnet settings now name Base Sepolia, HyperEVM testnet and
Robinhood testnet too (`core/node/node.testnet.yml`: their light client,
protocol and Conversion, a bid of at most 0.01 of the coin, and every RWA
token priced at ten sats per dollar of the real asset, dollar-like tokens
at 10 sats), with `run-testnet.sh` loading each package's `.env`; the
operator takes swaps there once its bond is locked (0.03 of the coin, by
`lock-bond.ts`) and the node restarted. Not Tempo: the node pays the
native coin with every call and cannot pay PathUSD yet. Not Solana's
tokens: the node's token buys are not built there.

## 2026-10-05: BetaBaskets redeployed, and the basket program upgraded (E4)

By the owner, once per network, for the creator's naming of a basket's
token and its metadata URI (E4, docs/drafts/ipow-beta-app.md). The
baskets' record changed, so on Ethereum it is a new contract: the old one
keeps its baskets under its own rules (`scripts/redeploy-beta.ts`). On
Solana the program was upgraded in place, after extending its account by
30,000 bytes for the larger build (374,672 bytes held, 360,224 used); a
basket of the first build has no metadata account and reads as "BETA".

| Network | New BetaBaskets | Replaced |
|---|---|---|
| Sepolia | `0x80f99051921754C0b2F7BeC7468b72a0E6e2850b` | `0xb5375fDaF8b6E04942dFc593E7065A606Fe5588d` |
| Base Sepolia | `0x6B70Af8a21a16F0c36dF37Ff1BAC581Cd3894Cb7` | `0xf35A1C4A9dE7B7FffF62c7cEfda2ac91a71cB6cc` |
| Robinhood testnet | `0x7EbA21758f149b18FDc0438B492D6059E37031b2` | `0xd9a6e550Ea8a3970d08F34b0b1a417338C839C4c` |
| HyperEVM testnet | `0x36BA462fF711C7261C4894Af40E5f0c49C98b280` | `0xf35A1C4A9dE7B7FffF62c7cEfda2ac91a71cB6cc` |
| Solana devnet | `3DuG4iNPptEyCAA7FM1YQja43G6HkTAwrswT3Ds2d94R` (upgraded, slot 507,711,970) | the same id |

Read back: each contract's limits (8, 100, 64, 16, 2,048) and an empty
naming by the script; `verify-deployments.ts` matches all seven networks,
Hedera, Polkadot and Tempo against the old build kept in
`deployments/source-<commit>/BetaBaskets.json`, since they still run it
(Tempo's redeployment needs a script of its own package, not written).
The SDK's `scripts/sync.ts` run; Greatwall's Create pages make named
baskets on the new contracts. The SDK's Solana test could not run on this
machine (its local validator stops a few seconds after starting, before
any transaction; not this change's doing), so the TypeScript client's
first real call is the first basket made from Greatwall.

## 2026-10-05: BetaBaskets redeployed again, and the program upgraded (E5)

By the owner, once per network, the same evening, for parts named before
their receipt exists (E5, docs/drafts/ipow-beta-app.md): the parts'
record changed again, so a new contract on each EVM network (the E4
contracts of the morning keep their baskets); Solana's program upgraded
in place, the build of 365,912 bytes within the account extended that
morning.

| Network | New BetaBaskets | Replaced (E4, the same morning) |
|---|---|---|
| Sepolia | `0x7311B038A8e2c1F1C2b6a266E6cA38ed3804F3BF` | `0x80f99051921754C0b2F7BeC7468b72a0E6e2850b` |
| Base Sepolia | `0xB6C7e5Db33F34D86102C8Df0AcACAd7052699162` | `0x6B70Af8a21a16F0c36dF37Ff1BAC581Cd3894Cb7` |
| Robinhood testnet | `0x529bbD58E239C515465137B1e23cc1Fd1360Fb4b` | `0x7EbA21758f149b18FDc0438B492D6059E37031b2` |
| HyperEVM testnet | `0x51d0bc4CE62Fa394242415d0DAa167A5AF6Bb45B` | `0x36BA462fF711C7261C4894Af40E5f0c49C98b280` |
| Solana devnet | `3DuG4iNPptEyCAA7FM1YQja43G6HkTAwrswT3Ds2d94R` (upgraded) | the same id |

Read back by the script (limits, empty naming); `verify-deployments.ts`
matches all seven networks. Two redeployments of one contract in a day:
the cost of deciding E4 and E5 on a live run, as with Conversion. What
the flow still needs to run end to end: the node's vault role (left out
of `node.testnet.yml`), so that any lock is carried and a receipt ever
made; and Solana's token locks in the SDK and the Bridge app (SOL only
so far), for an EVM basket's Solana parts.

## 2026-10-05: the first vault pair between two EVM networks (Sepolia–Base Sepolia)

By the owner, in one run of `scripts/deploy-pair.ts ethereum base` (ten
transactions). Each EVM network had been paired with Solana only; this
pairs two EVM vaults, each naming the other at deployment, the second's
address predicted from the deployer's settled nonce (spec Build status,
row 7) and read back where predicted.

Each side got factories of today's build first: the live factories are
the a9493c7 build, which made the parts from within the vault and keeps
no record of them, while today's vault asks its factories whether they
made its parts. The pair's vaults record their own factories;
`verify-deployments.ts` checks them against today's build
(`source: "current"` on the vault's entry) and the first pairs against
a9493c7 as before.

| Network | Vault paired with | Vault | Home | Receipts | Factories (home, receipts) |
|---|---|---|---|---|---|
| Sepolia | Base Sepolia (3) | `0x7901fd0a77124EBA55cB4599F67b0405F43E00c3` | `0x71b935346d2A8F7709abeE700fb7a2470A6F19A1` | `0x50Cb86cdB69b08a89C8a1e02C5cAA5177B1cf2B0` | `0xF4475a11a71940CF09468e6397565321AE49d333`, `0xD630Fe315F8F1b78e0aE2095dc9F93Dc64D43988` |
| Base Sepolia | Sepolia (1) | `0xCe6Cda46Ac4d02c6BbB3613de07821e0fd80b313` | `0x5D0174Bd9f6d10840d4d9FEeA2CcBdBbAb5777C8` | `0x54BC75a61D3F0D213F0828755Ef6E7ef1f2422c6` | `0x4F479930C9f65FA41bb9aBF09bCaE668e0857DFE`, `0x46bC1Efc370FDe6945014a52756E75f919AD8067` |

Read back by the script (networks, peer vault, protocol, amounts,
factories, parts, each contract's code) and by `verify-deployments.ts`.
The reviewer's dry read of the live factories found the a9493c7 build
before the first attempt was run; that attempt would have failed on its
first simulation, spending nothing. Greatwall's Create pickers list the
pair's assets on both sides. Not yet: the pair in the node's settings is
written under the vault role, which stays off (it registers a pair chain
on Bitcoin); the Bridge app has no Sepolia–Base direction; the four other
EVM–EVM pairs.

## 2026-10-05: the other five EVM–EVM vault pairs

By the owner, five runs of `scripts/deploy-pair.ts`, one after another,
the HyperEVM ones with `--big-blocks`: every EVM test network is now
paired with every other, six pairs, each vault at its predicted address
with its own factories and parts, read back by the script and by
`verify-deployments.ts` (24 to 25 contracts per network, all matching).

| Pair | First network's vault | Second network's vault |
|---|---|---|
| Sepolia–Robinhood testnet | `0x995D56414b04Fe2699978C7E0B945FF623c1326b` (home `0x0e81c951966FFE6E6067266a2Ff237AfE35E6729`, receipts `0x44DeBbBB5f4d91a1de3228C7796C7193a648c6Be`, factories `0xe5aA792d82281Bd86fD297290ac7eEe72C79b1C1`, `0x5B9148d4f4f91A83CD9642d3cd6E08F01bbd0826`) | `0x170685feEe5ac2bCddAE20DAe66747658E6fA7c0` (home `0x27824021Cd136F59C357C902b24b35CbE64e0C6A`, receipts `0x4994B8161A265CF219385Bd02Af23C033CC000F2`, factories `0x592659De4a7D5F31cfE95C1c6c2A0456343b82E0`, `0xC0fE6c22b0034E559CeBB4c064098aB887818E32`) |
| Sepolia–HyperEVM testnet | `0x181e3C5D35E4ecaC682FEFC375dEc9CC5D1d3A91` (home `0x122f0C67aa722861E68d03682707AA7C061cDE48`, receipts `0x947BcdA4F8F6787B38484A8389077c901e9e8A39`, factories `0xCC6B502c4bb87ffD2fBA3412F8EcC7c27659B12A`, `0x769d3CB24E20C77b45Bb0FCec35c7750f7E02CfC`) | `0xc949e35812679fF3Be6aF0B025e2ddFff16bC2b9` (home `0x03397024aC9191D9a13014d6A9aEAaFdAE7f3774`, receipts `0xE346843867B4756859Ea1a176cBC636d2074fFFA`, factories `0x7F0211E1BEA9A940f4E2F4A21B52f2f69C6e8dD5`, `0x405FaC50eB5d0134cf971310EBd6cdcDF2721b86`) |
| Base Sepolia–Robinhood testnet | `0xCA92aED3Bbb8Bab3e877Bf3092924aD51B2a00d4` (home `0x2E3Fe0bF1955188Da561EAb643f15bcd413A1512`, receipts `0x38E0Bf6cFcc850430D51f8b3E899e9F225aA8d5D`, factories `0xD44a36ac5549EbE7c7A54642104867539b2d01A1`, `0xE610e70B799b3d169BeA946a9029655226985805`) | `0x3221102aC4159048960e061FC0ADb81De3133496` (home `0xa6629b7F085114535f1F64deE27BE880e67D876C`, receipts `0x00c360219F11Cc6f4D0472353d411dd0D7891CB8`, factories `0x85A86D77357898D79b49c464ccaB03411109e722`, `0x96AC11A1Cf37C5c9704ecc080b539Eb7F81d9b81`) |
| Base Sepolia–HyperEVM testnet | `0xA4F697D5ED3Bcb96CA147c13a157779daE11d2aa` (home `0x74C2Bce2f8d7657d407AA01b28a40De592f6880C`, receipts `0x02FdE8720777E07D5C40BAAA048C1C398C4573eC`, factories `0x840ba28Ca3edDB6F8934529F56d8cf55357b13b9`, `0xc6d84d6bE65567a5f54f1dc92f378421EeB6b45A`) | `0xAfC5e11a1e2A0443248844757C9a2256Da5A742A` (home `0x70Cceb80c265168D1837030daD812e5e06f00321`, receipts `0x115C9E3c93F384E9f8fb177aA49538D09061B0cD`, factories `0x6B70Af8a21a16F0c36dF37Ff1BAC581Cd3894Cb7`, `0xB6C7e5Db33F34D86102C8Df0AcACAd7052699162`) |
| Robinhood testnet–HyperEVM testnet | `0x51cD31AC42a838d225f3B42F830Af72d99Eb003a` (home `0xD06ea940f00De09851c7acA6f0f6a66b71FaE16E`, receipts `0xC69EBc691174a55E240552674896bBE651ebBad2`, factories `0xB001c1Eb3B2D6dF4380682E119dDF1e3a9F18D89`, `0x2ab2c9487cC8f83816d556Da149132c705B82e48`) | `0x62EDd87078a43B9e915aF91FF0ea137b34e9d7CB` (home `0x36E1d9CFb7CBf13eC1ea6c80E4B5405f5327CfED`, receipts `0xe4aC5A816a24A0c7Ad56DCCdbcfB36F329e76F3a`, factories `0xCF87B06B09754ed3c9855b64Ae6FD295a7a1da10`, `0xCe6Cda46Ac4d02c6BbB3613de07821e0fd80b313`) |

Greatwall's Create pickers show every EVM network and Solana on each
network. The node's settings list all six pairs under the vault role,
still off. Not yet: the Bridge app's EVM–EVM directions; the operator's
pair chains on Bitcoin (one real transaction per pair) and bonds.

## 2026-10-06: Arbitrum Sepolia as network 9, and every token in every vault

`scripts/deploy-arbitrum.sh`, from commit `c9a2a60` (D141). In order:
Solana devnet's vault upgraded to the build that takes 9; the protocol on
Arbitrum Sepolia, its vault paired with Solana; Solana's pair account
`["config", 9]` set up; Arbitrum paired with Sepolia, Base Sepolia,
Robinhood and HyperEVM; Arbitrum's 11 mock RWA tokens; VTIon and IEMGon on
Sepolia, GLDx on Base, SPYx on HyperEVM; and every listed token registered
with every vault of its network (one per pair), so a token can be a
basket's part on any paired network. `verify-deployments.ts` then read every
network back: all match (Arbitrum 30 contracts).

Friction, three runs:

- **Arbitrum's gas.** A factory deployment on Arbitrum Sepolia ran out of
  gas at exactly its estimate (3,655,771): an Arbitrum transaction's gas
  includes posting its data to Ethereum, whose price moved between the
  estimate and the block. Nothing was left half made (a pair's records are
  written only when both vaults exist; one unused factory remains). Fixed:
  on an Arbitrum chain the pair and token scripts give the estimate plus
  half.
- **HyperEVM's price.** Its endpoint answered "method not found" to
  `eth_bigBlockGasPrice` once, between answers. Fixed: asked again, then
  the last price read.
- **Time.** About 250 registrations, most of them catching up: before this,
  tokens were registered only with each network's vault toward Solana, so
  no token could be a part across two EVM networks. HyperEVM takes about one
  such transaction a minute (big blocks), about 40 minutes of the run.

For the SDK and Greatwall: a network added later costs one vault pair per
network already there, and each of its tokens one registration per pair.

## Housekeeping

**2026-09-24**: a stale, expired, never-claimed test `Pending` account on
Solana devnet (from an early composition-support validation run) was
cleared via a real, permissionless `expire_pending` call, refunding its
locked SOL. `beta-factory` devnet had zero live `Pending` accounts
afterward — a clean slate, not evidence anything was ever broken.
