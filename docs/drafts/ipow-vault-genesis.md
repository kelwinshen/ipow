# Vault genesis: a one-time foundational step (proposal)

Status: **approved by the owner on 2026-10-08, being built.** Nothing
here is deployed yet. It would amend
[`../design/ipow-protocol.md`](../design/ipow-protocol.md) sections 11.7,
11.9 and 11.10, and add D142 to D147. Nothing here is true of the deployed
vaults.

## Why

Two rules of the vault make the first use of every asset slow:

- **A receipt is made by an accepted claim.** A receipt exists on the
  other network only once a claim carrying the asset's ASSET record is
  accepted: 7 days of objections, once per asset (section 11.9, D111),
  after a certified Bitcoin checkpoint (at least 36 hours, D28).
- **The fast path needs receipts as collateral.** An attester locks 1.25
  times the amount in receipts to issue a lock at once (section 11.7). With
  no receipt made, nobody holds any, so the first lock of an asset always
  takes the slow path, about 9 days.

On testnet, with 9 networks, 18 pairs and every pair's coin and RWA
tokens, that is weeks before any fast path can be shown, and each first
receipt needs its own Bitcoin-backed claim. The owner wants the vaults to
start with every receipt made and the operator holding enough receipts to
attest, through one foundational step that can never be used again.

## The proposal in one paragraph

Each vault starts in **genesis**. During genesis one key, named at
deployment, may do two things on the receipt side of a pair, and nothing
else: make an asset's receipt without a claim, and issue the receipts of a
real lock on the home vault without a claim. Genesis ends when that key
finalizes it, or at a deadline fixed at deployment, whichever comes first.
After that the two actions are gone for good and the vault is the vault of
sections 11.1 to 11.10, with no key (D59). Every genesis action names what
backs it, so anyone can check, after genesis, that each receipt made then
is backed by a real lock.

## What genesis allows

Only the receipt side changes. The home side (locks, burns paid, bonds,
claims) is untouched: genesis locks are ordinary locks.

| Action | What it does | Guard |
|---|---|---|
| G1. Make a receipt | Makes the receipt of peer asset `n`, with the name, symbol and decimals the claim path would give it. On an EVM network this is the `VaultReceipt` contract `makeReceipt` deploys; on Solana the `["receipt", config, n]` mint and its `["holding", config, n]` account that `make_receipt` makes | Genesis open, the genesis key, the receipt not made yet. Decimals at most 9 |
| G2. Issue a lock | Issues peer lock `id` of asset `n`: marks it issued and mints its value (amount plus fast fee) to its recipient, exactly as `issue` does after an accepted claim | Genesis open, the genesis key, the receipt made, the lock not issued, not given up and not attested |

G2 sets the same mark the slow and fast paths set: `_marks[id].issued` on
an EVM network (`VaultReceipts`), the `LockMark` at `["lock", config, id]`
on Solana. So:

- **No double mint.** If a LOCK record of that lock is later carried in a
  claim, `issue` refuses it with `AlreadyDone`, as it refuses any lock
  already issued. A fast attest of it is refused too: `attestLock` /
  `attest_lock` refuse an issued lock. (Today's node does not carry a lock
  already issued at all, `core/node/crates/node/src/vault.rs`, so a genesis
  lock's fee, zero here, is never earned.)
- **No give-up.** A give-up needs a lock not issued, so a genesis lock is
  never returned on its home.
- **Its value is backed.** The lock's amount and fast fee sit in the home
  vault's reserve from the moment it was locked (`VaultHome.lock`, on
  Solana the pair's holding). A burn of a genesis receipt is paid from it
  like any other.

The genesis key cannot mint without naming a lock, burn, pay, slash,
change a timer, a bond, a deposit or a peer, or act on the home side.

## How genesis ends

| Rule | |
|---|---|
| Finalize | The genesis key calls `finalizeGenesis` (Solana: `finalize_genesis`). One way: nothing reopens it |
| Deadline | Genesis is also over at `genesisEnd`, a time fixed at deployment (proposed: 14 days after it), with or without a finalize. A forgotten finalize cannot leave a key in the vault |
| After | G1 and G2 refuse for good. The genesis key has no other power, so it is then nothing. `genesisOpen()` reads false, and the vault behaves exactly as sections 11.1 to 11.10 |

Both sides of a pair finalize on their own: each vault has its own
genesis, and only ever acts on receipts made on its own network.

## The assets prepared at genesis

Every pair, both directions, and on each side every asset Greatwall uses:

- **The coin**, asset 0 of every home vault: ETH on Ethereum, Base,
  Robinhood and Arbitrum; SOL; HYPE; DOT; HBAR; and on Tempo PathUSD, the
  token build's coin.
- **Every RWA test token** registered in the home vaults (the tokens of
  `deployments/<network>-testnet-rwa.json`).

For each one, G1 makes its receipt on the other side, then the owner locks
an amount on the home vault to the operator's address there, and G2
issues it. The operator then holds receipts of every asset on every pair,
to put up as fast-path collateral (1.25 times each lock it attests) and
for its `bond_receipt` bonds. How much of each is the owner's choice; the
locked amounts come from the owner's own testnet funds and stay backed.

## What anyone can check after genesis

Each genesis action emits an event naming what backs it:

- `GenesisReceipt(asset, decimals)`, and on Solana a log of the same.
- `GenesisIssued(lockId, asset, value, recipient)`.

A script in `programmable-network/ethereum/scripts/` reads every
`GenesisIssued` of every pair and checks, on the home vault, that lock
`lockId` exists, is of `asset`, has `amount + fastFee == value` and that
recipient, and that no other `GenesisIssued` names the same lock. It also
checks that each `GenesisReceipt`'s decimals equal the home asset's record
decimals. Its result is published with the deployment (each network's
README) and in [`../../SECURITY.md`](../../SECURITY.md).

## What it costs in trust

- **During genesis** the receipts it makes are only as good as the genesis
  key: it could issue a lock that does not exist, or one twice on two
  vaults of the same pair, if it lied. The check above finds that after the
  fact; nothing on chain prevents it during genesis. This is a deliberate,
  one-time trust in the deployer, to be stated plainly in `SECURITY.md`.
- **After genesis** nothing changes from the approved design: no key, every
  receipt from an accepted claim or a fast attest that a claim settles.
- **The fast path still needs Bitcoin.** An attester must have a registered
  pair chain (one Bitcoin transaction per pair), and its collateral comes
  back only when the claim carrying that lock is accepted, about 9 days
  later. Genesis removes the wait for the first receipt, not the slow path
  behind every attest: the operator needs enough receipts for 9 days of
  attests.

## Decisions it would add

| | Decision |
|---|---|
| D142 | A vault starts in genesis: one key named at deployment may make receipts (G1) and issue named home locks (G2) on the receipt side, nothing else |
| D143 | G2 marks the lock issued with the slow path's own mark, so a lock is issued once whichever path comes first |
| D144 | Genesis ends at the key's finalize or at a deadline fixed at deployment, whichever is first, and never reopens |
| D145 | Each genesis action emits what backs it; a published check matches every genesis receipt to a real home lock |
| D146 | Genesis prepares every pair's coin and listed RWA tokens, both directions |
| D147 | Amends D59: the genesis key exists until genesis ends; no key exists after it |

It amends D59 for the genesis period only, and section 11.9's "Its
receipt" row (a receipt may also be made at genesis). D111, D28 and the
timers of section 11.7 are unchanged.

## What it would change

| Part | Change | Who runs it |
|---|---|---|
| `VaultReceipts.sol` (EVM) | `genesisMakeReceipt`, `genesisIssue`, `finalizeGenesis`, `genesisOpen()`; the genesis key and `genesisEnd` immutable, set by its constructor, which refuses an end not in the future or more than 30 days ahead | code: Claude |
| `VaultReceiptsFactory` (EVM) | `makeGenesis` makes a receipts part in genesis; `make` one with none. The core is unchanged: it accepts either part from its factory, so whoever checks a vault also reads its receipts part's `genesisKey()` and `genesisEnd()` (decided 2026-10-08, in place of the core taking them) | code: Claude |
| `ipow-vault` program (Solana) | `genesis_make_receipt`, `genesis_issue`, `finalize_genesis`; `genesis_key`, `genesis_end`, `genesis_done` in the pair's config, set at `initialize` | code: Claude |
| Tests | Hardhat and Anchor: G1 and G2 under genesis; refused after finalize and after the deadline; a later claim of a genesis lock refused with `AlreadyDone`; an attest of one refused; the check script against a local run | Claude |
| Deployments | New vault pairs on every EVM network (the core is new, so its receipts and home parts are new); a new Solana config layout, so either a new program or new configs (open question 3) | the owner |
| Genesis itself | The locks on each home vault, then G1 and G2 on each receipt side, then the check, then finalize | the owner, with a script |
| SDK, Greatwall, node | New addresses (generated deployments), the node's `vault` section switched on with the new pairs and the assets it carries | Claude, the node's run by the owner |

The locks already made on today's vaults (lock 1 of Ethereum–Solana, 2
VTIon) stay on today's vaults; they are not carried over.

## Built (2026-10-08), not deployed

| Part | Where | Tested |
|---|---|---|
| EVM receipts part | `VaultReceipts.sol`: `genesisKey`, `genesisEnd`, `genesisOpen()`, `genesisMakeReceipt`, `genesisIssue`, `finalizeGenesis`; `VaultReceiptsFactory.makeGenesis` (a part made by `make` has no genesis) | Hardhat: 3 genesis tests, 489 passing in all |
| Solana program | A new program, `3TZ1LJ4fVZVKbBUyzVMRjJ9FNPGaqVmGX56JuT5QdXEy` (its keypair in the git-ignored `solana/.keys/ipow_vault_genesis-keypair.json`): `genesis_make_receipt`, `genesis_issue`, `finalize_genesis`; the pair's config gains `genesis_key`, `genesis_end`, `genesis_done`, set by `initialize` | `cargo test -p ipow-vault`: 4 genesis tests, 36 passing in all |
| Node | The new program's IDL; its tests set up pairs with no genesis | `cargo test` in `core/node`: passing |
| Deploy tooling | `deploy/deploy.ts` (`existing`, `genesis`); `scripts/redeploy-solana-vault.ts`; `scripts/deploy-pair.ts --genesis-days`; Tempo's `deploy-new-protocol.ts --vaults-only`; Solana's `init-testnet.ts --genesis-days`, `--print-pairs` | Typechecked, not run |
| Genesis run and check | `packages/sdk/scripts/genesis.ts` (resumable: it reads the chain before locking or issuing again; `--dry` sends nothing), `genesis-check.ts` (reads every `GenesisReceipt` and `GenesisIssued` from the chain, EVM logs and Solana's Anchor events, and checks each against its home asset or lock, each genesis receipt's supply, and the run's ledger both ways); the SDK's `genesisOf`, `genesisMakeReceipt`, `genesisIssue`, `finalizeGenesis`, and `SolanaVault.genesis*` | Typechecked, not run |

Known gaps: the genesis run sends ethers transactions, so Tempo's pair
(its own transaction type) is skipped and reported; its genesis needs
Tempo's sender. Hedera and Polkadot are run as ordinary EVM networks.

## Running it (the owner runs every step that sends a transaction)

1. Commit the contracts and the program: deploys refuse uncommitted
   contracts (`contractsSource`).
2. Solana: deploy the new program with its keypair,
   `solana program deploy target/deploy/ipow_vault.so --program-id .keys/ipow_vault_genesis-keypair.json`
   (from `programmable-network/solana`, after `anchor build -p ipow_vault --ignore-keys`),
   then print the pair accounts the EVM vaults will name:
   `node scripts/init-testnet.ts --print-pairs`.
3. Each EVM network's vault with Solana, in genesis for 14 days, from
   `programmable-network/ethereum`:
   `node scripts/redeploy-solana-vault.ts <network> <pair account> --genesis-days 14`
   (Hyperliquid with `--big-blocks`); Tempo from `programmable-network/tempo`:
   `node scripts/deploy-new-protocol.ts --vaults-only --solana-pair <pair account> --genesis-days 14`.
4. Solana's pairs, naming those vaults, in genesis:
   `node scripts/init-testnet.ts --genesis-days 14`.
5. The 10 EVM pairs, each in genesis:
   `node scripts/deploy-pair.ts <a> <b> --genesis-days 14`.
6. `pnpm sync` in `packages/sdk`, then the SDK built and Greatwall
   reinstalled, so both read the new addresses.
7. The genesis itself, from `packages/sdk`:
   `node scripts/genesis.ts --operator-evm <0x…> --operator-solana <…> --coin ethereum=0.2 --coin solana=20 …`
   (`--dry` first).
8. The check: `node scripts/genesis-check.ts`, its result published in each
   network's README and in `SECURITY.md`.
9. Finalize every pair on both sides (the SDK's `finalizeGenesis`), or let
   the 14 days run out.
10. The node's `vault` section switched on with the new pairs and the
    assets it carries.

## Decided by the owner (2026-10-07 and 2026-10-08)

- **The deadline:** 14 days after deployment; a vault refuses at
  deployment a `genesisEnd` more than 30 days ahead.
- **The pairs:** all 18 vault pairs as deployed today, both directions.

- **Solana:** the vault with genesis is a new program id. Every peer's
  config is new; today's program and its locks stay as they are, readable
  by the SDK's old entries until they are dropped.
- **The genesis key:** the deployer, `0x9784…2BF1` on the EVM networks and
  the deployer's Solana key, for every vault of every network. It is not
  the operator's key.
- **How much at genesis:** enough of every asset for the operator to
  attest with, about 10,000 of each RWA token per pair and direction. The
  RWA test tokens can be minted for it: on EVM networks `MockRWA.mint`,
  which only their deployer may call (`contracts/testnet/MockRWA.sol`); on
  Solana their mint authority, the test key Greatwall's faucet holds. The
  coins cannot: testnet ETH, SOL, HYPE, DOT, HBAR and PathUSD come only
  from faucets, so each coin's amount is what the deployer holds (open
  question 5).

## Open questions for the owner

1. **The deadline.** 14 days after deployment, with a 30-day maximum
   enforced at deployment?
2. **The pairs.** All 18 (the 8 with Solana, and the 10 between
   Ethereum, Base, Robinhood, Hyperliquid and Arbitrum), as the owner
   asked, or the pairs Greatwall shows first?
3. **Solana.** The config gains three fields, so today's configs cannot be
   read by the new program. Deploy it as a new program id (clean, every
   peer's config new), or upgrade the devnet program and initialize new
   configs beside the old ones? A new program id is simpler and keeps the
   old locks readable by the old one.
4. **The genesis key.** One key for every vault of every network (the
   deployer, `0x9784…2BF1`, and its Solana counterpart), or the operator's?
   Keeping it apart from the operator's means the operator never had a
   power the others do not.
5. **The coins' amounts.** 10,000 ETH or SOL cannot be had on a testnet.
   Proposed: per pair and direction, whatever the deployer can spare of
   each coin (for example 0.2 ETH, 20 SOL, 5 HYPE), since an attest locks
   1.25 times its lock and fast locks in a demo are small.
6. **Mainnet.** Genesis is meant for testnet only. There is no build
   without it yet (decided 2026-10-08, for now): a mainnet vault keeps D59
   whole only if it is deployed with `make` (EVM) and the default key
   (Solana). A build flag that leaves genesis out is the safer later step.
