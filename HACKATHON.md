# iPoW at the Crypto World's Fair

Our submission to Colosseum's Crypto World's Fair (September 14 – October 12,
2026) has two parts, built in this order:

1. **iPoW**, this repository: a protocol that lets programmable networks talk
   to each other with Bitcoin as the shared source of truth.
2. **[Greatwall](https://github.com/Renrensan/greatwall)**: the app on top of
   it, for locking, converting and combining assets across networks.

## Where we started

iPoW isn't new as an idea. Before the hackathon we had a first version of
it, built for Paradapp, an earlier project of ours: Bitcoin headers relayed
to each network by a single operator, and a contract that converted Bitcoin
into a network's coin. This repository's first commits
([`8430abd`](https://github.com/kelwinshen/ipow/commit/8430abd),
[`6e1aeab`](https://github.com/kelwinshen/ipow/commit/6e1aeab),
[`d3bd1e8`](https://github.com/kelwinshen/ipow/commit/d3bd1e8),
July 25 – August 1) are that version, and it's kept as it was on the
[`legacy`](https://github.com/kelwinshen/ipow/tree/legacy) branch.

## What we built during the hackathon

### September 22–24: pushing the first version to its limits

We started by extending the first version: a token backed by assets on
several networks (BETA), settled with real Bitcoin blocks, and a conversion
anyone could take, deployed on every network
([`7e52ff3`](https://github.com/kelwinshen/ipow/commit/7e52ff3) …
[`8e12442`](https://github.com/kelwinshen/ipow/commit/8e12442)).
It worked live, and it showed us where a single trusted operator and a
contract per idea would stop scaling. So we redesigned.

### September 28 – October 6: the new protocol

On September 28 and 29 we wrote down the new design
([`docs/design/ipow-protocol.md`](docs/design/ipow-protocol.md), decisions
D1–D101): any network keeps its own Bitcoin light client, operators put up
stake to carry messages and lose it if they lie, and applications sit on top.
Then we built it, all of it from scratch:

| Date | Commit | What we built |
|---|---|---|
| Oct 1 | [`3249607`](https://github.com/kelwinshen/ipow/commit/3249607) | The core: the Bitcoin light client, operators and jobs (stake, auction, proof on Bitcoin, slashing, challenges), the node that runs operators, the vault, and the first two applications, Conversion and BETA |
| Oct 1 | [`7d308c7`](https://github.com/kelwinshen/ipow/commit/7d308c7) · [`77c8bbf`](https://github.com/kelwinshen/ipow/commit/77c8bbf) · [`9733065`](https://github.com/kelwinshen/ipow/commit/9733065) | The vault in full: guardians, fast paths, any token in both directions |
| Oct 2 | [`4934c8c`](https://github.com/kelwinshen/ipow/commit/4934c8c) · [`dd17d00`](https://github.com/kelwinshen/ipow/commit/dd17d00) · [`e6ff376`](https://github.com/kelwinshen/ipow/commit/e6ff376) · [`2aa97ca`](https://github.com/kelwinshen/ipow/commit/2aa97ca) | Any two networks paired, not just each with Solana; networks whose coin is a token; rollup fees |
| Oct 3 | [`bc09e28`](https://github.com/kelwinshen/ipow/commit/bc09e28) · [`b721e97`](https://github.com/kelwinshen/ipow/commit/b721e97) | Live on the test networks and Solana devnet |
| Oct 3 | [`a56496c`](https://github.com/kelwinshen/ipow/commit/a56496c) … [`cca9531`](https://github.com/kelwinshen/ipow/commit/cca9531) | The TypeScript SDK, so an app can use all of it, and the first live job |
| Oct 4 | [`279e20b`](https://github.com/kelwinshen/ipow/commit/279e20b) | Conversion between two networks in one step |
| Oct 5–6 | [`eafa7ac`](https://github.com/kelwinshen/ipow/commit/eafa7ac) · [`71f80a0`](https://github.com/kelwinshen/ipow/commit/71f80a0) | Index tokens across networks, test real-world-asset tokens, and Arbitrum as the ninth network |

With the protocol and its SDK in place, we turned to the app:
**Greatwall** was built on top of it from October 3 to 10 (its own
[HACKATHON.md](https://github.com/Renrensan/greatwall/blob/main/HACKATHON.md)).

### October 8–10: ready for real use

| Date | Commit | What we did |
|---|---|---|
| Oct 8–9 | [`6b8617d`](https://github.com/kelwinshen/ipow/commit/6b8617d) · [`479d93e`](https://github.com/kelwinshen/ipow/commit/479d93e) | A one-time genesis that backs the fast path on every pair: 409 receipts and 470 issues, each matched to its lock |
| Oct 9 | [`b279a06`](https://github.com/kelwinshen/ipow/commit/b279a06) | Retired the first version from the code (its last state is the tag [`legacy-v1`](https://github.com/kelwinshen/ipow/tree/legacy-v1)) |
| Oct 9 | [`642891c`](https://github.com/kelwinshen/ipow/commit/642891c) · [`ac8d94e`](https://github.com/kelwinshen/ipow/commit/ac8d94e) | Laid the repository out as protocol and applications, pinned every deployed build, rewrote the docs, and set up CI |
| Oct 9 | [`c39970d`](https://github.com/kelwinshen/ipow/commit/c39970d) · [`8d560eb`](https://github.com/kelwinshen/ipow/commit/8d560eb) | BETA at a fresh program address on Solana devnet |
| Oct 9–10 | [`dfcfb1e`](https://github.com/kelwinshen/ipow/commit/dfcfb1e) | The operator live on all 15 pairs it can serve: registered on Bitcoin, its checkpoints proven on every network. The node fast-paths every listed token, not only the 8 a chain can bond, from its genesis stock (a test-network choice, recorded in the design); opens a checkpoint when a waiting job can't help; and sends HyperEVM through a small router. Tempo's BETA redeployed to the current build, with a script of its own. The SDK collects every fee in one approval, and Solana sends confirm by polling |

The whole hackathon's work is `git log 7e52ff3^..master`.

## How to check it

- **Tests** run in CI on every push: contracts (358), Solana programs (157),
  the node (111), the SDK (13).
- **Deployments** match their code: `evm/scripts/verify-deployments.ts` reads
  every contract back from the eight EVM test networks, and
  `solana/scripts/verify-devnet.sh` checks every devnet program.
- **Addresses** are in each network's README under [`networks/`](networks)
  and in [`solana/README.md`](solana/README.md).
- **Live activity**: Greatwall's Explorer page lists every protocol
  transaction, read from the chains.

## Tools

Written with the help of AI coding assistants, under the rules in
[`AGENTS.md`](AGENTS.md): the design doc is the source of truth, every change
is reviewed against it, and nothing counts as deployed until it's read back
from the chain.
