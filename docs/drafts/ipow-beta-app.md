# BETA on the new protocol (stage 6)

**Status: approved by the owner on 2026-10-01; built on Solana with the RWA rules, and on Ethereum (see "Built"). Not deployed.** The second application of
stage 6 in [`ipow-build-plan.md`](ipow-build-plan.md). It is built on the
protocol's vault and its receipt tokens (spec section 11)
([`ipow-vault-claims.md`](ipow-vault-claims.md)), which the owner chose
on 2026-09-30 over carrying the old BETA's messages
([`../design/ipow-implementation.md`](../design/ipow-implementation.md),
"Beta: statement-bus mechanics") as an application.

## What BETA is

A token on Solana backed by a fixed basket, chosen when the basket is
created. In the first version, **1 BETA = 1 SOL + 1 vETH**. vETH is the
vault's receipt for 1 ETH locked in the vault on Ethereum.

A basket has up to 8 parts, one per network: the mint network's own
token, and one receipt for each other network, such as vETH from
Ethereum or vUSDC from Base. The program is built for 8 parts from the
start, so a basket with more networks is a new basket created in the same
program (E2), not a change to it.

All parts of the basket are tokens on Solana, so BETA checks its backing
itself. Nothing crosses networks inside BETA: moving assets in and out
is the vault's job. A lie that slipped through on one network's pair
affects only that receipt, not the other parts.

## Mint and burn

| Action | What happens |
|---|---|
| Mint | The user puts every part of the basket into BETA, for example 1 SOL and 1 vETH per unit, and receives the units at once |
| Burn | The holder burns units and receives every part back at once |

A user who holds ETH on Ethereum first turns it into vETH through the
vault (7 days, or at once with an attester). A holder who wants ETH back
burns the vETH through the vault.

## Kept from the old BETA, and dropped

| Kept | Dropped, because the vault or the protocol has it |
|---|---|
| A token backed by a fixed basket spanning several networks | MINT, RELEASE, ATTEST, VETO: the vault's records and objection window |
| A claim about a fact is checked against the network that holds it, and each author's statements form one ordered chain on Bitcoin: now the vault's message chain per operator | The old party registry's roles: vouching is done by protocol operators in the vault |
| A lie is paid for in the asset lied about | Insurance pools: the escrow is in the same asset |
| | The governance key (D59) |

## Parts whose issuer controls them (RWA tokens)

Decided by the owner on 2026-10-01 (E3, replacing its first answer): a
basket may hold tokens whose issuer can freeze, pause, move or allowlist
them, such as tokenised stocks or stablecoins. BETA makes them no worse to
hold than holding each token yourself, and keeps one issuer's action to its
own part.

| Rule | What it does | Example: 1 SOL + 1 vETH + 1 xAAPL |
|---|---|---|
| A burn pays each part on its own | The burner names the parts to defer: those become an amount owed to it, collected later; the others pay now. Solana cannot catch a failed transfer, so a frozen, paused or allowlisted part is deferred by choice rather than failing the whole burn | The issuer freezes the basket's xAAPL. Bob burns 1 BETA: he receives SOL and vETH now, and his xAAPL is owed to him until it can move |
| A burn pays a share | Each part pays the burner's fraction of what the basket actually holds of it, less what it owes | The issuer takes 10% of the basket's xAAPL: every holder's xAAPL falls 10%, and nobody can burn first to leave the loss to others |
| A mint follows the basket's ratio | The first mint of a basket is a whole number of BETA and deposits exactly the parts it was created with; after that, each part in proportion to what the basket holds per BETA. While any part holds nothing, because its issuer took it all, nothing can be minted: that part would be free for new holders. Rounded up on the way in and down on the way out, so every BETA stays fully backed | After the 10% loss, a new BETA deposits 0.9 xAAPL, not 1: new holders do not pay for the loss |
| Owed amounts | Set aside from the part and paid first come, in full, when collected. A later loss of that part falls on the holders, not on what is owed, as long as it is no larger than what the holders have; beyond that, the owed amounts are paid first come until the part runs out | |
| Risk flags | Recorded per part when the basket is created: the issuer can freeze, pause, move tokens out (permanent delegate), requires allowed accounts (accounts frozen by default), can close the mint, or can change what a unit is worth (interest, a scaled amount such as a stock split). A mint refuses a part whose token no longer has the rules recorded for it | A wallet shows "xAAPL: Backed Finance can freeze and move it" |
| Token-2022 | Accepted, with the extensions above and those that only describe a token (metadata, groups). Refused for now: transfer fees, transfer hooks, confidential transfers, tokens that cannot move, and any extension not known. Each breaks the accounting, or needs more than the program does yet | |
| The creator's fee | On each part when it moves: on a deferred part, when it is collected, so deferring never avoids it. Set aside in the basket and collected by the receiver, rather than sent at once: an issuer that freezes the receiver's account, or a receiver that refuses a payment, then stops no holder's mint, burn or collection. Decided by the owner on 2026-10-02, after a review found that a frozen fee account stopped mints and the collection of deferred parts | |

A basket with a part whose issuer requires allowed accounts works only if
the issuer allows the basket's account, and pays that part only to allowed
holders. A basket trusts the issuer of each such part for that part, as
holding the token does; BETA adds no trust of its own.

## Built

On Solana on 2026-10-01: `programmable-network/solana/programs/beta-basket`,
tests in `programs/beta-basket/tests/test_beta_basket.rs`. Not deployed.
The build settled these points, within the decisions below:

| Point | In the program |
|---|---|
| What a part is | A token of the classic program or Token-2022, with the extensions of "Parts whose issuer controls them", its issuer powers read from its mint and recorded. The program cannot tell which network a token stands for: one part per network is the creator's choice |
| Units | BETA has 9 decimals; a part's amount is what one whole BETA holds |
| Rounding | A mint pays each part rounded up, a burn pays it out rounded down, so every BETA stays fully backed |
| Deferring | A burn names the parts to defer in a bit mask; each deferred part is owed to the burner in a record of its own, collected in full when the part can move |
| The fee | A share of each part, rounded up so that splitting a mint or burn into small ones never avoids it: paid on top of the parts when minting, taken out of them when burning, and set aside in the basket. Each receiver has a record of its fees in a basket, by part: the creator's opened with the basket, a new receiver's when the fee is handed to it, its rent paid by the receiver handing it on and never closed, so a mint or a burn opens no fee record. A receiver collects into any account of the part, and keeps what it earned after handing the fee on. Until collected, fees are held with what deferred burners are owed: if an issuer takes more of a part than the holders have, the rest is paid first come to them both. With no fee, the record is not needed. Handed to the basket itself, every later fee goes into the backing for good |
| Size | A mint of a basket of 8 parts takes 40 accounts (4 per part) and more than Solana's default compute: a client sends it with a lookup table and a compute-budget instruction |
| A basket | Numbered by its creator; each number once. Its parts are distinct tokens |

On Ethereum on 2026-10-01: `programmable-network/ethereum/contracts/apps/BetaBaskets.sol`,
tests in `test/BetaBaskets.test.ts`. Not deployed. The same rules (up to 8
parts, fees of at most 1% fixed at creation and rounded up, a whole first
mint, mints by the basket's ratio rounded up, burns of a share rounded
down, deferring by choice, no mint while a part holds nothing, the fee
handed to the basket itself backing it for good), with these differences
made by Ethereum:

| Point | On Ethereum |
|---|---|
| A basket | `BetaBaskets` holds the rules; each basket is its own `BetaBasket` contract, its BETA token (9 decimals, as on Solana), which also holds its parts. A basket's backing is its own balance, as a Solana basket's token accounts are, so one issuer's action falls on that basket only. Its address is fixed by the creator and the basket's number |
| ETH | A part may be ETH itself, not a wrapped token; more ETH than a mint needs is sent back |
| Issuer powers | Not recorded: Ethereum cannot read them from a token (as for the vault, section 11.9), so neither can a mint refuse a token whose rules changed. A burn still pays each part on its own and a share of what the basket holds, so an issuer's freeze or seizure still falls only on its part, and equally. Accepted by the owner on 2026-10-02 as a limit of Ethereum, in place of E3's recorded powers there |
| Refused tokens | A token that takes a fee on transfer is refused when minting, by counting what arrived; one with no code is refused when the basket is created. A token that changes balances on its own (rebasing) is not refused: a burn pays a share of what the basket holds, so a rise goes to the holders, not to what is set aside, and a fall hits the holders first and what is set aside after, first come. A Solana token that rescales changes only how its amount is shown, not the amount |
| The fee | Set aside and collected as on Solana, kept per receiver and part by the contract, with no record to open. The receiver can never be address zero |
| Where a payout goes | A burn, a collection of what is owed and of fees pay the address the caller names: a holder whose own address a token blocks collects elsewhere, as a Solana holder names any account it owns |
| Owed amounts | Kept by the contract per basket, holder and part; no account to open or close |

## Decisions

| # | Question | Recommendation |
|---|---|---|
| E1 | The basket | The first basket: 1 SOL + 1 vETH, fixed when it is created. **Agreed by the owner on 2026-10-01**: up to 8 parts, one per network. Any basket is created in the same program (E2) |
| E2 | A fee for mint or burn? | **Agreed by the owner on 2026-10-01**: anyone may create a basket, with no approval, and set a creator fee on mint and on burn, each at most 1%, fixed at creation. It is paid as a share of each part, so no price is needed. The creator may hand the fee to another address, but never change the basket or the fee. Moving assets between networks pays the operators, not the creator |
| E3 | May a part be a token whose issuer can freeze accounts, such as USDC? | **Decided by the owner on 2026-10-01**: first no; then, the same day, yes, with the rules of "Parts whose issuer controls them": a burn pays each part on its own and a share of what the basket holds, and each part's issuer powers are recorded. Amended by the owner on 2026-10-02: on Ethereum, which cannot read them, no powers are recorded; on both networks the creator's fee is set aside and collected by its receiver |
