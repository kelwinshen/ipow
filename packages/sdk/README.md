# @ipow/sdk

Build on the iPoW protocol ([`docs/design/ipow-protocol.md`](../../docs/design/ipow-protocol.md)).
Design and plan: [`docs/drafts/ipow-sdk.md`](../../docs/drafts/ipow-sdk.md).

**Status:** steps 1 to 5 of the plan. On EVM networks:
- each deployed network as data;
- quotes with their margin stated;
- jobs by stage, with their times and what they wait for;
- the job's Bitcoin transaction, found before its proof through the
  operator's chain head and the job's tag;
- opening a vault checkpoint;
- the vault: its assets and receipts, a lock's quote in the asset's own
  units converted to record units, locking (a token approved first),
  burning a receipt, and following each on this network.

On Solana (`SolanaVault`): the vault's assets, registering an SPL token
with a pair (`registerToken`), locking SOL or a registered token for its
receipt on the pair's EVM network and following that lock, receipt balances,
burning a receipt, and whether a lock made on the EVM network had its
receipt issued on Solana. `followLock` follows an EVM lock to its receipt
on Solana as one journey: `Locked`, `Carried`, `Issued` (or `GivenUp`,
`Returned`).

BETA on both: create a basket, quote a mint (on Solana priced as the
program does, since it has no view of it), mint, burn with parts deferred,
collect what is owed and the creator's fees.

Conversion on EVM networks: quote a swap's fees, sell the coin or a token
for BTC to a Bitcoin address, buy it with BTC, follow a swap by stage (for a
buy, where and in which Bitcoin blocks to pay), refund a sell and cancel a
buy no operator took. Bitcoin addresses (bc1q, bc1p, 1, 3) are converted to
the scripts the contract holds, and back. Swaps are capped in size for
buys only (the user's own payment proof, Conversion's C4).

Two things an app must know. A call that opens a job (a checkpoint, a sell,
a buy) states its gas: an estimate runs at a price of zero and comes out
too low (spec V14); the SDK does it. And when no operator takes a job, its
fees do not come back with a refund: the job is expired (`expireJob`,
anyone may) and the fees withdrawn (`withdrawCredit`); an app does both for
the user.

Tested on a local chain (`programmable-network/ethereum/test/Sdk.test.ts`)
and on a local Solana validator running the production programs
(`npm test` here), and read against the live deployment on Sepolia. Not tested yet: a burn that succeeds, a lock reaching `Carried`
or `Issued`, which need an operator's accepted claim; and a token lock on
Solana, not built yet. Jobs on Solana are not read yet, nor Conversion on
Solana; a buy's own payment proof and completing a swap (the operator's
calls) are not in the SDK. Greatwall.finance can start on what is here.

```ts
import { network, quoteCheckpoint, openCheckpoint, getJob, findJobTx } from "@ipow/sdk";

const d = network("ethereum-testnet");
const quote = await quoteCheckpoint(provider, d);   // fee, escrow, margin, total
const { jobId } = await openCheckpoint(signer, d, quote);
const job = await getJob(provider, d, jobId);        // job.stage, job.waitingFor, job.times
const btc = await findJobTx(provider, d, job);       // btc.confirmations of job.confirmations, btc.link

const lockQuote = await quoteLock(provider, d, { amount: 10n ** 16n, fee: 10n ** 12n }); // 0.01 ETH
const { lockId } = await lock(signer, d, lockQuote, "<recipient's Solana address>");
const state = await getLock(provider, d, lockId);    // Locked, Carried or Returned
```

The SDK never holds keys: it takes the app's provider and signer. After a
deployment or a contract change, `node scripts/sync.ts` regenerates its
networks and ABIs from this repository.

## In an app

**Install.** Build it to `dist/` (JavaScript with type declarations) with
`pnpm build` here, then from another repository on the same machine:

```sh
pnpm add file:../ipow/packages/sdk     # or: npm install ../ipow/packages/sdk
```

After a change here, build again and reinstall there: a `file:` dependency
is a copy.

It runs in Node and in the browser: bundled for the browser with esbuild
on 2026-10-03, it needs no Node module. It uses `fetch`, and the `buffer`
package for Solana's transactions.

**EVM wallets** (MetaMask or any injected wallet), with ethers v6:

```ts
import { BrowserProvider } from "ethers";
import { network, quoteLock, lock } from "@ipow/sdk";

const provider = new BrowserProvider(window.ethereum);
const signer = await provider.getSigner();           // asks the wallet
const d = network("ethereum-testnet");                // its chain id: d.chainId
const quote = await quoteLock(provider, d, { amount: 10n ** 16n });
await lock(signer, d, quote, solanaAddress);
```

Check that the wallet is on `d.chainId` first (`await provider.getNetwork()`),
and ask it to switch if not.

**Solana wallets** (Phantom, or any wallet adapter): the SDK takes a wallet
with `publicKey`, `signTransaction` and `signAllTransactions`, which a
wallet adapter gives.

```ts
import { Connection } from "@solana/web3.js";
import { SolanaVault, SolanaBeta } from "@ipow/sdk";

const connection = new Connection("https://api.devnet.solana.com", "confirmed");
const vault = new SolanaVault(connection, 1 /* paired with Ethereum */, wallet);
await vault.lock({ recipient: evmAddress, amount: 10_000_000n });   // 0.01 SOL
const beta = new SolanaBeta(connection, wallet);
```

**What an app should know:**
- Amounts are bigints in each asset's smallest unit; a vault counts in
  record units (gwei for ETH), and `quoteLock` converts.
- Calls that open a job set their own gas; leave it to the SDK.
- When no operator takes a job, call `expireJob` and `withdrawCredit` for
  the user: the fees do not come back with a refund.
- Bitcoin addresses are mainnet (`bc1q`, `bc1p`, `1`, `3`).
