# iPoW node

One program that runs the roles of the iPoW protocol on any number of
networks. Spec: [`docs/design/ipow-protocol.md`](../../docs/design/ipow-protocol.md).

**Status:** the three roles, the vault's roles, and the adapters for
Ethereum and Solana are built and tested (`cargo test`): on a local Hardhat network and on an
in-process Solana, with the contracts and programs of
`programmable-network`, against a Bitcoin held in memory. The explorer
client has read Bitcoin mainnet. Nothing has run against a live network
yet. The service for the old contracts is [`core/operator`](../operator),
kept as reference.

## Roles

| Role | What it does each round | Needs |
|---|---|---|
| operator | Waits for a bond, then registers its chain head once. Bids on jobs from its free bond, up to `max_bid`. For a job it won: anchors one block below Bitcoin's best block, sends the tagged Bitcoin transaction (one transaction for all the networks where it holds a job with the same tag, waiting at most one Bitcoin block for the others to be ready), replaces it with one that pays 1.5 times more when it waits 2 blocks, and proves it once confirmed. Sends nothing after the deadline or past the proof range. Moves its chain head past a transaction no job can use (N25). Answers every question for a parent, and adds real blocks to its side of every competing-branch challenge until the lock ends. Settles, withdraws. With `conversion` set for a network, also takes Conversion's swaps at its own prices (below) | A bond on each network (locked by you; the node never locks more), a Bitcoin wallet with coins, coin for gas |
| guardian | Reports a missed deadline. Checks every proof against real Bitcoin; for one on made-up blocks, asks for the parent while the operator's oldest block is made up, and shows a branch of real blocks where the operator's branch leaves the real chain. Keeps its branch growing, resolves, withdraws | Coin for deposits and gas |
| attester | Attests a proven job no one challenges, up to `max_lock`, when its anchor and tip are in Bitcoin's best chain at their stated heights and epoch times. Defends it like the operator. Settles, withdraws | A bond, coin for gas |

With a `vault` section, the operator and the guardian also work for the
protocol's vault (below).

Choose any mix in the settings: `roles: [operator, guardian]`. Each role
on each network runs as its own task; one that fails, or takes more than
15 minutes, is restarted after a pause and the others go on.

The node keeps what it knows in memory. After a restart it reads every
job and challenge again. The operator finds the Bitcoin transactions it
already sent by following its chain head, so nothing is sent twice; the
guardian finds the challenges it opened and keeps growing its branches.

## Running

```sh
cp node.example.yml node.yml     # then fill in the addresses
cargo run -p ipow-node -- --settings node.yml --check   # settings only
cargo run -p ipow-node -- --settings node.yml           # run; Ctrl-C stops
```

Keys and endpoints go in `.env` (read at start), never in `node.yml`. The
settings refuse a field that is not known, so a key pasted into the file
is rejected. Every endpoint and key read from the environment is replaced
by `<hidden>` in the logs. The Bitcoin key is a mainnet WIF key; the light
client only accepts real Bitcoin, so there is no testnet mode.
`bitcoin.max_fee_rate` caps what the wallet pays per virtual byte.

## Conversion swaps

With a `conversion` section on a network, the operator takes part in the
Conversion application (`docs/drafts/ipow-conversion-app.md`), as the
other side of each swap it wins:

| Swap | What the operator does |
|---|---|
| A user sells the coin for BTC | Bids when `sats` per whole coin is at most its `pay_sats`, and its wallet can pay. Its tagged transaction pays the user. It collects the coin once the proof's lock has ended |
| A user buys the coin with BTC | Bids when `sats` per whole coin is at least its `ask_sats`, and it holds the coin. After anchoring it locks the coin and names a new address, derived from its Bitcoin key by BIP32 for this network and swap. Its tagged transaction spends the user's payment (a receipt), and it gives the user the coin; with no payment, it sends a close after the 12 payment blocks and takes the coin back once the lock has ended |

Only the coins listed are ever taken: a token's own rules are the
operator's to judge before listing it.

## The vault

With a `vault` section naming an EVM network and a Solana network, the
node works for the protocol's vault between them (spec section 11): ETH
locked on Ethereum for vETH on Solana, and back.

| Role | What it does each round |
|---|---|
| operator | Registers its pair chain once, by one Bitcoin transaction that names it on both networks. Keeps `bond` ETH bonded on Ethereum (and `veth_bond` vETH on Solana, to carry burns and give-ups) and `deposits` claim deposits in each vault. Gathers what is worth carrying: locks on Ethereum once their block is final, burn requests and give-ups on Solana, each with a fee of at least `min_fee_gwei`, within 80% of its bond counted on the acting network and the size limits (D119). Writes them as one batch on Bitcoin, spending its pair chain's coin, and submits the message to both vaults once a real block is above it (D108), storing any missing block below that real block. With `checkpoint_paid_*` set, opens a checkpoint job when no real block is above a message and none is under way. Answers every objection to its claims, decides and collects. With `fast` set (section 11.7): issues the receipt of a final lock at once, locking 1.25 times its amount of its own vETH, and pays a final burn's ETH at once from its own, for locks and burns whose fast fee is at least `min_fee_gwei` and amount at most `max_gwei`, the best-paid first. It earns the part of the fast fee for the time left to 8 days after the lock or burn (D124), so it never attests one whose claim was already accepted. It carries each lock it attested in a claim of its own whatever its fee, links the attest to that claim (only its own, D125), and settles it once the claim is accepted; it takes the vault's repayment of each burn it paid, also after a restart |
| guardian | Reads every claim on each network and checks each of its records against the other network: a lock or a burn request exists with exactly that record, a give-up happened, a stated bond is the bond there. Objects to a false claim, decides claims whose 7 days are over, and collects its winnings. Brings to each network the messages of an operator's chain that only the other network has processed, with the batch the other vault published, checked against its hash on Bitcoin: a lie shown to one network only is proven and slashed on the other, 20% to the guardian. It brings them only when one of the next 20 lagging messages is false there; honest lagging messages are left to their operator. With `checkpoint_paid_*` set, it opens a checkpoint job when no real block is above the lie. Reads every attest of a lock on Solana: burns one with no claim linked within 7 days, or whose linked claim was refused, and settles one whose lock an accepted claim states otherwise, taking the quarter above the amount. Several attests of one lock may be made for 7 days from its first (D126); they are settled oldest first, so the earliest true one counts, and with none true the recipient's receipt is issued once no attest can come |

Only a batch's hash is on Bitcoin. The operator writes each batch to its
`journal` file, keyed by that hash, before the message exists, so that
after a crash it can still submit any message a vault has not processed; a
message replaced to pay more carries the same hash. Keep the file, and
give it an absolute path: lose it and a message not yet submitted can
never be, and the pair chain stops.

How the operator keeps going:

| Case | What it does |
|---|---|
| An objection to one of its claims | Answered in every round right after the two chains are read, each claim on its own: one that fails does not keep the others from being answered |
| A top-up that fails (no vETH yet, too little ETH or SOL) | Logged; the round goes on |
| No real block within 100 blocks above a message | Records real blocks down from the nearest one above, in steps of 100 |
| A message unmined for 3 blocks | Replaced, paying half as much again |
| A message dropped by Bitcoin's nodes | The next batch is written instead; the dropped one's records are gathered again |
| A lock or burn larger than 80% of its bond, or one nobody ever finishes | Left open and looked at again each round; newer ones are still read and carried |
| A lock or burn whose fee another operator earned, but that no claim is handling | Carried again, for no fee, so its user is not stranded. A duplicate true record is never slashed |
| A record about Solana's own facts (a burn, a give-up) | Carried only once its block is finalized: a record of a block Solana could still roll back would be false |
| A message replaced to pay more | Pays for the waiting transactions it evicts too, as the protocol operator does |
| A claim whose records Ethereum's event logs cannot give yet | Tried again each round, holding nothing up |
| Its chain cannot open a claim on a network (refused, slashed, no deposit money) | Carries nothing toward that network |

Not built yet: pruning old batches from the journal; the fast paths
(D115).

## What the explorer is trusted with

Bitcoin is read from an explorer that serves the Esplora API
(`bitcoin.explorer`).

| Who | Trusts the explorer for | A lying explorer costs |
|---|---|---|
| operator | Its own coins, fee rates, and the headers and transactions it puts in its proofs | Failed transactions and fees. The light client checks every header's work and every Merkle proof, so a lie never makes a false proof |
| guardian | Whether a proof's blocks are in Bitcoin's best chain. It challenges a block only when the explorer has another block at that height, and only after 20 minutes | Deposits lost to an honest operator, up to `max_deposit` per question |
| attester | Whether a proof's blocks are in the best chain at their heights | Its locked x, if it attests a false proof |

Use an explorer you trust, or your own.

## Layout

| Crate | Holds |
|---|---|
| `crates/protocol` | The interface every network implements (`ProtocolNetwork`, and `VaultApp` for the vault with its records), the shared types, the settings |
| `crates/bitcoin` | Hashes, Merkle proofs, transactions, the wallet (signing with rust-bitcoin), the explorer client, and a Bitcoin in memory for tests |
| `crates/networks/evm` | The adapter for `iPoWLightClient.sol` and `iPoWProtocol.sol`; with the `testing` feature, a local Hardhat network with the test contracts |
| `crates/networks/svm` | The adapter for the Solana programs `ipow-light-client` and `ipow-protocol`. It builds the walks a proof or a challenge needs and closes them after, and computes a job's status the way the program does. With `testing`, an in-process Solana (LiteSVM) with the programs |
| `crates/testing` | Tests written once for every network |
| `crates/node` | The program: the roles, the light-client feed, the shared wallet, and the supervisor |

## Tests

```sh
cargo test                                        # every test, on both networks
cargo test -p ipow-bitcoin -- --ignored           # reads Bitcoin mainnet from mempool.space
```

Needs `programmable-network/ethereum` installed (`pnpm install`) and the
Solana programs built (`programmable-network/solana/target/deploy/ipow_protocol.so`
and the test builds `target/deploy-test/ipow_light_client.so` and
`target/deploy-test/ipow_vault.so`, built with the `test-limits` feature). The tests
start Hardhat themselves and stop it when done. The test contracts and the
test light client keep every rule but accept blocks mined at a low
difficulty.
