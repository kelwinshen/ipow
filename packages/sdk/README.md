# @ipow/sdk

Build on the iPoW protocol ([`docs/design/ipow-protocol.md`](../../docs/design/ipow-protocol.md)).
Design and plan: [`docs/drafts/ipow-sdk.md`](../../docs/drafts/ipow-sdk.md).

**Status:** steps 1 to 3 of the plan. On EVM networks:
- each deployed network as data;
- quotes with their margin stated;
- jobs by stage, with their times and what they wait for;
- the job's Bitcoin transaction, found before its proof through the
  operator's chain head and the job's tag;
- opening a vault checkpoint;
- the vault: its assets and receipts, a lock's quote in the asset's own
  units converted to record units, locking (a token approved first),
  burning a receipt, and following each on this network.

On Solana (`SolanaVault`): the vault's assets, locking SOL for its receipt
on the pair's EVM network and following that lock, receipt balances,
burning a receipt, and whether a lock made on the EVM network had its
receipt issued on Solana. `followLock` follows an EVM lock to its receipt
on Solana as one journey: `Locked`, `Carried`, `Issued` (or `GivenUp`,
`Returned`).

Tested on a local chain (`programmable-network/ethereum/test/Sdk.test.ts`)
and on a local Solana validator running the production programs
(`node --test test/solana.test.ts`), and read against the live deployment
on Sepolia. Not tested yet: a burn that succeeds, a lock reaching `Carried`
or `Issued`, which need an operator's accepted claim; and a token lock on
Solana, not built yet. Jobs on Solana are not read yet. BETA and Conversion
come next.

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
