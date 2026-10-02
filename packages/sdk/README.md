# @ipow/sdk

Build on the iPoW protocol ([`docs/design/ipow-protocol.md`](../../docs/design/ipow-protocol.md)).
Design and plan: [`docs/drafts/ipow-sdk.md`](../../docs/drafts/ipow-sdk.md).

**Status:** steps 1 and 2 of the plan, on EVM networks:
- each deployed network as data;
- quotes with their margin stated;
- jobs by stage, with their times and what they wait for;
- the job's Bitcoin transaction, found before its proof through the
  operator's chain head and the job's tag;
- opening a vault checkpoint;
- the vault: its assets and receipts, a lock's quote in the asset's own
  units converted to record units, locking (a token approved first),
  burning a receipt, and following each on this network.

Tested on a local chain (`programmable-network/ethereum/test/Sdk.test.ts`),
and read against the live deployment on Sepolia. Not tested yet: a burn
that succeeds and a lock reaching `Carried`, which need an operator's
accepted claim. What happens on the other network (a receipt issued, a burn
paid) comes with Solana, next; then BETA and Conversion.

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
