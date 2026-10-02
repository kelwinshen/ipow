# The iPoW SDK: a proposal

A TypeScript library for building on the new protocol
([`../design/ipow-protocol.md`](../design/ipow-protocol.md)), first for
Greatwall.finance, the BETA app, which lives in a repository of its own and
depends on this library. Not built yet; this proposes its shape. What it
must make easy comes from the first live run,
[`ipow-testnet-run-2026-10-02.md`](ipow-testnet-run-2026-10-02.md).

## Where and what

- **Location:** `packages/sdk` in this repository, published as `@ipow/sdk`.
  The SDK depends on the contracts' ABIs, the Solana programs' IDLs and the
  deployed addresses, which all live here, so it changes in the same commit
  as they do.
- **What it ships:** each network's deployment as data, generated from
  `programmable-network/ethereum/deployments` and the Solana README. Apps
  do not hard-code addresses.
- **Its dependencies:** ethers v6 for EVM networks, viem for Tempo's own
  transactions, and @solana/web3.js with Anchor for Solana. The repository
  already uses all of them.
- **Keys:** the SDK never holds keys. It takes the app's signer (a browser
  wallet, or a key in a script) and only builds, sends and reads.

## Principles, from the run

| Principle | Why |
|---|---|
| A quote states its margin and what the margin costs | The escrow rose 4.6% between quote and send; what is paid above the fees goes to the operator (D79) |
| Stages have names and timestamps, and say what they wait for | The raw status is a number in a non-obvious order |
| The Bitcoin side is first-class: txids, confirmations of the 6 needed, explorer links | Most of a job's time is Bitcoin's |
| Time estimates come from real block times | Blocks came 8 seconds and 58 minutes apart in one run |
| Reads retry, and Bitcoin explorers fall back on each other | Endpoints lag and explorers time out |
| One API hides each network's units and sending rules | Hedera: 8 decimals in contracts, 18 over its RPC. Tempo: PathUSD fees and its own transaction type. Polkadot: gas scaled by 8 |

## The API, by layer

- **`networks`:** every deployed network by name, with its number (D133),
  chain id, coin (native or token, its decimals as a contract sees them and
  as a wallet sends them), and its addresses.
- **`quote(network, kind)`:** the commitment fee at the network's price
  now, the escrow, the escrow fee, a margin, and the total to pay. For a
  checkpoint, a Conversion swap, or a vault lock's fees.
- **`jobs`:** a job by id. Its stage (`Auction`, `Assigned`, `Proven`,
  `Settled`, `Expired`, `Slashed`); when each stage began, read from
  events; what the stage waits for; the auction's end, the deadline and the
  lock's end; and, once tagged, its Bitcoin transaction with confirmations.
  `watch(job)` yields each change.
- **`bitcoin`:** a transaction's confirmations and block, the tip, and
  recent block times, from more than one explorer.
- **`vault`:** lock an asset for its receipt on the peer network, and burn
  a receipt for the asset back. Each lock is tracked through its stages:
  locked, carried in a claim, receipt issued (or attested fast), or
  returned. Receipt balances on each network.
- **`beta`:** create a basket, mint and burn BETA, see what a mint costs in
  each part, and collect fees and what a basket owes.
- **`conversion`:** sell and buy a coin for BTC, and follow a swap to its
  end.

## Built in order

| Step | What | Done when |
|---|---|---|
| 1 | `networks` (EVM), `quote`, `jobs` with stages and times, `bitcoin`, and opening a checkpoint | The run's job can be followed from the SDK alone, read-only, on Sepolia; tested on a local chain |
| 2 | `vault` on EVM: lock, burn, follow a lock, receipt balances | Tested on a local chain |
| 3 | Solana: the same layers | Tested on an in-process Solana |
| 4 | `beta` on both | Tested locally |
| 5 | `conversion` | Tested locally |
| 6 | Tempo and Hedera's sending rules; published to npm | Read back on their testnets |

Greatwall.finance can start once steps 1 to 4 are in; its main flow is to
lock coins on several networks and mint BETA from the receipts.
