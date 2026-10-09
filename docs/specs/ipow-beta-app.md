# BETA on the new protocol (stage 6)

**Status: approved by the owner on 2026-10-01; built on Solana with the RWA rules, and on Ethereum (see "Built"). Deployed on the test networks on 2026-10-02, redeployed for E4 and E5 on 2026-10-05 (spec, Build status rows 6 and 8; [`live-run-log.md`](../drafts/live-run-log.md)).** The second application of
stage 6 in [`ipow-build-plan.md`](../drafts/ipow-build-plan.md). It is built on the
protocol's vault and its receipt tokens (spec section 11)
([`ipow-vault-claims.md`](ipow-vault-claims.md)), which the owner chose
on 2026-09-30 over carrying the old BETA's messages
([`../archive/ipow-implementation.md`](../archive/ipow-implementation.md),
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

On Solana on 2026-10-01: `solana/programs/applications/beta-basket`,
tests in `programs/applications/beta-basket/tests/test_beta_basket.rs`. Not deployed.
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
| A part not made yet (E5, 2026-10-05) | A part whose mint account does not exist is recorded pending (its token program the default); the first mint reads the mint, records its program, decimals and powers, or fails with `PartNotMadeYet` while the account does not exist |
| Its name (E4, 2026-10-05) | `create_basket` takes the token's name, symbol and URI and makes its Token Metadata account by a `CreateMetadataAccountV3` sent as bytes (no crate of that program), the basket its update authority and the account immutable; the limits that program's (32, 10, 200 bytes). The tests run the program as deployed, dumped into `tests/fixtures/mpl_token_metadata.so` |

On Ethereum on 2026-10-01: `evm/contracts/applications/beta/BetaBaskets.sol`,
tests in `test/applications/BetaBaskets.test.ts`. Deployed on the test networks on 2026-10-02. The same rules (up to 8
parts, fees of at most 1% fixed at creation and rounded up, a whole first
mint, mints by the basket's ratio rounded up, burns of a share rounded
down, deferring by choice, no mint while a part holds nothing, the fee
handed to the basket itself backing it for good), with these differences
made by Ethereum:

| Point | On Ethereum |
|---|---|
| A basket | `BetaBaskets` holds the rules; each basket is its own `BetaBasket` contract, its BETA token (9 decimals, as on Solana), which also holds its parts. A basket's backing is its own balance, as a Solana basket's token accounts are, so one issuer's action falls on that basket only. Its address is fixed by the creator and the basket's number |
| A part not made yet (E5, 2026-10-05) | `createBasket` takes parts as `PartSpec` (a token, or a `receipts` contract and `asset` number); a receipt part resolves its token from `IVaultReceipts.receiptOf(asset)` at creation when made, else when first used, and `held`, `mintCost`, `mint`, `burn` and the collections refuse it with `PartNotMadeYet` until then; `partToken(key, i)` reads it. The receipts contract is the creator's choice, as a token is: an app shows it beside the vault's |
| Its name (E4, 2026-10-05) | `createBasket` takes the token's name, symbol and URI; the name and symbol are the token's own, read by its constructor from `BetaBaskets` while it is made, so that every token's init code is the same and `CREATE2` by the key keeps the address fixed by creator and number and a second basket with the same id refused; the URI is kept in the basket and in the `BasketCreated` event; limits 64, 16 and 2,048 bytes (`BadMetadata`) |
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
| E4 | May the creator name a basket's token, and describe it? | **Decided by the owner on 2026-10-05**: yes. At creation the creator gives the token's name and symbol and a metadata URI, all fixed for good like the parts. On Ethereum the name and symbol are the token's own (`name()`, `symbol()`) and the URI is kept in the basket, the limits 64, 16 and 2,048 bytes, a name and a symbol of at least one; the token's init code stays the same for every basket, the name read from `BetaBaskets` while it is created, so `CREATE2` by the basket's key still fixes its address by creator and number and refuses a second basket with the same id. On Solana they go in the token's Token Metadata account, made by the program with the basket as its authority and immutable, the limits that program's: 32, 10 and 200 bytes, a name and a symbol of at least one. The URI is JSON with a description and an image, as token metadata is read; Greatwall writes it as a data URI, so nothing is hosted. A basket of the first build has no name on Solana and reads as "BETA" |
| E5 | May a basket name another network's asset before its receipt exists here? | **Decided by the owner on 2026-10-05**: yes. A part may be the vault's receipt of the peer's asset by number, named at creation and resolved when first used: on Ethereum the part keeps the vault's receipts contract and the asset number and reads `receiptOf` on use; on Solana the client gives the receipt mint's derived address (a PDA), and the program records any part whose mint account does not exist yet as pending, with no token program, and reads it, its issuer powers with it, at the first mint (the SDK refuses a mint that does not exist unless the caller names the part pending, so a typed address does not quietly make a basket that can never mint). Nothing can be priced or minted while a part's receipt is not made (`PartNotMadeYet`): the asset must be bridged first, by the vault, as before. Creating is one transaction and bridging stays the vault's; BETA still never moves anything across networks |
